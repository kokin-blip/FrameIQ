"""Compare Rust pose artifacts to Python ORT using identical input tensors.
Development-only: onnxruntime==1.22.0, numpy==2.2.6, compatible Python.
This isolates runtime/binding parity from OpenCV reference and accuracy gates.
"""
import argparse
import hashlib
import json
from pathlib import Path
import numpy as np
import onnxruntime as ort


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("weights", type=Path)
    parser.add_argument("artifacts", type=Path)
    parser.add_argument("report", type=Path)
    args = parser.parse_args()
    if ort.__version__ != "1.22.0" or np.__version__ != "2.2.6":
        raise ValueError("Use pinned ORT 1.22.0 and NumPy 2.2.6")
    manifest = json.loads(args.manifest.read_text())
    digest = hashlib.sha256(args.weights.read_bytes()).hexdigest()
    if digest != manifest["sha256"]:
        raise ValueError("Weights do not match reviewed manifest")
    options = ort.SessionOptions()
    options.intra_op_num_threads = 1
    options.inter_op_num_threads = 1
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    session = ort.InferenceSession(str(args.weights), sess_options=options, providers=["CPUExecutionProvider"])
    inp = session.get_inputs()[0]
    regions = []
    for directory in sorted(args.artifacts.iterdir()):
        if not directory.is_dir():
            continue
        result = json.loads((directory/"result.json").read_text())
        if result["modelHash"] != digest:
            raise ValueError("Artifact weight hash mismatch")
        tensor = np.asarray(json.loads((directory/"input.json").read_text()), dtype=np.float32).reshape(inp.shape)
        expected = session.run(None, {inp.name: tensor})
        actual = {t["name"]: t for t in json.loads((directory/"tensors.json").read_text())}
        outputs = []
        for definition, values in zip(session.get_outputs(), expected):
            native = actual[definition.name]
            rust = np.asarray(native["values"], np.float32).reshape(native["shape"])
            if rust.shape != values.shape:
                raise ValueError("Output tensor shape differs")
            error = np.abs(rust-values)
            outputs.append(dict(name=definition.name, shape=list(values.shape),
                                maxAbsoluteError=float(error.max()),
                                failedElementCount=int(np.count_nonzero(~np.isclose(rust, values, atol=.003, rtol=.003))),
                                passed=bool(np.allclose(rust, values, atol=.003, rtol=.003))))
        regions.append(dict(region=directory.name, outputs=outputs))
    if not regions:
        raise ValueError("No pose artifacts found")
    report = dict(schemaVersion=1, passed=all(o["passed"] for r in regions for o in r["outputs"]),
                  runtimeVersion=ort.__version__, numpyVersion=np.__version__, provider="CPUExecutionProvider",
                  modelHash=digest, atol=.003, rtol=.003, regions=regions,
                  scope="Same-input Rust/Python ORT runtime parity only. Does not clear OpenCV discrepancies or held-out visibility/accuracy gates.")
    args.report.write_text(json.dumps(report, indent=2)+"\n")
    print(json.dumps(report, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
