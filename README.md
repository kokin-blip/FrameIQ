# FrameIQ

A local-first desktop video frame extractor. This checkout includes offline extraction, recovery, HDR/PNG16, model-free recommendations/comparison, guided regions and local model-management foundations. Automatic intelligent presets are not yet complete. Fashion identification is outside this implementation.

## Run on Apple Silicon macOS

Requires macOS 14+, Rust, Node 24, Xcode command-line tools, pkg-config, LLVM/libclang, autoconf, automake, libtool, and GnuPG. The native build downloads signature-verified FFmpeg 8.0.3 from its official distribution.

```sh
npm ci
bash tools/build-ffmpeg.sh
node tools/prepare-onnxruntime.mjs
npm run desktop
```

`npm run desktop -- build` produces `target/release/bundle/macos/FrameIQ.app`. This is a private, locally signed prototype; public signing/notarization is pending. Windows native FFmpeg packaging remains a release gate. CI checks portable contracts, SQLite, and frontend code on macOS and Windows.

`npm run dev` opens the design preview at http://127.0.0.1:1420. Local files require the Tauri desktop app.

## Implemented

- Projects and persistent source metadata.
- Progressive presentation-frame indexing with distinct ordinals, VFR timestamps, and source revision hashes.
- Validated seek anchors and sequential fallback.
- Muted preview, timeline thumbnails, precision wheel, keyboard stepping, timestamp selection, zoom/pan, and batch selection.
- Original-resolution PNG, JPEG, and lossless WebP exports with JSON provenance sidecars.
- Background workers, cancellation, recoverable job snapshots, changed-source checks, and bounded cache cleanup.
- Identical-content source relocation and immutable, verified export resume plans.
- SDR tone mapping and tagged PNG16 output, with conversion provenance.
- Adaptive quality analysis, duplicate suppression, cached ranking and frame comparison.
- Seven preset definitions, custom weights and per-frame guided regions; automatic regions remain unavailable.
- Persistent supervised ONNX worker, reviewed NCHW/NHWC downloads/imports, memory budgets and benchmarks.
- Source-indexed face-candidate backend/cache and reference-parity tooling; automatic preset eligibility remains disabled.
- Shared native CPU slots with interactive priority and explicit test-only export crash/restart coverage.
- Opt-in loopback Ollama/LM Studio discovery and synthetic probes, with optional OS-stored credentials.
- Editable Figma panel compositions preserving the approved Fluent editor.

Synthetic PQ/HLG color fixtures pass; independent analytical color fixtures and broader real HDR coverage remain release gates. Lossless image export preserves the documented RGB conversion of the decoded frame; it cannot reconstruct compressed source YUV values. Preview can skip display frames; selection/export uses a resolved frame identity.

No shopping providers or cloud inference are configured. Model downloads and localhost connections are opt-in. No model weights are distributed.

## Verify

```sh
source tools/native-env.sh
npm run native:prepare
cargo test --workspace
npm run contracts:check
npm run check
npm test
npm run build
```

Native tests generate synthetic CFR, VFR, and B-frame media and compare every exported frame's RGB pixels with an independent sequential FFmpeg reference. Application integration tests cover offline import, timestamp resolution, selections, three export formats, provenance, and changed-source rejection. Broader real-world coverage, accuracy evaluation, automatic model adapters/presets, acceleration and Windows native execution remain open gates. A release backend benchmark and the complete status/gap report are available under docs.

See [milestone implementation and remaining gates](docs/milestones-1-4-status.md), [architecture decisions](docs/architecture/decisions.md), and [Figma designs](https://www.figma.com/design/5L64rf754pinHPHimkme5e).
