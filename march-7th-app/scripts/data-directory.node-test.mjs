import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
import { createInterface } from "node:readline";
import { mkdtempSync, readFileSync, writeFileSync, existsSync, mkdirSync, rmSync, readdirSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const app = fileURLToPath(new URL("../", import.meta.url));
const source = name => readFileSync(path.join(app, "src-tauri/src", name), "utf8");

test("native assembly qualifies once before run and every store consumes managed qualification", () => {
  const lib = source("lib.rs");
  assert.equal((lib.match(/app_config_dir\(\)/g) ?? []).length, 1);
  const startup = [".build(app_context())", "data_directory::acquire(", "app.manage(directory)", "app.run("];
  let previous = -1;
  for (const step of startup) {
    const position = lib.indexOf(step);
    assert.ok(position > previous, `missing or out-of-order startup step: ${step}`);
    previous = position;
  }
  assert.match(lib, /Err\(data_directory::AlreadyRunning\) => return/);
  assert.equal((lib.match(/generate_context!/g) ?? []).length, 1);
  assert.match(lib, /\.setup\(\|app\| \{\s*let characters = characters::setup\(app\)\?;\s*let reminders = reminders::setup\(app\)\?;\s*desktop::setup\(app, &characters, &reminders\)/);
  for (const [file, kind] of [["characters/native.rs", "Characters"], ["reminders/native.rs", "Reminders"], ["desktop/mod.rs", "Desktop"]]) {
    const native = source(file);
    assert.ok(native.includes(`app.state::<DataDirectory>().path(DataFile::${kind})`));
    assert.doesNotMatch(native, /app_config_dir|create_dir_all/);
  }
  assert.match(source("desktop/mod.rs"), /None => \(\s*None,\s*None,\s*Some\("configuration directory unavailable/);
  const main = source("main.rs");
  assert.ok(main.indexOf('"--build-info"') < main.indexOf("march_7th_app_lib::run()"));
  assert.match(main, /return;\s*\}\s*march_7th_app_lib::run\(\)/);
  assert.doesNotMatch(main, /app_config_dir|data_directory::|generate_context!/);
  const config = JSON.parse(readFileSync(path.join(app, "src-tauri/tauri.conf.json"), "utf8"));
  assert.equal(config.app.trayIcon, undefined, "build must not start a configured tray before qualification");
});

test("actual OS lock coordinates processes, graceful release, killed owner and stale file", { timeout: 60_000 }, async t => {
  const root = mkdtempSync(path.join(os.tmpdir(), "march7 directory spaces "));
  const children = [];
  t.after(async () => {
    for (const { child, closed } of children) {
      if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
      await closed;
    }
    rmSync(root, { recursive: true, force: true });
  });
  const binary = path.join(root, `directory-fixture${process.platform === "win32" ? ".exe" : ""}`);
  execFileSync("rustc", ["--edition=2021", path.join(app, "scripts/fixtures/data-directory.rs"), "-o", binary], { timeout: 30_000, stdio: "pipe" });
  const data = path.join(root, "data");
  mkdirSync(data);
  writeFileSync(path.join(data, "instance.lock"), "stale content must survive");
  for (const file of ["desktop-state.json", "character-preferences.json", "reminders.json"]) writeFileSync(path.join(data, file), `original ${file}`);
  const probe = directory => execFileSync(binary, [directory, "probe"], { encoding: "utf8", timeout: 10_000 }).trim();
  async function hold() {
    const child = spawn(binary, [data, "hold"], { stdio: ["pipe", "pipe", "pipe"] });
    const closed = once(child, "close");
    children.push({ child, closed });
    let stderr = "";
    child.stderr.on("data", chunk => { stderr += chunk; });
    const lines = createInterface({ input: child.stdout });
    const ready = await Promise.race([
      once(lines, "line").then(([line]) => line),
      closed.then(() => { throw new Error(`holder exited before ready: ${stderr}`); }),
    ]);
    lines.close();
    assert.equal(ready, "ready");
    return { child, closed };
  }
  let owner = await hold();
  rmSync(path.join(data, "fixture-started"));
  assert.equal(probe(data), "alreadyRunning");
  assert.equal(existsSync(path.join(data, "fixture-started")), false, "duplicate must not initialize services");
  owner.child.stdin.end("release\n");
  assert.deepEqual(await owner.closed, [0, null]);
  assert.equal(probe(data), "ready");
  assert.equal(readFileSync(path.join(data, "instance.lock"), "utf8"), "stale content must survive");

  owner = await hold();
  assert.equal(probe(data), "alreadyRunning");
  owner.child.kill("SIGKILL");
  const [code, signal] = await owner.closed;
  assert.ok(code !== 0 || signal !== null, "fixture must actually exit abnormally");
  assert.equal(probe(data), "ready", "OS must release the killed owner's lock");
  for (const file of ["desktop-state.json", "character-preferences.json", "reminders.json"]) assert.equal(readFileSync(path.join(data, file), "utf8"), `original ${file}`);
  assert.equal(readFileSync(path.join(data, "instance.lock"), "utf8"), "stale content must survive");

  const blocked = path.join(root, "blocked");
  mkdirSync(blocked);
  mkdirSync(path.join(blocked, "instance.lock"));
  writeFileSync(path.join(blocked, "reminders.json"), "do not overwrite");
  assert.equal(probe(blocked), "unavailable:directoryLockOpenFailed");
  assert.deepEqual(readdirSync(blocked).sort(), ["instance.lock", "reminders.json"]);
  assert.equal(readFileSync(path.join(blocked, "reminders.json"), "utf8"), "do not overwrite");
});
