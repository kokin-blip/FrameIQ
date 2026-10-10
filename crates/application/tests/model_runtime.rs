//! Opt-in native integration: FRAMEIQ_TEST_MODEL points to reviewed YuNet weights.
//! The ordinary suite does not download weights or access the network.
use frameiq_application::Application;
use frameiq_contracts::*;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
fn wait(app: &Application, id: &str) -> JobEvent {
    let start = Instant::now();
    loop {
        let job = app.get_job(id).unwrap();
        if matches!(
            job.status,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return job;
        }
        assert!(start.elapsed() < Duration::from_secs(120));
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
#[ignore = "Requires reviewed YuNet weights, native AI worker/runtime, and sufficient available memory"]
fn supervised_import_benchmark_reopen_and_remove() {
    let weights = std::env::var_os("FRAMEIQ_TEST_MODEL")
        .expect("Set FRAMEIQ_TEST_MODEL to reviewed YuNet weights");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: ModelManifestV1 = serde_json::from_slice(
        &std::fs::read(root.join("models/manifests/yunet-2023mar.json")).unwrap(),
    )
    .unwrap();
    let worker = root.join(if cfg!(windows) {
        "target/debug/frameiq-worker.exe"
    } else {
        "target/debug/frameiq-worker"
    });
    let data = tempfile::tempdir().unwrap();
    let app = Application::new(data.path(), worker.clone(), Arc::new(|_| {})).unwrap();
    let imported = app
        .import_model(&PathBuf::from(&weights), manifest)
        .unwrap();
    let done = wait(&app, &imported.job_id);
    assert!(
        matches!(done.status, JobStatus::Succeeded),
        "{:?}",
        done.error
    );
    let id = done.result_ids[0].clone();
    let model = app
        .models()
        .unwrap()
        .into_iter()
        .find(|m| m.id == id)
        .unwrap();
    assert_eq!(model.status, "ready");
    for _ in 0..2 {
        let job = app.benchmark_model(&id).unwrap();
        let done = wait(&app, &job.job_id);
        assert!(
            matches!(done.status, JobStatus::Succeeded),
            "{:?}",
            done.error
        );
    }
    assert!(app.benchmarks().unwrap().iter().any(|b| b.model_id == id));
    drop(app);
    let app = Application::new(data.path(), worker, Arc::new(|_| {})).unwrap();
    assert!(app
        .models()
        .unwrap()
        .iter()
        .any(|m| m.id == id && m.status == "ready"));
    app.remove_model(&id).unwrap();
    assert!(!PathBuf::from(model.path).exists());
    assert!(
        PathBuf::from(weights).exists(),
        "Original imported weights must remain untouched"
    );
    assert_eq!(app.benchmark_model(&id).unwrap_err().code, "MODEL_MISSING");
}

#[test]
#[ignore = "Requires reviewed person/pose weights, native workers/runtime and a local fixture video"]
fn indexed_person_pose_cache_with_native_models() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let worker = root.join(if cfg!(windows) {
        "target/debug/frameiq-worker.exe"
    } else {
        "target/debug/frameiq-worker"
    });
    let data = tempfile::tempdir().unwrap();
    let app = Application::new(data.path(), worker, Arc::new(|_| {})).unwrap();
    let install = |file: &str, env: &str| {
        let manifest: ModelManifestV1 = serde_json::from_slice(
            &std::fs::read(root.join("models/manifests").join(file)).unwrap(),
        )
        .unwrap();
        let weights = PathBuf::from(std::env::var_os(env).expect("Supply reviewed model weights"));
        let job = app.import_model(&weights, manifest).unwrap();
        let done = wait(&app, &job.job_id);
        assert!(
            matches!(done.status, JobStatus::Succeeded),
            "{:?}",
            done.error
        );
        done.result_ids[0].clone()
    };
    let detector = install(
        "person_detection_mediapipe-2023mar.json",
        "FRAMEIQ_TEST_PERSON_MODEL",
    );
    let pose_model = install(
        "pose_estimation_mediapipe-2023mar.json",
        "FRAMEIQ_TEST_POSE_MODEL",
    );
    let project = app.create_project("Native body cache").unwrap();
    let video = PathBuf::from(
        std::env::var_os("FRAMEIQ_TEST_BODY_VIDEO")
            .expect("Supply a permitted person fixture video"),
    );
    let job = app.import_video(&project.id, &video, None).unwrap();
    let done = wait(&app, &job.job_id);
    assert!(
        matches!(done.status, JobStatus::Succeeded),
        "{:?}",
        done.error
    );
    let source = app.sources(&project.id).unwrap()[0].clone();
    let job = app.detect_persons(&source.id, "0", &detector).unwrap();
    let done = wait(&app, &job.job_id);
    assert!(
        matches!(done.status, JobStatus::Succeeded),
        "{:?}",
        done.error
    );
    let people = app
        .person_feature(&source.id, "0", &detector)
        .unwrap()
        .unwrap();
    assert!(
        !people.result.persons.is_empty(),
        "Fixture must have a usable detected subject"
    );
    app.choose_subject(&people.id, 0).unwrap();
    assert_eq!(
        app.subject_choice(&source.id, "0")
            .unwrap()
            .unwrap()
            .subject
            .person_feature_id,
        people.id
    );
    let region = app
        .measure_region(
            &source.id,
            "0",
            "custom",
            RegionRectV1 {
                x: 0.2,
                y: 0.2,
                width: 0.4,
                height: 0.4,
            },
        )
        .unwrap();
    let done = wait(&app, &region.job_id);
    assert!(
        matches!(done.status, JobStatus::Succeeded),
        "{:?}",
        done.error
    );
    assert!(app.regions(&source.id).unwrap()[0].subject.is_some());
    let job = app.estimate_pose(&people.id, 0, &pose_model).unwrap();
    let done = wait(&app, &job.job_id);
    assert!(
        matches!(done.status, JobStatus::Succeeded),
        "{:?}",
        done.error
    );
    let pose = app
        .pose_feature(&people.id, 0, &pose_model)
        .unwrap()
        .unwrap();
    assert!(
        pose.result.pose.is_some(),
        "Fixture must yield a usable pose"
    );
    assert_eq!(pose.result.pose.as_ref().unwrap().landmarks.len(), 39);
    let dimensions = image::image_dimensions(pose.result.mask_path.as_ref().unwrap()).unwrap();
    assert_eq!(dimensions, (people.result.width, people.result.height));
    let again = app.estimate_pose(&people.id, 0, &pose_model).unwrap();
    assert_eq!(wait(&app, &again.job_id).result_ids, vec![pose.id.clone()]);
    if let Some(path) = std::env::var_os("FRAMEIQ_TEST_BODY_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"scope":"Native model orchestration/cache smoke test; not accuracy or runtime parity evaluation",
            "ordinal":pose.frame.frame_ref.presentation_ordinal,"width":pose.result.width,"height":pose.result.height,
            "subjects":people.result.persons.len(),"landmarks":39,"maskSha256":pose.result.mask_sha256,
            "detectorHash":people.result.model_hash,"poseHash":pose.result.model_hash,
            "cacheReused":true,"subjectChoiceSaved":true,"guidedRegionLinked":true
        })).unwrap()).unwrap();
    }
    app.remove_model(&pose_model).unwrap();
    assert!(!PathBuf::from(pose.result.mask_path.unwrap()).exists());
    assert!(app
        .person_feature(&source.id, "0", &detector)
        .unwrap()
        .is_some());
}
