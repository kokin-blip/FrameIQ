"""Opt-in packaged-worker smoke check. No network or model installation.
Usage: verify-person-worker.py worker manifest model image report
"""
import json
import os
import subprocess
import sys
from pathlib import Path

worker, manifest, model, image, report = map(Path, sys.argv[1:])
m = json.loads(manifest.read_text())
request = dict(kind="detect_persons", manifest=m, path=str(model.resolve()), image=str(image.resolve()))
env = dict(os.environ, FRAMEIQ_CPU_THREADS="1")
env.pop("FRAMEIQ_ORT_LIBRARY", None)  # prove resolution from the packaged runtime
payload = "\n".join(json.dumps(r) for r in [request, request, dict(kind="unload")]) + "\n"
process = subprocess.run([str(worker.resolve())], input=payload, text=True, capture_output=True, check=True, timeout=30, env=env)
responses = [json.loads(line) for line in process.stdout.splitlines()]
assert len(responses) == 3, responses
assert responses[0]["kind"] == responses[1]["kind"] == "persons", responses
assert responses[0] == responses[1]
assert responses[0]["result"]["modelHash"] == m["sha256"]
assert responses[2]["kind"] == "unloaded"
result = dict(passed=True, requests=3, repeatedResultIdentical=True, personCount=len(responses[0]["result"]["persons"]), unloaded=True, modelSha256=m["sha256"], bundledRuntimeResolution=True, scope="Persistent packaged worker protocol; not accuracy, full pose execution, or preset eligibility.")
report.write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
