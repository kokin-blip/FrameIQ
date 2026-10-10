"""Opt-in numerical pose parity. No network, training or accuracy evaluation.
Usage: verify-pose.py pinned-reference.py pose.onnx image pose-artifacts report
Development dependencies: opencv-python-headless==4.12.0.88, numpy==2.2.6.
"""
import hashlib
import importlib.util
import json
import sys
from pathlib import Path
import cv2
import numpy as np

reference, weights, source, directory, report = map(Path, sys.argv[1:])
assert cv2.__version__ == "4.12.0"
assert hashlib.sha256(reference.read_bytes()).hexdigest() == "7e1953d1fb848c64fd3b44277e615e273e7fb2f325f7633eb9c1e32948d45e67"
spec = importlib.util.spec_from_file_location("pinned_pose", reference)
ref = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ref)
model = ref.MPPose(str(weights))
image = cv2.imread(str(source))
assert image is not None
reports = []
for d in sorted(directory.iterdir()):
    if not d.is_dir():
        continue
    r = json.loads((d / "result.json").read_text())
    p = r["person"]
    person = np.asarray(p["detectorBox"] + np.asarray(p["landmarks"]).flatten().tolist() + [p["score"]])
    blob, box, angle, matrix, bias = model._preprocess(image, person.copy())
    actual = np.asarray(json.loads((d / "input.json").read_text()), np.float32).reshape(blob.shape)
    # RGB8 resampling can differ by one quantization level across area implementations.
    np.testing.assert_allclose(actual, blob, atol=1 / 255 + 1e-6, rtol=0)
    model.model.setInput(actual)
    names = model.model.getUnconnectedOutLayersNames()
    outputs = model.model.forward(names)
    tensors = json.loads((d / "tensors.json").read_text())
    tensor_error = 0.0
    runtime_tensor_parity = True
    actual_outputs = []
    for name, expected in zip(names, outputs):
        t = next(t for t in tensors if t["name"] == name)
        values = np.asarray(t["values"], np.float32).reshape(t["shape"])
        runtime_tensor_parity &= bool(np.allclose(values, expected, atol=0.003, rtol=0.003))
        actual_outputs.append(values)
        tensor_error = max(tensor_error, float(np.max(np.abs(values - expected))))
    # Validate the decoder independently using the identical native output tensors.
    expected = model._postprocess([t.copy() for t in actual_outputs], box, angle, matrix, bias, np.asarray([image.shape[1], image.shape[0]]))
    pose = r["pose"]
    assert (pose is None) == (expected is None)
    entry = dict(region=d.name, maxInputAbsoluteError=float(np.max(np.abs(actual - blob))), maxTensorAbsoluteError=tensor_error, runtimeTensorParity=runtime_tensor_parity, abstained=pose is None)
    if pose is not None:
        landmarks = np.asarray([[p["x"], p["y"], p["z"], p["visibility"], p["presence"]] for p in pose["landmarks"]])
        np.testing.assert_allclose(landmarks[:, :3], expected[1][:, :3], atol=0.01, rtol=0)
        np.testing.assert_allclose(landmarks[:, 3:], expected[1][:, 3:], atol=1e-5, rtol=0)
        np.testing.assert_allclose(pose["worldLandmarks"], expected[2], atol=0.001, rtol=0)
        np.testing.assert_allclose(np.asarray(pose["boxXyxy"]).reshape(2, 2), expected[0], atol=0.01, rtol=0)
        mask = cv2.imread(str(d / "mask.png"), 0) > 0
        assert mask.shape == image.shape[:2]
        target = expected[3] > 0
        union = np.logical_or(mask, target).sum()
        iou = float(np.logical_and(mask, target).sum() / union) if union else 1.0
        assert iou >= 0.999, iou
        entry.update(maxLandmarkAbsoluteError=float(np.max(np.abs(landmarks - expected[1]))), maskIoU=iou)
        # Full reference preprocessing + inference may differ slightly after RGB8 rounding.
        model.model.setInput(blob)
        full = model._postprocess([t.copy() for t in model.model.forward(names)], box, angle, matrix, bias, np.asarray([image.shape[1], image.shape[0]]))
        assert full is not None
        np.testing.assert_allclose(landmarks[:, :3], full[1][:, :3], atol=1.0, rtol=0)
        target = full[3] > 0
        union = np.logical_or(mask, target).sum()
        full_iou = float(np.logical_and(mask, target).sum() / union) if union else 1.0
        assert full_iou >= 0.995
        entry.update(fullPipelineMaxLandmarkError=float(np.max(np.abs(landmarks[:, :3] - full[1][:, :3]))), fullPipelineMaskIoU=full_iou)
    reports.append(entry)
assert reports
result = dict(passed=all(r["runtimeTensorParity"] for r in reports), decoderParityPassed=True, opencvVersion=cv2.__version__, algorithm=r["algorithm"], modelHash=r["modelHash"], regions=reports, scope="Separate decoder and runtime tensor checks on correlated fixtures; not held-out pose/visibility accuracy or garment segmentation.")
report.write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
if not result["passed"]:
    sys.exit(1)
