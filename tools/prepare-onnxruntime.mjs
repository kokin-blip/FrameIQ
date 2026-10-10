import {
  readFileSync,
  mkdirSync,
  writeFileSync,
  rmSync,
  renameSync,
  existsSync,
} from "node:fs";
import { resolve } from "node:path";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
const root = resolve(import.meta.dirname, "..");
const lock = JSON.parse(
  readFileSync(resolve(import.meta.dirname, "onnxruntime-lock.json")),
);
const entry = lock[`${process.platform}-${process.arch}`];
if (!entry) throw new Error("No reviewed runtime archive for this platform.");
const staging = resolve(root, ".native/ort-staging");
mkdirSync(staging, { recursive: true });
const response = await fetch(entry.url);
if (!response.ok)
  throw new Error(`Runtime download failed (${response.status})`);
const data = Buffer.from(await response.arrayBuffer());
if (createHash("sha256").update(data).digest("hex") !== entry.sha256)
  throw new Error("Runtime archive checksum mismatch.");
const archive = resolve(staging, entry.url.split("/").at(-1));
writeFileSync(archive, data);
const extract = spawnSync("tar", ["-xf", archive, "-C", staging], {
  stdio: "inherit",
});
if (extract.status !== 0) throw new Error("Runtime extraction failed.");
const previous = resolve(root, ".native/ort-previous"),
  destination = resolve(root, ".native/onnxruntime");
rmSync(previous, { recursive: true, force: true });
if (existsSync(destination)) renameSync(destination, previous);
try {
  renameSync(resolve(staging, entry.folder), destination);
} catch (e) {
  if (existsSync(previous)) renameSync(previous, destination);
  throw e;
}
rmSync(staging, { recursive: true, force: true });
rmSync(previous, { recursive: true, force: true });
console.log(
  `Verified runtime ${lock.runtimeVersion}, Rust binding ${lock.rustBinding}, C API ${lock.cApi}.`,
);
