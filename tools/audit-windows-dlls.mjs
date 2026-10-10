import { readdirSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
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
for (const file of files) {
  const result = spawnSync("dumpbin", ["/dependents", file], {
    encoding: "utf8",
  });
  if (result.status !== 0)
    throw new Error("Run from an MSVC developer shell with dumpbin available.");
  for (const dep of result.stdout.match(/\b[\w.-]+\.dll\b/gi) ?? []) {
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
if (missing.size)
  throw new Error(`Bundle missing dependencies: ${[...missing].join(", ")}`);
console.log(`Audited ${files.length} workers and libraries.`);
