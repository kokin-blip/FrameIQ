"""Opt-in packaged pose worker check; no network or model installation.
Usage: verify-pose-worker.py worker manifest model image person-artifacts report
"""
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

worker, manifest, model, image, persons, report = map(Path, sys.argv[1:])
m = json.loads(manifest.read_text())
person = json.loads((persons / "result.json").read_text())["persons"][0]
env = dict(os.environ, FRAMEIQ_CPU_THREADS="1")
env.pop("FRAMEIQ_ORT_LIBRARY", None)
with tempfile.TemporaryDirectory(prefix="frameiq-pose-worker-") as staging:
    paths = [Path(staging) / "first.png", Path(staging) / "second.png"]
    base = dict(kind="estimate_pose", manifest=m, path=str(model.resolve()), image=str(image.resolve()), person=person)
    requests = [dict(base, mask_output=str(p)) for p in paths]
    requests += [dict(base, mask_output=str(paths[0])), dict(kind="unload")]
    result = subprocess.run([str(worker.resolve())], input="\n".join(json.dumps(r) for r in requests) + "\n", text=True, capture_output=True, check=True, timeout=30, env=env)
    responses = [json.loads(line) for line in result.stdout.splitlines()]
    assert len(responses) == 4, responses
    assert responses[0]["kind"] == responses[1]["kind"] == "pose", responses
    first, second = responses[0]["result"], responses[1]["result"]
    assert first["pose"] == second["pose"]
    assert len(first["pose"]["landmarks"]) == 39
    assert first["modelHash"] == m["sha256"]
    for p, response in zip(paths, [first, second]):
        assert response["maskPath"] == str(p)
        assert hashlib.sha256(p.read_bytes()).hexdigest() == response["maskSha256"]
    assert first["maskSha256"] == second["maskSha256"]
    assert responses[2]["kind"] == "error" and responses[2]["error"]["code"] == "OUTPUT_CONFLICT"
    assert responses[3]["kind"] == "unloaded"
    output = dict(passed=True, requests=4, modelHash=m["sha256"], repeatedPoseIdentical=True, maskChecksumsVerified=True, outputCollisionRejected=True, bundledRuntimeResolution=True, width=first["width"], height=first["height"], scope="Packaged worker protocol and atomic assets on a correlated fixture, not held-out accuracy or preset eligibility.")
report.write_text(json.dumps(output, indent=2) + "\n")
print(json.dumps(output, indent=2))
