"""Development-only parity against the pinned OpenCV Zoo person reference.
Usage: verify-person.py reference.py model.onnx source.png artifact-dir report.json
Dependencies: opencv-python-headless==4.12.0.88, numpy==2.2.6.
"""
import hashlib
import importlib.util
import json
import sys
from pathlib import Path

import cv2
import numpy as np

reference, model, source, directory, report = map(Path, sys.argv[1:])
assert hashlib.sha256(reference.read_bytes()).hexdigest() == "e530a8ebc3c218376d5dd1c13aa8ed39a850fac4dbcd6928144746b33477b9c7"
assert cv2.__version__ == "4.12.0"
module = importlib.util.spec_from_file_location("pinned_person", reference)
ref = importlib.util.module_from_spec(module)
module.loader.exec_module(ref)
detector = ref.MPPersonDet(str(model))
image = cv2.imread(str(source))
assert image is not None
blob, bias = detector._preprocess(image)
actual_input = np.asarray(json.loads((directory / "input.json").read_text()), np.float32).reshape(blob.shape)
np.testing.assert_allclose(actual_input, blob, atol=1e-6, rtol=1e-6)
detector.model.setInput(actual_input)
names = detector.model.getUnconnectedOutLayersNames()
outputs = detector.model.forward(names)
actual_tensors = json.loads((directory / "tensors.json").read_text())
tensor_error = 0.0
for name, expected in zip(names, outputs):
    actual = next(t for t in actual_tensors if t["name"] == name)
    actual = np.asarray(actual["values"], np.float32).reshape(actual["shape"])
    np.testing.assert_allclose(actual, expected, atol=0.003, rtol=0.003)
    tensor_error = max(tensor_error, float(np.max(np.abs(actual - expected))))
# Source-coordinate comparison includes the separately checked resize/normalization.
detector.model.setInput(blob)
reference_outputs = detector.model.forward(names)
expected = detector._postprocess(reference_outputs, np.array([image.shape[1], image.shape[0]]), bias)
results = json.loads((directory / "result.json").read_text())
actual = np.asarray([p["detectorBox"] + np.asarray(p["landmarks"]).flatten().tolist() + [p["score"]] for p in results["persons"]])
assert len(actual) == len(expected)
error = 0.0
if len(actual):
    np.testing.assert_allclose(actual[:, :12], expected[:, :12], atol=0.001, rtol=0)
    np.testing.assert_allclose(actual[:, 12], expected[:, 12], atol=1e-5, rtol=0)
    error = float(np.max(np.abs(actual - expected)))
result = dict(opencvVersion=cv2.__version__, algorithm=results["algorithm"], modelSha256=results["modelSha256"], personCount=len(actual), maxInputAbsoluteError=float(np.max(np.abs(actual_input - blob))), maxTensorAbsoluteError=tensor_error, maxDetectionAbsoluteError=error, passed=True, scope="Numerical parity only; detector boxes are not full-body boxes and scores are not calibrated visibility probabilities.")
report.write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
