# Independent HDR and export recovery checks

The native media test `crates/media/tests/hdr_ramps.rs` generates a lossless FFV1 fixture from ten explicitly quantized limited-range YUV16 neutral-gray stripes. The fixture generator performs no tone mapping. Both PQ and HLG have declared BT.2020 metadata. Indexed ordinal zero and its resolved PTS are used for every conversion.

The test computes expected values with separate double-precision equations: ST 2084 EOTF, HLG inverse OETF with a 1.2 system gamma and 1000-nit reference display, Hable mapping to the configured peak, and standard sRGB encoding. It never uses a production filter to produce reference pixels. Tone-map expectations follow the [pinned FFmpeg Hable implementation](https://github.com/FFmpeg/FFmpeg/blob/n8.0.3/libavfilter/vf_tonemap.c); transfer behavior follows [pinned zimg 3.0.5](https://github.com/sekrit-twc/zimg/blob/release-3.0.5/src/zimg/colorspace/gamma.cpp).

Assertions cover:

- Twenty RGB16 stripe samples within two 16-bit codes of independently scaled source values.
- Forty SDR stripe samples within one 8-bit code of analytical expectations, including default 1000-nit fallback and explicit 2000-nit peak.
- Monotonic SDR output and all three neutral channels.
- PNG IHDR bit depth and cICP parsed at actual chunk boundaries; HDR has no sRGB chunk, SDR does.

The oracle allows output quantization and native float arithmetic without changing production tolerances. These checks are synthetic grayscale evidence on macOS. They do not establish chromatic gamut behavior, mastering/light-level metadata correctness, real HDR appearance, Dolby Vision support, or Windows acceptance.

The offline integration regression also disconnects the export directory by renaming it and placing an unrelated file at its original path. Resume returns `DESTINATION_MISSING`, retains an inactive resumable plan and preserves the unrelated file. After restoring the folder, both completed outputs verify as skips. A modified invalid-JSON sidecar is preserved, reports `OUTPUT_CONFLICT`, and records one failed item plus one pending item. This found and corrected a generic-error classification in sidecar verification: saved checksums are now checked before parsing, and invalid saved JSON is classified as a conflict.

Disk exhaustion, filesystem permission failures, actual removable-drive behavior and Windows packaged recovery remain separate gates.

Verification: `cargo test -p frameiq-media -p frameiq-application --features frameiq-application/fault-injection` passes all 15 active tests, with three explicitly opt-in/helper tests ignored. The crash matrix executes all five helper checkpoints as subprocesses. All 18 frontend tests pass and `cargo fmt --all --check` passes.

The production frontend TypeScript/build checks and macOS desktop packaging pass. The rebuilt 53.96 MiB application passes deep/strict ad-hoc signature verification. This pass does not repeat native interaction or Windows installation checks.
