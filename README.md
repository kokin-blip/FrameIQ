# FrameIQ

A local-first desktop frame extractor with photo inspection, optional fashion analysis, source-backed product research and saved lookbooks. Intelligent presets and AI interpretations remain experimental and require their declared model/account capabilities. See the [phases 1–8 feature matrix](docs/phase-1-8-completion.md) for implemented, unavailable and externally blocked features.

## Run on Apple Silicon macOS

Requires macOS 14+, Rust, Node 24, Xcode command-line tools, pkg-config, LLVM/libclang, autoconf, automake, libtool, GnuPG, Meson, and Ninja. The native build downloads signature-verified FFmpeg 8.0.3 from its official distribution and checksum-verified dav1d 1.5.3 for software AV1 decoding, including YouTube downloads.

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
- Seven experimental intelligent presets, all five scoring weights, bidirectional tracking, target refinement, complementary views and custom guided regions.
- Persistent supervised ONNX worker, reviewed NCHW/NHWC downloads/imports, memory budgets and benchmarks.
- Source-indexed face/person/pose features and reviewed multi-input detection, segmentation and embedding task packs with explicit qualification gates.
- Shared native CPU slots with interactive priority and explicit test-only export crash/restart coverage.
- Opt-in Ollama/LM Studio discovery, probes and reviewed-image analysis, with optional OS-stored credentials.
- JPEG/PNG/WebP photo revisions, persistent item regions, polygon/outline masks, protected corrections and garment groups.
- Reviewed ChatGPT subscription product research, citations, local catalog retrieval, optional authorized eBay search and conservative exact-match confirmation.
- Saved looks, alternatives/unresolved items and rights-aware JSON/HTML/reference exports with verified recovery.
- Editable Figma panel compositions preserving the approved Fluent editor.

Synthetic PQ/HLG color fixtures pass; independent analytical color fixtures and broader real HDR coverage remain release gates. Lossless image export preserves the documented RGB conversion of the decoded frame; it cannot reconstruct compressed source YUV values. Preview can skip display frames; selection/export uses a resolved frame identity.

External analysis and provider connections are opt-in. Fashion research has no automatic paid API fallback. Model imports require review, and no model weights are committed or distributed in the application. Live provider acceptance and failed model qualifications remain explicit in the feature matrix.

## Verify

```sh
source tools/native-env.sh
npm run native:prepare
cargo build -p frameiq-worker -p frameiq-ai-worker
cargo test --workspace
npm run contracts:check
npm run check
npm test
npm run build
```

Native tests generate synthetic CFR, VFR, and B-frame media and compare every exported frame's RGB pixels with an independent sequential FFmpeg reference. Application integration tests cover offline import, timestamp resolution, selections, three export formats, provenance, and changed-source rejection. Broader accuracy evaluation, acceleration and Windows native acceptance remain outside the phases 1–8 feature work. A release backend benchmark and the complete status/gap report are available under docs.

See [milestone implementation and remaining gates](docs/milestones-1-4-status.md), [architecture decisions](docs/architecture/decisions.md), and [Figma designs](https://www.figma.com/design/5L64rf754pinHPHimkme5e).
