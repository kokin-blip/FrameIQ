# FrameIQ milestone acceptance

The Fluent offline extractor remains the baseline. Milestones are complete only when their implementation and acceptance evidence pass; synthetic tests and browser fixtures do not establish model accuracy or native Windows behavior. This matrix records the remaining work against the current implementation.

## Requirement and evidence matrix

| Requirement | Implementation | Evidence | Remaining gate |
|---|---|---|---|
| Exact ordinal navigation and offline exports | Implemented | Native fidelity and offline integration tests | Broader real codecs/VFR corpus; Windows packaged workflow |
| Missing-source relocation | Implemented | Identical-content preservation and mismatch tests | Disconnected drive and platform permission matrix |
| Interrupted immutable export recovery | Implemented | Five crash boundaries, reopening, repeated resume, image/sidecar collision protection and replaced-destination recovery | Disk-full/permissions/platform matrix |
| Per-attempt export accounting | Implemented in migration 008 | Resume writes/skips/failure isolation; historical upgrade test | Native outcome presentation if added later |
| HDR SDR conversion and PNG16 | Implemented | Independent analytical PQ/HLG gray ramps, peak overrides and parsed PNG color tags | Real HDR fixtures, chromatic/mastering metadata and Windows fidelity |
| Windows private installer | Prepared | MSVC build, DLL audit and CI scripts | Execute CI and clean Windows 11 installation |
| Quality analysis and adaptive sampling | Implemented | Versioned metrics and native smoke | Held-out ranking/duplicate evaluation; seek-batch profiling |
| Recommendations and comparison | Implemented | Exact frame restoration and macOS smoke | Full native size/focus/failure review on both platforms |
| Cached reranking | Implemented and locally timed | 2,000 measured synthetic-video frames rank in 1.07 ms p95 on current hardware | Repeat on documented 16 GiB Windows/macOS reference machines |
| Face/person/pose local inference | Experimental | Decoder checks, reviewed weights, indexed cache tests | Held-out quality; documented OpenCV cross-engine discrepancy |
| Rust/Python pinned-runtime pose parity | Tested | Four same-input reports; all tensor differences zero | These correlated fixtures do not establish real-video accuracy |
| Per-frame subject choice and guided regions | Implemented | Persistence, stale-frame UI tests, native backend smoke | Native subject-control review |
| Shot-scoped tracking and corrections | Experimental geometry/appearance tracking and new-anchor correction | Motion/cut/ambiguity/loss tests; portable indexed-media orchestration | Pose continuity, human tracking evaluation and native correction review |
| Automatic visibility and preset ranking | Experimental makeup/body/custom region-quality jobs; missing automatic capabilities disabled | Cached region reranking and exact restoration tests | Visibility calibration, region-specific refinement, hair/detailed regions and complementary views |
| Hair/accessory/footwear packs | Not validated | Candidate choices only | Rights, parity, resource and category-specific quality decisions |
| Model download/import/session management | Implemented foundation | Controlled HTTPS resume/cancellation/TLS/size/validator tests; transactional removal rollback and crash recovery; portable lifecycle helper; prior native inference evidence | Broader inference/resource stress, provider validation and Windows native packaging; process-crash checkpoints and public-host smoke now pass |
| Hardware recommendations and acceleration | Partial | Memory admission, CPU slots, exact budget/reserve tests, idle-worker unload/restart, ten-minute backend smoke | Validated provider/fallback and resource stress tests |
| Ollama and LM Studio | Adapter foundation | Loopback/capability/security and cancellation tests | Real-server interoperability and both credential stores |
| Evaluation corpus | Specification and validator implemented | Validator tests | 30 permissioned clips and 300 human-annotated frames |
| Sustained resource/performance tests | Harness extended | Report records actual duration, sampled RSS and cancellation | Reference-machine long runs and native UI latency |

## Execution order

1. Gather permissioned public/user clips and validate annotations; keep private footage outside Git. Prepare independent color oracles and run Windows packaging concurrently when Windows is available.
2. Close extractor recovery/color/platform gaps. Benchmark sampling before optimizing shared seek batches.
3. Calibrate recommendations on calibration subjects, freeze parameters, then evaluate held-out subjects. Finish native comparison review.
4. Complete inference lifecycle/resource tests and model feasibility decisions. Retain failed reference reports and gate automatic capabilities.
5. Implement shot-scoped tracking, visibility eligibility, automatic preset execution and complementary selection; design new interaction states in Figma first.
6. Run both packaged applications through feature preservation, accuracy and resource gates; publish pass/fail evidence per capability.

## Acceptance thresholds

- Recommendations: at least 80% human acceptance and improvement over uniform and sharpness-only baselines; at least 95% duplicate precision.
- Automatic visibility: at least 90% precision with abstention and coverage reported. Hair: median IoU at least 0.70 on supported visible examples. Evaluate accessory categories separately.
- Performance: UI p95 under 100 ms; cached display under 150 ms; indexed 1080p seek under 1.5 s; cancellation acknowledgement under 250 ms and managed worker stop under 2 s; 2,000-candidate reranking under 300 ms; standard extractor working set under 1 GiB; inference within its configured budget.
- Native UI: 1440×960 and 1000×700, all preserved workflows, long names, scrolling, focus, offline/errors and persisted state.

Accuracy reports must include human labels, sample counts, subject/video splits and confidence intervals. Minimum annotation counts alone are not a release gate. Windows, real footage and model validation remain explicit dependencies.

Native smoke verification exposed an export outcome-scoping issue, now fixed. The latest displayed outcome must match the active project's saved plan or resume destination. Previous-project plans are hidden while the new query is pending; three frontend regressions cover that boundary. Persisted plans and attempt counts remain correctly scoped.

## Acceptance work — 2026-10-10

The audited source baseline is published on `codex/milestones-1-4-acceptance`. Initial CI exposed and repaired native-build prerequisites, PowerShell argument handling, the absent pinned zimg port, Windows checkout line endings, and TLS test timing sensitivity. Successful Windows controlled-HTTPS tests do not imply native packaging acceptance. The latest run must still close native compilation, DLL closure, NSIS installation and isolated worker execution. Clean Windows 11 desktop and GPU review remain pending. macOS and Windows contract/frontend jobs now pass; the latest macOS native regressions passed and reached the timed resource smoke. Windows native compilation/tests passed in the preceding run, with installer staging failing because the Windows ICO resource was missing. The ICO has now been generated from the existing application icon with Tauri’s icon tool; verification reruns remain pending.

HTTP process termination now exercises reset, checkpoint, body-write and verification boundaries. The opt-in reviewed-host production download passed checksum/graph checks. Removal invalidation and status updates are transactional, with process-crash recovery. Model lifecycle tests additionally prove five-minute idle unload/restart and 20 repeated failure/restart attempts without remaining managed processes or staging growth. Fault timings are unavailable in release builds.

The first ten-minute benchmark covered a two-frame, 512×512 FFV1 fixture on the actual 24 GiB Apple Silicon machine. It completed 5,777 cached seek/rerank iterations with stable minute-level coordinator RSS after warm-up. No child worker was captured in that restricted run; its sampled peak cannot establish full extractor/worker memory compliance. The unrestricted repeat completed 5,787 iterations in 600.003 seconds with stable coordinator RSS. Its one-second checkpoints also missed short-lived workers. The summary counter was then corrected to observe every 50 ms sample; a focused follow-up captured one worker. Prior ten-minute reports retain their coverage limitation and are not retroactively relabeled. Both scopes exclude UI responsiveness, inference soak, descriptors/handles and repeated import/export/reopening. CI now includes a 600-second backend smoke and manual 7,200-second option; those reports must be inspected rather than assumed to pass.

Two individually reviewed public originals were acquired outside Git and produced 20 exact indexed PNG annotation inputs. Human fields are null and the validator correctly rejects them as unlabeled. This is not the required 30-clip/300-human-frame corpus. Source/subject split review, second-human annotation review, model feasibility, shot tracking, region-aware automatic execution and held-out preset evaluation remain open. No automatic preset capability was enabled by this acceptance batch, and no UI redesign was made.

The rebuilt 54.01 MiB macOS app passes ad-hoc signature verification. A bundled-worker smoke with only system PATH indexed the two-frame fixture and exported exact ordinal 1. This checks packaged media dependencies, not a new full desktop interaction review.

The Windows dependency audit completed before Tauri reported the missing ICO. The existing PNG artwork now supplies that resource. Native-engine cache keys include source/build scripts and platform architecture (plus Windows MSVC/SDK versions); cached restores still build workers and run regression tests. Installer and installed-worker acceptance remain pending until the corrected run finishes.

GitHub run [38033874156](https://github.com/kokin-blip/FrameIQ/actions/runs/38033874156) completed macOS native regressions and its ten-minute backend smoke successfully; both platform contract/frontend jobs passed. Its Windows native build/tests passed, with packaging still known to lack the newly supplied ICO. The corrected [run 38034675981](https://github.com/kokin-blip/FrameIQ/actions/runs/38034675981) is active. Downloaded macOS CI evidence is retained in `docs/verification/macos-ci-resource-2026-10-10.json`; its actual hardware and small synthetic scope are recorded, with no inference/UI/accuracy claims.


## Tracking/preset execution and broader inference stress — 2026-10-10

Experimental execution now follows the chosen anchor subject through exact adaptive indexed samples, measures target-region quality, and persists tracks and rankings in migration 009. The Fluent panel has explicit opt-in, progress/cancellation, exact restoration, partial-coverage and missing-capability states; corresponding editable Figma states precede the UI controls. The seven preset definitions remain unvalidated. New-anchor corrections fork tracks; uncertain continuity stops them. Visibility, coverage/composition scoring, pose continuity/orientation, hair/accessory/footwear automatic models, backward tracking and complementary selection remain open.

A real-model stress executable now supports cold/warm tensor execution, session changes, cancellation/recovery, cached tracking, exports/reopening, model removal and process-tree resource reports for up to two hours. Current Mac admission correctly rejected runs below the 15% available-memory reserve. These failed runs are retained, not counted as successful inference soak evidence. Portable protocol-double orchestration passed; completed real-model stress and held-out preset evaluation remain pending. See `verification/tracking-and-inference-stress-2026-10-10.md` for the implemented scope and remaining gates.
