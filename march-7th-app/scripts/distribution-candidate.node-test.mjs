import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { chmodSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { assertMacExecutableMode, prepareDistribution, verifyDistributionPacket, verifyCandidatePair } from "./distribution-candidate.mjs";

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
  const docsRoot = path.join(root, "docs");
  const docsDirectory = path.join(docsRoot, "distribution-candidate");
  mkdirSync(docsDirectory, { recursive: true });
  mkdirSync(path.join(docsRoot, "regression"));
  writeFileSync(path.join(docsDirectory, "README.md"), "[contract](DISTRIBUTION-READINESS.md)");
  writeFileSync(path.join(docsDirectory, "CHECKLIST.md"), "[regression](REGRESSION-CHECKLIST.md)");
  writeFileSync(path.join(docsDirectory, "RESULT-TEMPLATE.md"), "Result template");
  writeFileSync(path.join(docsRoot, "DISTRIBUTION-READINESS.md"), "Distribution limits");
  writeFileSync(path.join(docsRoot, "regression/CHECKLIST.md"), "Desktop regression checklist");
  writeFileSync(payloadPath, windows ? nativeBytes(target) : Buffer.from("fixture DMG bytes"));
  writeFileSync(builtExecutablePath, nativeBytes(target));
  writeFileSync(packagedExecutablePath, nativeBytes(target));
  if (!windows) chmodSync(packagedExecutablePath, 0o755);
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

test("Mac program requires executable permission before a candidate is written", t => {
  assert.throws(() => assertMacExecutableMode(0o100644), /not executable/);
  assert.doesNotThrow(() => assertMacExecutableMode(0o100755));
  if (process.platform === "darwin") {
    const options = fixture(t, "aarch64-apple-darwin");
    chmodSync(options.packagedExecutablePath, 0o644);
    assert.throws(() => prepareDistribution(options), /not executable/);
    assert.equal(existsSync(options.outputDirectory), false);
  }
});

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
    assert.deepEqual(manifest.documents.map(document => document.name), [
      "README.md", "CHECKLIST.md", "RESULT-TEMPLATE.md", "DISTRIBUTION-READINESS.md", "REGRESSION-CHECKLIST.md",
    ]);
    for (const document of manifest.documents) {
      const bytes = readFileSync(path.join(options.outputDirectory, document.name));
      assert.equal(bytes.length, document.bytes);
      assert.equal(hash(bytes), document.sha256);
    }
    for (const name of ["README.md", "CHECKLIST.md"]) {
      const markdown = readFileSync(path.join(options.outputDirectory, name), "utf8");
      for (const [, link] of markdown.matchAll(/\]\(([^)]+\.md)\)/g)) {
        assert.equal(existsSync(path.resolve(options.outputDirectory, link)), true, `Broken packet link: ${link}`);
      }
    }
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

test("accepts only Tauri's NSIS bundle marker rewrite in the packaged Windows program", t => {
  const options = fixture(t, "x86_64-pc-windows-msvc");
  const built = Buffer.concat([nativeBytes(options.target), Buffer.from("__TAURI_BUNDLE_TYPE_VAR_UNK")]);
  const packaged = Buffer.from(built);
  packaged.write("NSS", packaged.length - 3, "ascii");
  writeFileSync(options.builtExecutablePath, built);
  writeFileSync(options.packagedExecutablePath, packaged);
  assert.equal(prepareDistribution(options).packagedExecutable.sha256, hash(packaged));

  const changed = fixture(t, "x86_64-pc-windows-msvc");
  writeFileSync(changed.builtExecutablePath, built);
  packaged[80] ^= 1;
  writeFileSync(changed.packagedExecutablePath, packaged);
  assert.throws(() => prepareDistribution(changed), /differs/);
  assert.equal(existsSync(changed.outputDirectory), false);
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

test("missing distribution or regression document cannot produce an incomplete packet", t => {
  const options = fixture(t, "x86_64-pc-windows-msvc");
  rmSync(path.join(options.docsDirectory, "../DISTRIBUTION-READINESS.md"));
  assert.throws(() => prepareDistribution(options), /Missing candidate document|ENOENT/);
  assert.equal(existsSync(options.outputDirectory), false);
});

test("the real candidate documents keep all local Markdown links inside the packet", t => {
  const options = fixture(t, "x86_64-pc-windows-msvc");
  const docsDirectory = fileURLToPath(new URL("../../docs/development/app/distribution-candidate/", import.meta.url));
  const manifest = prepareDistribution({ ...options, docsDirectory });
  for (const document of manifest.documents) {
    const markdown = readFileSync(path.join(options.outputDirectory, document.name), "utf8");
    for (const [, href] of markdown.matchAll(/\]\(([^)]+)\)/g)) {
      if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("#")) continue;
      const local = href.split("#", 1)[0];
      if (local.endsWith(".md")) assert.equal(existsSync(path.resolve(options.outputDirectory, local)), true, `Broken ${document.name} link: ${href}`);
    }
  }
  writeFileSync(path.join(options.outputDirectory, "DISTRIBUTION-READINESS.md"), "changed");
  assert.throws(() => verifyDistributionPacket(options.outputDirectory), /hash|size/);
});

test("will not overwrite an existing candidate directory", t => {
  const options = fixture(t, "x86_64-pc-windows-msvc");
  mkdirSync(options.outputDirectory);
  writeFileSync(path.join(options.outputDirectory, "keep"), "keep");
  assert.throws(() => prepareDistribution(options), /EEXIST/);
  assert.equal(readFileSync(path.join(options.outputDirectory, "keep"), "utf8"), "keep");
});
