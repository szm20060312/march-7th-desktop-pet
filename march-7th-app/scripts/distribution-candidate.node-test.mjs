import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { prepareDistribution, verifyDistributionPacket, verifyCandidatePair } from "./distribution-candidate.mjs";

const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const commit = "a".repeat(40);

function nativeBytes(target) {
  const bytes = Buffer.alloc(128);
  if (target.startsWith("x86")) {
    bytes.write("MZ");
    bytes.writeUInt32LE(64, 0x3c);
    bytes.write("PE\0\0", 64, "binary");
    bytes.writeUInt16LE(0x8664, 68);
  } else {
    bytes.writeUInt32LE(0xfeedfacf, 0);
    bytes.writeUInt32LE(0x0100000c, 4);
  }
  return bytes;
}

function fixture(t, target) {
  const root = mkdtempSync(path.join(os.tmpdir(), "march7-candidate-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const windows = target.startsWith("x86");
  const payloadPath = path.join(root, windows ? "app-setup.exe" : "app.dmg");
  const builtExecutablePath = path.join(root, windows ? "build.exe" : "build");
  const packagedExecutablePath = path.join(root, windows ? "extracted.exe" : "mounted");
  const docsDirectory = path.join(root, "docs");
  mkdirSync(docsDirectory);
  for (const name of ["README.md", "CHECKLIST.md", "RESULT-TEMPLATE.md"]) writeFileSync(path.join(docsDirectory, name), name);
  writeFileSync(payloadPath, windows ? nativeBytes(target) : Buffer.from("fixture DMG bytes"));
  writeFileSync(builtExecutablePath, nativeBytes(target));
  writeFileSync(packagedExecutablePath, nativeBytes(target));
  const licenseInventoryPath = path.join(root, "licenses.json");
  writeFileSync(licenseInventoryPath, JSON.stringify({ schemaVersion: 1, sourceCommit: commit, target, npm: [], cargo: [] }));
  return {
    target, payloadPath, builtExecutablePath, packagedExecutablePath, docsDirectory, licenseInventoryPath,
    outputDirectory: path.join(root, "candidate"), commit, version: "0.2.0", runUrl: null,
    packagedExecutableLocation: windows ? "NSIS/march-7th-app.exe" : "March 7th.app/Contents/MacOS/march-7th-app",
    bundleIdentifier: windows ? null : "com.szm.march7th",
    buildInfo: { schemaVersion: 1, appVersion: "0.2.0", target, sourceCommit: commit, sourceState: "clean" },
  };
}

test("records full identity, size and hash for installer and its verified embedded program", t => {
  for (const target of ["x86_64-pc-windows-msvc", "aarch64-apple-darwin"]) {
    const options = fixture(t, target);
    const manifest = prepareDistribution(options);
    assert.equal(manifest.sourceCommit, commit);
    assert.equal(manifest.target, target);
    assert.equal(manifest.applicationVersion, "0.2.0");
    assert.equal(manifest.validationStatus, "unsigned-build-candidate-awaiting-human-install-tests");
    assert.equal(manifest.package.bytes, readFileSync(options.payloadPath).length);
    assert.equal(manifest.package.sha256, hash(readFileSync(options.payloadPath)));
    assert.equal(manifest.packagedExecutable.bytes, readFileSync(options.builtExecutablePath).length);
    assert.equal(manifest.packagedExecutable.sha256, hash(readFileSync(options.builtExecutablePath)));
    assert.deepEqual(manifest.packagedExecutable.binaryBuildInfo, options.buildInfo);
    assert.equal(manifest.dependencyInventory.name, "DEPENDENCY-LICENSES.json");
    assert.equal(manifest.dependencyInventory.sha256, hash(readFileSync(options.licenseInventoryPath)));
    assert.equal(verifyDistributionPacket(options.outputDirectory).sourceCommit, commit);
  }
});

test("rejects stale embedded program, target mismatch and malformed native program before output", t => {
  const options = fixture(t, "x86_64-pc-windows-msvc");
  writeFileSync(options.packagedExecutablePath, Buffer.concat([nativeBytes(options.target), Buffer.from("stale")]));
  assert.throws(() => prepareDistribution(options), /differs/);
  assert.equal(existsSync(options.outputDirectory), false);
  writeFileSync(options.packagedExecutablePath, nativeBytes(options.target));
  assert.throws(() => prepareDistribution({ ...options, buildInfo: { ...options.buildInfo, target: "aarch64-apple-darwin" } }), /identity/);
  writeFileSync(options.packagedExecutablePath, Buffer.from("not PE"));
  assert.throws(() => prepareDistribution(options), /format|differs/);
  assert.equal(existsSync(options.outputDirectory), false);
});

test("packet verification detects changed bytes and candidate pairs require one full commit", t => {
  const win = fixture(t, "x86_64-pc-windows-msvc");
  const mac = fixture(t, "aarch64-apple-darwin");
  prepareDistribution(win);
  prepareDistribution(mac);
  assert.equal(verifyCandidatePair(win.outputDirectory, mac.outputDirectory, commit).sourceCommit, commit);
  assert.throws(() => verifyCandidatePair(win.outputDirectory, mac.outputDirectory, "b".repeat(40)), /commit/);
  const winManifest = JSON.parse(readFileSync(path.join(win.outputDirectory, "BUILD-INFO.json"), "utf8"));
  writeFileSync(path.join(win.outputDirectory, winManifest.package.name), "tampered");
  assert.throws(() => verifyDistributionPacket(win.outputDirectory), /hash|size/);
});

test("rejects dependency inventory from another commit or target", t => {
  const options = fixture(t, "x86_64-pc-windows-msvc");
  writeFileSync(options.licenseInventoryPath, JSON.stringify({ schemaVersion: 1, sourceCommit: "b".repeat(40), target: options.target, npm: [], cargo: [] }));
  assert.throws(() => prepareDistribution(options), /inventory/);
  assert.equal(existsSync(options.outputDirectory), false);
});

test("will not overwrite an existing candidate directory", t => {
  const options = fixture(t, "x86_64-pc-windows-msvc");
  mkdirSync(options.outputDirectory);
  writeFileSync(path.join(options.outputDirectory, "keep"), "keep");
  assert.throws(() => prepareDistribution(options), /EEXIST/);
  assert.equal(readFileSync(path.join(options.outputDirectory, "keep"), "utf8"), "keep");
});
