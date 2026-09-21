import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { createHash } from "node:crypto";
import { prepareRegression } from "./prepare-regression.mjs";

function fixture(t, target = "x86_64-pc-windows-msvc") {
  const root = mkdtempSync(path.join(os.tmpdir(), "march7-regression-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const docsDirectory = path.join(root, "docs");
  mkdirSync(docsDirectory);
  for (const name of ["README.md", "CHECKLIST.md", "RESULT-TEMPLATE.md"]) writeFileSync(path.join(docsDirectory, name), `Fixture ${name}\n`);
  const payloadPath = path.join(root, target.startsWith("x86") ? "march-7th-app.exe" : "March 7th.app.zip");
  writeFileSync(payloadPath, "test payload");
  return { target, payloadPath, docsDirectory, outputDirectory: path.join(root, "output"), commit: "a".repeat(40), version: "0.2.0", runUrl: null };
}

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
