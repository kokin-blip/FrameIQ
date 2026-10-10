"""Opt-in development parity check. Requires opencv-python-headless==4.12.0.88.
Use the face-parity Rust executable first. Models and images remain local.
This checks numerical implementation parity, not accuracy or face identity.
"""
import json
import sys
from pathlib import Path
import cv2
import numpy as np

if len(sys.argv) > 1 and sys.argv[1] == "--variants":
    source, destination = sys.argv[2:]
    image = cv2.imread(source)
    if image is None:
        raise ValueError("Cannot read the source fixture")
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    for name, transformed in {
        "landscape": np.pad(image, ((0,0),(0,image.shape[1]//2),(0,0))),
        "portrait": np.pad(image, ((0,image.shape[0]//2),(0,0),(0,0))),
        "two-faces": np.concatenate((image,image), axis=1),
    }.items():
        if not cv2.imwrite(str(destination / (name + ".png")), transformed):
            raise RuntimeError("Fixture write failed")
    sys.exit(0)

model, directory, report_path = sys.argv[1:]
directory = Path(directory)
image = cv2.imread(str(directory / "prepared.png"))
result = json.loads((directory / "result.json").read_text())
raw = json.loads((directory / "tensors.json").read_text())
net = cv2.dnn.readNetFromONNX(model)
net.setInput(cv2.dnn.blobFromImage(image))
reference = net.forward([o["name"] for o in raw])
max_tensor_error = 0.0
for o, expected in zip(raw, reference):
    actual = np.asarray(o["values"], dtype=np.float32).reshape(o["shape"])
    assert actual.shape == expected.shape
    max_tensor_error = max(max_tensor_error, float(np.max(np.abs(actual - expected))))
    np.testing.assert_allclose(actual, expected, atol=0.002, rtol=0.002)

detector = cv2.FaceDetectorYN.create(model, "", (640, 640), 0.6, 0.3, 5000)
_, detected = detector.detect(image)
g = result["geometry"]
rw, rh = g["resizedWidth"], g["resizedHeight"]
expected_faces = []
for f in ([] if detected is None else detected):
    x, y, w, h = [float(v) for v in f[:4]]
    x1, y1 = min(rw, max(0, x)), min(rh, max(0, y))
    x2, y2 = min(rw, max(0, x + w)), min(rh, max(0, y + h))
    if x2 <= x1 or y2 <= y1:
        continue
    expected_faces.append(np.array([x1 / rw, y1 / rh, (x2-x1) / rw, (y2-y1) / rh,
                                   *[float(f[4+i]) / (rw if i % 2 == 0 else rh) for i in range(10)], float(f[14])]))
actual_faces = []
for f in result["faces"]:
    r = f["rectangle"]
    actual_faces.append(np.array([r["x"], r["y"], r["width"], r["height"],
                                 *[v for p in f["landmarks"] for v in (p["x"], p["y"])], f["detectionScore"]]))
assert len(actual_faces) == len(expected_faces), (len(actual_faces), len(expected_faces))
max_detection_error = 0.0
for a, e in zip(actual_faces, expected_faces):
    max_detection_error = max(max_detection_error, float(np.max(np.abs(a-e))))
    np.testing.assert_allclose(a, e, atol=0.0001, rtol=0.0001)
report = {"opencvVersion": cv2.__version__, "algorithm": result["algorithm"], "modelSha256": result["modelSha256"],
          "faceCount": len(actual_faces), "maxTensorAbsoluteError": max_tensor_error,
          "maxNormalizedDetectionError": max_detection_error, "passed": True,
          "scope": "Reference tensor/box/landmark parity on this fixture; no population accuracy or visibility precision claim."}
Path(report_path).write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
