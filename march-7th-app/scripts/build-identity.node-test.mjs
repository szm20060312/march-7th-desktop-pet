// A real, dependency-free Cargo application exercises build-script invalidation.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, copyFileSync, rmSync, statSync, existsSync } from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const support = fileURLToPath(new URL("../src-tauri/build_identity_support.rs", import.meta.url));
const exec = (file, args, cwd, env = process.env) => execFileSync(file, args, { cwd, env, encoding: "utf8", timeout: 120_000, stdio: ["ignore", "pipe", "pipe"] }).trim();
const git = (cwd, ...args) => exec("git", args, cwd);
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), "march7 identity spaces "));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  return root;
}
function app(root) {
  const directory = path.join(root, "app");
  mkdirSync(path.join(directory, "src-tauri/src"), { recursive: true });
  mkdirSync(path.join(directory, "src"));
  writeFileSync(path.join(directory, "src/example.ts"), "// original\n");
  writeFileSync(path.join(directory, "index.html"), "original\n");
  writeFileSync(path.join(directory, "src-tauri/Cargo.toml"), '[package]\nname="identity-fixture"\nversion="0.2.0"\nedition="2021"\n');
  for (const platform of ["windows", "macos"]) writeFileSync(path.join(directory, `src-tauri/tauri.${platform}.conf.json`), "{}\n");
  copyFileSync(support, path.join(directory, "src-tauri/build_identity_support.rs"));
  writeFileSync(path.join(directory, "src-tauri/build.rs"), 'mod build_identity_support; fn main() { build_identity_support::embed(); }');
  writeFileSync(path.join(directory, "src-tauri/src/main.rs"), 'fn main() { println!("{}|{}|{}|{}", env!("CARGO_PKG_VERSION"), env!("MARCH_BUILD_TARGET"), env!("MARCH_SOURCE_COMMIT"), env!("MARCH_SOURCE_STATE")); }');
  writeFileSync(path.join(root, ".gitignore"), "target/\napp/src/ignored.ts\n");
  return directory;
}
function initialize(root) {
  git(root, "init", "-b", "main");
  git(root, "config", "user.name", "Identity Fixture");
  git(root, "config", "user.email", "fixture@local.invalid");
  git(root, "add", ".");
  git(root, "commit", "-qm", "fixture");
}
function build(directory, target, overrides = {}) {
  const env = { ...process.env, CARGO_TARGET_DIR: target, ...overrides };
  exec("cargo", ["build", "--offline", "--manifest-path", "src-tauri/Cargo.toml"], directory, env);
  const [version, platform, commit, state] = exec(path.join(target, "debug", `identity-fixture${process.platform === "win32" ? ".exe" : ""}`), [], directory).split("|");
  return { version, platform, commit, state };
}

test("real Cargo cache follows source changes, new files, Git index, HEAD and worktree refs", t => {
  const root = fixture(t); const directory = app(root); const target = path.join(root, "target");
  // Materialize Cargo.lock before committing the baseline.
  exec("cargo", ["generate-lockfile", "--offline", "--manifest-path", "src-tauri/Cargo.toml"], directory); initialize(root);
  let result = build(directory, target);
  assert.equal(result.state, "clean"); assert.equal(result.commit, git(root, "rev-parse", "HEAD"));
  assert.equal(result.version, "0.2.0"); assert.match(result.platform, /-/);
  const executable = path.join(target, "debug", `identity-fixture${process.platform === "win32" ? ".exe" : ""}`);
  const builtAt = statSync(executable).mtimeMs;
  build(directory, target);
  assert.equal(statSync(executable).mtimeMs, builtAt, "unchanged input should reuse the compiled binary");
  writeFileSync(path.join(directory, "src/example.ts"), "// changed\n");
  assert.equal(build(directory, target).state, "modified");
  git(root, "checkout", "--", "app/src/example.ts");
  assert.equal(build(directory, target).state, "clean");
  writeFileSync(path.join(directory, "index.html"), "changed entry\n");
  git(root, "add", "app/index.html");
  assert.equal(build(directory, target).state, "modified");
  git(root, "restore", "--staged", "--worktree", "app/index.html");
  writeFileSync(path.join(directory, "src/new.ts"), "// new input\n");
  assert.equal(build(directory, target).state, "modified");
  rmSync(path.join(directory, "src/new.ts"));
  writeFileSync(path.join(directory, "src/ignored.ts"), "// ignored but compiled input\n");
  assert.equal(build(directory, target).state, "modified");
  rmSync(path.join(directory, "src/ignored.ts"));
  git(root, "commit", "--allow-empty", "-qm", "head only");
  result = build(directory, target);
  assert.equal(result.state, "clean"); assert.equal(result.commit, git(root, "rev-parse", "HEAD"));
  git(root, "checkout", "--detach", "HEAD~1");
  assert.equal(build(directory, target).commit, git(root, "rev-parse", "HEAD"));
  git(root, "checkout", "main");
  writeFileSync(path.join(root, "notes.md"), "outside compiled inputs\n");
  assert.equal(build(directory, target).state, "clean");
  const worktree = path.join(root, "linked worktree");
  git(root, "worktree", "add", "-b", "linked", worktree);
  const linkedApp = path.join(worktree, "app"); const linkedTarget = path.join(root, "target-linked");
  assert.equal(build(linkedApp, linkedTarget).state, "clean");
  git(worktree, "commit", "--allow-empty", "-qm", "linked head only");
  assert.equal(build(linkedApp, linkedTarget).commit, git(worktree, "rev-parse", "HEAD"));
  git(root, "pack-refs", "--all", "--prune");
  assert.equal(build(linkedApp, linkedTarget).commit, git(worktree, "rev-parse", "HEAD"));
  git(root, "update-ref", "refs/heads/linked", git(root, "rev-parse", "main"));
  assert.equal(build(linkedApp, linkedTarget).commit, git(worktree, "rev-parse", "HEAD"), "new loose ref after packing must invalidate Cargo");
});

test("exports, unrelated parent repositories and unavailable Git never invent provenance", t => {
  const root = fixture(t); const directory = app(root); const target = path.join(root, "target");
  let result = build(directory, target);
  assert.equal(result.state, "unknown"); assert.equal(result.commit, "");
  git(root, "init", "-b", "main"); git(root, "config", "user.name", "Fixture"); git(root, "config", "user.email", "fixture@local.invalid");
  writeFileSync(path.join(root, "unrelated.txt"), "parent\n"); git(root, "add", "unrelated.txt"); git(root, "commit", "-qm", "unrelated");
  result = build(directory, path.join(root, "target-unrelated"));
  assert.equal(result.state, "unknown"); assert.equal(result.commit, "");
  git(root, "add", "app", ".gitignore"); git(root, "commit", "-qm", "track actual app");
  assert.equal(build(directory, path.join(root, "target-tracked")).state, "clean");
  const driver = path.join(root, "probe.rs");
  writeFileSync(driver, `#[path = ${JSON.stringify(path.join(directory, "src-tauri/build_identity_support.rs"))}] mod support; fn main() { support::embed(); }`);
  const probe = path.join(root, `probe${process.platform === "win32" ? ".exe" : ""}`);
  exec("rustc", ["--edition=2021", driver, "-o", probe], root);
  const output = exec(probe, [], directory, { ...process.env, PATH: "", CARGO_MANIFEST_DIR: path.join(directory, "src-tauri"), TARGET: "x86_64-pc-windows-msvc", MARCH_BUILD_INPUT_DIAGNOSTICS: "1" });
  assert.match(output, /MARCH_SOURCE_STATE=unknown/);
  assert.match(output, /MARCH_SOURCE_COMMIT=\r?\n/);
  assert.doesNotMatch(output, /build-input-diagnostic/);
});

test("automatic Tauri platform overrides cannot leave a cached clean identity", t => {
  const root = fixture(t); const directory = app(root); const target = path.join(root, "target");
  mkdirSync(path.join(directory, "scripts"));
  const packetScript = path.join(directory, "scripts/prepare-regression.mjs");
  copyFileSync(fileURLToPath(new URL("./prepare-regression.mjs", import.meta.url)), packetScript);
  exec("cargo", ["generate-lockfile", "--offline", "--manifest-path", "src-tauri/Cargo.toml"], directory); initialize(root);
  assert.equal(build(directory, target).state, "clean");
  for (const platform of ["windows", "macos"]) {
    const config = path.join(directory, `src-tauri/tauri.${platform}.conf.json`);
    writeFileSync(config, '{"productName":"Local override"}\n');
    assert.equal(build(directory, target).state, "modified");
    rmSync(config);
    assert.throws(() => build(directory, target), error => {
      assert.match(error.stderr.toString(), /Missing required project build file/);
      return true;
    });
    const packet = path.join(root, `missing-${platform}-packet`);
    const staleBinary = path.join(target, "march-7th-app.exe");
    assert.throws(() => exec(process.execPath, [packetScript, "x86_64-pc-windows-msvc", staleBinary, packet, staleBinary], directory), error => {
      assert.match(error.stderr.toString(), /Refusing to label a build from a modified tracked worktree/);
      assert.ok(error.stderr.toString().includes(`tauri.${platform}.conf.json`));
      return true;
    });
    assert.equal(existsSync(packet), false);
    writeFileSync(config, "{}\n");
    assert.equal(build(directory, target).state, "clean");
  }
  const executable = path.join(target, "debug", `identity-fixture${process.platform === "win32" ? ".exe" : ""}`);
  const builtAt = statSync(executable).mtimeMs;
  build(directory, target);
  assert.equal(statSync(executable).mtimeMs, builtAt);
});

test("opt-in source diagnostics report only bounded fixed scopes and categories without changing identity", t => {
  const root = fixture(t); const directory = app(root); const target = path.join(root, "target");
  exec("cargo", ["generate-lockfile", "--offline", "--manifest-path", "src-tauri/Cargo.toml"], directory); initialize(root);
  const run = flag => {
    const result = spawnSync("cargo", ["build", "--offline", "--manifest-path", "src-tauri/Cargo.toml"], {
      cwd: directory, encoding: "utf8", timeout: 120_000,
      env: { ...process.env, CARGO_TARGET_DIR: target, MARCH_BUILD_INPUT_DIAGNOSTICS: flag },
    });
    assert.equal(result.status, 0, result.stderr);
    const diagnostics = result.stderr.split(/\r?\n/).filter(line => line.includes("build-input-diagnostic"));
    const identity = exec(path.join(target, "debug", `identity-fixture${process.platform === "win32" ? ".exe" : ""}`), [], directory);
    return { diagnostics, identity };
  };
  assert.deepEqual(run("1").diagnostics, [], "clean input needs no diagnostic queries/output");
  writeFileSync(path.join(directory, "src/example.ts"), "// tracked change\n");
  writeFileSync(path.join(directory, "src/ignored.ts"), "private ignored contents\n");
  for (let i = 0; i < 40; i++) writeFileSync(path.join(directory, `src/PRIVATE-NAME-${i}.ts`), "private untracked contents\n");
  writeFileSync(path.join(directory, "src-tauri/Cargo.toml"), '[package]\nname="identity-fixture"\nversion="0.2.0"\nedition="2021"\n# tracked diagnostic fixture\n');
  writeFileSync(path.join(root, "PRIVATE-OUTSIDE.txt"), "outside scope\n");
  const disabled = run("");
  assert.match(disabled.identity, /\|modified$/);
  assert.deepEqual(disabled.diagnostics, []);
  const enabled = run("1"); // Only the opt-in changes: Cargo must rerun.
  assert.equal(enabled.identity, disabled.identity);
  assert.equal(enabled.diagnostics.length, 2, "one bounded row per changed scope, not per filename");
  assert.ok(enabled.diagnostics.some(line => line.endsWith("build-input-diagnostic scope=frontend-source categories=tracked,untracked,ignored")));
  assert.ok(enabled.diagnostics.some(line => line.endsWith("build-input-diagnostic scope=native-manifest categories=tracked")));
  for (const line of enabled.diagnostics) {
    assert.ok(line.length < 180);
    assert.equal(line.includes(root), false);
    assert.equal(line.includes("PRIVATE-"), false);
    assert.equal(line.includes("contents"), false);
    assert.equal(line.includes("example.ts"), false);
    assert.equal(line.includes("Cargo.toml"), false);
  }
  const disabledAgain = run("true"); // Any value other than exact 1 is off.
  assert.equal(disabledAgain.identity, disabled.identity);
  assert.deepEqual(disabledAgain.diagnostics, []);
});
