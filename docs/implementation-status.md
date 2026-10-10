# Implementation status

The current milestone 1–4 implementation and explicit remaining gates are tracked in [milestones-1-4-status.md](milestones-1-4-status.md). The baseline below records the earlier extractor delivery; it is retained as historical context.

## Implemented in this checkout

FIQ-001: local and GitHub repositories reconciled; both empty, origin configured.
FIQ-002: architecture/fidelity decisions recorded.
FIQ-003: Rust-owned DTOs and checked TypeScript generation, structured errors, string integer identity.
FIQ-004: Figma foundations/components and all application view foundations created. Extraction UI inspected and implemented; later screens need deeper interaction/state variants.
FIQ-006: Tauri/React/Zustand scaffold and CI. macOS builds and launches. Windows portable checks configured; native packaging unverified.
FIQ-007: core SQLite schema, WAL/FKs/STRICT tables, migration checksums, recovery.
FIQ-008: supervised worker processes, bounded lanes, persisted jobs, throttled progress, cancellation, and exclusive application ownership of the database. Hung-worker cancellation and duplicate-application rejection are tested.
FIQ-009–014: offline extractor implementation, including preview, frame indexing, validated seek, precision UI, selections, and PNG/JPEG/lossless WebP exports. Source reimport reuses unchanged indexes and invalidates old selections on reindex. HDR/16-bit conversion, export-resume manifests, missing-source relocation, comprehensive real-media coverage, and performance gates remain incomplete.
FIQ-015: synthetic fixture generators and media correctness corpus started. No private or unlicensed fixture imagery committed.

## Verified locally

Apple Silicon macOS: Rust compilation, contracts/domain/persistence tests, synthetic pixel fidelity for CFR/VFR/B-frames, seek fallback, mismatch rejection, offline application integration with all three export formats, frontend type-check/build/tests, and a private macOS app bundle. Bundled worker launches without a development library environment. Packaged app opens and file picker opens. The full graphical smoke test passed: native video picker, decoded preview and timeline thumbnails, one-frame stepping, selection, native destination picker, PNG export, original dimensions and sidecar checksum. A macOS UTI extension-filter issue discovered in this test was fixed by letting FFmpeg validate chosen files.

No Windows native execution, AI-quality benchmark, real-world performance measurement, or shopping-provider access is claimed. CI configuration has not run remotely because no commit/push was made.

## Remaining implementation sequence

1. Finish FIQ-005 and extractor gates: Windows LGPL FFmpeg packaging, HDR paths, relocation/recovery, export resume, real-media corpus, memory/latency benchmarks.
2. FIQ-016–020: quality metrics/sampling/deduplication, cached ranking, comparison, model registry, validated face/pose/person regions, seven presets.
3. FIQ-021–024: model export feasibility and licensing reports, garment detection/masks/corrections, observable/inferred attributes, localhost runtimes and ONNX import validation.
4. FIQ-025–030: licensed local catalog, provider policies, consent/quota layer, independent identity rules, approved eBay adapter, optional policy-gated discovery, product evidence UI.
5. FIQ-031–032: garment association, complementary evidence bundles, saved looks, permitted reference/JSON/HTML exports.
6. FIQ-033–035: recovery/caches, held-out AI/product evaluations, cross-platform profiling, signed distribution.

Each later milestone must keep the offline integration suite passing. Model feasibility or provider-access failure leaves the feature unavailable and preserves manual/offline workflows. The original detailed acceptance criteria remain authoritative; the above is progress tracking, not a claim that every phase is complete.
