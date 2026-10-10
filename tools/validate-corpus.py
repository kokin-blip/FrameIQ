"""Validate permissioned JSONL annotations without decoding media or guessing labels."""
import argparse
import hashlib
import json
import re
from collections import Counter
from pathlib import Path


def validate(records, root=None):
    videos, subjects, hashes, frames = {}, {}, {}, set()
    errors = []
    splits = Counter()
    verified = set()
    required = ("videoId", "sha256", "subjectGroup", "license", "consentRef", "split", "ordinal")
    for line, r in enumerate(records, 1):
        def reject(message):
            errors.append(f"Line {line}: {message}")
        if not isinstance(r, dict):
            reject("record must be an object")
            continue
        if any(not isinstance(r.get(k), str) or not r[k].strip() for k in required):
            reject("required provenance and frame fields must be nonempty strings")
            continue
        if r["split"] not in ("calibration", "held_out"):
            reject("split must be calibration or held_out")
        if not re.fullmatch(r"[0-9a-f]{64}", r["sha256"]):
            reject("sha256 must be 64 lowercase hexadecimal characters")
        if not re.fullmatch(r"0|[1-9][0-9]*", r["ordinal"]) or int(r["ordinal"]) > 2**63 - 1:
            reject("ordinal must be a canonical nonnegative SQLite integer string")
        if type(r.get("acceptableRecommendation")) is not bool:
            reject("acceptableRecommendation requires a human boolean label")
        if type(r.get("qualityRelevance")) is not int or not 0 <= r["qualityRelevance"] <= 3:
            reject("qualityRelevance requires a human integer label from 0 to 3")
        if not isinstance(r.get("regions"), list):
            reject("regions must be an annotation list")
        else:
            for region in r["regions"]:
                if not isinstance(region, dict) or not isinstance(region.get("category"), str):
                    reject("region requires a category and normalized rectangle")
                    continue
                box = region.get("box", {})
                if not isinstance(box, dict):
                    reject("region box must be an object")
                    continue
                values = [box.get(k) for k in ("x", "y", "width", "height")]
                if any(type(v) not in (int, float) for v in values):
                    reject("region box requires numeric x/y/width/height")
                    continue
                x, y, w, h = values
                if not (0 <= x <= 1 and 0 <= y <= 1 and 0 < w <= 1 and 0 < h <= 1 and x+w <= 1+1e-9 and y+h <= 1+1e-9):
                    reject("region box must be finite, positive, and within the image")
        if "duplicateGroup" not in r or (r["duplicateGroup"] is not None and not isinstance(r["duplicateGroup"], str)):
            reject("duplicateGroup must be a string or explicit null")
        if r.get("origin") != "permissioned_real":
            reject("accuracy corpus requires origin=permissioned_real; synthetic/parity fixtures are separate")
        key = (r["sha256"], r["ordinal"])
        if key in frames:
            reject("duplicate frame annotation")
        frames.add(key)
        identity = (r["sha256"], r["subjectGroup"], r["split"], r["license"], r["consentRef"])
        if r["videoId"] in videos and videos[r["videoId"]] != identity:
            reject("video identity/provenance changes between annotations")
        videos[r["videoId"]] = identity
        for mapping, key, label in ((subjects, r["subjectGroup"], "subject"), (hashes, r["sha256"], "source content")):
            if key in mapping and mapping[key] != r["split"]:
                reject(f"{label} leaks between calibration and held_out")
            mapping[key] = r["split"]
        splits[r["split"]] += 1
        if root is not None and r["sha256"] not in verified:
            relative = r.get("path")
            if not isinstance(relative, str) or Path(relative).is_absolute():
                reject("file verification requires a relative path")
                continue
            path = (root / relative).resolve()
            if not path.is_relative_to(root.resolve()) or not path.is_file():
                reject("media path is missing or outside the corpus root")
                continue
            digest = hashlib.sha256()
            with path.open("rb") as media:
                for chunk in iter(lambda: media.read(1024*1024), b""):
                    digest.update(chunk)
            if digest.hexdigest() != r["sha256"]:
                reject("media checksum differs from source identity")
            else:
                verified.add(r["sha256"])
    ready = not errors and len(hashes) >= 30 and len(frames) >= 300 and all(splits[s] > 0 for s in ("calibration", "held_out"))
    return {"schemaVersion": 1, "valid": not errors, "annotationMinimumMet": ready,
            "sourceCount": len(hashes), "frameCount": len(frames), "subjectCount": len(subjects),
            "splitFrames": dict(splits), "verifiedFiles": len(verified), "errors": errors,
            "scope": "Annotation integrity and minimum size only; does not establish accuracy, consent validity, category coverage, or release readiness."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("annotations", type=Path)
    parser.add_argument("--media-root", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--require-minimum", action="store_true")
    args = parser.parse_args()
    try:
        records = [json.loads(line) for line in args.annotations.read_text().splitlines() if line.strip()]
        result = validate(records, args.media_root)
    except (OSError, ValueError) as error:
        result = {"valid": False, "annotationMinimumMet": False, "errors": [str(error)]}
    output = json.dumps(result, indent=2)
    if args.report:
        args.report.write_text(output + "\n")
    print(output)
    return 0 if result["valid"] and (not args.require_minimum or result["annotationMinimumMet"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
