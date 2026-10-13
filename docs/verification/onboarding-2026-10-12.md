# Model setup, tutorial, and selection retention

## Implemented

- First-run, skippable Choose → Review → Install flow for the three current pinned model packs. Existing ready installations are preserved and not redownloaded. Pose selection depends on a selected or installed person detector.
- Explicit review of source/size/license/checksum before download. Uses existing downloader validation, job progress, memory admission, and cancellation; no frontend transport/security bypasses.
- Sequential dispatch, cancellation of the active job and remaining queue, failed-pack errors, and retry of unfinished models. Setup is replayable from Settings or the missing person-detector control.
- Three-step tutorial operates on real workspace actions, with Skip and replay from General & shortcuts. It introduces import, exact stepping/selection, and Export Studio without automatically writing exports.
- Default-on selection clearing for explicit source/project changes and new-source imports. The outgoing project's persisted collection is cleared through the existing scoped command. Turning the setting off restores cross-source retention.
- Startup, same-project reopening, and selected-frame navigation are exempt. Cancelled dialogs and failed clears preserve the current source/collection. Active automatic selection blocks a clearing switch until cancelled/completed. Stale reload responses are invalidated at clearing/switch boundaries.
- Preferences and completion are persisted in scoped WebView local storage. No migration, Rust command, FrameRefV1, or stored export-plan change.

## Verification

- 43 frontend tests in 12 files pass. New tests cover dependency/review gates, queued cancellation, retry skipping successful packs, tutorial actions and persisted completion/settings, first-run setup, default-enabled switching, opt-out parity, same-project reopening, cancelled import, database failure, and active-preset guard.
- TypeScript, generated contracts, frontend production build, native macOS build, and strict ad-hoc signature verification pass. Native worker/Rust behavior is unchanged; this iteration did not repeat the full Rust suite.
- Isolated `FrameIQ Setup Preview.app` uses a separate identifier, preserving the main user's project and app state. Native first launch showed the actual three models with sizes and hardware information.
- Native review showed three downloads totaling 17.8 MB and a disabled Download action before review. Skipping entered the tutorial; imported a synthetic FFV1 clip, stepped 0→1, selected ordinal 1, and opened the real Export Studio.
- Native import of a second synthetic source cleared 1 selection to 0 with the default enabled. Disabling the setting preserved a selection while changing sources. Re-enabling and restarting retained the existing selection on startup, did not repeat setup/tutorial, and cleared it on an explicit source change.
- Native screens checked at 1000×700 and available 1440×889; no styling defect requiring a second implementation pass was found. The model-review content scrolls with accessible persistent actions.
- Figma interaction compositions use existing local Fluent adaptations; the model-choice composition was reviewed. They are not claimed pixel-identical native screenshots.

## Scope and open gates

This native pass deliberately did not accept model licenses or perform model downloads. The new queue's cancellation/retry behavior is fixture-tested; production HTTPS/runtime validation remains provided by the existing downloader/inference subsystem. Windows native interaction, real-model accuracy/resource evaluation, and unavailable hair/accessory/footwear capabilities remain open. No API keys, cloud uploads, paid requests, or GitHub release changes were made.

Logs: `/private/tmp/frameiq-onboarding-tests.log`, `frameiq-onboarding-build.log`, `frameiq-onboarding-contracts.log`, `frameiq-onboarding-native.log`.
