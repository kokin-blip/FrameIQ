# Direct optional model downloads

Onboarding and Settings now offer Install pack → review → Download and install for all four qualified fashion packs. File selection is unnecessary in onboarding. Each entry retains its benefit, approximate size and experimental status. Manual package import remains an advanced Settings option.

The immutable built-in manifests in `models/task-packs/` pin exact artifact sizes and SHA-256 digests to the published model-only prerelease:

https://github.com/kokin-blip/FrameIQ/releases/tag/fashion-packs-v1

All 21 release assets were compared with the prepared local files by byte size and GitHub's server-computed SHA-256 digest. The verification inventory is `docs/models/fashion-qualification/download-assets-verification.json`. The release includes converted ONNX models, qualification reports, manifests, upstream license texts and modification/attribution notices. It does not replace the current desktop application release. No source media or user images were uploaded.

Native installation accepts only a built-in catalog ID for direct downloads. Network and artifact hashing run in the supervised worker and reuse the existing HTTPS/redirect allowlist, resumable ETag/range transport and checksum enforcement. The legacy arbitrary-import size/graph restrictions are unchanged. Artifact transfer supports the larger reviewed detection model without reinterpreting it as a legacy single-input import. Every artifact is verified; reference evidence, reviewed graph restrictions, tensor contracts and runtime memory admission still gate activation.

Pack installation uses a cancellable lane and owned staging directories. Previous installations survive failed attempts. Failed or cancelled downloads retain their verified resume bodies; successful activation removes the download cache. Startup recovery clears recognizable abandoned installations while retaining unknown files, symlink targets and resumable downloads. Ready packs are reused on duplicate install requests. The frontend shows byte progress, cancellation, errors, retry and refreshed installed status; setup controls are unavailable while an installation is running.

Verification completed:

- 63 frontend tests passed across 16 files, including opt-in review, catalog-ID download dispatch, absence of an onboarding file chooser, progress, cancellation/retry, status failures, removal and existing model-download regressions. The focused installer tests passed again after final status/copy refinements.
- TypeScript, production frontend build, generated contracts, native application/Tauri checks and native worker builds passed.
- 14 HTTPS downloader tests passed, including real local TLS sockets, artifact resume/cancellation, checksum failures and the existing legacy download/crash matrix. Two explicitly opt-in/helper tests remained ignored as designed.
- Real reviewed hair-pack installation/removal and fashion workflow regressions passed. Additional checks covered catalog/review gating, failed reference evidence despite matching artifact checksums, and recovery preserving partial/unknown files and symlink targets.
- A real public HTTPS hair-pack download through Application::download_task_pack passed installation, duplicate install without another download, reopening, installed capability detection and removal. It ran in a fresh temporary application directory with no seeded download cache. One earlier trial met the computer's memory reserve limit and correctly failed before activation; the live retry passed after the upload and preview resources finished. Memory guards were not changed.
- Actual frontend components were inspected at 1000×700 and 1440×960 using isolated preview responses. Long source/hash content did not overflow; keyboard navigation reached the install action above the sticky footer. The preview visibly transitioned from download progress to Installed. The screenshot `direct-fashion-download-1440.png` shows fixture progress, not the native network test.
- No application-data models, credentials, source media, database migrations, signing or desktop release packaging changed. This work distributes the model assets needed by the requested direct installer; phase 9 remains excluded.
