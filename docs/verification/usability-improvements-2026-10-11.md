# Approved usability improvements — 2026-10-11

## Delivered behavior

- Essential frame ordinal, dimensions, and time stay visible. PTS, timestamp provenance, and indexing details are directly accessible under Frame metadata.
- Fit and actual-pixel 1:1 modes are distinct. Actual pixels account for device pixel ratio; reset restores fit and zero pan.
- Expanded precision loads nine exact neighboring presentation ordinals. Global scrubbing remains available. String ordinals retain identities beyond JavaScript's integer range.
- Library search, name/recent sorting, project rename, and source display aliases are available. Aliases do not move original files. Last-opened information is recorded prospectively; old projects without records are labeled honestly.
- Export action remains outside scrolling content. Advanced color controls collapse independently; configured HDR/preservation policies reveal them.
- Settings separates Local models, Cloud AI, and General & shortcuts. Three pinned experimental local packs can be selected without importing a manifest; existing source/license/size/checksum review still gates downloads.
- Recommendations keep scoring under Ranking settings and explain that comparison needs 2–4 frames.
- Cloud model listing is explicit; listed models are not claimed compatible merely by listing. Manual IDs remain available.
- Main-frame cropping supports drawing, moving, resizing, and numeric alternatives. A full-frame initial rectangle starts drawing on the first drag. Request changes invalidate existing cloud review/consent.

## Data and verification

Migration 010 adds only truthful project activity. Existing names use their existing columns, source identities and FrameRefV1 remain unchanged. Backup/checksum upgrade tests cover preservation of sources and selections from the previous schema.

- 34 frontend tests in 11 files pass, including exact neighbor identities, normalized crop drawing/movement/resizing, and first drag from a full-frame rectangle.
- TypeScript, generated-contract check, and production frontend build pass.
- Full Rust workspace regression/fault suite passes excluding the desktop package; the desktop package compiles in the native production build. Seven persistence unit tests pass including upgrade and alias/activity integrity.
- Native macOS preview built with an isolated application identifier, leaving the older running preview and user project untouched. Synthetic 640×480 FFV1 fixture, 20 indexed frames, no private footage or API credentials.
- At 1000×700 and available 1440×889 logical window size, checked studio controls, selection scrolling, true 1:1 display, expanded neighbors, and persistent export footer. Exact step 0→1 and timestamp 1.000→ordinal 10 passed; Select all returned 20.
- Native Local models selector displayed all three packs and Download stayed disabled without explicit review. General & shortcuts displayed separately.
- Native project rename, source-alias save, matching and no-match search, and recorded opening time passed. Numeric crop fields visibly changed the main-image overlay.
- Figma editable compositions were created before production controls; node ledger is `docs/design/usability-improvements-state.json`. Library was visually reviewed after targeted overlap fixes. These are interaction handoffs, not pixel-identical native screenshots.

Logs are local `/private/tmp/frameiq-usability-*.log`; review captures containing user media stay outside committed artifacts.

## Unlocked Mac follow-up — 2026-10-11

- Restarted the isolated Usability Preview from the rebuilt bundle. Its former `/private/tmp` clip had expired; imported a new synthetic 640×480 FFV1 clip from the ignored local review directory instead of replacing the original source revision.
- Remove all changed the isolated project's collection from 20 to zero and disabled Export selected. Adding frame 0 and reopening the renamed project restored the selection and decoded frame; the project name and source aliases remained saved.
- First drag from the initial full-frame crop created a rectangle (x 14.05%, y 20.04%, width 44.93%, height 41.39%). Moving it changed x/y to 22.22%/30.94% while retaining dimensions. Dragging the corner changed width/height to 58.01%/53.38% without changing its origin.
- Reviewed the crop overlay at the available large window and 1000×700; it stayed aligned to imported media after resizing. Capture: `.impeccable/review/usability-implementation/crop-1000x700.png` (ignored local artifact).
- Opened the updated main `target/release/bundle/macos/FrameIQ.app`. Its existing real project loaded with 365 indexed frames and three saved selections; collection data was not altered during verification. The main app is left open.

## Remaining platform/service acceptance

The available display did not permit a 1440×960 native window. Windows native UI checks, real provider-key/model listing and generation, and model accuracy remain separate gates. No paid provider request or model download was performed. Existing GitHub release was not changed.
