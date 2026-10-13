# Model installation and optional tutorial — v0.1.3

## Implementation

The download and import paths share staged validation/activation. Download jobs return installation UUIDs only after ready rows commit; they do not return child validation jobs. Migration 011 links immutable pack manifests/fingerprints to jobs and verified-download checkpoints. Catalog identity ignores mutable IDs/names and includes checksum, task, tensor contract, preprocessing, size, and runtime. Existing UUID references and FrameRefV1 remain unchanged.

A read-only setup-status command reports per-catalog state and recovery reasons. A separate retry command validates owned, checksum/graph-verified cached bytes without using HTTP. Concurrent attempts for one fingerprint are rejected. Completed verified bytes need no network request even without an ETag. Staged copying, cancellation before activation/commit, recovery checkpoints, and transactional model removal remain in place.

Onboarding, Settings, subjects and presets share the model registry; terminal jobs and returning to the workspace refresh it. The YuNet task is recognized by makeup presets. Only ready installations are selectable. Setup preserves tutorial completion and offers an optional tutorial independently.

## Automated evidence

49 frontend tests pass, including UUID/catalog recognition, blocked retry without download, live detector refresh, tutorial preservation and explicit replay. Rust lifecycle fixtures cover same-parent validation failure, cancellation, overlapping attempts, commit failure and crash recovery. Migration 010→011 checks installation/project preservation and backup creation; existing upgrade fixtures also retain selections and detector references. Controlled TLS tests cover completed-cache reuse and existing range/validator failures. Protocol doubles establish lifecycle behavior, not actual ONNX compatibility or accuracy.

Native and packaging results are appended after the isolated production-build acceptance pass. Windows and model accuracy are separate gates. The system reserve is never lowered to pass these checks.

## Native production-build evidence

An isolated `com.frameiq.desktop.model-fix-preview` profile downloaded all three actual pinned OpenCV Zoo files: YuNet 232,589 bytes, person detector 11,990,159 bytes, and pose 5,557,238 bytes. Their SHA-256 values match the pinned manifests; the downloader's graph validation completed. Runtime validation failed on the same jobs with `MEMORY_LIMIT`, preserving verified bytes and recording zero ready installations. Native Retry validation reused cached bytes and kept the reserve intact. Settings displayed the same pending states and reasons. No project images were transmitted.

Available memory during these attempts ranged from 2.63 to 3.46 GiB, below the unchanged 3.60 GiB reserve on this 24 GiB Mac. The user explicitly chose to prepare the patch with native model validation pending. Successful ready-installation restart/dropdown checks and actual face/person/pose inference on the licensed fixture therefore remain pending, rather than being claimed from protocol doubles.

Native Finish setup returned to the workspace with separate Start tutorial / Skip tutorial controls. Skipping remained persisted. The first native pass caught a concurrent initial-registry loading race; initial reads now share one committed snapshot, with subsequent fresh reads after mutation. Two dedicated regression tests cover both paths. 49 frontend tests pass.


## Final regression and package checks

The final Rust workspace suite (excluding the desktop shell's test harness) passes with explicit fault-injection builds, serial tests, and controlled loopback TLS permitted. TypeScript, generated contracts, all 49 frontend tests and the production build pass. Ad-hoc deep/strict signature checks pass for the production and isolated bundles. The packaged media worker decoded five SDR/PQ/HLG PNG cases, including 16-bit preservation, with development library paths removed; frame ordinals/PTS, 160×120 resolution, bit depth and file hashes were checked. Reproduce with `python3 tools/smoke-macos-release.py <FrameIQ.app> <fixture-directory> <output-directory>`.

The final native restart retained all three downloaded/blocked states and reasons in Settings and setup. All three setup choices load together from the shared registry. Finishing repeated setup retained the skipped tutorial and returned to the workspace without a tutorial. The final cached retry remained memory blocked despite a transient higher hardware snapshot; successful actual inference remains pending.

Restart and repeated setup preserve a skipped tutorial in the actual Mac app. The available large viewport was 1440×889 (2880×1778 Retina capture); exact 1440×960 exceeds this display's available height. The compact acceptance build uses the same frontend/backend and isolated profile with only the initial window dimensions set to 1000×700. Actual inference remains pending; no memory safeguard was changed. Earlier native validation estimates may differ from a later hardware snapshot because admission resamples memory in the worker.

The final 1000×700 native build was reviewed with all three persisted blocked packs: model names and error text wrap, setup actions remain in the docked footer, and the content has its own scroll area. Exiting setup leaves the skipped tutorial dismissed; General & shortcuts → Replay tutorial shows step 1 of 3 in the real compact workspace. Native captures are retained in `.cache/releases/v0.1.3/setup-large.png`, `setup-compact.png`, and `tutorial-compact.png`. The Figma states match the interaction hierarchy and Fluent tokens; they are explicitly semantic handoff compositions, not pixel-identical screenshots. Coordinate-based native resize/scroll automation returned `noWindowsAvailable`; compact dimensions were verified using a separately configured packaged build rather than claiming those actions succeeded.
