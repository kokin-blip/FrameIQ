"""Packaged offline-extractor smoke. No development library paths or model accuracy claims."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

bundle, inputs, destination = (Path(v).resolve() for v in sys.argv[1:4])
destination.mkdir(parents=True, exist_ok=True)
env = {k: v for k, v in os.environ.items() if not k.startswith(("DYLD_", "LD_LIBRARY", "FRAMEIQ_", "PKG_CONFIG", "VCPKG", "FFMPEG_"))}
env["PATH"] = "/usr/bin:/bin"
worker = bundle / "Contents/MacOS/frameiq-worker"


def request(payload):
    process = subprocess.run([str(worker)], input=json.dumps(payload) + "\n", text=True, capture_output=True, env=env, timeout=60)
    if process.returncode:
        raise RuntimeError(f"Packaged worker exited {process.returncode}: {process.stderr}")
    rows = [json.loads(line) for line in process.stdout.splitlines() if line]
    errors = [r for r in rows if r["kind"] == "error"]
    if errors:
        raise RuntimeError(errors)
    return rows


cases = []
for name in ("sdr", "pq", "hlg"):
    source = inputs / f"{name}.mkv"
    rows = request({"kind": "index", "path": str(source), "stream": None})
    frames = [f for r in rows if r["kind"] == "frames" for f in r["frames"]]
    assert len(frames) == 4 and any(r["kind"] == "complete" for r in rows)
    frame = frames[2]
    for mode in (("sdr",) if name == "sdr" else ("sdr", "preserve16_bit")):
        output = destination / f"{name}-{mode}.png"
        rows = request({"kind": "decode", "path": str(source), "stream": 0, "ordinal": int(frame["ordinal"]), "expected_pts": int(frame["ptsTicks"]) if frame["ptsTicks"] is not None else None, "anchor": None, "output": str(output), "max_width": None, "options": {"format": "png", "jpegQuality": 95, "colorMode": mode, "colorPolicy": None}})
        image = next(r for r in rows if r["kind"] == "image")
        png = output.read_bytes()
        digest = hashlib.sha256(png).hexdigest()
        assert digest == image["sha256"] and image["width"] == 160 and image["height"] == 120
        depth = 8 if mode == "sdr" else 16
        assert png[24] == depth
        cases.append({"fixture": name, "mode": mode, "ordinal": frame["ordinal"], "dimensions": [160, 120], "pngBitDepth": depth, "sha256": digest, "conversion": image.get("conversion")})
report = {"version": "0.1.3", "platform": "aarch64-apple-darwin", "developmentLibraryPathsRemoved": True, "cases": cases, "inference": "Pending native runtime validation: system reserve remains enforced."}
(destination / "packaged-worker-smoke.json").write_text(json.dumps(report, indent=2) + "\n")
print(f"Passed {len(cases)} packaged-worker exports")
