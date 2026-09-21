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

export function prepareRegression({ target, payloadPath, docsDirectory, outputDirectory, commit, version, runUrl }) {
  const payloadName = payloadNames[target];
  if (!Object.hasOwn(payloadNames, target)) throw new Error(`Unsupported target: ${target}`);
  if (!/^[a-f0-9]{40}$/.test(commit)) throw new Error("A full source commit is required");
  if (!/^\d+\.\d+\.\d+(?:[-+][\w.-]+)?$/.test(version)) throw new Error("Invalid application version");
  if (path.basename(payloadPath) !== payloadName) throw new Error(`Expected platform payload ${payloadName}`);

  // Validate all inputs before creating a fresh output directory. Never overwrite.
  const inputs = [
    { name: payloadName, source: payloadPath },
    ...documentNames.map(name => ({ name, source: path.join(docsDirectory, name) })),
  ];
  for (const input of inputs) {
    if (!statSync(input.source).isFile() || statSync(input.source).size === 0) throw new Error(`Missing or empty file: ${input.name}`);
  }
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
    const [target, payloadPath, outputDirectory] = process.argv.slice(2);
    if (!target || !payloadPath || !outputDirectory || process.argv.length !== 5) {
      throw new Error("Usage: node scripts/prepare-regression.mjs <target> <payload> <new-output-directory>");
    }
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
      target, payloadPath, outputDirectory,
      docsDirectory: path.resolve(appRoot, "../docs/development/app/regression"),
      commit: git(["rev-parse", "HEAD"]), version: pkg.version, runUrl,
    });
    console.log(`Prepared ${manifest.target} at ${manifest.sourceCommit}`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
