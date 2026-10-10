"""Verify ROI geometry against pinned OpenCV preprocessing, without claiming pose inference.
Usage: verify-pose-roi.py reference.py pose.onnx image.png person-artifacts report.json
"""
import hashlib
import importlib.util
import json
import sys
from pathlib import Path
import cv2
import numpy as np

reference, model, source, directory, report = map(Path, sys.argv[1:])
assert hashlib.sha256(reference.read_bytes()).hexdigest() == "7e1953d1fb848c64fd3b44277e615e273e7fb2f325f7633eb9c1e32948d45e67"
assert cv2.__version__ == "4.12.0"
module = importlib.util.spec_from_file_location("pinned_pose", reference)
ref = importlib.util.module_from_spec(module)
module.loader.exec_module(ref)
pose = ref.MPPose(str(model))
image = cv2.imread(str(source))
results = json.loads((directory / "result.json").read_text())
maximum = 0.0
tested = 0
for person, roi in zip(results["persons"], results["poseRois"]):
    if roi is None:
        continue
    p = np.asarray(person["detectorBox"] + np.asarray(person["landmarks"]).flatten().tolist() + [person["score"]])
    blob, box, angle, matrix, bias = pose._preprocess(image, p.copy())
    g = roi["geometry"]
    np.testing.assert_array_equal(g["origin"], bias)
    np.testing.assert_array_equal(g["size"], box[1] - box[0])
    np.testing.assert_allclose(g["angle_radians"], np.deg2rad(angle), atol=1e-12)
    inverse = cv2.invertAffineTransform(matrix)
    points = np.asarray([[0, 0], [128, 128], [256, 256]], np.float64) * np.asarray(g["size"]) / 256
    expected = np.c_[points, np.ones(3)] @ inverse.T + bias
    actual = np.asarray(roi["sourcePoints"])
    np.testing.assert_allclose(actual, expected, atol=5e-6, rtol=0)
    maximum = max(maximum, float(np.max(np.abs(actual - expected))))
    tested += 1
assert tested > 0
result = dict(opencvVersion=cv2.__version__, testedRois=tested, maxSourcePixelError=maximum, passed=True, scope="ROI geometry only; pose raster interpolation, landmark tensors, mask mapping and accuracy remain unvalidated.")
report.write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
