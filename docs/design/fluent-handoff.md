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

## Experimental preset execution

`preset-execution-state.json` records editable ready, running, stopped, and unavailable compositions on the existing Fluent page. `PresetExecutionPanel` reuses the shared Fluent Button and existing panel/row styles. Explicit opt-in is required; automatic validation status remains separate from experimental execution. The selected subject/current frame anchors each run. Stopped tracks report partial coverage, and a corrected subject on another anchor starts a new track. Accuracy-gated features remain unavailable rather than displaying illustrative detections.

## Cloud AI extension (2026-10-11)

`CloudProviders` adds OpenAI/Gemini credentials in Settings using the shared Fluent Button and inherited graphite field tokens. Save, remove, and connection testing are explicit actions. Keys use OS credential storage and are excluded from projects and exports. Connection testing lists provider models without sending media; saved-key status is distinct from a successful connection or compatible image model. Editable credential, testing, error, and focus states are recorded in `cloud-connections-state.json`.

`CloudAnalysisPanel` occupies the existing Studio dock. Its four navigation buttons use a two-by-two grid so labels remain visible in the 300px and compact 260px dock. The viewer, timeline, toolbar, selections, and local analysis retain their existing roles; no tokens or visual direction change.

Users choose a provider, an image-capable model ID, and either a description of the current exact frame/optional rectangular crop or an advisory ranking of 2–8 explicitly chosen saved frames. Preset goals guide the cloud request; cloud output does not overwrite local measurements, rankings, or automatic-preset validation status.

Review preparation happens locally. The panel displays the exact metadata-stripped JPEGs, dimensions, and sizes before transmission. All previews must load and the user must approve those images before the provider-specific Send action becomes available. Changing the request clears approval. Discard, progress, cancellation, actionable errors, and charge/data-use disclosures remain in the dock. Cancellation cannot recall data already sent.

Results retain the transmitted previews and exact-frame actions. They separate AI-observed descriptions from uncertain suggestions and label the response unverified. Open exact frame restores its source and identity; Add frame uses the existing selection collection. Reported model and token usage are shown when supplied. The images contain actual imported media at runtime; Figma placeholders are illustrative.

Editable analysis compositions on page `20:2` of the existing Figma file are setup `36:119`, review `36:173`, running `36:227`, results `36:281`, error `36:335`, and no-key `36:389`. Earlier iterations remain preserved. These compositions map to `CloudAnalysisPanel`, `.cloud-analysis`, `.cloud-region`, and `.cloud-candidates`; Settings maps to `CloudProviders`, `.cloud-providers`, and `.cloud-provider`.

Live provider authentication/generation and populated native review/result workflows still require real API keys and test media. Fixture tests and no-key native captures do not close those acceptance gates or establish cloud accuracy.

## Selection controls and presets — 2026-10-11

Presets move into the Selections panel under “Auto-select with a preset”; the separate Presets tab is removed. The accordion keeps frame collection actions directly accessible. Preset selection precedes subject controls; scoring/custom configuration is collapsed under “Weights and custom presets.” Existing model requirements and experimental opt-in remain factual.

Add/remove current frame reuses the exact-frame toggle. Each frame row has a separate Remove button with its source/ordinal in the accessible name. Select all covers the current fully indexed source; Remove all clears the current project's cross-source collection. Mutating controls show a busy state. Existing manual frames remain when Auto-select frames finishes; adding results belongs to the background job, so closing the panel does not prevent persistence. Cancelled/failed runs do not automatically alter selections.

FrameCards renders at most 100 rows per page with Previous/Next controls and bounded thumbnail generation for later pages. This also keeps Export Studio's frame list bounded. Native keyboard/focus styles and the approved graphite/Inter tokens remain in use. `selection-actions-state.json` records editable empty, ready, running, results, and error compositions. Native visual/interaction acceptance awaits a Mac unlock and Windows access; fixture tests are recorded separately.

## Approved usability refinements — 2026-10-11

`usability-improvements-state.json` records editable library, export, settings, precision, recommendations, and canvas-crop compositions. Prior Fluent iterations remain references. Refinements retain the docked structure, bundled Inter, neutral image surface, and shared Button/focus tokens.

Essential frame metadata stays visible; technical PTS/provenance/indexing fields are in Frame metadata. Fit is explicitly labeled and 1:1 is true source pixels per physical display pixels. Expanded precision shows nine indexed neighbors, with full timestamps available through accessible titles and global scrubbing retained.

Library offers search, recent/name sorting, display-name editing, and actual recorded activity. Export uses independent scrolling plus a persistent action footer; color interpretation is Advanced. Settings uses Local models / Cloud AI / General & shortcuts, with three experimental pack choices and advanced manifest import. Model selection does not bypass license/hash review or accuracy gates.

Recommendations collapse scoring and explain comparison requirements. Cloud AI offers explicit provider model listing and advanced ID entry. RegionEditor maps main-canvas drawing, moving, and resizing to normalized source coordinates; numeric fields are the keyboard alternative. Review and consent invalidate when these inputs change. Cloud requests retain their explicit approval gate.

Native and automated evidence, including lock/display/platform gaps, is in `docs/verification/usability-improvements-2026-10-11.md`. Earlier statements that metadata is always expanded and zoom is solely relative to fit describe the superseded iteration.

Selection controls received a subsequent synthetic native pass at compact and available large sizes; remaining checks are explicitly listed in the usability report rather than the earlier unlock status.

## Model onboarding and workspace tutorial — 2026-10-12

`onboarding-state.json` maps editable Choose, Review, Install, failure/cancellation, retention preference, and three workspace tutorial compositions. `ModelSetup` uses actual pinned catalog manifests and installed-model records. Model checkboxes explain purpose, size, and the pose/person dependency. Review exposes sources, licenses, exact byte sizes, and checksums. An explicit review checkbox gates the existing production download command. Downloads are sequential, progress is recoverable from jobs, cancellation stops dispatching the rest, and retry skips already-ready installations.

The setup page is optional and reopens through Local models or the empty subject-detector control. The original v0.1.2 iteration began a three-step, skippable tutorial when finishing/skipping setup: attach a video, step/select frames, and open Export Studio. It does not use separate simulated media or silently export. General & shortcuts can replay it. Completion and the selection preference persist per application installation in the WebView's local storage, without credentials.

General & shortcuts adds “Clear selections when switching files or projects”, enabled by default. Explicit source changes, imports of a different source, and opening/creating another project clear the outgoing project collection. Reopening the same project/app and selecting an existing frame are exempt. Disabling it restores cross-source collection behavior. Source assets, analysis, and immutable export plans are retained. Running automatic selection must finish or be cancelled before a clearing switch; in-flight selection mutations and stale reloads cannot refill the cleared UI.

The existing Fluent composition, tokens, shared buttons, focus styles, and neutral canvas remain. Native setup/review and tutorial/retention behavior were reviewed at 1000×700 and the available 1440×889 size. See `docs/verification/onboarding-2026-10-12.md` for evidence and limits.


## Truthful model setup and optional tutorial — v0.1.3

Editable revised states preserve the originals in the existing Fluent page: Ready `48:225`, Memory blocked `48:232`, and Partial summary `48:239`. Buttons are editable local adaptations of the existing Fluent button. Inter, graphite surfaces, established spacing/focus, and direct instructions are retained. These are interaction-state compositions, not pixel-identical native captures.

The shared registry reads catalog fingerprints, installations, and durable setup attempts. Person detection model, Body pose model, and Face detection model describe local task models, separately from vision language runtimes. A successful job means runtime validation, atomic activation, and the installation database commit all completed. Downloaded bytes alone never enable a dropdown. Memory-blocked bytes stay available for Retry validation, which makes no network request; Refresh memory status reads the current reserve and budget.

Finish setup returns to the workspace. An unfinished tutorial has separate Start tutorial and Skip tutorial actions. Setup never resets completion. Completed/skipped tutorials stay dismissed; Replay tutorial in General & shortcuts explicitly resets completion and opens step one. Final summaries list each pack's actual state. The 15% reserve and existing managed budget remain enforced.

Native v0.1.3 captures at 1000×700 and available 1440×889 preserve the docked Fluent layout. Repeated setup keeps a skipped tutorial dismissed; explicit replay starts at step one. Actual pinned downloads passed size/hash/graph checks, but runtime validation remains memory-blocked and explicitly pending. See `docs/verification/model-installation-v0.1.3.md`.
