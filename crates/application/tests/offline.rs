use frameiq_application::Application;
use frameiq_contracts::*;
use std::{
    path::Path,
    process::Command,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
#[cfg(unix)]
#[test]
fn interrupted_export_cleanup_rejects_a_redirected_staging_directory() {
    let d = tempfile::tempdir().unwrap();
    let data = d.path().join("data");
    let unrelated = d.path().join("unrelated");
    std::fs::create_dir_all(data.join("cache")).unwrap();
    std::fs::create_dir(&unrelated).unwrap();
    std::fs::write(unrelated.join("keep.txt"), b"user file").unwrap();
    std::os::unix::fs::symlink(&unrelated, data.join("cache/export-staging")).unwrap();
    let worker = d.path().join("worker");
    std::fs::write(&worker, b"not launched").unwrap();
    assert!(Application::new(&data, worker, Arc::new(|_| {})).is_err());
    assert_eq!(
        std::fs::read(unrelated.join("keep.txt")).unwrap(),
        b"user file"
    );
    assert!(data.join("cache/export-staging").is_symlink());
}
#[cfg(unix)]
#[test]
fn mask_orphan_recovery_ignores_redirected_cache_without_blocking_extraction() {
    let d = tempfile::tempdir().unwrap();
    let data = d.path().join("data");
    let unrelated = d.path().join("unrelated");
    std::fs::create_dir_all(data.join("cache")).unwrap();
    std::fs::create_dir(&unrelated).unwrap();
    let original = unrelated.join(format!("{}.png", uuid::Uuid::new_v4()));
    std::fs::write(&original, b"original file").unwrap();
    std::os::unix::fs::symlink(&unrelated, data.join("cache/pose-masks")).unwrap();
    let worker = d.path().join("worker");
    std::fs::write(&worker, b"not launched").unwrap();
    assert!(Application::new(&data, worker, Arc::new(|_| {})).is_ok());
    assert_eq!(std::fs::read(&original).unwrap(), b"original file");
}
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
        assert!(start.elapsed() < Duration::from_secs(30), "Job timed out");
        thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn offline_import_precise_selection_and_three_format_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let worker = root.join(if cfg!(windows) {
        "target/debug/frameiq-worker.exe"
    } else {
        "target/debug/frameiq-worker"
    });
    assert!(worker.exists(), "Build the media worker first");
    let d = tempfile::tempdir().unwrap();
    let video = d.path().join("video spaces & unicode café.mkv");
    let status = Command::new(root.join(if cfg!(windows) {
        ".native/ffmpeg/bin/ffmpeg.exe"
    } else {
        ".native/ffmpeg/bin/ffmpeg"
    }))
    .args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=160x120:rate=10:duration=1",
        "-vf",
        "setpts=N*N/(10*TB)",
        "-fps_mode",
        "passthrough",
        "-c:v",
        "ffv1",
        video.to_str().unwrap(),
    ])
    .status()
    .unwrap();
    assert!(status.success());
    let app =
        Application::new(&d.path().join("project"), worker.clone(), Arc::new(|_| {})).unwrap();
    let project = app.create_project("Offline fixture").unwrap();
    let imported = app.import_video(&project.id, &video, None).unwrap();
    let done = wait(&app, &imported.job_id);
    assert!(
        matches!(done.status, JobStatus::Succeeded),
        "{:?}",
        done.error
    );
    let source = &app.sources(&project.id).unwrap()[0];
    let reused = app.import_video(&project.id, &video, None).unwrap();
    assert!(matches!(reused.status, JobStatus::Succeeded));
    assert_eq!(app.sources(&project.id).unwrap().len(), 1);
    assert_eq!(source.frame_count, "10");
    let f = app.resolve(&source.id, 2.5).unwrap();
    assert_eq!(f.ordinal, "5");
    let preview = app.preview(&source.id, "5").unwrap();
    assert_eq!((preview.width, preview.height), (160, 120));
    app.select(&source.id, "5", true).unwrap();
    app.select(&source.id, "9", true).unwrap();
    assert_eq!(app.selections(&project.id).unwrap().len(), 2);
    for format in [ExportFormat::Png, ExportFormat::Jpeg, ExportFormat::Webp] {
        let job = app
            .export(
                &project.id,
                d.path(),
                ExportOptions {
                    format,
                    ..ExportOptions::default()
                },
            )
            .unwrap();
        let done = wait(&app, &job.job_id);
        assert!(
            matches!(done.status, JobStatus::Succeeded),
            "{:?}",
            done.error
        );
        let output = Path::new(&done.result_ids[0]);
        let files = std::fs::read_dir(output)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(files.len(), 4);
        for f in files
            .into_iter()
            .filter(|f| f.path().extension().unwrap() == "json")
        {
            let v: serde_json::Value =
                serde_json::from_slice(&std::fs::read(f.path()).unwrap()).unwrap();
            assert_eq!(v["schemaVersion"], 1);
            assert!(v["sourceSha256"].as_str().unwrap().len() == 64);
            assert_eq!(v["frameRef"]["indexGeneration"], source.index_generation);
        }
    }
    let analysis = app.analyze(&source.id).unwrap();
    assert!(matches!(
        wait(&app, &analysis.job_id).status,
        JobStatus::Succeeded
    ));
    let recommendations = app
        .recommendations(&source.id, RankingConfigV1::default())
        .unwrap()
        .unwrap();
    assert!(recommendations.coverage_complete);
    assert!(!recommendations.frames.is_empty());
    assert!(recommendations.frames.len() <= 8);
    assert!(recommendations
        .frames
        .iter()
        .all(|f| f.quality.frame.frame_ref.index_generation == source.index_generation));
    assert_eq!(app.selections(&project.id).unwrap().len(), 2);
    let reranked = app
        .recommendations(
            &source.id,
            RankingConfigV1 {
                sharpness_weight: 0.,
                exposure_weight: 100.,
                blur_weight: 0.,
                count: 3,
            },
        )
        .unwrap()
        .unwrap();
    assert!(reranked.frames.len() <= 3);
    let region_job = app
        .measure_region(
            &source.id,
            "5",
            "custom",
            RegionRectV1 {
                x: 0.2,
                y: 0.2,
                width: 0.5,
                height: 0.5,
            },
        )
        .unwrap();
    assert!(matches!(
        wait(&app, &region_job.job_id).status,
        JobStatus::Succeeded
    ));
    let regions = app.regions(&source.id).unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].frame.frame_ref.presentation_ordinal, "5");
    assert_eq!(regions[0].origin, "user_drawn_untracked");
    let custom = app
        .save_preset(PresetV1 {
            id: "ignored".into(),
            name: "My region".into(),
            version: 77,
            weights: [7., 4., 6., 1., 2.],
            count: 5,
            required_capabilities: vec!["region_tracking".into()],
            automatic_available: true,
            unavailable_reason: None,
        })
        .unwrap();
    assert!(!custom.automatic_available);
    assert!((custom.weights.iter().sum::<f64>() - 100.).abs() < 1e-9);
    assert!(app.presets().unwrap().iter().any(|p| p.id == custom.id));
    // Relocation verifies content and preserves the exact selection/index identity.
    let relocated = d.path().join("relocated.mkv");
    std::fs::rename(&video, &relocated).unwrap();
    assert_eq!(
        app.preview(&source.id, "5").unwrap_err().code,
        "SOURCE_MISSING"
    );
    let moved = app.relocate_source(&source.id, &relocated).unwrap();
    assert!(matches!(
        wait(&app, &moved.job_id).status,
        JobStatus::Succeeded
    ));
    assert_eq!(
        app.sources(&project.id).unwrap()[0].index_generation,
        source.index_generation
    );
    assert_eq!(app.selections(&project.id).unwrap().len(), 2);
    let plans = app.export_plans(&project.id).unwrap();
    assert_eq!(plans.len(), 3);
    let plan = plans[0].clone();
    let output = Path::new(&plan.items[0].image_path);
    // Retained image + missing sidecar is recoverable; immutable plans ignore new selections.
    let sidecar = output.with_extension(format!("{}.json", plan.options.format.extension()));
    std::fs::remove_file(sidecar).unwrap();
    app.select(&source.id, "5", false).unwrap();
    drop(app);
    let app = Application::new(&d.path().join("project"), worker, Arc::new(|_| {})).unwrap();
    let resumed = app.resume_export(&plan.id).unwrap();
    assert!(matches!(
        wait(&app, &resumed.job_id).status,
        JobStatus::Succeeded
    ));
    assert_eq!(app.export_plans(&project.id).unwrap()[0].items.len(), 2);
    let totals = app.export_attempt_summary(&resumed.job_id).unwrap();
    assert!(totals.accounting_available);
    assert_eq!(
        (
            totals.completed,
            totals.skipped,
            totals.failed,
            totals.pending
        ),
        (1, 1, 0, 0)
    );
    // A further attempt must count verified skips independently, not inherit prior writes.
    let again = app.resume_export(&plan.id).unwrap();
    assert!(matches!(
        wait(&app, &again.job_id).status,
        JobStatus::Succeeded
    ));
    let totals = app.export_attempt_summary(&again.job_id).unwrap();
    assert_eq!(
        (
            totals.completed,
            totals.skipped,
            totals.failed,
            totals.pending
        ),
        (0, 2, 0, 0)
    );
    // A disconnected/replaced destination must leave the plan resumable and unrelated
    // content untouched. Restoring the directory verifies outputs instead of rewriting.
    let destination = Path::new(&plan.destination);
    let parked = destination.with_extension("disconnected");
    std::fs::rename(destination, &parked).unwrap();
    std::fs::write(destination, b"unrelated destination file").unwrap();
    assert_eq!(
        app.resume_export(&plan.id).unwrap_err().code,
        "DESTINATION_MISSING"
    );
    assert!(!app.export_plans(&project.id).unwrap()[0].active);
    assert_eq!(
        std::fs::read(destination).unwrap(),
        b"unrelated destination file"
    );
    std::fs::remove_file(destination).unwrap();
    std::fs::rename(&parked, destination).unwrap();
    let reconnected = app.resume_export(&plan.id).unwrap();
    assert!(matches!(
        wait(&app, &reconnected.job_id).status,
        JobStatus::Succeeded
    ));
    assert_eq!(
        app.export_attempt_summary(&reconnected.job_id)
            .unwrap()
            .skipped,
        2
    );
    // Sidecars have the same collision protection as images, even after repeated resume.
    let sidecar = output.with_extension(format!("{}.json", plan.options.format.extension()));
    let original_sidecar = std::fs::read(&sidecar).unwrap();
    std::fs::write(&sidecar, b"unrelated sidecar").unwrap();
    let conflicting_sidecar = app.resume_export(&plan.id).unwrap();
    assert_eq!(
        wait(&app, &conflicting_sidecar.job_id).error.unwrap().code,
        "OUTPUT_CONFLICT"
    );
    assert_eq!(std::fs::read(&sidecar).unwrap(), b"unrelated sidecar");
    let totals = app
        .export_attempt_summary(&conflicting_sidecar.job_id)
        .unwrap();
    assert_eq!((totals.failed, totals.pending), (1, 1));
    std::fs::write(&sidecar, original_sidecar).unwrap();
    // Never overwrite a user-modified output.
    std::fs::write(output, b"unrelated file").unwrap();
    let conflicting = app.resume_export(&plan.id).unwrap();
    let failed = wait(&app, &conflicting.job_id);
    assert_eq!(failed.error.unwrap().code, "OUTPUT_CONFLICT");
    let totals = app.export_attempt_summary(&conflicting.job_id).unwrap();
    assert_eq!(
        (
            totals.completed,
            totals.skipped,
            totals.failed,
            totals.pending
        ),
        (0, 0, 1, 1)
    );
    assert_eq!(
        app.export_attempt_summary(&resumed.job_id)
            .unwrap()
            .completed,
        1
    );
    assert_eq!(std::fs::read(output).unwrap(), b"unrelated file");
    let wrong = d.path().join("wrong.mkv");
    std::fs::write(&wrong, b"different").unwrap();
    let rejected = app.relocate_source(&source.id, &wrong).unwrap();
    assert_eq!(
        wait(&app, &rejected.job_id).error.unwrap().code,
        "SOURCE_MISMATCH"
    );
    std::fs::write(&relocated, b"changed source").unwrap();
    assert_eq!(
        app.preview(&source.id, "5").unwrap_err().code,
        "SOURCE_CHANGED"
    );
    let failed = app
        .export(&project.id, d.path(), ExportOptions::default())
        .unwrap();
    assert!(matches!(
        wait(&app, &failed.job_id).status,
        JobStatus::Failed
    ));
}

#[cfg(unix)]
#[test]
fn cancellation_stops_a_hung_worker_and_database_has_one_application_owner() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let worker = d.path().join("waiting-worker");
    std::fs::write(&worker, "#!/bin/sh\nexec sleep 30\n").unwrap();
    std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755)).unwrap();
    let data = d.path().join("data");
    let app = Application::new(&data, worker.clone(), Arc::new(|_| {})).unwrap();
    assert!(
        matches!(Application::new(&data,worker,Arc::new(|_|{})),Err(AppError{code,..})if code=="INSTANCE_RUNNING")
    );
    let p = app.create_project("Cancellation fixture").unwrap();
    let source = d.path().join("source.mkv");
    std::fs::write(&source, b"synthetic input").unwrap();
    let job = app.import_video(&p.id, &source, None).unwrap();
    let started = Instant::now();
    while matches!(app.get_job(&job.job_id).unwrap().status, JobStatus::Queued) {
        assert!(started.elapsed() < Duration::from_secs(2));
        thread::sleep(Duration::from_millis(10));
    }
    let cancelled = Instant::now();
    app.cancel(&job.job_id).unwrap();
    let done = wait(&app, &job.job_id);
    assert!(matches!(done.status, JobStatus::Cancelled));
    assert!(cancelled.elapsed() < Duration::from_secs(2));
    assert_eq!(app.sources(&p.id).unwrap()[0].status, "cancelled");
}

#[cfg(unix)]
#[test]
fn local_runtime_cancellation_kills_only_the_managed_worker() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let worker = d.path().join("frameiq-worker");
    std::fs::write(&worker, "#!/bin/sh\nexit 0\n").unwrap();
    let ai = d.path().join("frameiq-ai-worker");
    std::fs::write(&ai, "#!/bin/sh\nread request\nexec sleep 30\n").unwrap();
    for path in [&worker, &ai] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let app = Application::new(&d.path().join("project"), worker, Arc::new(|_| {})).unwrap();
    let caller = app.clone();
    let task = thread::spawn(move || caller.discover_runtime("ollama".into(), 11434, None));
    let start = Instant::now();
    let job = loop {
        if let Some(j) = app
            .list_jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.stage == "runtime_discovery")
        {
            break j;
        }
        assert!(start.elapsed() < Duration::from_secs(2));
        thread::sleep(Duration::from_millis(20));
    };
    let start = Instant::now();
    app.cancel(&job.job_id).unwrap();
    assert_eq!(task.join().unwrap().unwrap_err().code, "CANCELLED");
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(matches!(
        app.get_job(&job.job_id).unwrap().status,
        JobStatus::Cancelled
    ));
}

#[cfg(unix)]
#[test]
fn oversized_media_messages_fail_before_unbounded_allocation() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let worker = d.path().join("oversized-worker");
    std::fs::write(
        &worker,
        "#!/bin/sh\nread request\nhead -c 2200000 /dev/zero\nexec sleep 30\n",
    )
    .unwrap();
    std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755)).unwrap();
    let app = Application::new(&d.path().join("data"), worker, Arc::new(|_| {})).unwrap();
    let p = app.create_project("Protocol fixture").unwrap();
    let source = d.path().join("source.mkv");
    std::fs::write(&source, b"synthetic input").unwrap();
    let job = app.import_video(&p.id, &source, None).unwrap();
    let started = Instant::now();
    let done = wait(&app, &job.job_id);
    assert_eq!(done.error.unwrap().code, "WORKER_PROTOCOL");
    assert!(started.elapsed() < Duration::from_secs(2));
}
