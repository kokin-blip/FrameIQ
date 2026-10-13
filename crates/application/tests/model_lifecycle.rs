//! Installation/recovery protocol faults with a deterministic validation worker.
//! This suite does not establish ONNX compatibility or model accuracy.
#![cfg(feature = "fault-injection")]
use frameiq_application::Application;
use frameiq_contracts::*;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

fn setup(root: &Path) -> (PathBuf, PathBuf, ModelManifestV1) {
    let worker = root.join(if cfg!(windows) {
        "frameiq-worker.exe"
    } else {
        "frameiq-worker"
    });
    std::fs::write(&worker, b"not used by model-only tests").unwrap();
    let ai = root.join(if cfg!(windows) {
        "frameiq-ai-worker.exe"
    } else {
        "frameiq-ai-worker"
    });
    std::fs::copy(env!("CARGO_BIN_EXE_frameiq-test-ai-worker"), &ai).unwrap();
    std::fs::write(root.join("ai-worker-mode.txt"), "").unwrap();
    let original = root.join("original.onnx");
    std::fs::write(&original, b"protocol fixture").unwrap();
    let mut manifest: ModelManifestV1 =
        serde_json::from_str(include_str!("../../../models/manifests/yunet-2023mar.json")).unwrap();
    manifest.byte_size = "16".into();
    manifest.sha256 = format!("{:x}", Sha256::digest(b"protocol fixture"));
    (worker, original, manifest)
}
fn terminal(app: &Application, id: &str) -> JobEvent {
    let start = Instant::now();
    loop {
        let job = app.get_job(id).unwrap();
        if matches!(
            job.status,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return job;
        }
        assert!(start.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn failed_commit_retains_previous_installation_and_original() {
    let root = tempfile::tempdir().unwrap();
    let (worker, original, manifest) = setup(root.path());
    let data = root.path().join("data");
    let app = Application::new(&data, worker, Arc::new(|_| {})).unwrap();
    let first = app.import_model(&original, manifest.clone()).unwrap();
    let first = terminal(&app, &first.job_id);
    assert!(matches!(first.status, JobStatus::Succeeded));
    let previous = app.models().unwrap()[0].clone();
    let db = rusqlite::Connection::open(data.join("frameiq.db")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_model_insert BEFORE INSERT ON model_installations BEGIN SELECT RAISE(ABORT, 'injected activation commit failure'); END;").unwrap();
    let attempt = app.import_model(&original, manifest).unwrap();
    let failed = terminal(&app, &attempt.job_id);
    assert!(matches!(failed.status, JobStatus::Failed));
    assert!(failed
        .error
        .unwrap()
        .message
        .contains("injected activation commit failure"));
    let models = app.models().unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, previous.id);
    assert_eq!(std::fs::read(&previous.path).unwrap(), b"protocol fixture");
    assert_eq!(std::fs::read(&original).unwrap(), b"protocol fixture");
    assert_eq!(std::fs::read_dir(data.join("models")).unwrap().count(), 1);
}

#[test]
#[cfg(unix)]
fn redirected_storage_fails_import_without_touching_originals() {
    let root = tempfile::tempdir().unwrap();
    let (worker, original, manifest) = setup(root.path());
    let data = root.path().join("data");
    std::fs::create_dir(&data).unwrap();
    let outside = root.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, data.join("models")).unwrap();
    let app = Application::new(&data, worker, Arc::new(|_| {})).unwrap();
    let attempt = app.import_model(&original, manifest).unwrap();
    assert_eq!(
        terminal(&app, &attempt.job_id).error.unwrap().code,
        "MODEL_STORAGE"
    );
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
    assert_eq!(std::fs::read(original).unwrap(), b"protocol fixture");
}

#[test]
#[cfg(unix)]
fn model_removal_rejects_redirected_storage_before_mutating_records() {
    let root = tempfile::tempdir().unwrap();
    let (worker, original, manifest) = setup(root.path());
    let data = root.path().join("data");
    let app = Application::new(&data, worker, Arc::new(|_| {})).unwrap();
    let attempt = app.import_model(&original, manifest).unwrap();
    assert!(matches!(
        terminal(&app, &attempt.job_id).status,
        JobStatus::Succeeded
    ));
    let model = app.models().unwrap()[0].clone();
    let installation = Path::new(&model.path).parent().unwrap();
    let parked = root.path().join("parked-installation");
    std::fs::rename(installation, &parked).unwrap();
    std::os::unix::fs::symlink(&parked, installation).unwrap();
    assert_eq!(
        app.remove_model(&model.id).unwrap_err().code,
        "MODEL_STORAGE"
    );
    assert_eq!(app.models().unwrap()[0].status, "ready");
    assert_eq!(
        std::fs::read(parked.join("model.onnx")).unwrap(),
        b"protocol fixture"
    );
    std::fs::remove_file(installation).unwrap();
    std::fs::rename(&parked, installation).unwrap();
    let model_root = data.join("models");
    let parked_root = root.path().join("parked-models");
    std::fs::rename(&model_root, &parked_root).unwrap();
    std::os::unix::fs::symlink(&parked_root, &model_root).unwrap();
    assert_eq!(
        app.remove_model(&model.id).unwrap_err().code,
        "MODEL_STORAGE"
    );
    assert_eq!(app.models().unwrap()[0].status, "ready");
    assert!(parked_root.join(&model.id).join("model.onnx").exists());
    std::fs::remove_file(&model_root).unwrap();
    std::fs::rename(&parked_root, &model_root).unwrap();
    app.remove_model(&model.id).unwrap();
    assert!(app.models().unwrap().is_empty());
    assert_eq!(std::fs::read(original).unwrap(), b"protocol fixture");
}

#[test]
fn validation_failure_crash_and_cancellation_preserve_previous_models() {
    for mode in ["rejected", "crashed", "cancelled"] {
        let root = tempfile::tempdir().unwrap();
        let (worker, original, manifest) = setup(root.path());
        let data = root.path().join("data");
        let app = Application::new(&data, worker.clone(), Arc::new(|_| {})).unwrap();
        let first = app.import_model(&original, manifest.clone()).unwrap();
        assert!(matches!(
            terminal(&app, &first.job_id).status,
            JobStatus::Succeeded
        ));
        let previous = app.models().unwrap()[0].clone();
        drop(app);
        std::fs::write(root.path().join("ai-worker-mode.txt"), mode).unwrap();
        let app = Application::new(&data, worker, Arc::new(|_| {})).unwrap();
        let attempt = app.import_model(&original, manifest).unwrap();
        if mode == "cancelled" {
            let start = Instant::now();
            while !root.path().join("frameiq-ai-worker.started").exists() {
                assert!(start.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(10));
            }
            app.cancel(&attempt.job_id).unwrap();
        }
        let result = terminal(&app, &attempt.job_id);
        assert_eq!(
            result.error.unwrap().code,
            match mode {
                "rejected" => "MODEL_TEMPLATE",
                "crashed" => "AI_WORKER_CRASH",
                _ => "CANCELLED",
            }
        );
        assert_eq!(app.models().unwrap().len(), 1);
        assert_eq!(std::fs::read_dir(data.join("models")).unwrap().count(), 1);
        assert_eq!(std::fs::read(previous.path).unwrap(), b"protocol fixture");
        assert_eq!(std::fs::read(&original).unwrap(), b"protocol fixture");
        // A failed worker is discarded; a new import can succeed in this same app.
        let (_, _, manifest) = setup(root.path());
        let retry = app.import_model(&original, manifest).unwrap();
        assert!(matches!(
            terminal(&app, &retry.job_id).status,
            JobStatus::Succeeded
        ));
        assert_eq!(app.models().unwrap().len(), 2);
    }
}

#[cfg(feature = "fault-injection")]
#[test]
#[ignore = "Invoked as a subprocess by the activation crash matrix"]
fn model_crash_helper() {
    let root = PathBuf::from(std::env::var_os("FRAMEIQ_TEST_MODEL_ROOT").unwrap());
    let (worker, original, manifest) = setup(&root);
    let app = Application::new(&root.join("data"), worker, Arc::new(|_| {})).unwrap();
    if std::env::var("FRAMEIQ_TEST_MODEL_CRASH")
        .unwrap()
        .contains("removal")
    {
        app.remove_model(&app.models().unwrap()[0].id).unwrap();
        panic!("Removal crash point was not reached");
    }
    let attempt = app.import_model(&original, manifest).unwrap();
    let result = terminal(&app, &attempt.job_id);
    panic!("Configured crash point was not reached: {:?}", result.error);
}

#[cfg(feature = "fault-injection")]
#[test]
fn removal_crash_matrix_recovers_committed_cleanup_and_retains_originals() {
    for point in [
        "before_removal_commit",
        "after_removal_commit",
        "after_removal_files",
    ] {
        let root = tempfile::tempdir().unwrap();
        let (worker, original, manifest) = setup(root.path());
        let data = root.path().join("data");
        let app = Application::new(&data, worker.clone(), Arc::new(|_| {})).unwrap();
        let attempt = app.import_model(&original, manifest).unwrap();
        assert!(matches!(
            terminal(&app, &attempt.job_id).status,
            JobStatus::Succeeded
        ));
        let installed = app.models().unwrap()[0].clone();
        drop(app);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "model_crash_helper"])
            .env("FRAMEIQ_TEST_MODEL_ROOT", root.path())
            .env("FRAMEIQ_TEST_MODEL_CRASH", point)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86));
        let app = Application::new(&data, worker, Arc::new(|_| {})).unwrap();
        assert_eq!(
            app.models().unwrap().len(),
            usize::from(point == "before_removal_commit")
        );
        assert_eq!(
            Path::new(&installed.path).exists(),
            point == "before_removal_commit"
        );
        assert_eq!(std::fs::read(original).unwrap(), b"protocol fixture");
    }
}

#[cfg(feature = "fault-injection")]
#[test]
fn activation_crash_matrix_retains_only_committed_installations() {
    for point in ["before_activation", "after_activation", "after_commit"] {
        let root = tempfile::tempdir().unwrap();
        let (worker, original, manifest) = setup(root.path());
        let data = root.path().join("data");
        let app = Application::new(&data, worker.clone(), Arc::new(|_| {})).unwrap();
        let first = app.import_model(&original, manifest).unwrap();
        assert!(matches!(
            terminal(&app, &first.job_id).status,
            JobStatus::Succeeded
        ));
        let previous = app.models().unwrap()[0].clone();
        drop(app);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "model_crash_helper", "--nocapture"])
            .env("FRAMEIQ_TEST_MODEL_ROOT", root.path())
            .env("FRAMEIQ_TEST_MODEL_CRASH", point)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86), "{point}");
        let app = Application::new(&data, worker, Arc::new(|_| {})).unwrap();
        let models = app.models().unwrap();
        let expected = if point == "after_commit" { 2 } else { 1 };
        assert_eq!(models.len(), expected, "{point}");
        assert_eq!(
            std::fs::read_dir(data.join("models")).unwrap().count(),
            expected,
            "{point}"
        );
        assert!(models.iter().any(|m| m.id == previous.id));
        assert_eq!(std::fs::read(&previous.path).unwrap(), b"protocol fixture");
        assert_eq!(std::fs::read(&original).unwrap(), b"protocol fixture");
    }
}

#[test]
fn idle_worker_unloads_and_restarts_without_orphans() {
    let root = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "idle_worker_helper"])
        .env("FRAMEIQ_TEST_MODEL_ROOT", root.path())
        .env("FRAMEIQ_TEST_IDLE_MS", "100")
        .status()
        .unwrap();
    assert!(status.success());
}

fn worker_exists(pid: u32) -> bool {
    let mut system = sysinfo::System::new();
    let pid = sysinfo::Pid::from_u32(pid);
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).is_some()
}
fn wait_stopped(pid: u32) {
    let start = Instant::now();
    while worker_exists(pid) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "orphan worker {pid}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
#[ignore = "Invoked in a subprocess with test-only idle timing"]
fn idle_worker_helper() {
    let root = PathBuf::from(std::env::var_os("FRAMEIQ_TEST_MODEL_ROOT").unwrap());
    let (worker, original, manifest) = setup(&root);
    let data = root.join("data");
    let app = Application::new(&data, worker.clone(), Arc::new(|_| {})).unwrap();
    let first = app.import_model(&original, manifest.clone()).unwrap();
    assert!(matches!(
        terminal(&app, &first.job_id).status,
        JobStatus::Succeeded
    ));
    let pid = std::fs::read_to_string(root.join("ai-worker.pid"))
        .unwrap()
        .parse()
        .unwrap();
    wait_stopped(pid);
    let second = app.import_model(&original, manifest).unwrap();
    assert!(matches!(
        terminal(&app, &second.job_id).status,
        JobStatus::Succeeded
    ));
    let next = std::fs::read_to_string(root.join("ai-worker.pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(pid, next);
    drop(app);
    wait_stopped(next);
    // The monitor must not retain the application or its instance lock.
    drop(Application::new(&data, worker, Arc::new(|_| {})).unwrap());
}

#[test]
fn repeated_worker_failures_leave_no_processes_or_staging_growth() {
    let root = tempfile::tempdir().unwrap();
    let (worker, original, manifest) = setup(root.path());
    let data = root.path().join("data");
    let app = Application::new(&data, worker, Arc::new(|_| {})).unwrap();
    let first = app.import_model(&original, manifest.clone()).unwrap();
    assert!(matches!(
        terminal(&app, &first.job_id).status,
        JobStatus::Succeeded
    ));
    let installed = app.models().unwrap()[0].clone();
    for i in 0..20 {
        std::fs::write(
            root.path().join("ai-worker-mode.txt"),
            if i % 2 == 0 { "crashed" } else { "rejected" },
        )
        .unwrap();
        let job = app.import_model(&original, manifest.clone()).unwrap();
        assert!(matches!(
            terminal(&app, &job.job_id).status,
            JobStatus::Failed
        ));
        let pid = std::fs::read_to_string(root.path().join("ai-worker.pid"))
            .unwrap()
            .parse()
            .unwrap();
        wait_stopped(pid);
        assert_eq!(std::fs::read_dir(data.join("models")).unwrap().count(), 1);
        assert_eq!(app.models().unwrap().len(), 1);
        assert_eq!(std::fs::read(&installed.path).unwrap(), b"protocol fixture");
    }
    assert_eq!(std::fs::read(original).unwrap(), b"protocol fixture");
}

#[test]
fn download_success_waits_for_validation_and_cached_retry_commits_the_same_job() {
    let root = tempfile::tempdir().unwrap();
    let (worker, _original, mut manifest) = setup(root.path());
    let payload = b"\x3a\x0c\x0a\x0a\x22\x08Identity";
    std::fs::write(root.path().join("original.onnx"), payload).unwrap();
    manifest.byte_size = payload.len().to_string();
    manifest.sha256 = format!("{:x}", Sha256::digest(payload));
    let data = root.path().join("data");
    std::fs::write(root.path().join("ai-worker-mode.txt"), "memory").unwrap();
    let app = Application::new(&data, worker, Arc::new(|_| {})).unwrap();
    let job = app.download_model(manifest.clone(), true).unwrap();
    let failed = terminal(&app, &job.job_id);
    assert!(matches!(failed.status, JobStatus::Failed));
    assert_eq!(failed.error.unwrap().code, "MEMORY_LIMIT");
    assert!(failed.result_ids.is_empty());
    assert!(app.models().unwrap().is_empty());
    assert_eq!(
        app.list_jobs().unwrap().len(),
        1,
        "no independent validation job"
    );
    let cached = data
        .join("model-downloads")
        .join(format!("{}.partial", manifest.sha256));
    assert_eq!(std::fs::read(&cached).unwrap(), payload);
    std::fs::write(root.path().join("ai-worker-mode.txt"), "").unwrap();
    let retry = app.retry_model_validation(manifest).unwrap();
    let ready = terminal(&app, &retry.job_id);
    assert!(matches!(ready.status, JobStatus::Succeeded));
    assert_eq!(ready.result_ids, vec![app.models().unwrap()[0].id.clone()]);
    assert!(cached.exists());
}

#[test]
fn overlapping_pack_installations_are_rejected_and_validation_cancels_with_parent() {
    let root = tempfile::tempdir().unwrap();
    let (worker, original, manifest) = setup(root.path());
    std::fs::write(root.path().join("ai-worker-mode.txt"), "cancelled").unwrap();
    let app = Application::new(&root.path().join("data"), worker, Arc::new(|_| {})).unwrap();
    let job = app.download_model(manifest.clone(), true).unwrap();
    let start = Instant::now();
    while !root.path().join("frameiq-ai-worker.started").exists() {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        app.import_model(&original, manifest).unwrap_err().code,
        "MODEL_BUSY"
    );
    app.cancel(&job.job_id).unwrap();
    assert!(matches!(
        terminal(&app, &job.job_id).status,
        JobStatus::Cancelled
    ));
    assert!(app.models().unwrap().is_empty());
}

#[test]
fn catalog_status_recognizes_existing_installation_uuid_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let (worker, original, _) = setup(root.path());
    let data = root.path().join("data");
    let app = Application::new(&data, worker.clone(), Arc::new(|_| {})).unwrap();
    // The protocol double isolates identity handling; this is not runtime compatibility evidence.
    let manifest = app.model_catalog().unwrap().remove(0);
    let job = app.import_model(&original, manifest.clone()).unwrap();
    let id = terminal(&app, &job.job_id).result_ids[0].clone();
    assert_ne!(id, manifest.id);
    drop(app);
    let app = Application::new(&data, worker, Arc::new(|_| {})).unwrap();
    let status = app
        .model_setup_status()
        .unwrap()
        .into_iter()
        .find(|s| s.manifest_id == manifest.id)
        .unwrap();
    assert_eq!(status.state, "ready");
    assert_eq!(status.installation_ids, vec![id]);
    let download = app.download_model(manifest, true).unwrap();
    assert_eq!(
        terminal(&app, &download.job_id).result_ids,
        status.installation_ids
    );
    assert_eq!(app.models().unwrap().len(), 1);
}
