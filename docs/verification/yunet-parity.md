# YuNet reference-parity procedure

The face adapter now implements aspect-preserving resize/right-bottom padding, the stride-8/16/32 YuNet decoder, score combination, integer-rectangle NMS and five landmarks. Coordinates are normalized to the oriented source image; padding-only boxes are discarded and clipped-area coverage is retained. Coverage and detector scores do not establish occlusion, visibility probability or identity.

Unit tests cover landscape padding, source coordinates, duplicate suppression and output-shape rejection. Application protocol/cache tests use a deterministic test double; they do not validate the ONNX model. The existing supervised real-model lifecycle test is separate.

Numerical reference check (development only):

```sh
FRAMEIQ_ORT_LIBRARY="$PWD/.native/onnxruntime/lib/libonnxruntime.1.22.0.dylib" \
  cargo run -p frameiq-inference --bin face-parity -- \
  models/manifests/yunet-2023mar.json /path/to/reviewed/model.onnx \
  tests/fixtures/images/nasa-astronaut.png /tmp/frameiq-yunet-parity

python tools/verify-yunet.py /path/to/reviewed/model.onnx \
  /tmp/frameiq-yunet-parity /tmp/frameiq-yunet-report.json
```

The Python reference tool uses opencv-python-headless 4.12.0.88. It compares every raw output tensor with OpenCV DNN, then boxes/landmarks/scores with FaceDetectorYN. Tolerances: raw float tensors atol/rtol 0.002; normalized detections atol/rtol 0.0001. It fails on count, shape or numerical mismatch. Passing one image does not certify model accuracy. Square, landscape, portrait and multi-face transformations must also pass, and remain correlated parity tests rather than independent accuracy examples.

ONNX/OpenCV parity passed on 2026-10-09 for square, padded landscape, padded portrait and two-face variants. Maximum raw-tensor errors were below 0.000006 and normalized detection errors below 0.00000012. Reports: yunet-reference-2026-10-09.json, yunet-landscape-2026-10-09.json, yunet-portrait-2026-10-09.json, yunet-two-faces-2026-10-09.json. Initial attempts were blocked by the memory reserve; reruns passed after memory recovered without relaxing policy. Automatic preset eligibility remains disabled pending held-out visibility evaluation and the remaining person/pose capabilities. The required permissioned 30-clip/300-frame corpus remains incomplete.

Reference decoder: https://github.com/opencv/opencv/blob/4.x/modules/objdetect/src/face_detect.cpp . NASA/scikit-image fixture provenance and checksum: tests/fixtures/images/README.md and manifest.json.

Generate reproducible correlated variants with `python tools/verify-yunet.py --variants tests/fixtures/images/nasa-astronaut.png /tmp/frameiq-face-variants`, then run the same Rust/reference pair per image. Memory admission is checked on every inference, including warm cached sessions; low-memory admission unloads the idle session and reports the available/reserve/load-budget numbers.
