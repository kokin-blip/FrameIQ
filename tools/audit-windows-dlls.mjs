import { readdirSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
const stage = resolve(process.argv[2]);
const files = [
  ...readdirSync(resolve(stage, "native"))
    .filter((n) => n.endsWith(".dll"))
    .map((n) => resolve(stage, "native", n)),
  ...readdirSync(resolve(stage, "binaries"))
    .filter((n) => n.endsWith(".exe"))
    .map((n) => resolve(stage, "binaries", n)),
];
const bundled = new Set(
  readdirSync(resolve(stage, "native")).map((n) => n.toLowerCase()),
);
const missing = new Set();
const manifest = [];
for (const file of files) {
  const result = spawnSync("dumpbin", ["/dependents", file], {
    encoding: "utf8",
  });
  if (result.status !== 0)
    throw new Error("Run from an MSVC developer shell with dumpbin available.");
  const dependencies = result.stdout.match(/\b[\w.-]+\.dll\b/gi) ?? [];
  manifest.push({ name: file.split(/[\\/]/).at(-1), sha256: createHash("sha256").update(readFileSync(file)).digest("hex"), dependencies });
  for (const dep of dependencies) {
    if (/^api-ms-|^ext-ms-/i.test(dep) || bundled.has(dep.toLowerCase()))
      continue;
    // Microsoft CRT redistributables must be bundled, even when present on the build agent.
    if (
      /^(vcruntime|msvcp|concrt)/i.test(dep) ||
      !existsSync(resolve(process.env.SystemRoot, "System32", dep))
    )
      missing.add(dep);
  }
}
if (process.argv[3]) writeFileSync(process.argv[3], JSON.stringify({ schemaVersion: 1, files: manifest, missing: [...missing] }, null, 2));
if (missing.size)
  throw new Error(`Bundle missing dependencies: ${[...missing].join(", ")}`);
console.log(`Audited ${files.length} workers and libraries.`);
