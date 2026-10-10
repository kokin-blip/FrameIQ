# Tracking and inference stress — 2026-10-10

## Implemented

- Experimental, opt-in preset jobs use exact indexed adaptive-analysis samples from a chosen anchor. Model-required paths reject missing/incompatible installations. Makeup uses a uniquely associated in-frame landmark face; outfit uses person-detector extent geometry; custom rectangles follow their anchor's body-relative geometry.
- Conservative geometry and mean-color continuity stop at scene changes, sampling gaps over 1.5 seconds, competing candidates, missing subjects, or escaped/undersized target regions. There is no face identification. Choosing a corrected subject on another anchor creates a separate track; this first implementation does not bridge scene cuts or merge tracks.
- Region measurements persist separately from weights/counts. Re-execution with changed quality weights reuses the track and performs neither decoding nor inference. Interrupted tracks retry using retained person/face features; completed stopped tracks remain partial. Source index, task, model hashes/installation IDs, anchor, subject, region, color policy, and algorithm version participate in the cache key.
- Migration 009 adds tracks, normalized observation dependencies, and immutable job results. Previous SQL remains unchanged. Model removal invalidates associated tracks/results and derived region measurements; independently drawn regions remain.
- Cancellation propagates to active child work and ignores the benign already-finished child race. Completed measurements remain available after cancellation. Crashed/interrupted execution is never presented as complete coverage.
- Figma ready/running/stopped/unavailable states were added before the new Fluent panel controls. The panel requires explicit experimental opt-in and a chosen subject; it restores exact recommended frames and displays partial coverage and missing capabilities.

## Limits

This is experimental tracking and region-quality execution, **not validated intelligent-preset completion**. It runs from the selected anchor forward and stops at uncertain continuity. Its adaptive shortlist/refinement is inherited from whole-frame quality analysis; region-specific resampling and backward tracking are still needed. Body geometry is not a clothing mask. Tracking thresholds are conservative starting heuristics and are not frozen calibration results. Pose-based association/orientation, calibrated visibility/coverage/composition scoring, hair masks, detailed accessory/footwear regions, and Complete Look complementary-view selection remain unavailable. The seven definitions remain automaticAvailable=false. No human accuracy gate is claimed.

The available sharpness and lighting weights are renormalized; unvalidated weights are explicitly not scored. A configuration assigning zero to both available features is rejected. Guided single-frame measurements remain available for every preset.

## Broader real inference harness

`cargo run -p frameiq-application --bin inference-stress -- <video> <absolute-media-worker> <reviewed-person.onnx> <reviewed-pose.onnx> <report.json> <seconds:1..7200>`

Set `FRAMEIQ_ORT_LIBRARY` and the platform's native media environment. The harness verifies/imports reviewed weights locally; it downloads nothing. Its fixture must contain a usable person and pose at exact ordinal zero. It runs at least ten cycles and the requested elapsed duration: real cold/warm CPU tensor benchmarks, model replacement, cancellation and recovery, indexed preview, pose-cache reuse, cached preset reranking, exports, reopening, model removal, and orphan-process checks. Sampled process-tree RSS/CPU, worker counts, cache bytes, cancellation outcomes, hardware, stage timings, and limitations are recorded. Single-image execution is the baseline; no batching claim is made.

The initial real-model run started but later hit MEMORY_LIMIT (3.56 GiB available versus a 3.60 GiB reserve on this 24 GiB Mac). A retry was refused during installation at 3.23 GiB available; its failed admission report is retained. The real native preset integration was also refused at 3.44 GiB. The reserve was not lowered or bypassed and unrelated applications were not terminated. **A completed real-model soak remains pending on a machine with sufficient available memory.** The protocol-double tests below establish orchestration, not ONNX accuracy or real stress acceptance.

## Verification

- Portable fault-injection preset integration uses real indexed FFV1 media and a clearly labeled detector double. It verifies tracked regions, cache-only weight changes (no new detector calls), reopening, missing-capability rejection, explicit opt-in, child cancellation, and model-removal invalidation.
- Tracker tests cover motion, competing subjects, cuts, disappearance, long gaps, and obvious appearance mismatch.
- Frontend tests cover explicit opt-in, large exact ordinal strings, chosen subject, cancellation, partial results, exact frame restoration, and disabled missing capabilities.
- Existing native regression/fault suite and controlled HTTPS tests passed before the final focused additions; final rerun evidence is recorded below.
- Windows native desktop, GPU execution, five-minute real-session idle observation, handles/descriptors, two-hour real inference soak, and human visibility/tracking/preset evaluation remain separate gates.

Final checks: the Rust workspace regression/fault suite passed with the new portable preset workflow; TypeScript, generated contracts, 21 frontend tests, and production frontend build passed. The macOS bundle built and opened existing projects. Native review confirmed missing-model gating and independently scrolling preset controls while the viewer and precision controls stayed docked. The initial native capture was approximately 1440×886 logical pixels (screen-height constrained), with a subsequent shorter window around 1431×700. Exact 1440×960 and 1000×700 native size review remains pending; those captures are not mislabeled as the required sizes. A visual review identified the new execution button's native default appearance; it now uses the shared Fluent Button, with the same existing panel styles.

Existing GitHub run 38034675981 completed macOS native checks and both contract/frontend jobs. Windows built the private installer, then the installed-worker smoke failed with process exit 3221226505 and no stderr. This predates the tracking changes; Windows installed inference/package acceptance remains open, and no Windows pass is inferred from successful compilation.

The stress reporter atomically replaces its own JSON file at every checkpoint and retains partial progress after failures. Its hardlink-preservation regression passed. The final macOS bundle passed strict deep code-signature verification; this is an ad-hoc private build, not a signed public distribution.
