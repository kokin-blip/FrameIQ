//! Protocol/cache integration with a deterministic AI test double, not a model accuracy test.
#![cfg(unix)]
use frameiq_application::Application;
use frameiq_contracts::*;
use sha2::{Digest, Sha256};
use std::{
    os::unix::fs::PermissionsExt,
    path::Path,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
fn wait(app: &Application, id: &str) -> JobEvent {
    let start = Instant::now();
    loop {
        let j = app.get_job(id).unwrap();
        if matches!(
            j.status,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return j;
        }
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn face_jobs_restore_exact_frames_reuse_cache_and_clear_dependencies() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let d = tempfile::tempdir().unwrap();
    let worker = d.path().join("frameiq-worker");
    std::fs::copy(root.join("target/debug/frameiq-worker"), &worker).unwrap();
    let ai = d.path().join("frameiq-ai-worker");
    // Shell response is deterministic. The imported bytes deliberately are not a real model.
    std::fs::write(&ai, r##"#!/bin/sh
while read request; do
case "$request" in
*'"kind":"validate"'*) echo '{"kind":"validated"}' ;;
*'"kind":"detect_faces"'*) echo call >> "$0.calls"; echo '{"kind":"faces","result":{"width":160,"height":120,"algorithm":"yunet-2023-letterbox-v1","modelHash":"PLACEHOLDER","faces":[]}}' ;;
*) exit 1 ;;
esac
done
"##.replace("PLACEHOLDER", &format!("{:x}",Sha256::digest(b"test-double")))).unwrap();
    std::fs::set_permissions(&ai, std::fs::Permissions::from_mode(0o755)).unwrap();
    let video = d.path().join("source.mkv");
    assert!(Command::new(root.join(".native/ffmpeg/bin/ffmpeg"))
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x120:rate=10:duration=1",
            "-c:v",
            "ffv1",
            video.to_str().unwrap()
        ])
        .status()
        .unwrap()
        .success());
    let app = Application::new(&d.path().join("data"), worker, Arc::new(|_| {})).unwrap();
    let project = app.create_project("Detection cache").unwrap();
    let imported = app.import_video(&project.id, &video, None).unwrap();
    let imported_result = wait(&app, &imported.job_id);
    assert!(
        matches!(imported_result.status, JobStatus::Succeeded),
        "Import failed: {:?}",
        imported_result.error
    );
    let source = app.sources(&project.id).unwrap()[0].clone();
    app.select(&source.id, "3", true).unwrap();
    let bytes = d.path().join("test-double.onnx");
    std::fs::write(&bytes, b"test-double").unwrap();
    let mut manifest: ModelManifestV1 =
        serde_json::from_str(include_str!("../../../models/manifests/yunet-2023mar.json")).unwrap();
    manifest.byte_size = "11".into();
    manifest.sha256 = format!("{:x}", Sha256::digest(b"test-double"));
    let model = app.import_model(&bytes, manifest).unwrap();
    let done = wait(&app, &model.job_id);
    assert!(
        matches!(done.status, JobStatus::Succeeded),
        "{:?}",
        done.error
    );
    let model_id = &done.result_ids[0];
    let detection = app.detect_faces(&source.id, "3", model_id).unwrap();
    let done = wait(&app, &detection.job_id);
    assert!(
        matches!(done.status, JobStatus::Succeeded),
        "{:?}",
        done.error
    );
    let result = app
        .face_feature(&source.id, "3", model_id)
        .unwrap()
        .unwrap();
    assert_eq!(result.frame.frame_ref.presentation_ordinal, "3");
    assert_eq!(
        result.frame.frame_ref.index_generation,
        source.index_generation
    );
    assert!(app
        .face_feature(&source.id, "4", model_id)
        .unwrap()
        .is_none());
    let calls = ai.with_file_name("frameiq-ai-worker.calls");
    assert_eq!(std::fs::read_to_string(&calls).unwrap().lines().count(), 1);
    // The next request must be satisfied by cache, without another inference call.
    std::fs::remove_file(&ai).unwrap();
    let again = app.detect_faces(&source.id, "3", model_id).unwrap();
    assert_eq!(wait(&app, &again.job_id).result_ids, vec![result.id]);
    assert_eq!(std::fs::read_to_string(&calls).unwrap().lines().count(), 1);
    assert_eq!(app.selections(&project.id).unwrap().len(), 1);
    app.remove_model(model_id).unwrap();
    assert_eq!(
        app.face_feature(&source.id, "3", model_id)
            .unwrap_err()
            .code,
        "MODEL_MISSING"
    );
    assert!(bytes.exists());
}

#[test]
fn body_jobs_cache_each_subject_verify_masks_and_remove_dependencies() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let d = tempfile::tempdir().unwrap();
    let worker = d.path().join("frameiq-worker");
    std::fs::copy(root.join("target/debug/frameiq-worker"), &worker).unwrap();
    let ai = d.path().join("frameiq-ai-worker");
    // Development-only protocol double: PNG encoding uses Python's standard library.
    // This verifies orchestration and integrity, not pose quality or numerical parity.
    std::fs::write(&ai, r#"#!/usr/bin/env python3
import sys,json,hashlib,zlib,struct,time
from pathlib import Path
for line in sys.stdin:
 r=json.loads(line); k=r['kind']
 if k=='validate': out={'kind':'validated'}
 elif k=='detect_persons':
  with open(sys.argv[0]+'.calls','a') as f: f.write('person\n')
  person={'detectorBox':[40,10,100,100],'landmarks':[[80,60],[80,10],[80,40],[80,20]],'score':.9}
  out={'kind':'persons','result':{'width':160,'height':120,'algorithm':'mediapipe-person-224-v1','modelHash':r['manifest']['sha256'],'persons':[person,person]}}
 elif k=='estimate_pose':
  with open(sys.argv[0]+'.calls','a') as f: f.write('pose\n')
  def chunk(tag,data): return struct.pack('>I',len(data))+tag+data+struct.pack('>I',zlib.crc32(tag+data)&0xffffffff)
  png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',160,120,8,0,0,0,0))+chunk(b'IDAT',zlib.compress((b'\0'+b'\xff'*160)*120))+chunk(b'IEND',b'')
  Path(r['mask_output']).write_bytes(png)
  if Path(sys.argv[0]+'.slow').exists(): time.sleep(3)
  out={'kind':'pose','result':{'width':160,'height':120,'algorithm':'mediapipe-pose-roi-v1','modelHash':r['manifest']['sha256'],'pose':{'score':.9,'boxXyxy':[0,0,160,120],'landmarks':[],'worldLandmarks':[]},'maskPath':r['mask_output'],'maskSha256':hashlib.sha256(png).hexdigest()}}
 else: sys.exit(1)
 print(json.dumps(out),flush=True)
"#).unwrap();
    std::fs::set_permissions(&ai, std::fs::Permissions::from_mode(0o755)).unwrap();
    let video = d.path().join("source.mkv");
    assert!(Command::new(root.join(".native/ffmpeg/bin/ffmpeg"))
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x120:rate=10:duration=1",
            "-c:v",
            "ffv1",
            video.to_str().unwrap()
        ])
        .status()
        .unwrap()
        .success());
    let app = Application::new(&d.path().join("data"), worker.clone(), Arc::new(|_| {})).unwrap();
    let project = app.create_project("Body cache").unwrap();
    let import = app.import_video(&project.id, &video, None).unwrap();
    assert!(matches!(
        wait(&app, &import.job_id).status,
        JobStatus::Succeeded
    ));
    let source = app.sources(&project.id).unwrap()[0].clone();
    app.select(&source.id, "3", true).unwrap();
    let bytes = d.path().join("double.onnx");
    std::fs::write(&bytes, b"test-double").unwrap();
    let install = |json: &str| {
        let mut m: ModelManifestV1 = serde_json::from_str(json).unwrap();
        m.byte_size = "11".into();
        m.sha256 = format!("{:x}", Sha256::digest(b"test-double"));
        let job = app.import_model(&bytes, m).unwrap();
        let done = wait(&app, &job.job_id);
        assert!(
            matches!(done.status, JobStatus::Succeeded),
            "{:?}",
            done.error
        );
        done.result_ids[0].clone()
    };
    let detector = install(include_str!(
        "../../../models/manifests/person_detection_mediapipe-2023mar.json"
    ));
    let pose = install(include_str!(
        "../../../models/manifests/pose_estimation_mediapipe-2023mar.json"
    ));
    let a = app.detect_persons(&source.id, "3", &detector).unwrap();
    let b = app.detect_persons(&source.id, "3", &detector).unwrap();
    let first = wait(&app, &a.job_id);
    assert!(
        matches!(first.status, JobStatus::Succeeded),
        "{:?}",
        first.error
    );
    assert_eq!(first.result_ids, wait(&app, &b.job_id).result_ids);
    let parent = app
        .person_feature(&source.id, "3", &detector)
        .unwrap()
        .unwrap();
    assert_eq!(parent.frame.frame_ref.presentation_ordinal, "3");
    assert_eq!(
        parent.frame.frame_ref.index_generation,
        source.index_generation
    );
    assert!(app
        .person_feature(&source.id, "4", &detector)
        .unwrap()
        .is_none());
    assert_eq!(
        app.estimate_pose(&parent.id, 2, &pose).unwrap_err().code,
        "INVALID_SUBJECT"
    );
    assert_eq!(
        app.choose_subject(&parent.id, 2).unwrap_err().code,
        "INVALID_SUBJECT"
    );
    let choice = app.choose_subject(&parent.id, 1).unwrap();
    assert_eq!(choice.model_id, detector);
    assert_eq!(
        app.subject_choice(&source.id, "3")
            .unwrap()
            .unwrap()
            .subject
            .person_index,
        1
    );
    assert!(app.subject_choice(&source.id, "4").unwrap().is_none());
    app.clear_subject(&source.id, "3").unwrap();
    assert!(app.subject_choice(&source.id, "3").unwrap().is_none());
    app.choose_subject(&parent.id, 1).unwrap();
    let measured = app
        .measure_region(
            &source.id,
            "3",
            "custom",
            RegionRectV1 {
                x: 0.25,
                y: 0.25,
                width: 0.5,
                height: 0.5,
            },
        )
        .unwrap();
    // Changing the choice cannot relabel the immutable measurement job.
    app.choose_subject(&parent.id, 0).unwrap();
    let measurement = wait(&app, &measured.job_id);
    assert!(
        matches!(measurement.status, JobStatus::Succeeded),
        "{:?}",
        measurement.error
    );
    assert_eq!(
        app.regions(&source.id).unwrap()[0]
            .subject
            .as_ref()
            .unwrap()
            .person_index,
        1
    );
    let a = app.estimate_pose(&parent.id, 0, &pose).unwrap();
    let b = app.estimate_pose(&parent.id, 0, &pose).unwrap();
    let first = wait(&app, &a.job_id);
    assert!(
        matches!(first.status, JobStatus::Succeeded),
        "{:?}",
        first.error
    );
    assert_eq!(first.result_ids, wait(&app, &b.job_id).result_ids);
    let cached = app.pose_feature(&parent.id, 0, &pose).unwrap().unwrap();
    assert_eq!(cached.frame.frame_ref.presentation_ordinal, "3");
    assert!(app.pose_feature(&parent.id, 1, &pose).unwrap().is_none());
    let other = app.estimate_pose(&parent.id, 1, &pose).unwrap();
    assert!(matches!(
        wait(&app, &other.job_id).status,
        JobStatus::Succeeded
    ));
    let other = app.pose_feature(&parent.id, 1, &pose).unwrap().unwrap();
    assert_ne!(cached.id, other.id);
    assert_ne!(cached.result.mask_path, other.result.mask_path);
    // A modified or missing mask invalidates only its dependent pose result.
    let mask = cached.result.mask_path.as_ref().unwrap();
    std::fs::write(mask, b"modified").unwrap();
    assert!(app.pose_feature(&parent.id, 0, &pose).unwrap().is_none());
    assert!(!Path::new(mask).exists());
    let rerun = app.estimate_pose(&parent.id, 0, &pose).unwrap();
    assert!(matches!(
        wait(&app, &rerun.job_id).status,
        JobStatus::Succeeded
    ));
    let regenerated = app.pose_feature(&parent.id, 0, &pose).unwrap().unwrap();
    assert_ne!(cached.id, regenerated.id);
    let calls = ai.with_file_name("frameiq-ai-worker.calls");
    assert_eq!(
        std::fs::read_to_string(&calls)
            .unwrap()
            .lines()
            .filter(|l| *l == "person")
            .count(),
        1
    );
    assert_eq!(
        std::fs::read_to_string(&calls)
            .unwrap()
            .lines()
            .filter(|l| *l == "pose")
            .count(),
        3
    );
    // Cancellation after a worker writes a mask must remove the uncommitted asset.
    let people = app.detect_persons(&source.id, "4", &detector).unwrap();
    assert!(matches!(
        wait(&app, &people.job_id).status,
        JobStatus::Succeeded
    ));
    let next = app
        .person_feature(&source.id, "4", &detector)
        .unwrap()
        .unwrap();
    std::fs::write(ai.with_file_name("frameiq-ai-worker.slow"), b"slow").unwrap();
    let masks = d.path().join("data/cache/pose-masks");
    let before = std::fs::read_dir(&masks).unwrap().count();
    let pending = app.estimate_pose(&next.id, 0, &pose).unwrap();
    let start = Instant::now();
    while std::fs::read_dir(&masks).unwrap().count() == before {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    }
    app.cancel(&pending.job_id).unwrap();
    assert!(matches!(
        wait(&app, &pending.job_id).status,
        JobStatus::Cancelled
    ));
    assert_eq!(std::fs::read_dir(&masks).unwrap().count(), before);
    assert!(app.pose_feature(&next.id, 0, &pose).unwrap().is_none());
    // Restart recovery removes only uncommitted app-named masks.
    let orphan = masks.join(format!("{}.png", uuid::Uuid::new_v4()));
    let unknown = masks.join("user-note.png");
    let external = d.path().join("original.png");
    let linked = masks.join(format!("{}.png", uuid::Uuid::new_v4()));
    std::fs::write(&orphan, b"uncommitted mask").unwrap();
    std::fs::write(&unknown, b"keep unknown").unwrap();
    std::fs::write(&external, b"original file").unwrap();
    std::os::unix::fs::symlink(&external, &linked).unwrap();
    // Cache is durable across application reopening; no detector call is needed.
    drop(app);
    std::thread::sleep(Duration::from_millis(100));
    let app = Application::new(&d.path().join("data"), worker, Arc::new(|_| {})).unwrap();
    assert!(!orphan.exists());
    assert_eq!(std::fs::read(&unknown).unwrap(), b"keep unknown");
    assert!(linked.is_symlink());
    assert_eq!(std::fs::read(&external).unwrap(), b"original file");
    assert_eq!(
        app.person_feature(&source.id, "3", &detector)
            .unwrap()
            .unwrap()
            .id,
        parent.id
    );
    assert_eq!(
        app.pose_feature(&parent.id, 0, &pose).unwrap().unwrap().id,
        regenerated.id
    );
    assert_eq!(
        app.subject_choice(&source.id, "3")
            .unwrap()
            .unwrap()
            .subject
            .person_index,
        0
    );
    app.remove_model(&detector).unwrap();
    assert!(app.subject_choice(&source.id, "3").unwrap().is_none());
    let preserved_regions = app.regions(&source.id).unwrap();
    assert_eq!(preserved_regions.len(), 1);
    assert!(preserved_regions[0].subject.is_none());
    assert_eq!(
        app.pose_feature(&parent.id, 0, &pose).unwrap_err().code,
        "DETECTION_MISSING"
    );
    assert!(!Path::new(regenerated.result.mask_path.as_ref().unwrap()).exists());
    assert!(!Path::new(other.result.mask_path.as_ref().unwrap()).exists());
    assert_eq!(app.selections(&project.id).unwrap().len(), 1);
    assert!(bytes.exists());
}
