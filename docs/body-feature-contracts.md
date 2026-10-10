# Indexed person and pose candidate jobs

These internal APIs provide local candidate regions for later subject selection. They do not enable automatic preset eligibility or imply calibrated visibility.

- `detect_persons(source, ordinal, model)` returns a cancellable job. Its result ID identifies a `PersonFrameV1`.
- `person_feature(source, ordinal, model)` returns the current cached record or null.
- `estimate_pose(person, index, model)` returns a cancellable job for one zero-based person index in that record. Its result ID identifies a `PoseFrameV1`.
- `pose_feature(person, index, model)` returns the current cached record or null. An invalid/missing parent or out-of-range index produces a structured error.

Both records retain the exact `Selection`/`FrameRefV1`, color policy and model provenance. Person features have immutable IDs. The pose cache references its detector feature, person index, pose-model hash and algorithm; multiple subjects cannot reuse one another's masks. There is no cross-frame subject association yet.

Migration 006 extends the existing backup/checksum/foreign-key migration runner. It does not change earlier SQL files or existing frame references. Detector removal cascades its pose records; pose-model removal leaves detector results available. Application-owned masks are deleted with those dependencies. Original imported weight files remain untouched.

Masks are source-sized binary **person** masks, not clothing segments. Each committed mask carries a SHA-256 checksum. Missing or modified masks invalidate the dependent pose cache. Unique output paths avoid overwriting existing files. Cancellation/failure removes uncommitted outputs, while complete cached measurements remain available. Cleanup ignores paths outside the owned mask directory and refuses symbolic-link mask directories.

Numerical parity and held-out accuracy are separate gates. The landscape segmentation-logit parity failure remains documented; decoder/caching success does not clear it. Preset activation requires that gate and calibrated visibility evaluation, plus the subject selection UI and corrections.

Verification lives in `crates/application/tests/detection_jobs.rs` (development-only protocol double), persistence upgrade/dependency tests and the opt-in `indexed_person_pose_cache_with_native_models` integration test. The native test requires already-downloaded reviewed weights and a permissioned local video; ordinary CI makes no model downloads.


## Explicit subject choices

Migration 007 adds `subject_choices` per indexed frame and `region_subjects` links. `choose_subject(person, index)` validates and returns `SubjectChoiceV1`, including the detector installation ID needed to restore the matching UI model. `subject_choice(source, ordinal)` restores that frame's choice; `clear_subject(source, ordinal)` clears it. No choice propagates to another frame.

`GuidedRegionV1.subject` is an additive nullable field; older JSON defaults to no association. Region jobs snapshot the choice at submission. Associations are normalized outside region JSON so detector removal drops the links while retaining manual measurements. UI subject controls do not enable calibrated visibility, tracking or automatic ranking.

## Pinned runtime diagnostic

`tools/verify-pose-ort.py` compares saved native tensors against Python ONNX Runtime 1.22.0/NumPy 2.2.6 using identical input tensors, CPU execution and reviewed weight checksums. Square, landscape, portrait and two-person reports have zero tensor differences. This isolates the previously recorded landscape discrepancy to the comparison with OpenCV DNN; it does not relax the 0.003 tolerances or erase the failed OpenCV report. Automatic preset activation still requires a documented reference decision and held-out visibility evaluation. Python remains a development-only dependency.

Restart mask recovery runs under the exclusive application lock before jobs start. It retains database-referenced masks and only removes unreferenced regular files with app-generated canonical UUID-v4 PNG names in the mask directory. Unknown names, subdirectories and symlinks are preserved. A redirected or unreadable optional AI cache is skipped so offline extraction can still start; pose jobs independently reject unsafe output directories. This covers orphan mask recovery, not incomplete model downloads or installation crash recovery.
