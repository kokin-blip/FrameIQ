//! Portable real-media orchestration with deterministic detector output. Not model accuracy evidence.
#![cfg(feature = "fault-injection")]
use frameiq_application::Application;
use frameiq_contracts::*;
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
fn wait(app: &Application, id: &str) -> JobEvent {
    let start = Instant::now();
    loop {
        let e = app.get_job(id).unwrap();
        if matches!(
            e.status,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return e;
        }
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn success(app: &Application, e: JobEvent) -> JobEvent {
    let e = wait(app, &e.job_id);
    assert!(matches!(e.status, JobStatus::Succeeded), "{:?}", e.error);
    e
}
#[test]
fn tracked_presets_rerank_reopen_invalidate_and_cancel_children() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = tempfile::tempdir().unwrap();
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let worker = data.path().join(format!("frameiq-worker{suffix}"));
    std::fs::copy(
        root.join(format!("target/debug/frameiq-worker{suffix}")),
        &worker,
    )
    .unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_frameiq-test-ai-worker"),
        data.path().join(format!("frameiq-ai-worker{suffix}")),
    )
    .unwrap();
    std::fs::write(data.path().join("ai-worker-mode.txt"), "tracking").unwrap();
    let video = data.path().join("source.mkv");
    assert!(
        std::process::Command::new(root.join(format!(".native/ffmpeg/bin/ffmpeg{suffix}")))
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=160x120:rate=10:duration=2",
                "-c:v",
                "ffv1",
                video.to_str().unwrap()
            ])
            .status()
            .unwrap()
            .success()
    );
    let appdata = data.path().join("data");
    let app = Application::new(&appdata, worker.clone(), Arc::new(|_| {})).unwrap();
    let original = data.path().join("weights.onnx");
    std::fs::write(&original, b"protocol fixture").unwrap();
    let mut manifest: ModelManifestV1 = serde_json::from_str(include_str!(
        "../../../models/manifests/person_detection_mediapipe-2023mar.json"
    ))
    .unwrap();
    manifest.byte_size = "16".into();
    manifest.sha256 = format!("{:x}", Sha256::digest(b"protocol fixture"));
    let model = success(&app, app.import_model(&original, manifest).unwrap()).result_ids[0].clone();
    let project = app.create_project("Tracking test").unwrap();
    success(&app, app.import_video(&project.id, &video, None).unwrap());
    let source = app.sources(&project.id).unwrap()[0].clone();
    let preset = app
        .presets()
        .unwrap()
        .into_iter()
        .find(|p| p.id == "outfit")
        .unwrap();
    let mut request = PresetExecutionV1 {
        source_id: source.id.clone(),
        preset,
        anchor_ordinal: "0".into(),
        person_model_id: model.clone(),
        person_index: 0,
        face_model_id: None,
        custom_rectangle: None,
        experimental: true,
    };
    app.select(&source.id, "19", true).unwrap();
    let job = success(&app, app.select_preset_frames(request.clone()).unwrap());
    let result = app.preset_result(&job.job_id).unwrap().unwrap();
    let selected = app.selections(&project.id).unwrap();
    assert!(selected
        .iter()
        .any(|f| f.frame_ref.presentation_ordinal == "19"));
    for f in &result.frames {
        assert!(selected
            .iter()
            .any(|s| s.frame_ref.presentation_ordinal
                == f.quality.frame.frame_ref.presentation_ordinal));
    }
    assert!(result.track.observations.len() >= 2);
    assert!(result.track.coverage_complete);
    assert_eq!(result.validation_status, "experimental_unvalidated");
    let calls = std::fs::read(data.path().join("ai-worker.calls")).unwrap();
    let region_count = app.regions(&source.id).unwrap().len();
    request.preset.weights = [90., 10., 0., 0., 0.];
    let rerank = success(&app, app.execute_preset(request.clone()).unwrap());
    assert_eq!(
        app.preset_result(&rerank.job_id).unwrap().unwrap().track.id,
        result.track.id
    );
    assert_eq!(
        std::fs::read(data.path().join("ai-worker.calls")).unwrap(),
        calls,
        "Reranking must not run inference"
    );
    assert_eq!(app.regions(&source.id).unwrap().len(), region_count);
    drop(app);
    let app = Application::new(&appdata, worker, Arc::new(|_| {})).unwrap();
    assert!(app.preset_result(&job.job_id).unwrap().is_some());
    assert_eq!(app.selections(&project.id).unwrap().len(), selected.len());
    request.preset.required_capabilities = vec!["hair_mask".into()];
    assert_eq!(
        app.execute_preset(request.clone()).unwrap_err().code,
        "CAPABILITY_UNAVAILABLE"
    );
    request.preset.required_capabilities = vec!["person", "pose", "person_mask"]
        .into_iter()
        .map(String::from)
        .collect();
    request.experimental = false;
    assert_eq!(
        app.execute_preset(request.clone()).unwrap_err().code,
        "CAPABILITY_UNVALIDATED"
    );
    request.experimental = true;
    // New anchor forks a shot track; cancellation propagates into the active detector child.
    request.anchor_ordinal = "1".into();
    let fresh_manifest = app
        .models()
        .unwrap()
        .into_iter()
        .find(|m| m.id == model)
        .unwrap()
        .manifest;
    let fresh_model =
        success(&app, app.import_model(&original, fresh_manifest).unwrap()).result_ids[0].clone();
    request.person_model_id = fresh_model.clone();
    std::fs::write(data.path().join("ai-worker-mode.txt"), "cancelled").unwrap();
    app.clear_selections(&project.id).unwrap();
    app.select(&source.id, "19", true).unwrap();
    let cancelled = app.select_preset_frames(request).unwrap();
    let started = Instant::now();
    while !data.path().join("frameiq-ai-worker.started").exists() {
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    }
    app.cancel(&cancelled.job_id).unwrap();
    let cancelled_result = wait(&app, &cancelled.job_id);
    assert!(
        matches!(cancelled_result.status, JobStatus::Cancelled),
        "{:?}",
        cancelled_result
    );
    assert_eq!(
        app.selections(&project.id).unwrap().len(),
        1,
        "Cancelled runs must not add selections"
    );
    assert!(app.list_jobs().unwrap().iter().all(|e| !matches!(
        e.status,
        JobStatus::Queued | JobStatus::Running | JobStatus::Cancelling
    )));
    app.remove_model(&fresh_model).unwrap();
    app.remove_model(&model).unwrap();
    assert!(app.preset_result(&job.job_id).unwrap().is_none());
    assert!(original.exists());
}
