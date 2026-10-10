# Milestones 1–4 panel handoff

The approved Fluent editor remains the visual authority. New editable compositions were made in the existing Figma file before production panels, using the existing Inter styles and local Fluent component adaptations. Earlier iterations remain preserved.

File: https://www.figma.com/design/5L64rf754pinHPHimkme5e . Exact node IDs are recorded in milestones-state.json.

| Composition | Implementation |
| --- | --- |
| Recommendations | AnalysisPanel, dock tab, ranking controls and result rows |
| Comparison | Comparison, linked fit/actual-pixel view and independent metadata |
| Presets / guided regions | PresetPanel, unavailable automatic capabilities, rectangle correction and per-frame measurements |
| Export recovery | ExportStudio, immutable plan status, resume/dismiss and item outcomes |
| Models | ModelManager in Settings, hardware, reviewed manifest/import/download, benchmarks, loopback connections |
| Recovery states | ProjectLibrary/Studio missing-source actions, persistent Activity progress/error/cancellation |

Graphite surfaces, 4px controls, 6px panels, bundled Inter, outlined icons, blue actions and focus outlines continue unchanged. The opaque media canvas is not tinted by panel effects. No new navigation sidebar, glass styling, permanent privacy badge or shopping workflow was added.

Analysis restores indexed frame references. Ranking weights reuse measurements. Partial coverage is labeled; manual selections remain intact. Comparison distinguishes fit-relative zoom from original-image pixel sizing. Keyboard panning is provided on focused comparison panes.

Guided regions apply only to the selected source and indexed frame. Users can draw or enter normalized coordinates, measure, reopen, redraw and remove regions. Automatic face/pose/hair/accessory/footwear/tracking capabilities remain unavailable until their separate adapter, parity and accuracy gates pass. A saved preset definition is not a claim that automatic analysis has run.

Model downloads require manifest review. Compatibility checks are distinct from accuracy validation. Local-runtime discovery is opt-in; synthetic probes do not transmit imported project images. Progress and cancellation use the existing Activity surface. Optional credentials use OS storage.

Verification: packaged macOS recommendations and two-frame comparison were exercised, including actual-pixel mode. TypeScript, frontend tests, production bundle and contract checks pass. Full 1440×960 / 1000×700 native visual review, all interaction states and Windows screenshots remain open. The Mac locked again during resizing; no completed minimum-size result is claimed. See docs/milestones-1-4-status.md for functional release gates.

Resource label correction (2026-10-09): Settings describes the hardware-derived count as shared CPU slots, with one AI operation. Managed native operations use one compute thread per slot; this is a copy correction within the existing composition.


Subject-selection continuation (2026-10-09): editable board 27:55 and screen 27:56 extend the existing preset dock, preserving earlier designs. Local SubjectRow component set 27:54 covers default, selected, focus and disabled states; adjacent examples cover model absence, no people, running, cancelling, error and pose abstention. All 60 text descendants use Inter; the board contains 15 component instances and no flattened UI image. This is a local Fluent adaptation, not an official library import.

`SubjectPanel` maps those states to reviewed-model candidates, numbered detector regions, explicit per-frame choices, pose inspection and the existing job/cancellation service. Landmarks and an optional person-mask overlay are shown only in the inspection preview. The opaque main frame is unchanged. Detector rectangles locate people; they are not body or garment outlines. Manual drawing/numeric corrections remain available without a model or subject. The panel scrolls independently in the existing 300px/260px dock.

Browser fixture captures: docs/verification/subject-ui/1440-selected.png and 1000-selected.png. The images use the permitted NASA astronaut fixture and real decoder artifacts through a temporary mocked command client; they are layout/interaction evidence, not native or accuracy validation. Model absence, keyboard choice/clear, stale asynchronous frame responses, cancellation, failures and restored experimental pose results and saved detector restoration with multiple installations are tested in SubjectPanel.test.tsx. The temporary fixture entry was removed before the production build. The Mac was locked during this pass; native dialogs and packaged UI interaction remain unverified.
