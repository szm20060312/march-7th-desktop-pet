import { test } from "node:test";
import assert from "node:assert/strict";
import { normalizeLicenseInventory } from "./dependency-licenses.mjs";

test("records installed npm and resolved Cargo dependencies without machine paths or inferred permissions", () => {
  const npm = {
    MIT: [{ name: "z", versions: ["1.0.0"], paths: ["C:/private/path"], license: "MIT" }],
    UNLICENSED: [{ name: "a", versions: ["2.0.0"], paths: ["C:/private/path"], license: "UNLICENSED" }],
  };
  const cargo = { packages: [
    { name: "march-7th-app", version: "0.2.0", license: null, source: null },
    { name: "serde", version: "1.0.0", license: "MIT OR Apache-2.0", source: "registry+https://github.com/rust-lang/crates.io-index" },
  ] };
  const inventory = normalizeLicenseInventory(npm, cargo, "a".repeat(40), "x86_64-pc-windows-msvc", "npmhash", "cargohash");
  assert.deepEqual(inventory.npm.map(p => p.name), ["a", "z"]);
  assert.equal(inventory.npm[0].license, null);
  assert.deepEqual(inventory.cargo, [{ name: "serde", version: "1.0.0", license: "MIT OR Apache-2.0" }]);
  assert.equal(inventory.unresolvedLicenseCount, 1);
  assert.equal(JSON.stringify(inventory).includes("C:/private"), false);
  assert.equal(inventory.sourceCommit, "a".repeat(40));
});
