import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, appendFileSync, rmSync, symlinkSync } from "node:fs";
import { execFileSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { summarizeFrontendIgnored } from "./diagnose-frontend-ignore.mjs";

function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), "march7-ignore-class-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const appRoot = path.join(root, "app");
  mkdirSync(path.join(appRoot, "src/nested"), { recursive: true });
  execFileSync("git", ["init", "-q"], { cwd: root, stdio: "pipe" });
  writeFileSync(path.join(root, ".gitignore"), "*.log\nnode_modules/\n");
  writeFileSync(path.join(appRoot, ".gitignore"), "*.local\n");
  writeFileSync(path.join(appRoot, "src/nested/.gitignore"), "PRIVATE_PATTERN*\n");
  writeFileSync(path.join(root, ".git/info/exclude"), "GLOBAL_PRIVATE*\n");
  return { root, appRoot };
}
const phase = "after-check";
const unavailable = `ignored-input phase=${phase} state=unavailable total=unknown groups=unknown overflow=no`;

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
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: failedGit }), unavailable);
  assert.equal(summarizeFrontendIgnored({ appRoot, phase: "PRIVATE\nPHASE", enabled: "1", git: failedGit }), "ignored-input phase=unknown state=unavailable total=unknown groups=unknown overflow=no");
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: () => Buffer.from("PRIVATE_BAD_NUL") }), unavailable);
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
  let query = 0;
  const malformed = () => Buffer.from(++query === 1 ? root + "\n" : query === 2 ? "!! app/src/PRIVATE\nNAME.local\0" : "PRIVATE_RULE_SOURCE\0NaN\0PRIVATE_PATTERN\0src/PRIVATE\nNAME.local\0");
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: malformed }), unavailable);
  assert.equal(query, 3, "malformed rule output must be reached after a valid root and status");
});

function injectedRecords(root, status, ruleOutput) {
  const calls = [];
  const git = args => {
    calls.push(args[0]);
    if (args[0] === "rev-parse") return Buffer.from(root + "\n");
    if (args[0] === "status") return Buffer.isBuffer(status) ? status : Buffer.from(status);
    if (args[0] === "check-ignore") return Buffer.from(ruleOutput);
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
    assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: probe.git }), unavailable);
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
    const probe = injectedRecords(root, status, "app/.gitignore\0" + "1\0*.local\0src/PRIVATE.local\0");
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
    "app/.gitignore\0" + "1\0*.local\0src/PRIVATE.local", // Missing terminating NUL.
    "app/.gitignore\0" + "1\0*.local\0", // Missing path field.
    "app/.gitignore\0" + "0\0*.local\0src/PRIVATE.local\0",
    "app/.gitignore\0" + "NaN\0*.local\0src/PRIVATE.local\0",
    "\0" + "1\0*.local\0src/PRIVATE.local\0",
    "app/.gitignore\0" + "1\0\0src/PRIVATE.local\0",
    "app/.gitignore\0" + "1\0!*.local\0src/PRIVATE.local\0",
    "app/.gitignore\0" + "1\0*.local\0src/PRIVATE-other.local\0",
  ]) {
    const probe = injectedRecords(root, "!! app/src/PRIVATE.local\0", record);
    assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: probe.git }), unavailable);
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
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: oversized }), unavailable);
  assert.equal(calls, 2);
});
