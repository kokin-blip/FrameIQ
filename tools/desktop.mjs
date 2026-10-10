import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  copyFileSync,
  rmSync,
  readdirSync,
  existsSync,
} from "node:fs";
import { resolve } from "node:path";
const root = resolve(import.meta.dirname, "..");
const native = resolve(root, ".native/ffmpeg/lib");
const env = {
  ...process.env,
  PKG_CONFIG_PATH: resolve(native, "pkgconfig"),
  DYLD_LIBRARY_PATH: native,
  LD_LIBRARY_PATH: native,
};
function run(program, args, options = {}) {
  const r = spawnSync(program, args, {
    cwd: root,
    env,
    stdio: "inherit",
    ...options,
  });
  if (r.status !== 0) process.exit(r.status ?? 1);
}
if (!existsSync(native)) {
  console.error("Build FFmpeg first: bash tools/build-ffmpeg.sh");
  process.exit(1);
}
const release = process.argv.includes("build");
run("cargo", [
  "build",
  "-p",
  "frameiq-worker",
  "-p",
  "frameiq-ai-worker",
  ...(release ? ["--release"] : []),
]);
const info = spawnSync("rustc", ["-vV"], { encoding: "utf8" }).stdout;
const triple = /host: (.+)/.exec(info)[1];
const shell = resolve(root, "apps/desktop/src-tauri");
const worker = resolve(
  root,
  `target/${release ? "release" : "debug"}/frameiq-worker${process.platform === "win32" ? ".exe" : ""}`,
);
mkdirSync(resolve(shell, "binaries"), { recursive: true });
rmSync(resolve(shell, "native"), { recursive: true, force: true });
mkdirSync(resolve(shell, "native"), { recursive: true });
const staged = resolve(
  shell,
  `binaries/frameiq-worker-${triple}${process.platform === "win32" ? ".exe" : ""}`,
);
copyFileSync(worker, staged);
const aiWorker = worker.replace("frameiq-worker", "frameiq-ai-worker");
const stagedAi = staged.replace("frameiq-worker", "frameiq-ai-worker");
copyFileSync(aiWorker, stagedAi);
const ortNative = resolve(root, ".native/onnxruntime/lib");
if (process.platform === "darwin" && existsSync(ortNative)) {
  copyFileSync(resolve(ortNative, "libonnxruntime.1.22.0.dylib"), resolve(shell, "native/libonnxruntime.1.22.0.dylib"));
  copyFileSync(resolve(root, ".native/onnxruntime/LICENSE"), resolve(shell, "native/ONNXRuntime-LICENSE"));
  run("codesign", ["--force", "--sign", "-", stagedAi]);
  run("codesign", ["--force", "--sign", "-", resolve(shell, "native/libonnxruntime.1.22.0.dylib")]);
}
for (const [src, dst] of [
  ["LICENSE", "FFmpeg-LICENSE"],
  ["zimg-LICENSE", "zimg-LICENSE"],
  ["source.sha256", "source.sha256"],
  ["build-config.mak", "build-config.mak"],
]) {
  copyFileSync(
    resolve(root, ".native/ffmpeg", src),
    resolve(shell, "native", dst),
  );
}
for (const name of ["OpenCV-Apache-2.0.txt", "FaceDecoder-NOTICE.txt"]) {
  copyFileSync(resolve(root, "docs/licenses", name), resolve(shell, "native", name));
}
if (process.platform === "darwin") {
  const libraries = readdirSync(native).filter((n) =>
    /^lib[a-z0-9]+\.\d+\.dylib$/.test(n),
  );
  for (const name of libraries) {
    const dst = resolve(shell, "native", name);
    copyFileSync(resolve(native, name), dst);
    run("install_name_tool", ["-id", `@rpath/${name}`, dst]);
    for (const dep of libraries)
      run("install_name_tool", [
        "-change",
        resolve(native, dep),
        `@rpath/${dep}`,
        dst,
      ]);
  }
  for (const dep of libraries)
    run("install_name_tool", [
      "-change",
      resolve(native, dep),
      `@rpath/${dep}`,
      staged,
    ]);
  run("install_name_tool", [
    "-add_rpath",
    "@executable_path/../Resources/native",
    staged,
  ]);
  run("codesign", ["--force", "--sign", "-", staged]);
  for (const name of libraries)
    run("codesign", ["--force", "--sign", "-", resolve(shell, "native", name)]);
}
if (process.argv.includes("--prepare")) {
  console.log("Native worker and libraries prepared.");
  process.exit(0);
}
run(resolve(root, "node_modules/.bin/tauri"), [release ? "build" : "dev"], {
  cwd: resolve(root, "apps/desktop"),
  env: { ...env, FRAMEIQ_WORKER: worker, FRAMEIQ_ORT_LIBRARY: resolve(ortNative, "libonnxruntime.1.22.0.dylib") },
});

if (release && process.platform === "darwin") {
  const bundle = resolve(root, "target/release/bundle/macos/FrameIQ.app");
  run("codesign", ["--force", "--deep", "--sign", "-", bundle]);
  run("codesign", ["--verify", "--deep", "--strict", bundle]);
}
