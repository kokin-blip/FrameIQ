# Fluent docked editor handoff

## Visual authority and provenance

The approved reference is `mockups/fluent-option-1.png`, option #1 from the latest Fluent 2 pair. Its composition supersedes the floating Canvas/DesignCode iterations. The reference image is illustrative; it is not embedded in the app.

Official inspected previews: https://fluent2.microsoft.design/get-started/design . The Microsoft Fluent 2 Web community kit (https://www.figma.com/community/file/836828295772957889/Microsoft-Fluent-2-Web) was not available as an editable library through the connected file. New components are local editable adaptations of official previews and the approved reference, not imported official Fluent components.

Figma file: https://www.figma.com/design/5L64rf754pinHPHimkme5e . Page **10 Fluent Docked Editor** was created before frontend changes. Earlier pages 01–09 are preserved. Node ledger: `fluent-state.json`.

## Component mapping

| Figma composition | React / CSS |
|---|---|
| Compact header | StudioHeader / studio-header |
| Docked precision toolbar | StudioToolbar / studio-toolbar |
| Opaque frame viewer | Studio / canvas-stage, image-viewport |
| Filmstrip, scrubber, frame wheel | Timeline |
| Selections/details dock | FrameCollection, FrameCards, Inspector |
| Project Library | ProjectLibrary |
| Export Studio | ExportStudio |
| Settings | Settings |
| Background activity/error | Activity, App error banner |

All presentation uses React, TypeScript, Zustand, and plain CSS. Existing outlined Lucide icons use a consistent 1.6px stroke as a local Fluent-style adaptation. No new styling framework or native API was introduced.

## Tokens and necessary adaptations

Graphite background #1b1b1b; surfaces #202020/#292929; divider #3d3d3d; text #f5f5f5/#bdbdbd; actions #0f6cbd with white text; focus #479ef5. The darker action blue provides text contrast. Control radius 4px, panel radius 6px. Inter remains bundled locally.

Toolbar and header stay docked. Viewer and timeline occupy the left column; right panel is 300px, reduced to 260px at compact sizes. The selections list scrolls independently. Metadata is always visible, including required PTS/origin/index fields missing from the illustrative reference. Export opens the format/folder workflow. Studio/Export navigation remains compact in the header so both working views are directly accessible.

No decorative glow or blur remains. Hover uses a restrained surface change; focus is a separate visible outline; reduced motion disables transitions. Imported frame images fit the viewer, retain their aspect ratio, and remain on an opaque neutral canvas. Zoom is relative to that fit, not exported pixel scaling.

Plain attachment copy remains “Drop a video here / or click to attach.” No permanent privacy badge. Actual runtime frames, thumbnails, metadata, selection counts, and outcomes replace illustrative Figma values.

## Verification artifacts

Browser captures: `review/fluent/` at both required sizes. Tests and native-build evidence and remaining platform limitations are recorded in `fluent-feature-parity.md`. Earlier handoffs and captures are preserved as iteration references.
