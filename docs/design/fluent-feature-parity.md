# Fluent docked editor preservation checklist

The existing workspace controller, command client, Rust APIs, IPC contracts, database, and stored projects remain unchanged. The earlier Canvas checklist supplied the behavior baseline before replacing presentation.

| Behavior | Rebuilt location | Evidence / remaining work |
|---|---|---|
| Import and native drop | Header and direct attachment area | Existing native-drop frontend integration test passes; native picker smoke pending |
| Project creation/reopening | Visual Project Library | Reopening and retained selections tested with command fixtures; native smoke pending |
| Source switching | Compact header source selector | Cross-source exact frame restoration integration test passes |
| Playback, exact stepping, timestamps | Docked toolbar | Timestamp 0.7 resolves ordinal 7 / PTS 700 in integration tests; focused Space behavior retained |
| Precision wheel and shortcuts | Timeline below viewer | Shift+arrow stepping and wheel control integration tests pass; physical trackpad not exercised |
| Thumbnails and timeline scrub | Horizontal filmstrip and scrubber | Existing controller unchanged; resolved thumbnail carries selected state |
| Zoom/pan/reset | Docked toolbar and opaque viewer | Zoom/reset integration assertions pass; pointer pan retained; native drag smoke pending |
| Actual frame metadata | Permanently accessible right dock | Ordinal, resolution, presentation time, PTS, origin, index state retained; metadata test passes |
| Cross-source selections | Independently scrolling vertical right list | Exact source and ordinal restoration integration test passes |
| PNG, JPEG quality, lossless WebP, native folder, sidecars | Export Studio | PNG, JPEG quality, and lossless WebP command arguments tested after simulated job completion; native output/sidecar smoke pending |
| Jobs, cancellation, errors | Activity panel and visible error banner | Cancel/cancelling states verified by integration test; cancellation under native load pending |
| Privacy/media information, keyboard reference | Settings | Present; no permanent privacy badge; AI/shop remain unavailable |

## Current verification

- TypeScript check, seven frontend tests, production frontend build, and macOS native bundle build passed on October 8, 2026.
- Browser captures cover all four views at 1440×960 and 1000×700. No horizontal overflow. Right dock measured 300px / 260px. Header has no backdrop blur. All surfaces are opaque.
- Native startup initially encountered the dev instance’s database lock. The agent-owned development instance was stopped; the rebuilt packaged app then started successfully. CUA still reports the Mac locked even after the user reported unlocking it, preventing click-through native verification. Earlier Canvas native checks are historical evidence, not a claim that the rebuilt native interface has been exercised.
- Windows native rendering/dialogs, physical trackpad navigation, OS reduced-motion toggling, and cancellation under native load remain unverified.
- Browser keyboard focus was measured as a 2px #479ef5 outline with 2px offset; long project-name input caused no document overflow. Settings scrolls independently (780px content in a 618px viewport at the minimum size). Actual long native filenames remain a smoke-test gap.
- Since this design uses opaque surfaces throughout, a blur fallback is unnecessary. Reduced-motion CSS disables transitions; source data and processing remain local.

- Fresh finish review found one source-floor icon inconsistency. The breadcrumb glyph was replaced with Lucide ChevronRight; the reviewer scored that sole fix resolved. This scoped verdict does not imply full native QA. DESIGN.md and its v2 sidecar now document the Fluent world.
