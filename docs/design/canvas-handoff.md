# FrameIQ canvas studio handoff

The user-approved canvas studio follows DesignCode expressive dark glass. Product truth is in `apps/desktop/PRODUCT.md`; visual tokens are in `packages/ui/src/tokens.css`. `apps/desktop/DESIGN.md` captures the implemented system and `apps/desktop/.impeccable/design.json` contains preview snippets and extensions. The previous lime/sidebar document is superseded. Original Figma designs and the intermediate glass iteration remain preserved.

## Figma provenance and mapping

Working file: [FrameIQ](https://www.figma.com/design/5L64rf754pinHPHimkme5e), canvas page `12:2`. The node inventory is `docs/design/canvas-state.json`. Reference: [DesignCode kit](https://www.figma.com/design/NKlo830IkWSROn9bVbPtNQ). Its unpublished component keys were unavailable for import. FrameIQ therefore uses locally editable adaptations of inspected kit material, palette, typography, and control patterns. These are not claimed to be imported kit instances. Locked Pro imagery was not used.

| Figma node | Implemented surface / source |
| --- | --- |
| `12:3` Canvas Studio · empty | `Studio.tsx`, `ToolDock.tsx`, shared header, timeline, collection |
| `12:54` Precision selection · expanded timeline | `Timeline.tsx`, `Inspector.tsx`, resolved frame state |
| `12:106` Project Library | `ProjectLibrary.tsx` |
| `12:145` Export Studio | `ExportStudio.tsx`, shared frame cards |
| `12:175` Settings and Model Management | `Settings.tsx`; current factual information only |
| `12:365` States | Code-specific empty, resolving, disabled, selected, error, and activity treatments |
| Local reused foundations `10:24`, `10:35` | Editable foundations recorded by the canvas inventory; not external kit instances |
| `VariableCollectionId:10:3` | Shared palette, radius, and spacing foundations; code values take precedence where implementation adapts them |

The variable mapping recorded in `docs/design/designcode-state.json` is: background `10:4`, surface `10:5`, raised `10:6`, text `10:7`, muted `10:8`, border `10:9`, accent `10:10`, danger `10:11`, purple `10:12`, cyan `10:13`; control/panel radius `10:14`/`10:15`; spacing 1–6 `10:16`–`10:21` (each prefixed `VariableID:`). These map to identically named CSS roles (`--color-*`, `--radius-*`, `--space-*`). Added implementation roles such as canvas, fallback, selected, ink, hover, and error surface are documented from current CSS rather than assigned speculative Figma IDs. Typography and glass styles are recorded in the same inventory.

AI Presets `12:200`, Frame Recommendations `12:230`, Frame Comparison `12:257`, Fashion Detection `12:284`, Product Identification `12:311`, and Shop the Look `12:338` are editable design foundations. They are not enabled product capabilities and require detailed follow-up handoff before shipping. Model management is likewise planned; the current Settings implementation says so explicitly.

## Composition and assets

The shared header provides project/source context, Studio/Export navigation, import, and Settings. The opaque canvas replaces the prior rail/persistent-inspector arrangement. A floating dock contains playback, stepping, timestamp entry, frame identity, and selection. The bottom timeline has expandable precision guidance; a collapsible collection retains selections from all sources; Frame Details opens contextually over the canvas. Library, Export, and Settings are implemented production views split into components while retaining workspace state and IPC behavior. This redesign makes no Rust or database changes.

Ambient fields are authored static CSS radial gradients using the same purple, blue, and cyan palette. They replace SVG filters that rendered incorrectly in native WebKit. No downloaded bitmap background or locked kit imagery is needed. Image content comes from local sources; verification uses synthetic media.

The attachment copy is exactly “Drop a video here” / “or click to attach”. The bottom-left shield/local badge was removed. Factual privacy details are in Settings. Avoid promotional instructions or unsupported AI/shopping promises.

## Precision and implemented states

Muted preview is approximate and may skip displayed frames. Selection and export use resolved presentation identity. Previous/next, wheel, arrows, and Shift+arrows operate on frames; timestamp seek resolves presentation time. Selection retains source plus ordinal; reopening a collection card restores both. Zoom/pan change inspection only. Frame Details retains resolution, presentation time, PTS ticks, timestamp origin, and index status.

Empty playback/step/selection controls are disabled when their prerequisites are missing; select is also disabled while resolving. “Resolving frame…” uses status semantics. The selected action reads “Selected” and can deselect. Timeline thumbnails appear as indexing finishes. Export supports PNG, JPEG with quality, and lossless WebP; every exported frame has a JSON provenance sidecar. HDR output is unavailable. Progress/errors/cancellation remain in the existing activity flow; missing-source recovery remains in Library.

## Accessibility and platform adaptations

Implemented adaptations include labeled icon controls, expansion/current-page semantics, labeled timeline and timestamp controls, frame identity alt text, keyboard wheel equivalents, and a cyan focus outline (2px with 3px offset). Space preserves focused button/link activation and requires an available frame. Inputs/selects retain typing. Reduced motion removes animation/transition; unavailable backdrop blur uses an opaque fallback surface. Native file and destination dialogs remain native. Numerals use tabular figures in precision and metadata displays.

The supported minimum is 1000×700. Browser captures at that size and 1440×960 show the four implemented views. Width ≤1200px and height ≤760px compact the desktop composition; collection collapse helps preserve canvas area. These are observed desktop adaptations, not a mobile implementation or a completed assistive-technology audit.

## Validation and limits

Reported completed checks: seven frontend tests, TypeScript checking, production frontend build, and macOS bundle. Native smoke covered stepping, timestamp seek, selection, and PNG export of two files with their sidecars. The replacement macOS bundle was launched; project creation, file-picker attachment, a 20-frame index, playback, zoom reset, expanded precision, and persisted project reopening passed.

Evidence is under `docs/design/review/canvas/`: four-view browser captures at 1000 and 1440 widths, `studio-native-fixed.png` for smooth radial glows and readable navy primary-action text, and `empty-native.png` for the plain attachment target and removed footer badge. Earlier `studio-native.png` records the pre-fix native state and must not be treated as the final glow reference. Consult the current preservation evidence for individually checked behaviors. The old `docs/design/review/verdict.md` describes the superseded lime workspace and is not a canvas verdict.

The finishing reviewer’s disposition was **ship** within the supplied captures/source scope. Windows and post-fix native rendering were unverified at the time of that review; the later macOS replacement-bundle launch supplies the native evidence above. Windows, physical trackpad interaction, cancellation under load, and preference-toggle behavior remain unverified manually. No new broad audit or testing claim is implied by this documentation.
