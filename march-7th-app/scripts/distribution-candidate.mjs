import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readBinaryBuildInfo } from "./prepare-regression.mjs";

const targets = {
  "x86_64-pc-windows-msvc": { platform: "windows-x64", extension: ".exe", kind: "nsis" },
  "aarch64-apple-darwin": { platform: "macos-arm64", extension: ".dmg", kind: "dmg" },
};
const documents = [
  { name: "README.md", source: directory => path.join(directory, "README.md") },
  { name: "CHECKLIST.md", source: directory => path.join(directory, "CHECKLIST.md") },
  { name: "RESULT-TEMPLATE.md", source: directory => path.join(directory, "RESULT-TEMPLATE.md") },
  { name: "DISTRIBUTION-READINESS.md", source: directory => path.resolve(directory, "../DISTRIBUTION-READINESS.md") },
  { name: "REGRESSION-CHECKLIST.md", source: directory => path.resolve(directory, "../regression/CHECKLIST.md") },
];
const fullCommit = /^[a-f0-9]{40}$/;
const versionPattern = /^\d+\.\d+\.\d+(?:[-+][\w.-]+)?$/;
const digest = bytes => createHash("sha256").update(bytes).digest("hex");
const fileRecord = (name, bytes) => ({ name, bytes: bytes.length, sha256: digest(bytes) });

function ensureNativeProgram(bytes, target) {
  if (target === "x86_64-pc-windows-msvc") {
    const offset = bytes.length >= 64 ? bytes.readUInt32LE(0x3c) : -1;
    if (bytes.toString("ascii", 0, 2) !== "MZ" || offset < 64 || offset + 6 > bytes.length
      || bytes.toString("binary", offset, offset + 4) !== "PE\0\0" || bytes.readUInt16LE(offset + 4) !== 0x8664) {
      throw new Error("Packaged program format is not x64 PE");
    }
  } else if (target === "aarch64-apple-darwin") {
    if (bytes.length < 8 || bytes.readUInt32LE(0) !== 0xfeedfacf || bytes.readUInt32LE(4) !== 0x0100000c) {
      throw new Error("Packaged program format is not arm64 Mach-O");
    }
  } else throw new Error("Unsupported target");
}

function ensureInstallerFormat(bytes, target) {
  if (target === "x86_64-pc-windows-msvc" && bytes.toString("ascii", 0, 2) !== "MZ") {
    throw new Error("NSIS candidate is not a PE executable");
  }
}

function samePackagedProgram(built, packaged, target) {
  if (built.equals(packaged)) return true;
  if (target !== "x86_64-pc-windows-msvc" || built.length !== packaged.length) return false;
  // Tauri rewrites this one bundle-type marker in the copy placed inside NSIS.
  const before = Buffer.from("__TAURI_BUNDLE_TYPE_VAR_UNK");
  const after = Buffer.from("__TAURI_BUNDLE_TYPE_VAR_NSS");
  const offset = built.indexOf(before);
  if (offset < 0 || offset !== packaged.indexOf(after)
    || built.lastIndexOf(before) !== offset || packaged.lastIndexOf(after) !== offset) return false;
  const restored = Buffer.from(packaged);
  before.copy(restored, offset);
  return restored.equals(built);
}

export function assertMacExecutableMode(mode) {
  if ((mode & 0o111) === 0) throw new Error("Packaged macOS program is not executable");
}

export function prepareDistribution({ target, payloadPath, builtExecutablePath, packagedExecutablePath, packagedExecutableLocation,
  docsDirectory, licenseInventoryPath, outputDirectory, commit, version, runUrl, buildInfo, bundleIdentifier, bundleVersion }) {
  const spec = targets[target];
  if (!spec) throw new Error("Unsupported target");
  if (!fullCommit.test(commit)) throw new Error("A full source commit is required");
  if (!versionPattern.test(version)) throw new Error("Invalid version");
  if (!buildInfo || buildInfo.schemaVersion !== 1 || buildInfo.appVersion !== version || buildInfo.target !== target
    || buildInfo.sourceCommit !== commit || buildInfo.sourceState !== "clean") throw new Error("Built program identity does not match the clean source commit, version and target");
  if (path.extname(payloadPath).toLowerCase() !== spec.extension) throw new Error("Unexpected installer extension");
  if (!packagedExecutableLocation || packagedExecutableLocation.includes("..") || !packagedExecutableLocation.endsWith(target.startsWith("x86") ? "/march-7th-app.exe" : "/march-7th-app")) {
    throw new Error("Unexpected packaged program location");
  }
  if (target === "aarch64-apple-darwin" && bundleIdentifier !== "com.szm.march7th") throw new Error("Unexpected macOS bundle identifier");
  if (target === "aarch64-apple-darwin" && bundleVersion !== undefined && bundleVersion !== version) throw new Error("Unexpected macOS bundle version");
  if (target.startsWith("x86") && bundleIdentifier !== null) throw new Error("Unexpected Windows bundle identifier");
  if (existsSync(outputDirectory)) throw new Error("EEXIST: candidate output directory already exists");
  const payload = readFileSync(payloadPath);
  const builtProgram = readFileSync(builtExecutablePath);
  const packagedProgram = readFileSync(packagedExecutablePath);
  if (!payload.length || !builtProgram.length) throw new Error("Missing or empty candidate input");
  ensureInstallerFormat(payload, target);
  ensureNativeProgram(builtProgram, target);
  ensureNativeProgram(packagedProgram, target);
  if (!samePackagedProgram(builtProgram, packagedProgram, target)) throw new Error("Packaged program differs from the probed build program");
  if (target === "aarch64-apple-darwin" && process.platform === "darwin") assertMacExecutableMode(statSync(packagedExecutablePath).mode);
  const inventoryBytes = readFileSync(licenseInventoryPath);
  const inventory = JSON.parse(inventoryBytes.toString("utf8"));
  if (inventory.schemaVersion !== 1 || inventory.sourceCommit !== commit || inventory.target !== target
    || !Array.isArray(inventory.npm) || !Array.isArray(inventory.cargo)) throw new Error("Dependency inventory identity does not match candidate");
  const documentInputs = documents.map(document => {
    const source = document.source(docsDirectory);
    if (!statSync(source).isFile() || !statSync(source).size) throw new Error(`Missing candidate document ${document.name}`);
    const bytes = readFileSync(source);
    return { ...fileRecord(document.name, bytes), bytesContent: bytes };
  });

  const candidateName = `March-7th-UNSIGNED-CANDIDATE-${spec.platform}-${commit}${spec.extension}`;
  const manifest = {
    schemaVersion: 1, candidateKind: spec.kind, applicationVersion: version, sourceCommit: commit, target,
    buildRunUrl: runUrl ?? null, builtAt: new Date().toISOString(),
    validationStatus: "unsigned-build-candidate-awaiting-human-install-tests",
    signingStatus: "no-production-signing-or-notarization-configured",
    installMode: spec.kind === "nsis" ? "currentUser" : "drag-app-to-Applications",
    webviewInstallMode: spec.kind === "nsis" ? "downloadBootstrapper" : null,
    package: fileRecord(candidateName, payload),
    packagedExecutable: {
      location: packagedExecutableLocation, bytes: packagedProgram.length, sha256: digest(packagedProgram),
      binaryBuildInfo: Object.fromEntries(["schemaVersion", "appVersion", "target", "sourceCommit", "sourceState"].map(key => [key, buildInfo[key]])),
    },
    bundleIdentifier, dependencyInventory: fileRecord("DEPENDENCY-LICENSES.json", inventoryBytes),
    documents: documentInputs.map(({ name, bytes, sha256 }) => ({ name, bytes, sha256 })),
  };
  mkdirSync(outputDirectory);
  copyFileSync(payloadPath, path.join(outputDirectory, candidateName));
  const sums = [manifest.package];
  for (const document of documentInputs) {
    writeFileSync(path.join(outputDirectory, document.name), document.bytesContent);
    sums.push(fileRecord(document.name, document.bytesContent));
  }
  copyFileSync(licenseInventoryPath, path.join(outputDirectory, "DEPENDENCY-LICENSES.json"));
  sums.push(manifest.dependencyInventory);
  const manifestBytes = Buffer.from(JSON.stringify(manifest, null, 2) + "\n");
  writeFileSync(path.join(outputDirectory, "BUILD-INFO.json"), manifestBytes);
  sums.push(fileRecord("BUILD-INFO.json", manifestBytes));
  writeFileSync(path.join(outputDirectory, "SHA256SUMS.txt"), sums.map(record => `${record.sha256}  ${record.name}`).join("\n") + "\n");
  return manifest;
}

export function verifyDistributionPacket(directory) {
  const manifest = JSON.parse(readFileSync(path.join(directory, "BUILD-INFO.json"), "utf8"));
  if (manifest.schemaVersion !== 1 || !targets[manifest.target] || !fullCommit.test(manifest.sourceCommit)
    || manifest.packagedExecutable?.binaryBuildInfo?.sourceCommit !== manifest.sourceCommit
    || manifest.packagedExecutable.binaryBuildInfo.target !== manifest.target
    || manifest.packagedExecutable.binaryBuildInfo.appVersion !== manifest.applicationVersion
    || manifest.packagedExecutable.binaryBuildInfo.sourceState !== "clean") throw new Error("Invalid candidate identity manifest");
  const sums = readFileSync(path.join(directory, "SHA256SUMS.txt"), "utf8").trimEnd().split("\n");
  const spec = targets[manifest.target];
  if (manifest.package?.name !== `March-7th-UNSIGNED-CANDIDATE-${spec.platform}-${manifest.sourceCommit}${spec.extension}`
    || manifest.dependencyInventory?.name !== "DEPENDENCY-LICENSES.json") throw new Error("Invalid candidate file names");
  const documentNames = documents.map(document => document.name);
  if (!Array.isArray(manifest.documents) || manifest.documents.length !== documentNames.length
    || manifest.documents.some((document, index) => document.name !== documentNames[index])) throw new Error("Invalid candidate document manifest");
  const expectedNames = [manifest.package.name, ...documentNames, "DEPENDENCY-LICENSES.json", "BUILD-INFO.json"];
  if (sums.length !== expectedNames.length) throw new Error("Invalid candidate checksum list");
  for (let i = 0; i < sums.length; i++) {
    const match = /^([a-f0-9]{64})  (.+)$/.exec(sums[i]);
    if (!match || match[2] !== expectedNames[i]) throw new Error("Invalid candidate checksum entry");
    const bytes = readFileSync(path.join(directory, expectedNames[i]));
    if (digest(bytes) !== match[1]) throw new Error(`Candidate hash mismatch: ${expectedNames[i]}`);
    if (i === 0 && (bytes.length !== manifest.package.bytes || digest(bytes) !== manifest.package.sha256)) throw new Error("Candidate package size or hash mismatch");
    const documentIndex = documentNames.indexOf(expectedNames[i]);
    if (documentIndex !== -1 && (bytes.length !== manifest.documents[documentIndex].bytes
      || digest(bytes) !== manifest.documents[documentIndex].sha256)) throw new Error(`Candidate document size or hash mismatch: ${expectedNames[i]}`);
    if (expectedNames[i] === "DEPENDENCY-LICENSES.json") {
      if (bytes.length !== manifest.dependencyInventory.bytes || digest(bytes) !== manifest.dependencyInventory.sha256) throw new Error("Dependency inventory size or hash mismatch");
      const inventory = JSON.parse(bytes.toString("utf8"));
      if (inventory.sourceCommit !== manifest.sourceCommit || inventory.target !== manifest.target) throw new Error("Dependency inventory commit or target mismatch");
    }
  }
  return manifest;
}

export function verifyCandidatePair(windowsDirectory, macDirectory, expectedCommit) {
  if (!fullCommit.test(expectedCommit)) throw new Error("Expected full commit is required");
  const windows = verifyDistributionPacket(windowsDirectory);
  const mac = verifyDistributionPacket(macDirectory);
  if (windows.target !== "x86_64-pc-windows-msvc" || mac.target !== "aarch64-apple-darwin"
    || windows.sourceCommit !== expectedCommit || mac.sourceCommit !== expectedCommit
    || windows.applicationVersion !== mac.applicationVersion) throw new Error("Candidate pair commit, target or version mismatch");
  return { sourceCommit: expectedCommit, applicationVersion: windows.applicationVersion,
    windowsPackageSha256: windows.package.sha256, macPackageSha256: mac.package.sha256 };
}

function children(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(item => {
    const entry = path.join(directory, item.name);
    return item.isDirectory() ? children(entry) : item.isFile() ? [entry] : [];
  });
}

function oneCandidate(directory, extension) {
  const paths = readdirSync(directory).filter(name => name.toLowerCase().endsWith(extension)).map(name => path.join(directory, name));
  if (paths.length !== 1) throw new Error(`Expected exactly one ${extension} installer in ${directory}`);
  return paths[0];
}

function inspectNativePackage(target, payloadPath, temporaryDirectory) {
  if (target === "x86_64-pc-windows-msvc") {
    if (process.platform !== "win32") throw new Error("NSIS inspection requires Windows");
    const bundledSevenZip = path.join(process.env.ProgramFiles ?? "C:\\Program Files", "7-Zip/7z.exe");
    const sevenZip = existsSync(bundledSevenZip) ? bundledSevenZip : "7z.exe";
    execFileSync(sevenZip, ["x", "-y", `-o${temporaryDirectory}`, path.resolve(payloadPath)], { timeout: 120_000, windowsHide: true });
    const programs = children(temporaryDirectory).filter(entry => path.basename(entry).toLowerCase() === "march-7th-app.exe");
    if (programs.length !== 1) throw new Error("NSIS candidate must contain exactly one main program");
    return { packagedExecutablePath: programs[0], packagedExecutableLocation: `NSIS/${path.relative(temporaryDirectory, programs[0]).replaceAll("\\", "/")}`, bundleIdentifier: null };
  }
  if (process.platform !== "darwin") throw new Error("DMG inspection requires macOS");
  execFileSync("hdiutil", ["verify", path.resolve(payloadPath)], { timeout: 120_000 });
  const mount = path.join(temporaryDirectory, "mount");
  mkdirSync(mount);
  execFileSync("hdiutil", ["attach", "-readonly", "-nobrowse", "-noautoopen", "-mountpoint", mount, path.resolve(payloadPath)], { timeout: 120_000 });
  try {
    const bundle = path.join(mount, "March 7th.app");
    const plist = path.join(bundle, "Contents/Info.plist");
    const plutil = key => execFileSync("plutil", ["-extract", key, "raw", "-o", "-", plist], { encoding: "utf8", timeout: 10_000 }).trim();
    const executableName = plutil("CFBundleExecutable");
    if (!/^[A-Za-z0-9._-]+$/.test(executableName)) throw new Error("Invalid macOS executable name");
    const original = path.join(bundle, "Contents/MacOS", executableName);
    const snapshot = path.join(temporaryDirectory, executableName);
    copyFileSync(original, snapshot);
    const mode = statSync(original).mode;
    // copyFileSync does not guarantee preservation of mode on every host.
    if ((mode & 0o111) === 0) throw new Error("DMG program is not executable");
    chmodSync(snapshot, mode & 0o777);
    return { packagedExecutablePath: snapshot, packagedExecutableLocation: `March 7th.app/Contents/MacOS/${executableName}`,
      bundleIdentifier: plutil("CFBundleIdentifier"), bundleVersion: plutil("CFBundleShortVersionString") };
  } finally {
    execFileSync("hdiutil", ["detach", mount], { timeout: 120_000 });
  }
}

function isCliEntry() {
  if (!process.argv[1]) return false;
  try { return realpathSync.native(process.argv[1]) === realpathSync.native(fileURLToPath(import.meta.url)); }
  catch { return false; }
}

if (isCliEntry()) {
  try {
    const [command, ...args] = process.argv.slice(2);
    if (command === "verify-pair" && args.length === 3) {
      console.log(JSON.stringify(verifyCandidatePair(...args)));
    } else if (command === "verify-one" && args.length === 1) {
      const info = verifyDistributionPacket(args[0]);
      console.log(`${info.target} ${info.sourceCommit} ${info.package.sha256}`);
    } else if (command === "prepare" && args.length === 5) {
      const [target, installerDirectory, builtExecutablePath, licenseInventoryPath, outputDirectory] = args;
      if (!targets[target]) throw new Error("Unsupported target");
      const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
      const git = values => execFileSync("git", values, { cwd: appRoot, encoding: "utf8" }).trim();
      if (git(["status", "--porcelain", "--untracked-files=no"])) throw new Error("Tracked application inputs are modified");
      const pkg = JSON.parse(readFileSync(path.join(appRoot, "package.json"), "utf8"));
      const config = JSON.parse(readFileSync(path.join(appRoot, "src-tauri/tauri.conf.json"), "utf8"));
      if (pkg.version !== config.version) throw new Error("Frontend and Tauri versions differ");
      if (config.bundle?.windows?.nsis?.installMode !== "currentUser" || config.bundle?.windows?.webviewInstallMode?.type !== "downloadBootstrapper") throw new Error("Windows installer configuration differs from candidate contract");
      const commit = git(["rev-parse", "HEAD"]);
      const payloadPath = oneCandidate(installerDirectory, targets[target].extension);
      const temp = mkdtempSync(path.join(os.tmpdir(), "march7-inspect-"));
      try {
        const packageInfo = inspectNativePackage(target, payloadPath, temp);
        const runUrl = process.env.GITHUB_RUN_ID ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}` : null;
        const manifest = prepareDistribution({ target, payloadPath, builtExecutablePath, ...packageInfo, outputDirectory,
          docsDirectory: path.join(appRoot, "../docs/development/app/distribution-candidate"), licenseInventoryPath, commit, version: pkg.version, runUrl,
          buildInfo: readBinaryBuildInfo(builtExecutablePath) });
        console.log(`Prepared ${manifest.target} candidate at ${manifest.sourceCommit}`);
      } finally { rmSync(temp, { recursive: true, force: true }); }
    } else throw new Error("Usage: distribution-candidate.mjs prepare <target> <installer-directory> <built-program> <license-inventory> <new-output-directory> | verify-one <packet-directory> | verify-pair <windows-directory> <mac-directory> <full-commit>");
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
