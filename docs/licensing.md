# Distribution register and open gates

FrameIQ source is a private prototype. No public source-code license has been selected; Cargo packages disable publication. Third-party components retain their own licenses.

FFmpeg: 8.0.3, official signed release, signing-key fingerprint FCF986EA15E6E293A5644F10B4322F04D67658D8. Build source and signature remain in ignored .native/src; the reproducible script is tools/build-ffmpeg.sh. Shared libraries are built without GPL/nonfree/external codec options, with network support disabled. The application bundle includes the upstream LGPL notice, source checksum, and build configuration. Corresponding source/build information and codec-patent review are public-distribution gates; private process separation does not waive obligations.

Tauri, Rust bindings, SQLite, React, Zustand, image encoders, fonts, and the complete transitive dependency set require a release license/SBOM review against Cargo.lock and package-lock.json. Inter is locally served through @fontsource/inter; retain its SIL Open Font License in public distribution notices. No model weights, fashion datasets, retailer catalogs, or provider result imagery are distributed.

No API registrations, paid subscriptions, credential storage, image transmissions, or commercial catalog licenses were activated. Provider-specific terms, production access, retention/export rights, and crop consent remain implementation gates.


Milestone 1–4 additions: zimg 3.0.5 (WTFPL v2 notice retained), ONNX Runtime 1.22.0 (MIT notice bundled), ort 2.0.0-rc.13 and reqwest/prost/sysinfo/keyring transitive dependencies are pinned through Cargo.lock and still require the full release SBOM review. YuNet 2023mar manifest pins OpenCV Zoo revision 47534e27c9851bb1128ccc0102f1145e27f23f98, weight SHA-256 and its directory-specific MIT license. Weights are user-reviewed downloads/imports, not bundled assets. Hair/GroundingDINO conversions and distribution rights remain unapproved release gates. MediaPipe-derived person/pose FP32 candidates now have pinned official OpenCV Zoo manifests; both directory READMEs explicitly apply Apache-2.0 to all directory files. They remain experimental pending full pose parity/accuracy and release-register review. Optional localhost tokens are stored in OS credential storage; no paid service or project-image upload was enabled.


The YuNet postprocessor follows the OpenCV FaceDetectorYN stride/score/landmark contract (Apache-2.0 source); model weights retain their separate MIT manifest license. Reference: https://github.com/opencv/opencv/blob/4.x/modules/objdetect/src/face_detect.cpp . The NASA/scikit-image parity fixture has public-domain provenance and a SHA-256 manifest under tests/fixtures/images; it is not used for product endorsement or face identification. Development-only opencv-python-headless 4.12.0.88 and NumPy are not shipped in the desktop bundle.

The person decoder and pose ROI geometry follow the pinned OpenCV Zoo Apache-2.0 implementations; attribution is included in the bundled FaceDecoder-NOTICE.txt with the OpenCV license. The pinned person detector reference passes xyxy fields to Rect2d NMS; this convention is preserved for parity and must be reevaluated for multi-person quality rather than silently corrected. Detector boxes must not be used as complete body coverage. No person/pose weights are bundled.
