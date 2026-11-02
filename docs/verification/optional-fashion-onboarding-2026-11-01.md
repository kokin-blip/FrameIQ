# Optional fashion packs in onboarding

The initial local-import implementation recorded below is superseded by [direct model downloads](direct-fashion-downloads-2026-11-01.md).

The initial setup now offers Item detection (GroundingDINO-T, about 695 MB), Hair selection (MediaPipe hair, about 16.5 MB), Item cutouts (SAM ViT-B, about 375 MB) and Visual matching (SigLIP 2 Base, about 372 MB). Each entry explains its purpose, including which presets benefit. The packs are experimental and individually opt-in. ChatGPT item suggestions, attributes and research remain usable without these packs.

The onboarding and Settings installers share the same component. Choosing a local package opens an explicit license/source/reference review; installation uses the existing native import command and validation. The review shows actual artifact sizes, explains the additional installed copy, and resets consent when a different package is chosen. In-progress installation blocks setup navigation and supports cancellation. Failures and cancelled installs retain a retry path; successful installations refresh capability state. Existing packs are recognized on reopening and can be removed.

The four converted packages currently exist as local reviewed exports. Published, pinned downloads are not available; the interface states this and does not promise a download or substitute incompatible upstream weights. Existing pinned person/pose/face downloads retain their original queue and review behavior. “Continue without installing” remains available when idle.

Verification:

- All 63 frontend tests in 16 files passed, including eight new checks covering onboarding skip, opt-in review/import, package mismatch and consent reset, cancelled chooser, running installation/cancellation/retry, failed status/install recovery, installed-state recognition and removal. Existing download dependency, retry and cancellation checks still pass.
- TypeScript, generated contract checks and the production frontend build passed.
- Browser preview of the actual components at 1440×960 and 1000×700: no horizontal overflow; keyboard Tab navigation reached the pack buttons above the persistent action area. The skip action was exercised with the keyboard. Preview model/hardware responses are isolated fixtures, not a native installation.
- Mechanical design scan found only existing stylesheet advisories outside the changed rules. No native commands, model validation, model weights, database migrations or credentials changed. Native file dialogs and a fresh native model import were not repeated in this UI-only pass; the existing qualified model import checks remain documented separately.
