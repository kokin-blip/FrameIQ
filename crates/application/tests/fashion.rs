use frameiq_application::Application;
use frameiq_contracts::*;
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
fn app(path: &Path) -> Application {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Application::new(
        path,
        std::env::var_os("FRAMEIQ_TEST_WORKER")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                root.join(if cfg!(windows) {
                    "target/debug/frameiq-worker.exe"
                } else {
                    "target/debug/frameiq-worker"
                })
            }),
        Arc::new(|_| {}),
    )
    .unwrap()
}
fn wait(app: &Application, job: JobEvent) -> JobEvent {
    let start = Instant::now();
    loop {
        let e = app.get_job(&job.job_id).unwrap();
        if matches!(
            e.status,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return e;
        }
        assert!(start.elapsed() < Duration::from_secs(120));
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn success(app: &Application, job: JobEvent) -> JobEvent {
    let e = wait(app, job);
    assert!(matches!(e.status, JobStatus::Succeeded), "{:?}", e.error);
    e
}
fn photo(app: &Application, project: &str, path: &Path) -> PhotoSourceV1 {
    image::RgbImage::from_fn(40, 30, |x, y| image::Rgb([x as u8 * 5, y as u8 * 6, 100]))
        .save(path)
        .unwrap();
    success(app, app.import_photo(project, path).unwrap());
    app.photos(project).unwrap().remove(0)
}
fn item(project: &str, revision: &str) -> FashionItemV1 {
    FashionItemV1 {
        id: String::new(),
        project_id: project.into(),
        media: MediaRefV1::Photo {
            revision: revision.into(),
        },
        revision: 0,
        category: "dress".into(),
        rectangle: RegionRectV1 {
            x: 0.1,
            y: 0.1,
            width: 0.6,
            height: 0.7,
        },
        polygon: vec![[0.1, 0.1], [0.7, 0.1], [0.6, 0.8], [0.1, 0.7]],
        mask: None,
        crop: None,
        attributes: vec![FashionAttributeV1 {
            name: "color".into(),
            value: "blue".into(),
            origin: "user".into(),
            confidence: None,
            evidence_ids: vec![],
        }],
        suppressed_attributes: vec![],
        detection_score: None,
        provenance: "user".into(),
        model_hash: None,
        corrected: true,
        deleted: false,
    }
}
#[test]
fn photos_corrections_groups_catalog_looks_and_immutable_exports_survive_reopening() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let a = app(&data);
    let project = a.create_project("Fashion test").unwrap();
    let p = photo(&a, &project.id, &temp.path().join("photo.png"));
    assert!(
        a.sources(&project.id).unwrap().is_empty(),
        "Photos must not manufacture video frame indexes"
    );
    let saved = success(
        &a,
        a.save_fashion_item(item(&project.id, &p.revision)).unwrap(),
    );
    let mut i = a.fashion_items(&project.id).unwrap().remove(0);
    assert_eq!(i.id, saved.result_ids[0]);
    let mask = i.mask.clone().unwrap();
    assert_eq!((mask.width, mask.height), (40, 30));
    let image = image::open(mask.path).unwrap().to_luma8();
    assert_eq!(image.get_pixel(0, 0)[0], 0);
    assert_eq!(image.get_pixel(10, 10)[0], 255);
    i.category = "skirt".into();
    success(&a, a.save_fashion_item(i.clone()).unwrap());
    i = a.fashion_items(&project.id).unwrap().remove(0);
    let stale = wait(
        &a,
        a.save_fashion_item(FashionItemV1 {
            revision: 0,
            ..i.clone()
        })
        .unwrap(),
    );
    assert_eq!(stale.error.unwrap().code, "STALE_ITEM");
    a.undo_fashion_item(&i.id).unwrap();
    assert_eq!(a.fashion_items(&project.id).unwrap()[0].category, "dress");
    let g = a
        .save_garment_group(GarmentGroupV1 {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project.id.clone(),
            name: "Blue dress".into(),
            revision: 0,
            item_ids: vec![i.id.clone()],
            notes: "User reviewed observation".into(),
        })
        .unwrap();
    let catalog = temp.path().join("catalog.json");
    std::fs::write(&catalog,serde_json::to_vec(&serde_json::json!([{"name":"Fixture dress","category":"dress","sourceUrl":"https://example.test/product/dress","brand":"Fixture","styleId":"TEST-42"}])).unwrap()).unwrap();
    success(&a, a.import_catalog(&catalog).unwrap());
    let cs = a.search_catalog(&g.id, "Fixture").unwrap();
    assert_eq!(cs.len(), 1);
    assert!(matches!(
        cs[0].classification,
        MatchClassV1::VisuallySimilar
    ));
    assert!(a
        .confirm_candidate(
            &g.id,
            &cs[0].id,
            "wrong",
            "https://example.test/product/dress",
            true
        )
        .is_err());
    let c = a
        .confirm_candidate(
            &g.id,
            &cs[0].id,
            "TEST-42",
            "https://example.test/product/dress",
            true,
        )
        .unwrap();
    assert!(matches!(c.classification, MatchClassV1::VerifiedExact));
    let look = a
        .save_look(SavedLookV1 {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project.id.clone(),
            name: "Look <script>".into(),
            notes: "Notes & evidence".into(),
            items: vec![LookItemV1 {
                group_id: g.id.clone(),
                candidate_id: Some(c.id.clone()),
                alternative_ids: vec![],
            }],
            revision: 0,
        })
        .unwrap();
    let out = temp.path().join("exports");
    std::fs::create_dir(&out).unwrap();
    let exported = success(
        &a,
        a.export_look(&project.id, &look.id, &out, "html").unwrap(),
    );
    let plan = a.fashion_exports(&project.id).unwrap().remove(0);
    assert_eq!(plan.id, exported.job_id);
    let html = std::fs::read_to_string(Path::new(&plan.destination).join("index.html")).unwrap();
    assert!(html.contains("Look &lt;script&gt;"));
    assert!(html.contains("data:image/png;base64,"));
    let json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(Path::new(&plan.destination).join("look.json")).unwrap(),
    )
    .unwrap();
    assert!(json["items"][0]["crop"]["path"]
        .as_str()
        .unwrap()
        .starts_with("assets/"));
    drop(a);
    let a = app(&data);
    assert_eq!(a.photos(&project.id).unwrap().len(), 1);
    assert_eq!(a.looks(&project.id).unwrap()[0].id, look.id);
    success(&a, a.resume_look_export(&plan.id).unwrap());
    let output = Path::new(&plan.destination).join("look.json");
    std::fs::write(&output, b"user changed output").unwrap();
    let failed = wait(&a, a.resume_look_export(&plan.id).unwrap());
    assert_eq!(failed.error.unwrap().code, "OUTPUT_CONFLICT");
    assert_eq!(std::fs::read(&output).unwrap(), b"user changed output");
    let mut i = a.fashion_items(&project.id).unwrap().remove(0);
    i.attributes[0].value = "red".into();
    success(&a, a.save_fashion_item(i).unwrap());
    assert!(a.candidates(&g.id).unwrap().iter().all(|c| c.stale));
    assert!(a.save_look(look).is_err());
    a.delete_look(&a.looks(&project.id).unwrap()[0].id).unwrap();
    assert_eq!(
        a.fashion_exports(&project.id).unwrap().len(),
        1,
        "Immutable export plans survive deleting a look"
    );
}
#[test]
fn reviewed_images_reject_missing_consent_tampering_and_changed_observations() {
    let temp = tempfile::tempdir().unwrap();
    let a = app(&temp.path().join("data"));
    let project = a.create_project("Consent").unwrap();
    let p = photo(&a, &project.id, &temp.path().join("photo.png"));
    let request = FashionReviewRequestV1 {
        project_id: project.id.clone(),
        media: vec![MediaRefV1::Photo {
            revision: p.revision.clone(),
        }],
        item_ids: vec![],
        group_id: None,
        task: FashionTaskV1::Detect,
        model: "test-model".into(),
        account_id: Some("account-a".into()),
        runtime: None,
        port: None,
        shopping_provider: None,
    };
    let job = success(&a, a.prepare_fashion_review(request.clone()).unwrap());
    let r = a.fashion_review(&job.job_id).unwrap();
    assert_eq!(
        a.run_fashion_review(&r.id, None, false).unwrap_err().code,
        "CONSENT_REQUIRED"
    );
    std::fs::write(&r.images[0].path, b"modified").unwrap();
    assert_eq!(
        a.run_fashion_review(&r.id, None, true).unwrap_err().code,
        "ASSET_CHANGED"
    );
    a.discard_fashion_review(&r.id).unwrap();
    assert!(a.fashion_review(&r.id).is_err());
    let other = a.create_project("Other").unwrap();
    let mut cross = request;
    cross.project_id = other.id;
    let failed = wait(&a, a.prepare_fashion_review(cross).unwrap());
    assert_eq!(failed.error.unwrap().code, "MEDIA_SCOPE");
}
#[test]
fn jpeg_orientation_is_applied_and_identical_reimports_reuse_revision() {
    let temp = tempfile::tempdir().unwrap();
    let a = app(&temp.path().join("data"));
    let project = a.create_project("Photo orientation").unwrap();
    let path = temp.path().join("rotated.jpg");
    image::RgbImage::new(40, 20).save(&path).unwrap();
    let jpg = std::fs::read(&path).unwrap();
    let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut oriented = vec![0xff, 0xd8, 0xff, 0xe1];
    oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    oriented.extend_from_slice(exif);
    oriented.extend_from_slice(&jpg[2..]);
    std::fs::write(&path, &oriented).unwrap();
    success(&a, a.import_photo(&project.id, &path).unwrap());
    let first = a.photos(&project.id).unwrap().remove(0);
    assert_eq!((first.width, first.height), (20, 40));
    success(&a, a.import_photo(&project.id, &path).unwrap());
    assert_eq!(a.photos(&project.id).unwrap()[0].revision, first.revision);
}
#[test]
fn mixed_media_revisions_and_group_split_preserve_user_work() {
    let temp = tempfile::tempdir().unwrap();
    let a = app(&temp.path().join("data"));
    let project = a.create_project("Mixed media").unwrap();
    let path = temp.path().join("photo.png");
    let p = photo(&a, &project.id, &path);
    success(
        &a,
        a.save_fashion_item(item(&project.id, &p.revision)).unwrap(),
    );
    let first = a.fashion_items(&project.id).unwrap()[0].clone();
    let p2 = photo(&a, &project.id, &temp.path().join("other.webp"));
    success(
        &a,
        a.save_fashion_item(item(&project.id, &p2.revision))
            .unwrap(),
    );
    let second = a
        .fashion_items(&project.id)
        .unwrap()
        .into_iter()
        .find(|i| i.id != first.id)
        .unwrap();
    let combined = a
        .save_garment_group(GarmentGroupV1 {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project.id.clone(),
            revision: 0,
            name: "Two views".into(),
            notes: "".into(),
            item_ids: vec![first.id.clone(), second.id.clone()],
        })
        .unwrap();
    let split = a
        .save_garment_group(GarmentGroupV1 {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project.id.clone(),
            revision: 0,
            name: "Reassigned view".into(),
            notes: "".into(),
            item_ids: vec![second.id.clone()],
        })
        .unwrap();
    let groups = a.garment_groups(&project.id).unwrap();
    let old = groups.iter().find(|g| g.id == combined.id).unwrap();
    assert!(old.revision > combined.revision);
    assert_eq!(old.item_ids, vec![first.id.clone()]);
    assert_eq!(split.item_ids, vec![second.id]);
    a.delete_fashion_item(&first.id).unwrap();
    assert!(a
        .fashion_items(&project.id)
        .unwrap()
        .iter()
        .all(|i| i.id != first.id));
    a.undo_fashion_item(&first.id).unwrap();
    assert!(a
        .fashion_items(&project.id)
        .unwrap()
        .iter()
        .any(|i| i.id == first.id));
    image::RgbImage::new(60, 50).save(&path).unwrap();
    assert_eq!(
        a.media_asset(
            &project.id,
            &MediaRefV1::Photo {
                revision: p.revision.clone()
            }
        )
        .unwrap_err()
        .code,
        "SOURCE_CHANGED"
    );
    success(&a, a.import_photo(&project.id, &path).unwrap());
    let new = a
        .photos(&project.id)
        .unwrap()
        .into_iter()
        .find(|i| i.id == p.id)
        .unwrap();
    assert_ne!(new.revision, p.revision);
    assert_eq!((new.width, new.height), (60, 50));
    assert_eq!(
        serde_json::to_string(
            &a.fashion_items(&project.id)
                .unwrap()
                .iter()
                .find(|i| i.id == first.id)
                .unwrap()
                .media
        )
        .unwrap(),
        serde_json::to_string(&first.media).unwrap()
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let video = temp.path().join("mixed.mkv");
    assert!(std::process::Command::new(root.join(if cfg!(windows) {
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
        "testsrc2=size=160x120:rate=4:duration=1",
        "-c:v",
        "ffv1",
        video.to_str().unwrap()
    ])
    .status()
    .unwrap()
    .success());
    success(&a, a.import_video(&project.id, &video, None).unwrap());
    let source = a.sources(&project.id).unwrap().remove(0);
    let frame = a.preview(&source.id, "1").unwrap();
    let media = MediaRefV1::Frame {
        selection: Selection {
            source_id: source.id,
            frame_ref: frame.frame_ref,
            time_seconds: frame.time_seconds,
        },
    };
    let asset = a.media_asset(&project.id, &media).unwrap();
    assert_eq!((asset.width, asset.height), (160, 120));
    assert_eq!(a.photos(&project.id).unwrap().len(), 2);
}
#[test]
fn reviewed_task_pack_round_trip_when_reference_pack_is_supplied() {
    let Ok(manifest) = std::env::var("FRAMEIQ_TEST_TASK_PACK") else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let a = app(&temp.path().join("data"));
    assert_eq!(
        a.import_task_pack(Path::new(&manifest), false)
            .unwrap_err()
            .code,
        "MODEL_REVIEW_REQUIRED"
    );
    let imported = success(&a, a.import_task_pack(Path::new(&manifest), true).unwrap());
    assert!(a
        .fashion_capabilities()
        .unwrap()
        .iter()
        .any(|c| c.status == "experimental" && c.model_id.as_ref() == imported.result_ids.first()));
    let pack: TaskPackV2 = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    let mut retained = None;
    if pack.task == "mask" || pack.task == "hair" {
        let project = a.create_project("Real mask protocol").unwrap();
        let p = photo(&a, &project.id, &temp.path().join("mask-source.png"));
        let mut observation = item(&project.id, &p.revision);
        if pack.task == "hair" {
            observation.category = "hair".into();
        }
        success(&a, a.save_fashion_item(observation).unwrap());
        let original = a.fashion_items(&project.id).unwrap().remove(0);
        success(&a, a.refine_item_mask(&original.id).unwrap());
        let masked = a.fashion_items(&project.id).unwrap().remove(0);
        assert!(masked.corrected);
        let mask = masked.mask.unwrap();
        assert_eq!((mask.width, mask.height), (40, 30));
        assert!(Path::new(&mask.path).is_file());
        let restored = a.undo_fashion_item(&original.id).unwrap();
        assert_eq!(restored.polygon, original.polygon);
        retained = Some((project.id, original.id, restored.mask.unwrap().path));
    }
    a.remove_task_pack(&imported.result_ids[0]).unwrap();
    if let Some((project, id, mask)) = retained {
        assert!(a
            .fashion_items(&project)
            .unwrap()
            .iter()
            .any(|i| i.id == id && i.corrected));
        assert!(
            Path::new(&mask).is_file(),
            "User mask survives model removal"
        );
    }
    assert!(a
        .fashion_capabilities()
        .unwrap()
        .iter()
        .all(|c| c.model_id.as_ref() != imported.result_ids.first()));
}

#[test]
fn downloadable_catalog_is_pinned_and_requires_review() {
    let root = tempfile::tempdir().unwrap();
    let a = app(root.path());
    let packs = a.fashion_model_catalog().unwrap();
    assert_eq!(packs.len(), 4);
    for pack in packs {
        assert_eq!(pack.qualification, "reference_checked");
        assert!(pack.artifacts.iter().any(|a| a.name == "LICENSE.txt"));
        assert!(pack.artifacts.iter().any(|a| a.name == "NOTICE.txt"));
        for artifact in &pack.artifacts {
            assert!(artifact.source_url.starts_with("https://github.com/kokin-blip/FrameIQ/releases/download/fashion-packs-v1/"));
            assert_eq!(artifact.sha256.len(), 64);
        }
        assert_eq!(a.download_task_pack(&pack.id, false).unwrap_err().code, "MODEL_REVIEW_REQUIRED");
    }
    assert_eq!(a.download_task_pack("https://example.com/unreviewed.onnx", true).unwrap_err().code, "TASK_PACK");
    assert!(a.list_jobs().unwrap().is_empty());
}

#[test]
fn published_task_pack_download_round_trip_when_enabled() {
    let Ok(task) = std::env::var("FRAMEIQ_TEST_DOWNLOAD_TASK") else { return; };
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    let a = app(&data);
    let pack = a.fashion_model_catalog().unwrap().into_iter().find(|p| p.task == task).unwrap();
    if let Ok(source) = std::env::var("FRAMEIQ_TEST_PACK_DOWNLOAD_CACHE") {
        let cache = data.join("cache/fashion/task-downloads").join(&pack.id);
        std::fs::create_dir_all(&cache).unwrap();
        for artifact in &pack.artifacts {
            std::fs::hard_link(Path::new(&source).join(&artifact.name), cache.join(format!("{}.partial", artifact.sha256))).unwrap();
        }
    }
    let first = success(&a, a.download_task_pack(&pack.id, true).unwrap());
    assert_eq!(first.result_ids, vec![pack.id.clone()]);
    assert!(a.fashion_capabilities().unwrap().iter().any(|c| c.task == task && c.status == "experimental"));
    // Duplicate requests preserve the installation, require no network and leave one owned pack.
    success(&a, a.download_task_pack(&pack.id, true).unwrap());
    assert_eq!(std::fs::read_dir(data.join("cache/fashion/task-packs")).unwrap().count(), 1);
    assert!(!data.join("cache/fashion/task-downloads").join(&pack.id).exists());
    drop(a);
    let a = app(&data);
    assert!(a.fashion_capabilities().unwrap().iter().any(|c| c.model_id.as_ref() == Some(&pack.id)));
    a.remove_task_pack(&pack.id).unwrap();
}

#[test]
fn a_checksummed_package_with_failed_reference_evidence_cannot_become_ready() {
    let Ok(manifest_path) = std::env::var("FRAMEIQ_TEST_TASK_PACK") else { return; };
    use sha2::{Digest, Sha256};
    let root = tempfile::tempdir().unwrap();
    let package = root.path().join("package");
    std::fs::create_dir(&package).unwrap();
    let manifest_path = Path::new(&manifest_path);
    let mut pack: TaskPackV2 = serde_json::from_slice(&std::fs::read(manifest_path).unwrap()).unwrap();
    for artifact in &mut pack.artifacts {
        let source = manifest_path.parent().unwrap().join(&artifact.name);
        let destination = package.join(&artifact.name);
        if artifact.name == "qualification.json" {
            let mut report: serde_json::Value = serde_json::from_slice(&std::fs::read(source).unwrap()).unwrap();
            report["passed"] = false.into();
            let bytes = serde_json::to_vec(&report).unwrap();
            artifact.byte_size = bytes.len().to_string();
            artifact.sha256 = format!("{:x}", Sha256::digest(&bytes));
            std::fs::write(destination, bytes).unwrap();
        } else {
            std::fs::hard_link(source, destination).unwrap();
        }
    }
    let manifest = package.join("manifest.json");
    std::fs::write(&manifest, serde_json::to_vec(&pack).unwrap()).unwrap();
    let a = app(&root.path().join("data"));
    let failed = wait(&a, a.import_task_pack(&manifest, true).unwrap());
    assert!(matches!(failed.status, JobStatus::Failed));
    assert_eq!(failed.error.unwrap().code, "MODEL_REFERENCE");
    assert!(a.fashion_capabilities().unwrap().iter().all(|c| c.model_id.is_none()));
}
