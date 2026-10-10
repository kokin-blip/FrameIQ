# Canvas studio preservation checklist

The canvas rebuild replaces presentation only. Rust commands, frame identity, SQLite data, and offline media processing remain authoritative.

| Existing behavior | New location | Verification |
| --- | --- | --- |
| Import; create and reopen projects | Header and visual Project Library | Native dialogs and project reload |
| Switch source videos | Header source selector | Restore selected source and exact ordinal |
| Muted preview; previous/next frame; timestamp entry | Floating tool dock | Playback, boundary stepping, timestamp |
| Mouse wheel, trackpad, arrows, Shift+arrows, S, Space | Precision wheel and studio keyboard handling | Existing shortcuts and focused-control behavior |
| Thumbnails and timestamp scrub | Bottom timeline | Resolve thumbnails and scrub by presentation time |
| Zoom, pan, reset | Canvas zoom controls | Inspect without altering export |
| Resolution, PTS, presentation time, provenance, index state | Collapsible Frame Details inspector | Original fields retained |
| Cross-source frame selections | Collapsible frame collection | Restore exact source/ordinal; select and deselect |
| PNG, JPEG quality, lossless WebP; sidecars and folder choice | Export Studio | Export native fixture with provenance |
| Persisted progress, errors, cancellation | Expandable activity panel and error banner | Job updates and cancellation |
| Offline/privacy/media information and shortcuts | Settings | Factual current capabilities |

No cloud/AI feature is enabled by this redesign. Future workflows are Figma designs only.

## Verification evidence — October 8, 2026

- TypeScript check and seven frontend tests passed. The production frontend and macOS application bundle built successfully.
- All four views captured and reviewed at 1440×960 and 1000×700 in the browser; screenshots are in `review/canvas/`.
- Packaged macOS app: created a project, attached the synthetic fixture through the native file picker, indexed 20 frames, played muted preview, stepped exact frames, resolved timestamp 00:00:00.700 to ordinal 7 / PTS 700, selected frames, expanded metadata and precision controls, zoomed/reset, and reopened the persisted project with its selections.
- Native export: two 640×480 PNGs and their JSON sidecars succeeded through the native folder picker (`/private/tmp/frameiq-check/FrameIQ-export-7bb1e70d`).
- Integration tests verify native drop dispatch, cross-source exact-selection recovery, timestamp resolution, JPEG quality/export arguments, project reopening, wheel keyboard stepping, and cancellation/cancelling UI states.
- CSS implements reduced-motion and opaque backdrop-blur fallbacks. Neither OS preference toggling nor a real no-blur WebView was exercised.
- Fresh finish review: `ship` at the supplied capture/source scope. Native SVG glow artifacts were then corrected with static CSS radial fields; the final macOS capture confirms no visible edges and correct primary-action text color.
- Direct utility copy appears in Figma and the packaged empty state. The workspace footer has no local-processing shield or badge.

Remaining platform coverage: Windows native dialogs/rendering and manual long-file/trackpad/cancellation-under-load scenarios require their platform smoke pass. Browser captures represent responsive layout, not native file-access behavior. Future AI workflows remain design foundations only.
