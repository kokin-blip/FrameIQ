// Installed-worker acceptance; development executables are never put on worker PATH.
import { spawn } from "node:child_process";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, join } from "node:path";
import { createHash } from "node:crypto";
const [install, fixtures, report, model] = process.argv.slice(2).map((p) => resolve(p));
const cleanEnv = { ...process.env };
for (const key of Object.keys(cleanEnv)) {
  if (/^(FFMPEG_|VCPKG|LIBCLANG|FRAMEIQ_ORT|DYLD_|LD_LIBRARY)/i.test(key) || key.toLowerCase() === "path") delete cleanEnv[key];
}
cleanEnv.PATH = `${process.env.SystemRoot}\\System32;${process.env.SystemRoot}`;
function request(executable, payload) {
  return new Promise((accept, reject) => {
    const child = spawn(join(install, executable), [], { env: cleanEnv, cwd: install, stdio: ["pipe", "pipe", "pipe"] });
    let stdout = "", stderr = "";
    const timeout = setTimeout(() => { child.kill(); reject(new Error(`${executable} timed out`)); }, 60_000);
    child.stdout.on("data", (data) => { stdout += data; });
    child.stderr.on("data", (data) => { stderr += data; });
    child.on("error", (error) => { clearTimeout(timeout); reject(error); });
    child.on("close", (code) => {
      clearTimeout(timeout);
      try {
        const rows = stdout.trim().split("\n").filter(Boolean).map(JSON.parse);
        const error = rows.find((row) => row.kind === "error");
        if (code !== 0 || error) throw new Error(JSON.stringify({ code, error, stderr }));
        accept(rows);
      } catch (error) { reject(error); }
    });
    child.stdin.end(`${JSON.stringify(payload)}\n`);
  });
}
const cases = [];
for (const name of ["sdr", "pq", "hlg"]) {
  const path = join(fixtures, `${name}.mkv`);
  const rows = await request("frameiq-worker.exe", { kind: "index", path, stream: null });
  const frames = rows.filter((r) => r.kind === "frames").flatMap((r) => r.frames);
  if (frames.length !== 4 || !rows.some((r) => r.kind === "complete")) throw new Error(`Bad index: ${name}`);
  const frame = frames[2];
  for (const mode of name === "sdr" ? ["sdr"] : ["sdr", "preserve16_bit"]) {
    const output = join(fixtures, `${name}-${mode}.png`);
    const messages = await request("frameiq-worker.exe", {
      kind: "decode", path, stream: 0, ordinal: Number(frame.ordinal),
      expected_pts: frame.ptsTicks === null ? null : Number(frame.ptsTicks), anchor: null, output, max_width: null,
      options: { format: "png", jpegQuality: 95, colorMode: mode, colorPolicy: null },
    });
    const image = messages.find((r) => r.kind === "image");
    if (!image || !existsSync(output)) throw new Error("No decoded image");
    const png = readFileSync(output);
    const hash = createHash("sha256").update(png).digest("hex");
    if (hash !== image.sha256 || image.width !== 160 || image.height !== 120 || png[24] !== (mode === "sdr" ? 8 : 16)) throw new Error("Export fidelity metadata mismatch");
    cases.push({ source: name, mode, ordinal: Number(frame.ordinal), pts: frame.ptsTicks, hash, conversion: image.conversion });
  }
}
const manifest = JSON.parse(readFileSync(new URL("../models/manifests/yunet-2023mar.json", import.meta.url)));
const validated = await request("frameiq-ai-worker.exe", { kind: "validate", manifest, path: model });
if (!validated.some((r) => r.kind === "validated")) throw new Error("Installed inference validation failed");
writeFileSync(report, JSON.stringify({ schemaVersion: 1, installedPath: install, workerPath: cleanEnv.PATH, cases, inference: "reviewed YuNet CPU validation", limitations: "No desktop UI, clean Windows 11 machine, GPU or held-out accuracy claims." }, null, 2));
