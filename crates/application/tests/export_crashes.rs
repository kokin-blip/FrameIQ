//! Real application-process interruptions; enabled only for explicit fault-injection test builds.
#![cfg(feature = "fault-injection")]
use frameiq_application::Application;
use frameiq_contracts::*;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
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
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn worker() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows) {
        "../../target/debug/frameiq-worker.exe"
    } else {
        "../../target/debug/frameiq-worker"
    })
}
#[test]
#[ignore = "Invoked as a subprocess by recovery_matrix"]
fn export_crash_helper() {
    let data = PathBuf::from(std::env::var_os("FRAMEIQ_TEST_DATA").unwrap());
    let video = PathBuf::from(std::env::var_os("FRAMEIQ_TEST_VIDEO").unwrap());
    let app = Application::new(&data, worker(), Arc::new(|_| {})).unwrap();
    let project = app.create_project("Crash fixture").unwrap();
    let import = app.import_video(&project.id, &video, None).unwrap();
    assert!(matches!(
        terminal(&app, &import.job_id).status,
        JobStatus::Succeeded
    ));
    let source = app.sources(&project.id).unwrap()[0].clone();
    app.select(&source.id, "2", true).unwrap();
    let export = app
        .export(
            &project.id,
            data.parent().unwrap(),
            ExportOptions::default(),
        )
        .unwrap();
    terminal(&app, &export.job_id);
    panic!("The configured crash point did not terminate the process");
}
#[test]
fn recovery_matrix_verifies_image_sidecar_and_database_boundaries() {
    let d = tempfile::tempdir().unwrap();
    let video = d.path().join("fixture.mkv");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ffmpeg = root.join(if cfg!(windows) {
        ".native/ffmpeg/bin/ffmpeg.exe"
    } else {
        ".native/ffmpeg/bin/ffmpeg"
    });
    assert!(Command::new(ffmpeg)
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
    for point in [
        "before_checkpoint",
        "after_checkpoint",
        "after_image",
        "after_sidecar",
        "after_item_checkpoint",
    ] {
        let data = d.path().join(point);
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "export_crash_helper", "--ignored"])
            .env("FRAMEIQ_TEST_DATA", &data)
            .env("FRAMEIQ_TEST_VIDEO", &video)
            .env("FRAMEIQ_TEST_EXPORT_CRASH", point)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86), "{point}");
        assert!(
            std::fs::read_dir(data.join("cache/export-staging"))
                .unwrap()
                .next()
                .is_some(),
            "The interruption should leave app-owned staging files"
        );
        let emitted = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = emitted.clone();
        let app = Application::new(
            &data,
            worker(),
            Arc::new(move |event| {
                captured.lock().unwrap().push(event.message_code);
            }),
        )
        .unwrap();
        assert!(
            std::fs::read_dir(data.join("cache/export-staging"))
                .unwrap()
                .next()
                .is_none(),
            "Startup must remove interrupted staging files"
        );
        let project = app.projects().unwrap()[0].clone();
        let plan = app.export_plans(&project.id).unwrap()[0].clone();
        assert_eq!(plan.items.len(), 1);
        let unrelated = Path::new(&plan.destination).join("keep.txt");
        std::fs::write(&unrelated, b"user file").unwrap();
        // A selection edit after the interruption does not change the persisted plan.
        app.select(&plan.items[0].source_id, "2", false).unwrap();
        let resume = app.resume_export(&plan.id).unwrap();
        let done = terminal(&app, &resume.job_id);
        assert!(
            matches!(done.status, JobStatus::Succeeded),
            "{point}: {:?}",
            done.error
        );
        let finished = app.export_plans(&project.id).unwrap()[0].clone();
        assert_eq!(finished.items[0].status, "succeeded");
        assert!(Path::new(&finished.items[0].image_path).is_file());
        assert!(Path::new(&finished.items[0].image_path)
            .with_extension("png.json")
            .is_file());
        assert_eq!(std::fs::read(&unrelated).unwrap(), b"user file");
        // A second resume checks ownership/checksums rather than rewriting verified output pairs.
        emitted.lock().unwrap().clear();
        let again = app.resume_export(&plan.id).unwrap();
        assert!(matches!(
            terminal(&app, &again.job_id).status,
            JobStatus::Succeeded
        ));
        assert!(emitted
            .lock()
            .unwrap()
            .iter()
            .any(|code| code == "FRAME_SKIPPED"));
    }
}
