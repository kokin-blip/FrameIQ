# Architecture decisions

The local checkout and GitHub remote were confirmed empty before scaffolding; origin is kokin-blip/FrameIQ. No existing implementation was replaced.

Rust owns versioned contracts, source revisions, presentation ordinals, SQLite, supervised jobs, and export fidelity. React owns interaction and transient state; Zustand stores recoverable job snapshots. A thin Tauri 2 shell exposes typed commands. FFmpeg 8.0.3 uses a pinned, signature-verified LGPL shared build with network protocols disabled. The worker runs separately and accepts one bounded NDJSON request per process. It never receives credentials.

PTS is not unique. Export identifies a specific index generation and presentation ordinal. Timestamp resolution handles VFR intervals and boundary ties. Seek anchors are validated before ordinal counting; failure falls back to a sequential reference decode. Source size/mtime is checked for interactive reuse and content SHA-256 is verified before export.

Preview and thumbnails use separate bounded worker lanes. Export is unscaled RGB8 with a documented source-matrix/range conversion and lossless rotation. PNG and WebP are lossless; JPEG quality is configurable. HDR/wide-gamut and 16-bit exports are rejected until validated, rather than silently clipped. The first preview is muted and may skip display frames.

SQLite enables foreign keys, WAL, STRICT tables, transactional checksum-verified migrations, startup recovery, and short transactions behind a serialized writer. Native and CPU work stays outside the frontend main thread. Frame and timestamp integer identities are strings across IPC.

Future intelligence and providers remain separate modules. Only validated models may be offered. Model similarity never proves product identity. Evidence must retain source and independence groups. User review is required for verified exact claims. External calls require provider policy, explicit crop consent, atomic quota reservation, and a $0 ceiling. Offline extraction is independently testable.
