"""Prepare exact indexed frames for human labeling; never generate ground truth."""
import argparse
import hashlib
import html
import json
import subprocess
from pathlib import Path


def request(worker, payload):
    result = subprocess.run([str(worker)], input=json.dumps(payload) + "\n",
                            capture_output=True, text=True, timeout=600)
    rows = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
    errors = [r for r in rows if r["kind"] == "error"]
    if result.returncode or errors:
        raise RuntimeError(f"Worker failed: {errors} {result.stderr}")
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("media_root", type=Path)
    parser.add_argument("worker", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--count", type=int, default=10)
    args = parser.parse_args()
    if not 2 <= args.count <= 500:
        parser.error("Choose 2–500 frames per source")
    if args.output.exists() and any(args.output.iterdir()):
        parser.error("Use an empty output directory; existing annotations are preserved")
    args.output.mkdir(parents=True, exist_ok=True)
    sources = json.loads(args.manifest.read_text())["sources"]
    drafts, cards = [], []
    for source in sources:
        path = args.media_root / (source["id"] + ".webm")
        sha = hashlib.sha256()
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1048576), b""):
                sha.update(block)
        if sha.hexdigest() != source["sha256"]:
            raise ValueError(f"Changed source: {source['id']}")
        rows = request(args.worker.resolve(), {"kind": "index", "path": str(path.resolve()), "stream": None})
        if not any(r["kind"] == "complete" for r in rows):
            raise ValueError("Incomplete reference index")
        metadata = next(r["metadata"] for r in rows if r["kind"] == "metadata")
        frames = [f for r in rows if r["kind"] == "frames" for f in r["frames"]]
        indexes = sorted({(len(frames)-1)*i//(args.count-1) for i in range(args.count)})
        for index in indexes:
            frame = frames[index]
            filename = f"{source['id']}-{frame['ordinal']}.png"
            image = args.output / filename
            request(args.worker.resolve(), {"kind": "decode", "path": str(path.resolve()),
                    "stream": metadata["selectedStream"], "ordinal": int(frame["ordinal"]),
                    "expected_pts": int(frame["ptsTicks"]) if frame["ptsTicks"] is not None else None,
                    "anchor": None, "output": str(image.resolve()), "max_width": None,
                    "options": {"format": "png", "jpegQuality": 95, "colorMode": "sdr", "colorPolicy": None}})
            drafts.append({"origin": "permissioned_real", "videoId": source["id"],
                           "sha256": source["sha256"], "path": path.name,
                           "subjectGroup": source["sourceGroup"], "split": source["split"],
                           "license": source["license"], "consentRef": source["page"],
                           "ordinal": frame["ordinal"], "ptsTicks": frame["ptsTicks"],
                           "timestampOrigin": frame["timestampOrigin"], "image": filename,
                           "qualityRelevance": None, "acceptableRecommendation": None,
                           "regions": [], "duplicateGroup": None,
                           "annotationStatus": "unlabeled", "annotator": None,
                           "secondReviewer": None})
            cards.append(f'<figure><img src="{html.escape(filename)}"><figcaption>{html.escape(source["id"])} · frame {frame["ordinal"]}</figcaption></figure>')
    (args.output / "drafts.jsonl").write_text("".join(json.dumps(r) + "\n" for r in drafts))
    (args.output / "review.html").write_text('<!doctype html><meta charset="utf-8"><title>FrameIQ annotation inputs</title><style>body{font-family:system-ui}main{display:grid;grid-template-columns:repeat(2,1fr)}img{max-width:100%}</style><h1>Unlabeled evaluation frames</h1><p>Enter human labels in drafts.jsonl. Review subject groups and splits before calibration. These frames are uniformly sampled inputs, not recommended or annotated results.</p><main>' + "".join(cards) + '</main>')
    print(f"Prepared {len(drafts)} exact frames. Human labels are still required.")


if __name__ == "__main__":
    main()
