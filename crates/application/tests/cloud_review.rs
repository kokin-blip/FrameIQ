//! Real media preparation and consent gates; never contacts a cloud provider.
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
fn prepared_images_are_exact_bounded_and_consent_is_single_use() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tempfile::tempdir().unwrap();
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let video = dir.path().join("Cloud review 日本語.mkv");
    assert!(
        std::process::Command::new(root.join(format!(".native/ffmpeg/bin/ffmpeg{suffix}")))
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=160x120:rate=5:duration=1",
                "-c:v",
                "ffv1",
                video.to_str().unwrap()
            ])
            .status()
            .unwrap()
            .success()
    );
    let data = dir.path().join("data");
    let app = Application::new(
        &data,
        root.join(format!("target/debug/frameiq-worker{suffix}")),
        Arc::new(|_| {}),
    )
    .unwrap();
    let project = app.create_project("Cloud review test").unwrap();
    success(&app, app.import_video(&project.id, &video, None).unwrap());
    let source = app.sources(&project.id).unwrap()[0].clone();
    let view = app.preview(&source.id, "2").unwrap();
    let request = CloudReviewRequestV1 {
        provider: CloudProviderV1::Openai,
        model: "user-selected-vision-model".into(),
        task: CloudTaskV1::Describe,
        goal: "outfit".into(),
        images: vec![CloudImageInputV1 {
            frame: Selection {
                source_id: source.id.clone(),
                frame_ref: view.frame_ref.clone(),
                time_seconds: view.time_seconds,
            },
            region: Some(RegionRectV1 {
                x: 0.25,
                y: 0.25,
                width: 0.5,
                height: 0.5,
            }),
        }],
    };
    let event = success(&app, app.prepare_cloud_review(request.clone()).unwrap());
    let review = app.cloud_review(&event.job_id).unwrap();
    let image = &review.images[0];
    assert_eq!((image.width, image.height), (80, 60));
    assert_eq!(image.input.frame.frame_ref.presentation_ordinal, "2");
    let bytes = std::fs::read(&image.path).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), image.sha256);
    assert_eq!(&bytes[..2], &[0xff, 0xd8]);
    assert!(bytes.len() <= 1024 * 1024);
    assert_eq!(
        app.analyze_cloud_review(&review.id, "fake-key".into(), false)
            .unwrap_err()
            .code,
        "CONSENT_REQUIRED"
    );
    assert!(app.cloud_review(&review.id).is_ok());
    // Alteration is detected before any provider request. The review is consumed, never auto-retried.
    std::fs::write(&image.path, b"changed").unwrap();
    let job = app
        .analyze_cloud_review(&review.id, "fake-key".into(), true)
        .unwrap();
    let failed = wait(&app, &job.job_id);
    assert_eq!(failed.error.unwrap().code, "REVIEW_CHANGED");
    assert!(app.cloud_review(&review.id).is_err());
    assert!(!Path::new(&image.path).exists());
    let mut stale = request.clone();
    stale.images[0].frame.frame_ref.index_generation = "stale".into();
    assert_eq!(
        app.prepare_cloud_review(stale).unwrap_err().code,
        "SOURCE_CHANGED"
    );
    let mut bad = request.clone();
    bad.images[0].region = Some(RegionRectV1 {
        x: 1.,
        y: 0.,
        width: 0.1,
        height: 1.,
    });
    assert_eq!(
        app.prepare_cloud_review(bad).unwrap_err().code,
        "INVALID_REGION"
    );
    let event = success(&app, app.prepare_cloud_review(request).unwrap());
    let pending = app.cloud_review(&event.job_id).unwrap();
    let original = std::fs::read(&video).unwrap();
    drop(app);
    assert!(!Path::new(&pending.images[0].path).exists());
    assert_eq!(original, std::fs::read(&video).unwrap());
}
