import { copyFileSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const payloadNames = {
  "x86_64-pc-windows-msvc": "march-7th-app.exe",
  "aarch64-apple-darwin": "March 7th.app.zip",
};
const documentNames = ["README.md", "CHECKLIST.md", "RESULT-TEMPLATE.md"];
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");

export function readBinaryBuildInfo(executablePath) {
  // Only the caller's explicit current build is executed. Never discover or run
  // an imported packet. GUI-subsystem Windows binaries receive a stdout pipe.
  const output = execFileSync(path.resolve(executablePath), ["--build-info"], {
    encoding: "utf8", timeout: 10_000, maxBuffer: 16_384, windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
  if (output.split(/\r?\n/).length !== 1) throw new Error("Invalid binary identity output");
  try { return JSON.parse(output); } catch { throw new Error("Invalid binary identity JSON"); }
}

function verifyMacArchiveExecutable(payloadPath, executablePath) {
  if (!executablePath || path.dirname(path.resolve(executablePath)) !== path.resolve(payloadPath.replace(/\.zip$/, ""), "Contents/MacOS")) {
    throw new Error("macOS archive identity requires this payload's app bundle executable");
  }
  const executable = readFileSync(executablePath);
  const executableName = path.basename(executablePath);
  // Restrict the known native binary name to literal archive path characters;
  // bsdtar's member selector must never become a wildcard pattern.
  if (!/^[A-Za-z0-9._-]+$/.test(executableName)) throw new Error("Invalid macOS archive executable name");
  const member = `March 7th.app/Contents/MacOS/${executableName}`;
  const tar = process.platform === "win32" ? path.join(process.env.SystemRoot, "System32/tar.exe") : "/usr/bin/tar";
  let archived;
  try {
    // Stock bsdtar reads exactly this member into a bounded pipe. No extraction
    // to disk, archive code execution, or third-party ZIP parser is involved.
    archived = execFileSync(tar, ["-xOf", path.resolve(payloadPath), member], {
      timeout: 10_000, maxBuffer: executable.length + 1, windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch {
    throw new Error("macOS archive executable could not be verified");
  }
  if (!archived.equals(executable)) throw new Error("macOS archive executable differs from the probed executable");
}

export function prepareRegression({ target, payloadPath, docsDirectory, outputDirectory, commit, version, runUrl, buildInfo, executablePath }) {
  const payloadName = payloadNames[target];
  if (!Object.hasOwn(payloadNames, target)) throw new Error(`Unsupported target: ${target}`);
  if (!/^[a-f0-9]{40}$/.test(commit)) throw new Error("A full source commit is required");
  if (!/^\d+\.\d+\.\d+(?:[-+][\w.-]+)?$/.test(version)) throw new Error("Invalid application version");
  if (path.basename(payloadPath) !== payloadName) throw new Error(`Expected platform payload ${payloadName}`);
  if (!buildInfo || buildInfo.schemaVersion !== 1 || buildInfo.appVersion !== version
    || buildInfo.target !== target || buildInfo.sourceCommit !== commit || buildInfo.sourceState !== "clean") {
    throw new Error("Binary identity must match version, target and commit with clean application inputs");
  }

  // Validate all inputs before creating a fresh output directory. Never overwrite.
  const inputs = [
    { name: payloadName, source: payloadPath },
    ...documentNames.map(name => ({ name, source: path.join(docsDirectory, name) })),
  ];
  for (const input of inputs) {
    if (!statSync(input.source).isFile() || statSync(input.source).size === 0) throw new Error(`Missing or empty file: ${input.name}`);
  }
  if (target === "aarch64-apple-darwin") verifyMacArchiveExecutable(payloadPath, executablePath);
  mkdirSync(outputDirectory);
  const files = inputs.map(input => {
    const destination = path.join(outputDirectory, input.name);
    copyFileSync(input.source, destination);
    const bytes = readFileSync(destination);
    return { name: input.name, bytes: bytes.length, sha256: sha256(bytes) };
  });
  const manifest = {
    schemaVersion: 1,
    applicationVersion: version,
    sourceCommit: commit,
    target,
    binaryBuildInfo: { schemaVersion: 1, appVersion: buildInfo.appVersion, target: buildInfo.target, sourceCommit: buildInfo.sourceCommit, sourceState: buildInfo.sourceState },
    buildRunUrl: runUrl ?? null,
    builtAt: new Date().toISOString(),
    validationStatus: "build-only-awaiting-human-regression",
    files,
  };
  const manifestBytes = Buffer.from(JSON.stringify(manifest, null, 2) + "\n");
  writeFileSync(path.join(outputDirectory, "BUILD-INFO.json"), manifestBytes);
  const sums = [...files, { name: "BUILD-INFO.json", sha256: sha256(manifestBytes) }]
    .map(file => `${file.sha256}  ${file.name}`).join("\n") + "\n";
  writeFileSync(path.join(outputDirectory, "SHA256SUMS.txt"), sums);
  return manifest;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [target, payloadPath, outputDirectory, executablePath] = process.argv.slice(2);
    if (!target || !payloadPath || !outputDirectory || !executablePath || process.argv.length !== 6) {
      throw new Error("Usage: node scripts/prepare-regression.mjs <target> <payload> <new-output-directory> <this-build-executable>");
    }
    if (!Object.hasOwn(payloadNames, target)) throw new Error(`Unsupported target: ${target}`);
    if (target === "x86_64-pc-windows-msvc" && path.resolve(payloadPath) !== path.resolve(executablePath)) throw new Error("Windows identity probe must use the payload executable");
    if (target === "aarch64-apple-darwin" && path.dirname(path.resolve(executablePath)) !== path.resolve(payloadPath.replace(/\.zip$/, ""), "Contents/MacOS")) throw new Error("macOS identity probe must use this payload's app bundle executable");
    const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
    const git = args => execFileSync("git", args, { cwd: appRoot, encoding: "utf8" }).trim();
    const changed = git(["status", "--porcelain", "--untracked-files=no"]);
    if (changed) {
      console.error(git(["diff", "--stat"]));
      throw new Error(`Refusing to label a build from a modified tracked worktree:\n${changed}`);
    }
    const pkg = JSON.parse(readFileSync(path.join(appRoot, "package.json"), "utf8"));
    const config = JSON.parse(readFileSync(path.join(appRoot, "src-tauri/tauri.conf.json"), "utf8"));
    if (pkg.version !== config.version) throw new Error("Frontend and Tauri versions differ");
    const runUrl = process.env.GITHUB_RUN_ID
      ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}`
      : null;
    const manifest = prepareRegression({
      target, payloadPath, outputDirectory, executablePath,
      docsDirectory: path.resolve(appRoot, "../docs/development/app/regression"),
      commit: git(["rev-parse", "HEAD"]), version: pkg.version, runUrl,
      buildInfo: readBinaryBuildInfo(executablePath),
    });
    console.log(`Prepared ${manifest.target} at ${manifest.sourceCommit}`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
