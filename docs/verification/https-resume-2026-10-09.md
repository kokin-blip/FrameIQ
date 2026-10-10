# Controlled HTTPS and removal recovery

The private unit-test transport runs the production download state machine against real loopback TLS sockets. Only that transport trusts the disposable localhost certificate; production still requires permitted HTTPS origins, valid TLS, restricted redirects and no proxy. No frontend bypass or test certificate is included in the application.

The downloader now uses connection and per-read idle deadlines rather than a whole-transfer deadline. Cancellation is polled while connection/header/body futures are pending. Strong entity tags and complete numeric content ranges are validated. A `200` restart truncates and syncs retained bytes before committing the new validator, preventing old bytes from being associated with a new entity after a crash.

Twelve ordinary downloader tests pass, including eight real TLS scenarios: interrupted body/range continuation with captured headers, ignored range and weak/missing validators, malformed range/changed tag/HTTP failure/unsafe redirect, mid-body cancellation and retry, untrusted TLS and stalled body, progressing transfers longer than the idle deadline, and oversized/checksum/unsafe-graph failures. The socket tests require loopback permission in restricted environments. They use a small standard-operator graph for download verification, not a compatible production detector.

Model removal now commits status and dependent feature invalidation in one SQLite transaction. An injected status-write failure proves person/pose features and the ready installation remain intact. Files and masks are removed only after commit. Before/after-commit and after-files process-crash tests verify restart recovery and original-file retention. Deletion removes only `model.onnx` and an empty directory; unknown files remain untouched.

The lifecycle protocol worker is now a portable Rust test binary available only with `fault-injection`, enabling Windows activation/removal/validation/cancellation tests. Symlink-specific cases remain Unix tests.

The real TLS process-crash matrix now covers before/after reset, during metadata writes, before metadata commit, after metadata publication, after body write, and before verification. Restart never associates old bytes with a new validator and final hashes match. An opt-in download through the production transport from the reviewed YuNet public host passed; it verifies size, checksum and graph, not detector accuracy. Ordinary CI does not execute that external request.

Verification: full Rust workspace tests excluding the desktop shell pass, along with TypeScript, all 18 frontend tests and generated-contract checks. GitHub access succeeds with network permissions. The empty remote was reconciled and the audited baseline published on `codex/milestones-1-4-acceptance`. Controlled TLS tests passed on a Windows runner. Native build and installed Windows evidence remain separate gates.

Checkpoint temporaries now use a reserved hash-qualified name. Startup and explicit retry remove only matching regular checkpoint files; partial bodies, resume state, symlinks and unknown files remain protected. The crash matrix confirms checkpoint orphans are cleared.
