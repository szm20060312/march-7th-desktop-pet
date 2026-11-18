import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../src");
const allowed = {
  domain: ["domain"],
  characters: ["domain", "characters"],
  application: ["domain", "application"],
  adapters: ["domain", "application", "adapters"],
  entry: ["domain", "characters", "application", "adapters"],
};
const globals = new Set(["window", "document", "performance", "Date", "fetch", "localStorage", "sessionStorage", "navigator", "globalThis", "process", "setTimeout", "setInterval", "requestAnimationFrame", "require"]);
const errors = [];
const graph = new Map();
function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const name = path.join(dir, entry.name);
    return entry.isDirectory() ? walk(name) : [name];
  });
}
function layer(file) {
  const relative = path.relative(root, file).replaceAll("\\", "/");
  return ["main.ts", "entries/settings.ts", "entries/reminder.ts"].includes(relative) ? "entry" : relative.split("/")[0];
}
for (const file of walk(root).filter(name => name.endsWith(".ts") && !name.endsWith(".test.ts") && !name.endsWith(".d.ts"))) {
  const current = layer(file);
  if (!allowed[current]) { errors.push(`${path.relative(root, file)}: unknown layer`); continue; }
  const source = ts.createSourceFile(file, readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true);
  const edges = [];
  graph.set(file, edges);
  const fail = message => errors.push(`${path.relative(root, file)}: ${message}`);
  function checkImport(specifier) {
    if (!specifier.startsWith(".")) {
      if (current !== "adapters") fail(`external dependency ${specifier} belongs in adapters`);
      return;
    }
    const resolved = ts.resolveModuleName(specifier, file, { moduleResolution: ts.ModuleResolutionKind.Bundler }, ts.sys).resolvedModule?.resolvedFileName;
    if (!resolved) { fail(`unresolved dependency ${specifier}`); return; }
    const target = path.resolve(resolved);
    if (target.endsWith(".json")) {
      if (current !== "characters" || layer(target) !== "characters") {
        fail(`${current} may only consume JSON through the characters catalog`);
      }
      return;
    }
    if (target.endsWith(".test.ts") || !allowed[current].includes(layer(target))) {
      fail(`${current} may not import ${specifier}`);
    }
    edges.push(target);
  }
  function visit(node) {
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) {
      checkImport(node.moduleSpecifier.text);
    }
    if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword) {
      const argument = node.arguments[0];
      if (argument && ts.isStringLiteral(argument)) checkImport(argument.text);
      else fail("computed dynamic imports obscure the dependency boundary");
    }
    if (["domain", "characters", "application"].includes(current) && ts.isIdentifier(node) && globals.has(node.text)) {
      fail(`host global ${node.text} belongs behind a port`);
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
}
const visited = new Set();
const active = new Set();
function visitGraph(file) {
  if (active.has(file)) { errors.push(`circular dependency: ${path.relative(root, file)}`); return; }
  if (visited.has(file)) return;
  visited.add(file);
  active.add(file);
  for (const target of graph.get(file) ?? []) visitGraph(target);
  active.delete(file);
}
for (const file of graph.keys()) visitGraph(file);
if (errors.length) {
  console.error([...new Set(errors)].join("\n"));
  process.exitCode = 1;
} else {
  console.log(`Architecture boundaries passed (${graph.size} production modules).`);
}
