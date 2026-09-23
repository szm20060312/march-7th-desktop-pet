import { execFileSync } from "node:child_process";
import { lstatSync, realpathSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const phases = new Set(["before-install", "after-install", "after-check", "after-cargo-test", "after-clippy", "after-identity-fixture", "identity-sample"]);
// Porcelain v1 XY combinations from git-status's documented short-format table.
// Unknown/future states are unavailable, never evidence of an empty scope.
const porcelainStates = new Set([
  " M", " A", " D", " T", " R", " C",
  "M ", "MM", "MT", "MD", "T ", "TM", "TT", "TD", "A ", "AM", "AT", "AD", "D ",
  "R ", "RM", "RT", "RD", "C ", "CM", "CT", "CD",
  "DD", "AU", "UD", "UA", "DU", "AA", "UU", "??", "!!",
]);
const rules = new Map([
  ["dependency-directory", ["node_modules", "node_modules/", ".pnpm-store/"]],
  ["build-output", ["dist", "dist/", "dist-ssr", "target/", "src-tauri/target/", "coverage/", "/regression-output/"]],
  ["logs", ["logs", "*.log", "npm-debug.log*", "yarn-debug.log*", "yarn-error.log*", "pnpm-debug.log*", "lerna-debug.log*"]],
  ["local-config", ["*.local", ".env", ".env.*"]],
  ["editor-metadata", [".DS_Store", ".vscode/*", ".idea", "*.suo", "*.ntvs*", "*.njsproj", "*.sln", "*.sw?", ".obsidian/"]],
]);
const bucket = count => count === 0 ? "zero" : count === 1 ? "one" : count < 10 ? "few" : "many";
const decode = bytes => new TextDecoder("utf-8", { fatal: true }).decode(bytes);
function nulRecords(bytes) {
  const text = decode(bytes);
  if (text === "") return [];
  if (!text.endsWith("\0")) throw Error("invalid records");
  return text.slice(0, -1).split("\0");
}

function within(base, candidate, foldCase = false) {
  const normalize = value => foldCase ? value.normalize("NFC").toLowerCase() : value;
  const relative = path.relative(normalize(base), normalize(candidate));
  return relative === ".." || relative.startsWith(".." + path.sep) || path.isAbsolute(relative) ? null : relative;
}

function repoRelativeName(name) {
  if (!name || path.posix.isAbsolute(name) || path.win32.isAbsolute(name) || /^[a-z]:/i.test(name)) return false;
  const parts = name.replace(/\/$/, "").split(/[\\/]/);
  return parts.every(part => part !== "" && part !== "." && part !== "..");
}

export function summarizeFrontendIgnored({ appRoot, phase, enabled, git }) {
  if (enabled !== "1") return "";
  const safePhase = phases.has(phase) ? phase : "unknown";
  const unavailable = stage => `ignored-input phase=${safePhase} state=unavailable total=unknown groups=unknown overflow=no failureStage=${stage}`;
  if (safePhase === "unknown") return unavailable("phase");
  let failureStage = "root";
  try {
    appRoot = realpathSync(appRoot);
    const env = { ...process.env, GIT_OPTIONAL_LOCKS: "0" };
    for (const key of ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR"]) delete env[key];
    const query = git ?? ((args, input) => execFileSync("git", args, { cwd: appRoot, input, env, timeout: 5_000, maxBuffer: 2 * 1024 * 1024, stdio: ["pipe", "pipe", "pipe"] }));
    let repoRoot = decode(query(["rev-parse", "--show-toplevel"])).trim();
    if (!path.isAbsolute(repoRoot) || repoRoot.includes("\0")) return unavailable("root");
    repoRoot = realpathSync(repoRoot);
    if (within(repoRoot, appRoot) === null) return unavailable("root");
    failureStage = "path";
    const declaredFrontend = path.join(appRoot, "src");
    if (!lstatSync(declaredFrontend).isDirectory()) return unavailable("path");
    const frontend = realpathSync(declaredFrontend);
    const frontendIdentity = statSync(frontend, { bigint: true });
    const scope = within(repoRoot, frontend);
    if (scope === null || scope === "") return unavailable("path");
    const repoScope = scope.split(path.sep).join("/");
    failureStage = "status-query";
    const statusBytes = query(["-C", repoRoot, "status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignored=matching", "--", `:(literal)${repoScope}`]);
    failureStage = "status-format";
    const status = nulRecords(statusBytes);
    const entries = [];
    for (let index = 0; index < status.length; index++) {
      const record = status[index];
      if (record.length < 4 || record[2] !== " ") return unavailable("status-format");
      const flags = record.slice(0, 2);
      if (!porcelainStates.has(flags)) return unavailable("status-format");
      if (flags === "!!") entries.push(record.slice(3));
      if (/[RC]/.test(flags) && (++index >= status.length || status[index] === "")) return unavailable("status-format");
    }
    if (entries.length > 512) return unavailable("limit");
    if (entries.length === 0) return `ignored-input phase=${phase} state=ok total=zero groups=none overflow=no`;
    const metadata = [];
    for (const entry of entries) {
      failureStage = "path";
      if (!repoRelativeName(entry)) return unavailable("path");
      const location = path.resolve(repoRoot, entry);
      if (within(frontend, location, true) === null) return unavailable("path");
      failureStage = "metadata";
      const canonical = realpathSync(location);
      failureStage = "path";
      const relative = within(frontend, canonical, true);
      if (relative === null) return unavailable("path");
      // Case folding is only a candidate check. Confirm the actual scope object
      // so a distinct case-sensitive directory or a symlink escape cannot pass.
      let scopeRoot = canonical;
      for (const _ of relative === "" ? [] : relative.split(path.sep)) scopeRoot = path.dirname(scopeRoot);
      failureStage = "metadata";
      const identity = statSync(scopeRoot, { bigint: true });
      if (frontendIdentity.ino === 0n || identity.dev !== frontendIdentity.dev || identity.ino !== frontendIdentity.ino) return unavailable("path");
      metadata.push(lstatSync(location));
    }
    failureStage = "ignore-query";
    const ruleBytes = query(["-C", repoRoot, "check-ignore", "-v", "-z", "--stdin"], Buffer.from(entries.join("\0") + "\0"));
    failureStage = "ignore-format";
    const records = nulRecords(ruleBytes);
    if (records.length !== entries.length * 4) return unavailable("ignore-format");
    const groups = new Map();
    for (let index = 0; index < entries.length; index++) {
      failureStage = "ignore-format";
      const [source, line, pattern, entry] = records.slice(index * 4, index * 4 + 4);
      if (entry !== entries[index] || !source || !/^[1-9]\d*$/.test(line) || !pattern || pattern.startsWith("!")) return unavailable("ignore-format");
      const ruleSource = path.resolve(repoRoot, source);
      const origin = ruleSource === path.join(repoRoot, ".gitignore") ? "root-rules"
        : ruleSource === path.resolve(appRoot, ".gitignore") ? "app-rules"
          : within(frontend, ruleSource) !== null ? "nested-rules" : "internal-external-rules";
      const rule = [...rules].find(([, patterns]) => patterns.includes(pattern))?.[0] ?? "other";
      const stat = metadata[index];
      const kind = stat.isSymbolicLink() ? "link" : stat.isDirectory() ? "directory" : stat.isFile() ? "file" : "other";
      const key = `${origin}:${rule}:${kind}`;
      groups.set(key, (groups.get(key) ?? 0) + 1);
    }
    const rows = [...groups].sort(([a], [b]) => a.localeCompare(b));
    const summary = rows.slice(0, 8).map(([key, count]) => `${key}:${bucket(count)}`).join(",");
    return `ignored-input phase=${phase} state=ok total=${bucket(entries.length)} groups=${summary} overflow=${rows.length > 8 ? "yes" : "no"}`;
  } catch {
    // No Git stderr, exception message, path, rule or input content leaves here.
    return unavailable(failureStage);
  }
}

let isEntry = false;
try { isEntry = Boolean(process.argv[1]) && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url)); } catch { /* import/eval */ }
if (isEntry) {
  const line = summarizeFrontendIgnored({ appRoot: process.cwd(), phase: process.argv[2], enabled: process.env.MARCH_BUILD_INPUT_DIAGNOSTICS });
  if (line) console.log(line);
}
