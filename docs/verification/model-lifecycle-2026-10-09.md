# Model lifecycle recovery

Model installation now stages a validated file in an explicitly application-named temporary directory and activates it with one directory rename. The registered installation is committed afterwards. A failed commit deletes the new directory; a process crash before that cleanup is repaired at startup under the exclusive instance lock. Existing installations remain separate and usable throughout.

Startup recovery removes unregistered canonical UUID-v4 installation directories and recognized new staging directories only when empty or containing a regular `model.onnx` file alone. It preserves registered installations, legacy unrecognized temporary names, unknown contents and symlinks. Cleanup is best effort for optional filesystem storage and never follows redirected model/download roots. Recognized staging names contain the installation prefix, a UUID-v4 and tempfile's six-character random suffix.

Retained downloads remain available for resume. A full download duplicate is removed only when a ready installation and the retained file both match the declared size and SHA-256. Hashing uses bounded streaming buffers; it does not allocate the model's entire file. Missing or damaged installations never justify deleting a retained copy. This startup check can read both files once when a duplicate exists; storage timing remains a performance acceptance item.

Imports and removals reject redirected model storage before changing records. Downloads reject symlinked storage/retained files before dispatch. Cancellation already present before dispatch returns `CANCELLED` without requesting the model URL or altering resume files. Completed retained files that fail verification are removed and require a subsequent explicit retry.

## Evidence

`crates/application/tests/model_lifecycle.rs` uses a deterministic validation worker and deliberate process exits. It establishes orchestration and recovery behavior, not ONNX/model quality:

- Before-activation crash: abandoned staging removed on restart.
- After-activation crash: unregistered new installation removed on restart.
- After-commit crash: committed new installation preserved on restart.
- An injected SQLite insert failure leaves the prior installation and imported original untouched.
- Validation rejection, worker crash and cancellation remove staging, preserve earlier models and allow successful retry in the same application.
- Redirected import/removal storage is rejected; restoring storage permits normal removal while retaining the original import.

Unit tests in `model_recovery` cover committed/orphan/unknown/symlink cases, duplicate cleanup and preservation of interrupted downloads. Download unit tests cover cancellation before dispatch, corrupt completed-cache verification and redirected-file rejection with no network requests.

The existing opt-in native `indexed_person_pose_cache_with_native_models` test also passes with the reviewed person and pose weights, the bundled ONNX Runtime 1.22.0 CPU runtime and the permitted local body fixture. It exercises actual model imports, inference, cache reuse, masks and dependency removal. This is one correlated fixture, not a visibility or pose accuracy evaluation. Automatic preset capability gates remain closed.

## Remaining gates

Actual HTTP interruption/range/ETag/timeout recovery, download-worker crashes, removal transaction failure recovery, accelerator fallback/resource stress, real Ollama/LM Studio servers and Windows lifecycle tests remain open. Legacy unrecognized staging folders are intentionally preserved. This change adds no migrations, IPC fields or production UI controls.

Verification: the Rust workspace regression/fault suite passes. After the final removal guard, all five active lifecycle integration tests pass (the ignored crash helper is invoked by the matrix), and the reviewed native person/pose integration passes again. Formatting, production frontend TypeScript/build checks and the 53.96 MiB macOS package pass; deep/strict ad-hoc signature verification passes. No desktop interaction, live provider or Windows acceptance is inferred from these results.
