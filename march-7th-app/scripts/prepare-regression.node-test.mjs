import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync, copyFileSync, symlinkSync, linkSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { prepareRegression } from "./prepare-regression.mjs";

function cliAliases(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), "march7-cli-alias-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const realDirectory = path.join(root, "real scripts");
  const aliasDirectory = path.join(root, "alias scripts");
  mkdirSync(realDirectory);
  const script = path.join(realDirectory, "prepare-regression.mjs");
  copyFileSync(fileURLToPath(new URL("./prepare-regression.mjs", import.meta.url)), script);
  symlinkSync(realDirectory, aliasDirectory, process.platform === "win32" ? "junction" : "dir");
  const fileAlias = path.join(root, "file-alias.mjs");
  if (process.platform === "win32") linkSync(script, fileAlias); // No administrator-only symlink privilege needed.
  else symlinkSync(script, fileAlias, "file");
  return { root, scripts: [script, path.join(aliasDirectory, "prepare-regression.mjs"), fileAlias] };
}

test("CLI validates arguments through real paths, directory aliases and file aliases", t => {
  const { scripts } = cliAliases(t);
  for (const script of scripts) {
    const result = spawnSync(process.execPath, [script], { encoding: "utf8", timeout: 10_000 });
    assert.equal(result.status, 1, `CLI must run rather than silently skip: ${script}`);
    assert.match(result.stderr, /Usage: node scripts\/prepare-regression.mjs/);
    assert.equal(result.stdout, "");
  }
});

test("module imports through aliases never enter the CLI, including arbitrary eval argv", t => {
  const { root, scripts } = cliAliases(t);
  for (const script of scripts) {
    const source = `const module = await import(${JSON.stringify(pathToFileURL(script).href)}); if (typeof module.prepareRegression !== "function") throw Error("missing export"); console.log("import only");`;
    const result = spawnSync(process.execPath, ["--input-type=module", "-e", source, path.join(root, "nonexistent-argument")], { encoding: "utf8", timeout: 10_000 });
    assert.equal(result.status, 0);
    assert.equal(result.stdout.trim(), "import only");
    assert.equal(result.stderr, "");
  }
});

function fixture(t, target = "x86_64-pc-windows-msvc") {
  const root = mkdtempSync(path.join(os.tmpdir(), "march7-regression-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const docsDirectory = path.join(root, "docs");
  mkdirSync(docsDirectory);
  for (const name of ["README.md", "CHECKLIST.md", "RESULT-TEMPLATE.md"]) writeFileSync(path.join(docsDirectory, name), `Fixture ${name}\n`);
  const payloadPath = path.join(root, target.startsWith("x86") ? "march-7th-app.exe" : "March 7th.app.zip");
  let executablePath = payloadPath;
  if (target === "aarch64-apple-darwin") {
    executablePath = path.join(root, "March 7th.app/Contents/MacOS/march-7th-app");
    mkdirSync(path.dirname(executablePath), { recursive: true });
    writeFileSync(executablePath, "test payload");
    // Both supported native hosts ship bsdtar; generate a real ZIP, not a mock.
    const tar = process.platform === "win32" ? path.join(process.env.SystemRoot, "System32/tar.exe") : "/usr/bin/tar";
    execFileSync(tar, ["-a", "-cf", payloadPath, "-C", root, "March 7th.app"], { timeout: 10_000 });
  } else writeFileSync(payloadPath, "test payload");
  const buildInfo = { schemaVersion: 1, appVersion: "0.2.0", target, sourceCommit: "a".repeat(40), sourceState: "clean" };
  return { target, payloadPath, docsDirectory, outputDirectory: path.join(root, "output"), commit: "a".repeat(40), version: "0.2.0", runUrl: null, buildInfo, executablePath };
}

test("rejects a stale real Mac ZIP even when its rebuilt sibling executable has matching identity", t => {
  const options = fixture(t, "aarch64-apple-darwin");
  // Keep the old archive; replace the executable exactly as a rebuild would.
  writeFileSync(options.executablePath, "new! payload");
  assert.throws(() => prepareRegression(options), /archive.*executable/i);
  assert.equal(existsSync(options.outputDirectory), false);
});

test("rejects a Mac ZIP missing the probed member before creating a packet", t => {
  const options = fixture(t, "aarch64-apple-darwin");
  const executablePath = path.join(path.dirname(options.executablePath), "another-executable");
  writeFileSync(executablePath, "test payload");
  assert.throws(() => prepareRegression({ ...options, executablePath }), /archive.*executable/i);
  assert.equal(existsSync(options.outputDirectory), false);
});

test("refuses missing, modified, unknown, malformed or mismatched binary identities before writing", t => {
  const options = fixture(t);
  for (const buildInfo of [undefined, null, {},
    { ...options.buildInfo, schemaVersion: 2 },
    { ...options.buildInfo, sourceState: "modified" },
    { ...options.buildInfo, sourceState: "unknown", sourceCommit: null },
    { ...options.buildInfo, sourceCommit: "b".repeat(40) },
    { ...options.buildInfo, appVersion: "0.3.0" },
    { ...options.buildInfo, target: "aarch64-apple-darwin" },
  ]) assert.throws(() => prepareRegression({ ...options, buildInfo }), /identity/);
  assert.equal(existsSync(options.outputDirectory), false);
});

function rejectionDiagnostic(options) {
  let message;
  assert.throws(() => prepareRegression(options), error => {
    assert.match(error.message, /^Binary identity must match version, target and commit with clean application inputs/);
    message = error.message;
    return true;
  });
  assert.equal(existsSync(options.outputDirectory), false);
  assert.ok(message.length <= 1200, "diagnostic must remain bounded");
  assert.equal(message.split("\n").length, 1);
  assert.ok(message.includes("; diagnostic="), "identity rejection must include bounded whitelisted diagnostics");
  return { message, diagnostic: JSON.parse(message.split("; diagnostic=")[1]) };
}

test("identity rejection diagnostic identifies only the mismatching public fields", t => {
  const options = fixture(t);
  for (const patch of [
    { sourceCommit: "b".repeat(40) }, { target: "aarch64-apple-darwin" },
    { appVersion: "0.3.0" }, { schemaVersion: 2 }, { sourceState: "modified" },
    { sourceCommit: null, sourceState: "unknown" },
  ]) {
    const actual = { ...options.buildInfo, ...patch };
    const { diagnostic } = rejectionDiagnostic({ ...options, buildInfo: actual });
    assert.deepEqual(diagnostic.expected, options.buildInfo);
    assert.deepEqual(diagnostic.actual, actual);
    assert.deepEqual(diagnostic.mismatchedFields, Object.keys(options.buildInfo).filter(key => Object.hasOwn(patch, key)));
    assert.deepEqual(Object.keys(diagnostic), ["expected", "actual", "mismatchedFields"]);
  }
});

test("identity rejection diagnostic strips private extras and replaces invalid values without stringifying them", t => {
  const options = fixture(t);
  const secret = "DO_NOT_LOG_PRIVATE_FIXTURE";
  const actual = {
    schemaVersion: { config: secret }, appVersion: `0.2.0\n${secret}`,
    target: `C:/Users/${secret}`, sourceCommit: { token: secret }, sourceState: secret,
    environment: { token: secret }, privatePath: `/private/${secret}`,
    toJSON() { throw Error("raw object must never be stringified"); },
  };
  const { message, diagnostic } = rejectionDiagnostic({ ...options, buildInfo: actual });
  assert.equal(message.includes(secret), false);
  assert.equal(message.includes("privatePath"), false);
  assert.equal(message.includes("environment"), false);
  assert.deepEqual(diagnostic.actual, Object.fromEntries(Object.keys(options.buildInfo).map(key => [key, "<invalid>"])));
  assert.deepEqual(diagnostic.mismatchedFields, Object.keys(options.buildInfo));
});

test("identity rejection diagnostic bounds expected and actual values and labels missing or non-object payloads", t => {
  const options = fixture(t);
  const keys = Object.keys(options.buildInfo);
  const oversizedVersion = `0.2.0-${"x".repeat(2000)}`;
  const { diagnostic, message } = rejectionDiagnostic({ ...options, version: oversizedVersion, buildInfo: { ...options.buildInfo, target: "x-".repeat(100), appVersion: oversizedVersion, sourceState: "modified" } });
  assert.equal(diagnostic.expected.appVersion, "<invalid>");
  assert.equal(diagnostic.actual.appVersion, "<invalid>");
  assert.equal(diagnostic.actual.target, "<invalid>");
  assert.equal(message.includes(oversizedVersion), false);
  assert.deepEqual(diagnostic.mismatchedFields, ["target", "sourceState"]);
  for (const value of [undefined, {}, null, ["PRIVATE_ARRAY"], "PRIVATE_STRING", 42]) {
    const result = rejectionDiagnostic({ ...options, buildInfo: value });
    const marker = value === undefined || value?.constructor === Object ? "<missing>" : "<invalid>";
    assert.deepEqual(result.diagnostic.actual, Object.fromEntries(keys.map(key => [key, marker])));
    assert.deepEqual(result.diagnostic.mismatchedFields, keys);
    assert.equal(result.message.includes("PRIVATE_"), false);
  }
  const trailing = rejectionDiagnostic({ ...options, buildInfo: {
    ...options.buildInfo, appVersion: "0.2.0\n", target: `${options.target}\u2028`, sourceCommit: `${options.commit}\r`,
  } });
  for (const field of ["appVersion", "target", "sourceCommit"]) assert.equal(trailing.diagnostic.actual[field], "<invalid>");
});

for (const target of ["x86_64-pc-windows-msvc", "aarch64-apple-darwin"]) {
  test(`packages ${target} with a verifiable manifest and complete checklist`, t => {
    const options = fixture(t, target);
    prepareRegression(options);
    const manifest = JSON.parse(readFileSync(path.join(options.outputDirectory, "BUILD-INFO.json"), "utf8"));
    assert.equal(manifest.sourceCommit, options.commit);
    assert.equal(manifest.target, target);
    assert.equal(manifest.applicationVersion, "0.2.0");
    assert.equal(manifest.validationStatus, "build-only-awaiting-human-regression");
    assert.equal(manifest.files.length, 4);
    for (const file of manifest.files) {
      const bytes = readFileSync(path.join(options.outputDirectory, file.name));
      assert.equal(bytes.length, file.bytes);
      assert.equal(createHash("sha256").update(bytes).digest("hex"), file.sha256);
    }
    const sums = readFileSync(path.join(options.outputDirectory, "SHA256SUMS.txt"), "utf8").trim().split("\n");
    assert.equal(sums.length, 5);
    for (const line of sums) {
      const [hash, filename] = line.split("  ");
      assert.equal(createHash("sha256").update(readFileSync(path.join(options.outputDirectory, filename))).digest("hex"), hash);
    }
  });
}

test("rejects unknown targets or missing source identity before writing", t => {
  const options = fixture(t);
  assert.throws(() => prepareRegression({ ...options, target: "linux" }), /target/);
  assert.throws(() => prepareRegression({ ...options, commit: "main" }), /commit/);
  assert.equal(existsSync(options.outputDirectory), false);
});

test("requires a nonempty platform payload and all human testing documents", t => {
  const options = fixture(t);
  writeFileSync(options.payloadPath, "");
  assert.throws(() => prepareRegression(options), /empty/);
  writeFileSync(options.payloadPath, "test payload");
  rmSync(path.join(options.docsDirectory, "CHECKLIST.md"));
  assert.throws(() => prepareRegression(options), /ENOENT/);
  assert.equal(existsSync(options.outputDirectory), false);
});

test("never overwrites an existing output directory", t => {
  const options = fixture(t);
  mkdirSync(options.outputDirectory);
  writeFileSync(path.join(options.outputDirectory, "keep.txt"), "keep");
  assert.throws(() => prepareRegression(options), /EEXIST/);
  assert.equal(readFileSync(path.join(options.outputDirectory, "keep.txt"), "utf8"), "keep");
});
