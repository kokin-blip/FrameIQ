#!/usr/bin/env python3
"""Prepare checksum-pinned release assets from already qualified local exports.

Development/distribution helper only; the desktop never runs Python.
Download the official upstream license texts to --licenses before running.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path

PACKS = {
    "detection": ("dino-unpadded", "GroundingDINO-T", "IDEA Research", "https://github.com/IDEA-Research/GroundingDINO"),
    "hair": ("hair-references", "MediaPipe Selfie Multiclass hair channel", "Google", "https://developers.google.com/edge/mediapipe/solutions/vision/image_segmenter"),
    "mask": ("sam-base", "SAM ViT-B", "Meta Platforms, Inc. and affiliates", "https://github.com/facebookresearch/segment-anything"),
    "embedding": ("siglip2", "SigLIP 2 Base", "Google", "https://huggingface.co/google/siglip2-base-patch16-224"),
}
def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()
def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pack-root", type=Path, default=Path(".cache/fashion-qualification"))
    parser.add_argument("--output", type=Path, default=Path(".cache/fashion-download-release/assets"))
    parser.add_argument("--licenses", type=Path, default=Path(".cache/fashion-download-release/licenses"))
    parser.add_argument("--catalog", type=Path, default=Path("models/task-packs"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    args.catalog.mkdir(parents=True, exist_ok=True)
    base = "https://github.com/kokin-blip/FrameIQ/releases/download/fashion-packs-v1"
    checksums = []
    for task, (directory, name, owner, upstream) in PACKS.items():
        source = args.pack_root / directory
        manifest = json.loads((source / "manifest.json").read_text())
        report = json.loads((source / "qualification.json").read_text())
        assert manifest["qualification"] == "reference_checked" and report["passed"]
        assert report["nativeCases"] and all(c["passed"] for c in report["nativeCases"])
        assert report["preprocessingSha256"] == hashlib.sha256(manifest["preprocessing"].encode()).hexdigest()
        for artifact in manifest["artifacts"]:
            original = source / artifact["name"]
            assert original.stat().st_size == int(artifact["byteSize"])
            assert digest(original) == artifact["sha256"]
            destination = args.output / (task + "-" + artifact["name"])
            if not destination.exists():
                os.link(original, destination)
            assert digest(destination) == artifact["sha256"]
            artifact["sourceUrl"] = base + "/" + destination.name
        license_path = args.licenses / ("embedding-license.txt" if task == "embedding" else task + ".txt")
        license_text = license_path.read_text()
        assert "Apache License" in license_text
        notice = (f"{name}\nUpstream contributors: {owner}\nUpstream project/model: {upstream}\n"
                  f"Original model: {report.get('upstream', upstream)}\n"
                  f"Pinned revision/checksum: {report.get('upstreamRevision', report.get('upstreamSha256'))}\n\n"
                  "Modified for FrameIQ: exported to a fixed-shape, self-contained ONNX graph; "
                  "task-specific preprocessing and output adaptation. Hair exports retain the hair channel; "
                  "image retrieval exports retain the vision encoder. No retraining is claimed.\n"
                  "Licensed under Apache-2.0; see LICENSE.txt. No upstream endorsement is implied.\n"
                  "qualification.json preserves reference/native numerical checks. "
                  "These are experimental models; reference parity is not broad accuracy evaluation.\n")
        if task == "embedding":
            notice += "\nOfficial model-card license declaration and attribution:\n" + (args.licenses / "embedding.txt").read_text()
        for name, body in [("LICENSE.txt", license_text), ("NOTICE.txt", notice)]:
            destination = args.output / (task + "-" + name)
            destination.write_text(body)
            manifest["artifacts"].append({"name": name, "sha256": digest(destination),
                "byteSize": str(destination.stat().st_size), "sourceUrl": base + "/" + destination.name})
        content = json.dumps(manifest, indent=2) + "\n"
        (args.catalog / (task + ".json")).write_text(content)
        (args.output / (task + "-manifest.json")).write_text(content)
        for artifact in manifest["artifacts"]:
            checksums.append(artifact["sha256"] + "  " + task + "-" + artifact["name"])
        checksums.append(digest(args.output / (task + "-manifest.json")) + "  " + task + "-manifest.json")
        print(task, sum(int(a["byteSize"]) for a in manifest["artifacts"]), "bytes")
    (args.output / "SHA256SUMS.txt").write_text("\n".join(checksums) + "\n")
if __name__ == "__main__":
    main()
