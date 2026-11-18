import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, realpathSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const fullCommit = /^[a-f0-9]{40}$/;
const targets = new Set(["x86_64-pc-windows-msvc", "aarch64-apple-darwin"]);
const byNameVersion = (a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version);

export function normalizeLicenseInventory(npmLicenses, cargoMetadata, sourceCommit, target, npmLockSha256, cargoLockSha256) {
  if (!fullCommit.test(sourceCommit) || !targets.has(target)) throw new Error("Invalid dependency inventory identity");
  if (!npmLicenses || typeof npmLicenses !== "object" || !Array.isArray(cargoMetadata?.packages)) throw new Error("Invalid dependency metadata");
  const npm = Object.entries(npmLicenses).flatMap(([group, entries]) => {
    if (!Array.isArray(entries)) throw new Error("Invalid npm license group");
    return entries.flatMap(entry => {
      if (typeof entry.name !== "string" || !Array.isArray(entry.versions)) throw new Error("Invalid npm license entry");
      const license = entry.license && entry.license !== "UNLICENSED" ? entry.license : group === "UNLICENSED" ? null : group;
      return entry.versions.map(version => ({ name: entry.name, version, license }));
    });
  }).sort(byNameVersion);
  const cargo = cargoMetadata.packages.filter(pkg => pkg.source !== null).map(pkg => ({
    name: pkg.name, version: pkg.version, license: pkg.license || null,
  })).sort(byNameVersion);
  if (!npm.length || !cargo.length) throw new Error("Empty dependency inventory");
  return {
    schemaVersion: 1, sourceCommit, target,
    scope: "npm packages installed on this native runner and Cargo resolved dependency graph; license expressions require human obligation review",
    lockfiles: { pnpmSha256: npmLockSha256, cargoSha256: cargoLockSha256 },
    npm, cargo, unresolvedLicenseCount: [...npm, ...cargo].filter(pkg => pkg.license === null).length,
  };
}

function isCliEntry() {
  if (!process.argv[1]) return false;
  try { return realpathSync.native(process.argv[1]) === realpathSync.native(fileURLToPath(import.meta.url)); }
  catch { return false; }
}

if (isCliEntry()) {
  try {
    const [target, npmPath, cargoPath, outputPath] = process.argv.slice(2);
    if (process.argv.length !== 6) throw new Error("Usage: dependency-licenses.mjs <target> <pnpm-licenses-json> <cargo-metadata-json> <new-output-json>");
    const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
    const git = args => execFileSync("git", args, { cwd: appRoot, encoding: "utf8" }).trim();
    if (git(["status", "--porcelain", "--untracked-files=no"])) throw new Error("Tracked application inputs are modified");
    const inventory = normalizeLicenseInventory(
      JSON.parse(readFileSync(npmPath, "utf8")), JSON.parse(readFileSync(cargoPath, "utf8")),
      git(["rev-parse", "HEAD"]), target,
      sha256(readFileSync(path.join(appRoot, "pnpm-lock.yaml"))),
      sha256(readFileSync(path.join(appRoot, "src-tauri/Cargo.lock"))),
    );
    writeFileSync(outputPath, JSON.stringify(inventory, null, 2) + "\n", { flag: "wx" });
    console.log(`Recorded ${inventory.npm.length} npm and ${inventory.cargo.length} Cargo entries; ${inventory.unresolvedLicenseCount} missing license expressions`);
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
