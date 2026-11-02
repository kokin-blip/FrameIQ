# Reviewed local task packages

The JSON files here are metadata and numerical evidence. Model weights are deliberately excluded from Git. The complete qualified packages in this checkout are:

| Task | Local import manifest | State |
|---|---|---|
| Fashion detection | `.cache/fashion-qualification/dino-unpadded/manifest.json` | GroundingDINO-T, experimental, reference checked |
| Hair masks | `.cache/fashion-qualification/hair-references/manifest.json` | MediaPipe multiclass hair channel, experimental, reference checked |
| Prompted masks | `.cache/fashion-qualification/sam-base/manifest.json` | Original SAM ViT-B, experimental, reference checked |
| Catalog embeddings | `.cache/fashion-qualification/siglip2/manifest.json` | SigLIP 2 Base, experimental, reference checked |

FrameIQ offers all four packs during onboarding under **Optional fashion packs**, with their purpose and approximate installed size. Each pack is opt-in: choose **Install pack**, review its size, license, sources and reference checks, then choose **Download and install**. FrameIQ downloads and verifies every artifact, checks the task graph and runtime compatibility in its supervised worker, and activates the pack. No package-file selection or Python installation is needed. The same installer is available later in **Settings → Local models → Optional fashion packs**. Users can continue onboarding without installing anything.

The built-in catalog in `models/task-packs/` pins the converted artifacts, qualification reports and license/notice files to the [fashion-packs-v1 model release](https://github.com/kokin-blip/FrameIQ/releases/tag/fashion-packs-v1), including exact byte sizes and SHA-256 digests. Upstream checkpoint URLs are provenance; they are not substituted for the reviewed exports. The release is a model-asset prerelease and does not replace the latest desktop app release. `tools/prepare-fashion-downloads.py` stages assets from the already qualified exports; weights remain outside Git.

Downloads share the existing HTTPS allowlist, redirect, ETag/range resume, cancellation and checksum checks. Failed or cancelled downloads remain resumable. A pack stays unavailable until reference and native runtime validation complete. Successful activation removes its download cache; temporary storage is needed during installation. Ready installations are preserved on retries and reopening, and can be removed later. Advanced local manifest import remains available in Settings. All four installed packs use about 1.46 GB, including their notices.

`tools/export-fashion-pack.py`, `tools/export-hair-pack.py` and `tools/qualify-fashion-pack.py` are development-only reproduction tools. Their pinned requirements are in `tools/fashion-export-requirements.txt` and `tools/hair-export-requirements.txt`. Export/reference and native stages run separately to respect memory admission. Native reports bind both model and preprocessing hashes.

The original HairSegmenter custom operators and SAM 2 comparisons failed qualification; they are not enabled by substituting a successful status. Qualified alternatives are listed above. Numerical parity and small representative examples do not establish broad accuracy or exact product identity.
