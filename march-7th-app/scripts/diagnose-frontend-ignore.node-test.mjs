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
  const { appRoot } = fixture(t);
  for (let i = 0; i < 40; i++) writeFileSync(path.join(appRoot, `src/PRIVATE-${i}.local`), "private");
  if (process.platform !== "win32") writeFileSync(path.join(appRoot, "src/PRIVATE\nnewline.local"), "private");
  const line = summarizeFrontendIgnored({ appRoot, phase, enabled: "1" });
  assert.equal(line, `ignored-input phase=${phase} state=ok total=many groups=app-rules:local-config:file:many overflow=no`);
  let query = 0;
  const malformed = () => Buffer.from(++query === 1 ? "src/PRIVATE\nNAME.local\0" : "PRIVATE_RULE_SOURCE\0NaN\0PRIVATE_PATTERN\0src/PRIVATE\nNAME.local\0");
  assert.equal(summarizeFrontendIgnored({ appRoot, phase, enabled: "1", git: malformed }), unavailable);
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
