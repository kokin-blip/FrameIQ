# Selection actions and preset placement — 2026-10-11

## Behavior

- Presets are available inside Selections; the separate tab is removed.
- Add/remove current frame uses its exact indexed ordinal. Remove on each row does not navigate or seek first.
- Select all adds every indexed frame of the current source, preserving other sources and avoiding duplicates. It requires completed indexing.
- Remove all clears selections across the current project; source files, analysis, other projects, and existing immutable export plans are unaffected.
- Auto-select frames runs the existing eligible experimental preset pipeline and adds successful results automatically. It preserves manual selections and works without a mounted result panel. Cancelled and failed runs retain analysis evidence but do not automatically add partial results. Successful early-stop tracks can add usable partial results, with coverage explanations preserved.
- Missing/unvalidated model capabilities remain unavailable; this UI change does not close accuracy gates. Explicit experimental opt-in and subject/region requirements remain.
- Weights/custom configuration collapses independently. Results can be manually added again after collection removal.

## Implementation

No migrations, stored-project format changes, or FrameRefV1 changes. Additive commands `select_all_frames`, `clear_selections`, `add_selections`, and `select_preset_frames` reuse existing contracts and jobs. Existing `execute_preset` retains analysis-only behavior for scripts/benchmarks.

Select-all uses one atomic INSERT SELECT; clearing uses a project-scoped DELETE. Recommendation batches validate every source/project/exact frame reference before a transaction and accept at most 50 entries. Selection loading joins frame rows and caches source metadata instead of performing per-frame database queries; native reads and bulk operations run on background tasks. FrameCards displays 100 rows per page and generates bounded later-page thumbnails. No frame ordinal is computed from FPS.

## Evidence

- Full Rust workspace regression/fault suite passed (excluding desktop tests, covered by native build).
- Persistence tests cover repeated PTS, ordinals beyond JS integer precision, incomplete indexing, duplicate addition, cross-source/project scope, stale batch rejection without partial writes, reopening, and original-file preservation.
- Real-media portable detector-double orchestration proves backend auto-selection, preserved manual selection, reopening, cached ranking, and no changes after cancellation. Detector doubles do not establish model accuracy.
- All 32 frontend tests in ten files pass, including current/row/bulk actions through the workspace controller, preset refresh and exact restoration, cancellation, disabled states, and 205-frame pagination retaining exact identities.
- TypeScript, generated-contract checks, frontend production build, and the 56.52 MiB macOS app build pass. The isolated preview passes strict ad-hoc signature verification. DTOs are unchanged by these controls.
- Editable Figma states precede production controls. Inter and composition screenshot were reviewed; the primary action reuses the existing Fluent component.

Logs: `/private/tmp/frameiq-selection-*.log`.

## Pending native acceptance

The Mac was locked when native UI inspection was attempted. Visual and interaction checks at 1000×700 and the available large-window size remain pending until unlock. Windows native interaction and model accuracy remain separate gates. This report does not claim those checks passed.

### Native follow-up

The usability pass subsequently imported an isolated synthetic 20-frame source, stepped and sought exact ordinals, added the current frame, and used Select all to obtain 20 selected frames. Independent scrolling and the anchored export action were visible at 1000×700 and the available 1440×889 size. Presets were in Selections, with no separate Presets tab. The Mac locked before the final removal/reopening/crop checks. See `usability-improvements-2026-10-11.md` for remaining checks; Windows interaction and model accuracy remain open.

The unlocked follow-up passed native Remove all (20→0), added a new synthetic source selection, and reopened the renamed project with that selection intact. The updated main app opened its existing project with three selections preserved. See the usability report for crop checks and remaining platform/service gates.
