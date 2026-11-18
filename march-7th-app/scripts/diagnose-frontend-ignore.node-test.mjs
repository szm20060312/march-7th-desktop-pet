import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, appendFileSync, rmSync, symlinkSync, renameSync, existsSync, realpathSync, statSync, copyFileSync } from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import os from "node:os";
import path from "node:path";
import { summarizeFrontendIgnored } from "./diagnose-frontend-ignore.mjs";

function fixture(t, appName = "app") {
  const root = mkdtempSync(path.join(os.tmpdir(), "march7-ignore-class-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const appRoot = path.join(root, appName);
  mkdirSync(path.join(appRoot, "src/nested"), { recursive: true });
  execFileSync("git", ["init", "-q"], { cwd: root, stdio: "pipe" });
  writeFileSync(path.join(root, ".gitignore"), "*.log\nnode_modules/\n");
  writeFileSync(path.join(appRoot, ".gitignore"), "*.local\n");
  writeFileSync(path.join(appRoot, "src/nested/.gitignore"), "PRIVATE_PATTERN*\n");
  writeFileSync(path.join(root, ".git/info/exclude"), "GLOBAL_PRIVATE*\n");
  return { root, appRoot };
}
const phase = "after-check";
const unavailable = stage => `ignored-input phase=${phase} state=unavailable total=unknown groups=unknown overflow=no failureStage=${stage}`;

function shortDirectory(t, directory) {
  // Read existing filesystem metadata only; never enable or alter 8.3 settings.
  let short;
  try {
    const script = "$ErrorActionPreference='Stop'; $fso=New-Object -ComObject Scripting.FileSystemObject; $fso.GetFolder($env:MARCH_TEST_DIRECTORY).ShortPath";
    short = execFileSync(process.env.MARCH_TEST_PWSH ?? "pwsh.exe", ["-NoProfile", "-Command", script], {
      timeout: 10_000, maxBuffer: 16_384, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, MARCH_TEST_DIRECTORY: realpathSync.native(directory) },
    }).trim();
  } catch { throw Error("Windows short-name metadata query failed; MARCH_TEST_PWSH may select an existing PowerShell 7"); }
  if (path.relative(short, realpathSync.native(directory)) === "") { t.skip("No distinct 8.3 directory alias on this volume"); return null; }
  assert.ok(realpathSync.native(short) === realpathSync.native(directory), "short and long names must resolve to the same directory");
  const a = statSync(short, { bigint: true }), b = statSync(directory, { bigint: true });
  assert.ok(a.dev === b.dev && a.ino === b.ino && a.ino !== 0n, "directory identity must be reliable and identical");
  return short;
}

test("Windows 8.3 directory aliases classify identically to their native long names", { skip: process.platform !== "win32" }, t => {
  const { appRoot } = fixture(t, "Long Application Directory");
  writeFileSync(path.join(appRoot, "src/PRIVATE.local"), "private");
  const short = shortDirectory(t, appRoot);
  if (!short) return;
  const expected = `ignored-input phase=${phase} state=ok total=one groups=app-rules:local-config:file:one overflow=no`;
  assert.equal(summarizeFrontendIgnored({ appRoot: realpathSync.native(appRoot), phase, enabled: "1" }), expected);
  assert.equal(summarizeFrontendIgnored({ appRoot: short, phase, enabled: "1" }), expected);
});

for (const entry of ["diagnose-frontend-ignore.mjs", "prepare-regression.mjs"]) {
  test(`Windows 8.3 path actually executes ${entry} CLI`, { skip: process.platform !== "win32" }, t => {
    const { appRoot } = fixture(t, "Long Application Directory");
    writeFileSync(path.join(appRoot, "src/PRIVATE.local"), "private");
    mkdirSync(path.join(appRoot, "scripts"));
    copyFileSync(fileURLToPath(new URL(`./${entry}`, import.meta.url)), path.join(appRoot, "scripts", entry));
    const short = shortDirectory(t, appRoot);
    if (!short) return;
    const diagnostic = entry === "diagnose-frontend-ignore.mjs";
    const result = spawnSync(process.execPath, [path.join(short, "scripts", entry), ...(diagnostic ? [phase] : [])], {
      cwd: short, encoding: "utf8", timeout: 10_000,
      env: { ...process.env, MARCH_BUILD_INPUT_DIAGNOSTICS: "1" },
    });
    assert.equal(result.status, diagnostic ? 0 : 1);
    if (diagnostic) {
      assert.equal(result.stdout.trim(), `ignored-input phase=${phase} state=ok total=one groups=app-rules:local-config:file:one overflow=no`);
      assert.equal(result.stderr, "");
    } else {
      assert.equal(result.stdout, "");
      assert.match(result.stderr, /^Usage: node scripts\/prepare-regression.mjs/);
    }
  });
}

test("literal source directory scope does not match the native target sibling", t => {
  const { root, appRoot } = fixture(t);
  appendFileSync(path.join(root, ".gitignore"), "target/\n");
  mkdirSync(path.join(appRoot, "src-tauri/target"), { recursive: true });
  writeFileSync(path.join(appRoot, "src-tauri/target/generated.txt"), "ignored cache\n");
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1" }), `ignored-input phase=${phase} state=ok total=zero groups=none overflow=no`);
  writeFileSync(path.join(appRoot, "src/PRIVATE.local"), "real ignored source\n");
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1" }), `ignored-input phase=${phase} state=ok total=one groups=app-rules:local-config:file:one overflow=no`);
});

test("a real ignored source root directory is part of the declared scope", t => {
  const { root, appRoot } = fixture(t);
  appendFileSync(path.join(root, ".gitignore"), "app/src/\n");
  writeFileSync(path.join(appRoot, "src/PRIVATE.local"), "private");
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1" }), `ignored-input phase=${phase} state=ok total=one groups=root-rules:other:directory:one overflow=no`);
});

test("real Git case-equivalent source entries respect the filesystem scope", t => {
  const { appRoot } = fixture(t);
  const source = path.join(appRoot, "src");
  writeFileSync(path.join(source, "PRIVATE.local"), "private");
  renameSync(source, path.join(appRoot, "rename-temp"));
  renameSync(path.join(appRoot, "rename-temp"), path.join(appRoot, "SRC"));
  const git = (args, input) => execFileSync("git", args.includes("status") ? ["--icase-pathspecs", ...args] : args, { cwd: appRoot, input, stdio: ["pipe", "pipe", "pipe"] });
  const expected = existsSync(source) ? `ignored-input phase=${phase} state=ok total=one groups=app-rules:local-config:file:one overflow=no` : unavailable("path");
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git }), expected);
});

test("ignored rule summaries classify actual NUL queries using only fixed vocabulary and coarse counts", t => {
  const { root, appRoot } = fixture(t);
  writeFileSync(path.join(appRoot, "src/PRIVATE-NAME-你好.local"), "PRIVATE_CONTENT");
  writeFileSync(path.join(appRoot, "src/PRIVATE-NAME.log"), "PRIVATE_CONTENT");
  writeFileSync(path.join(appRoot, "src/nested/PRIVATE_PATTERN-secret"), "PRIVATE_CONTENT");
  writeFileSync(path.join(appRoot, "src/GLOBAL_PRIVATE-secret"), "PRIVATE_CONTENT");
  mkdirSync(path.join(appRoot, "src/node_modules"));
  writeFileSync(path.join(appRoot, "src/node_modules/PRIVATE-PACKAGE"), "PRIVATE_CONTENT");
  const line = summarizeFrontendIgnored({ appRoot, phase, enabled: "1" });
  assert.match(line, /^ignored-input phase=after-check state=ok total=few groups=/);
  for (const group of ["app-rules:local-config:file:one", "root-rules:logs:file:one", "nested-rules:other:file:one", "internal-external-rules:other:file:one", "root-rules:dependency-directory:directory:one"]) assert.ok(line.includes(group), group);
  assert.equal(line.includes(root), false);
  assert.equal(line.includes("PRIVATE"), false);
  assert.equal(line.includes("*."), false);
  assert.equal(line.split("\n").length, 1);
  assert.ok(line.length < 1024);
});

test("ignored rule summaries distinguish empty, unavailable, disabled and invalid phases", t => {
  const { appRoot } = fixture(t);
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1" }), `ignored-input phase=${phase} state=ok total=zero groups=none overflow=no`);
  let calls = 0;
  const failedGit = () => { calls++; throw Error("PRIVATE_GIT_STDERR /user/path ENV=secret"); };
  for (const enabled of [undefined, "", "0", "true"]) assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled, git: failedGit }), "");
  assert.equal(calls, 0);
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: failedGit }), unavailable("root"));
  assert.equal(summarizeFrontendIgnored({ appRoot, phase: "PRIVATE\nPHASE", enabled: "1", git: failedGit }), "ignored-input phase=unknown state=unavailable total=unknown groups=unknown overflow=no failureStage=phase");
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: () => Buffer.from("PRIVATE_BAD_NUL") }), unavailable("root"));
});

test("ignored rule summaries preserve origins when the app directory is an alias", t => {
  const { root, appRoot } = fixture(t);
  writeFileSync(path.join(appRoot, "src/PRIVATE.local"), "private");
  const alias = path.join(root, "alias-app");
  symlinkSync(appRoot, alias, process.platform === "win32" ? "junction" : "dir");
  assert.equal(summarizeFrontendIgnored({ appRoot: alias, phase, enabled: "1" }), `ignored-input phase=${phase} state=ok total=one groups=app-rules:local-config:file:one overflow=no`);
});

test("ignored rule summaries never leak newline names and coarsen large groups", t => {
  const { root, appRoot } = fixture(t);
  for (let i = 0; i < 40; i++) writeFileSync(path.join(appRoot, `src/PRIVATE-${i}.local`), "private");
  if (process.platform !== "win32") writeFileSync(path.join(appRoot, "src/PRIVATE\nnewline.local"), "private");
  const line = summarizeFrontendIgnored({ appRoot, phase, enabled: "1" });
  assert.equal(line, `ignored-input phase=${phase} state=ok total=many groups=app-rules:local-config:file:many overflow=no`);
  const recordName = process.platform === "win32" ? "PRIVATE-NAME.local" : "PRIVATE\nNAME.local";
  writeFileSync(path.join(appRoot, "src", recordName), "private");
  let query = 0;
  const malformed = () => Buffer.from(++query === 1 ? root + "\n" : query === 2 ? `!! app/src/${recordName}\0` : `PRIVATE_RULE_SOURCE\nNAME\0NaN\0PRIVATE_PATTERN\0app/src/${recordName}\0`);
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: malformed }), unavailable("ignore-format"));
  assert.equal(query, 3, "malformed rule output must be reached after a valid root and status");
});

function injectedRecords(root, status, ruleOutput) {
  const calls = [];
  const git = args => {
    const command = args[0] === "-C" ? args[2] : args[0];
    calls.push(command);
    if (command !== "rev-parse") assert.ok(args[0] === "-C" && args[1] === realpathSync.native(root), "all record queries must use the verified repository root");
    if (command === "rev-parse") return Buffer.from(root + "\n");
    if (command === "status") return Buffer.isBuffer(status) ? status : Buffer.from(status);
    if (command === "check-ignore") return Buffer.from(ruleOutput);
    throw Error("unexpected query");
  };
  return { git, calls };
}

test("porcelain rejects unknown states and malformed NUL or rename records at the status query", t => {
  const { root, appRoot } = fixture(t);
  for (const status of [
    "XX app/src/PRIVATE\0", "MZ app/src/PRIVATE\0", "?M app/src/PRIVATE\0", "!M app/src/PRIVATE\0", "  app/src/PRIVATE\0",
    "MA app/src/PRIVATE\0", "DM app/src/PRIVATE\0", "m  app/src/PRIVATE\0", "!!\tapp/src/PRIVATE\0",
    "!! app/src/PRIVATE", "!! \0", "R  app/src/PRIVATE\0", " C app/src/PRIVATE\0\0", Buffer.from([0xff, 0]),
  ]) {
    const probe = injectedRecords(root, status);
    assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: probe.git }), unavailable("status-format"));
    assert.deepEqual(probe.calls, ["rev-parse", "status"]);
  }
});

test("porcelain accepts documented tracked untracked ignored and rename/copy states without consuming the next record", t => {
  const { root, appRoot } = fixture(t);
  writeFileSync(path.join(appRoot, "src/PRIVATE.local"), "private");
  const flags = [" M", " A", " D", " T", " R", " C", "M ", "MM", "MT", "MD", "T ", "TM", "TT", "TD", "A ", "AM", "AT", "AD", "D ", "R ", "RM", "RT", "RD", "C ", "CM", "CT", "CD", "DD", "AU", "UD", "UA", "DU", "AA", "UU", "??"];
  for (const flag of flags) {
    const secondPath = /[RC]/.test(flag) ? "XX PRIVATE-original\nname\0" : "";
    const status = `${flag} app/src/PRIVATE-tracked\0${secondPath}!! app/src/PRIVATE.local\0`;
    const probe = injectedRecords(root, status, "app/.gitignore\0" + "1\0*.local\0app/src/PRIVATE.local\0");
    assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: probe.git }), `ignored-input phase=${phase} state=ok total=one groups=app-rules:local-config:file:one overflow=no`);
    assert.deepEqual(probe.calls, ["rev-parse", "status", "check-ignore"]);
  }
});

test("a real staged rename and untracked file do not hide the following ignored entry", t => {
  const { root, appRoot } = fixture(t);
  writeFileSync(path.join(appRoot, "src/PRIVATE-original.ts"), "// tracked baseline\n");
  const git = args => execFileSync("git", args, { cwd: root, stdio: "pipe" });
  git(["add", "--", "app/src/PRIVATE-original.ts"]);
  git(["-c", "user.name=Diagnostic Fixture", "-c", "user.email=fixture@local.invalid", "commit", "-qm", "fixture"]);
  git(["mv", "--", "app/src/PRIVATE-original.ts", "app/src/PRIVATE-renamed.ts"]);
  writeFileSync(path.join(appRoot, "src/PRIVATE-untracked.ts"), "private\n");
  writeFileSync(path.join(appRoot, "src/PRIVATE.local"), "private\n");
  const records = git(["status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignored=matching", "--", "app/src"]).toString("utf8").split("\0");
  assert.ok(records.some(record => record.startsWith("R ")), "fixture must produce an actual staged rename");
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1" }), `ignored-input phase=${phase} state=ok total=one groups=app-rules:local-config:file:one overflow=no`);
});

test("malformed ignore-rule output fails at the rule query after valid root and ignored status", t => {
  const { root, appRoot } = fixture(t);
  writeFileSync(path.join(appRoot, "src/PRIVATE.local"), "private");
  for (const record of [
    "app/.gitignore\0" + "1\0*.local\0app/src/PRIVATE.local", // Missing terminating NUL.
    "app/.gitignore\0" + "1\0*.local\0", // Missing path field.
    "app/.gitignore\0" + "0\0*.local\0app/src/PRIVATE.local\0",
    "app/.gitignore\0" + "NaN\0*.local\0app/src/PRIVATE.local\0",
    "\0" + "1\0*.local\0app/src/PRIVATE.local\0",
    "app/.gitignore\0" + "1\0\0app/src/PRIVATE.local\0",
    "app/.gitignore\0" + "1\0!*.local\0app/src/PRIVATE.local\0",
    "app/.gitignore\0" + "1\0*.local\0app/src/PRIVATE-other.local\0",
  ]) {
    const probe = injectedRecords(root, "!! app/src/PRIVATE.local\0", record);
    assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: probe.git }), unavailable("ignore-format"));
    assert.deepEqual(probe.calls, ["rev-parse", "status", "check-ignore"]);
  }
});

test("ignored rule summaries cap groups and fail conservatively above the enumeration bound", t => {
  const { root, appRoot } = fixture(t);
  appendFileSync(path.join(root, ".gitignore"), "dist/\n.env\n.DS_Store\n");
  for (const name of ["PRIVATE.log", "PRIVATE.local", ".env", ".DS_Store", "nested/PRIVATE_PATTERN", "GLOBAL_PRIVATE"]) writeFileSync(path.join(appRoot, "src", name), "private");
  for (const name of ["node_modules", "dist", "PRIVATE-DIR.log"]) {
    mkdirSync(path.join(appRoot, "src", name));
    writeFileSync(path.join(appRoot, "src", name, "PRIVATE_CHILD"), "private");
  }
  const line = summarizeFrontendIgnored({ appRoot, phase, enabled: "1" });
  assert.match(line, /overflow=yes$/);
  assert.equal(line.split(" groups=")[1].split(" ")[0].split(",").length, 8);
  assert.ok(line.length < 1024);
  assert.equal(line.includes("PRIVATE"), false);
  let calls = 0;
  const oversized = () => Buffer.from(++calls === 1 ? root + "\n" : Array.from({ length: 513 }, (_, i) => `!! app/src/PRIVATE-${i}\0`).join(""));
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: oversized }), unavailable("limit"));
  assert.equal(calls, 2);
});

test("fixed failure stages identify each failing operation without exposing errors or inputs", t => {
  const { root, appRoot } = fixture(t);
  writeFileSync(path.join(appRoot, "src/PRIVATE.local"), "private");
  const cases = [
    { stage: "root", root: "PRIVATE_BAD_ROOT", calls: ["rev-parse"] },
    { stage: "status-query", fail: "status", calls: ["rev-parse", "status"] },
    { stage: "status-format", status: "XX app/src/PRIVATE\0", calls: ["rev-parse", "status"] },
    { stage: "limit", status: Array.from({ length: 513 }, () => "!! app/src/PRIVATE.local\0").join(""), calls: ["rev-parse", "status"] },
    { stage: "path", status: "!! PRIVATE-OUTSIDE.local\0", calls: ["rev-parse", "status"] },
    { stage: "ignore-query", fail: "check-ignore", calls: ["rev-parse", "status", "check-ignore"] },
    { stage: "ignore-format", rule: "PRIVATE_BAD_NUL", calls: ["rev-parse", "status", "check-ignore"] },
    { stage: "metadata", status: "!! app/src/MISSING.local\0", calls: ["rev-parse", "status"] },
  ];
  for (const scenario of cases) {
    const calls = [];
    const git = args => {
      const command = args[0] === "-C" ? args[2] : args[0];
      calls.push(command);
      if (command === scenario.fail) throw Error("PRIVATE_FAILURE /private/path ENV=secret\nraw error");
      if (command === "rev-parse") return Buffer.from((scenario.root ?? root) + "\n");
      if (command === "status") return Buffer.from(scenario.status ?? "!! app/src/PRIVATE.local\0");
      return Buffer.from(scenario.rule ?? "app/.gitignore\0" + "1\0*.local\0app/src/PRIVATE.local\0");
    };
    const line = summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git });
    assert.equal(line, unavailable(scenario.stage));
    assert.deepEqual(calls, scenario.calls);
    assert.equal(line.includes("PRIVATE"), false);
    assert.equal(line.split("\n").length, 1);
    assert.ok(Buffer.byteLength(line) <= 1024);
  }
  rmSync(path.join(appRoot, "src/PRIVATE.local"));
  for (const phase of ["after-cargo-test", "after-clippy", "after-identity-fixture"]) {
    assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1" }), `ignored-input phase=${phase} state=ok total=zero groups=none overflow=no`);
  }
});

test("real queries share one repository cwd, literal scope and repo-relative NUL input", t => {
  const { root, appRoot } = fixture(t, "app[private]");
  writeFileSync(path.join(appRoot, "src/PRIVATE.local"), "private");
  const seen = [];
  const git = (args, input) => {
    const command = args[0] === "-C" ? args[2] : args[0];
    seen.push(command);
    if (command !== "rev-parse") assert.ok(args[0] === "-C" && args[1] === realpathSync.native(root), "fixed repository context");
    if (command === "status") assert.ok(args.at(-1) === ":(literal)app[private]/src/", "literal source directory boundary");
    if (command === "check-ignore") assert.ok(input.equals(Buffer.from("app[private]/src/PRIVATE.local\0")), "repo-relative stdin must match porcelain records");
    return execFileSync("git", args, { cwd: appRoot, input, stdio: ["pipe", "pipe", "pipe"] });
  };
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git }), `ignored-input phase=${phase} state=ok total=one groups=app-rules:local-config:file:one overflow=no`);
  assert.deepEqual(seen, ["rev-parse", "status", "check-ignore"]);
});

test("absolute traversal sibling-prefix and app-relative records never escape the declared scope", t => {
  const { root, appRoot } = fixture(t);
  for (const directory of [path.join(appRoot, "src"), path.join(appRoot, "src-neighbor"), path.join(root, "src")]) {
    mkdirSync(directory, { recursive: true });
    writeFileSync(path.join(directory, "PRIVATE.local"), "private");
  }
  for (const entry of [
    path.join(appRoot, "src/PRIVATE.local"), "/PRIVATE/absolute", "C:\\PRIVATE\\absolute", "C:PRIVATE", "\\\\PRIVATE\\share",
    "app/src/../src/PRIVATE.local", "app/src/./PRIVATE.local", "app//src/PRIVATE.local", "app\\src\\..\\src\\PRIVATE.local",
    "app/src-neighbor/PRIVATE.local", "src/PRIVATE.local",
  ]) {
    const probe = injectedRecords(root, `!! ${entry}\0`);
    assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: probe.git }), unavailable("path"));
    assert.deepEqual(probe.calls, ["rev-parse", "status"]);
  }
  // On case-sensitive volumes, a similarly spelled directory is a different
  // object and must not be accepted merely because a lowercase prefix matches.
  const distinctCase = path.join(appRoot, "SRC");
  if (!existsSync(distinctCase)) {
    mkdirSync(distinctCase);
    writeFileSync(path.join(distinctCase, "PRIVATE.local"), "private");
    const probe = injectedRecords(root, "!! app/SRC/PRIVATE.local\0");
    assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: probe.git }), unavailable("path"));
    assert.deepEqual(probe.calls, ["rev-parse", "status"]);
  }
});

test("real symlink escapes are rejected while links remaining inside the source root are classifiable", t => {
  const { root, appRoot } = fixture(t);
  const outside = path.join(root, "outside");
  mkdirSync(outside);
  writeFileSync(path.join(outside, "PRIVATE.local"), "private");
  const link = path.join(appRoot, "src/PRIVATE-link.local");
  const kind = process.platform === "win32" ? "junction" : "dir";
  symlinkSync(outside, link, kind);
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1" }), unavailable("path"));
  const child = injectedRecords(root, "!! app/src/PRIVATE-link.local/PRIVATE.local\0");
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: child.git }), unavailable("path"));
  assert.deepEqual(child.calls, ["rev-parse", "status"]);
  rmSync(link, { recursive: true, force: true });
  const inside = path.join(appRoot, "src/inside");
  mkdirSync(inside);
  writeFileSync(path.join(inside, "PRIVATE-content"), "private");
  symlinkSync(inside, link, kind);
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1" }), `ignored-input phase=${phase} state=ok total=one groups=app-rules:local-config:link:one overflow=no`);
  rmSync(link, { recursive: true, force: true });
  renameSync(path.join(appRoot, "src"), path.join(appRoot, "source-original"));
  symlinkSync(outside, path.join(appRoot, "src"), kind);
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1" }), unavailable("path"));
});
