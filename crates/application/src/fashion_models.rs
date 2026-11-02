use super::*;
use crate::fashion::CATEGORIES;
impl Application {
    pub fn fashion_capabilities(&self) -> AppResult<Vec<FeatureCapabilityV1>> {
        let packs = lock(&self.inner.db)?.task_packs()?;
        Ok(["detection","hair","mask","embedding"].into_iter().map(|task|{let ready=packs.iter().find(|(p,_,s)|p.task==task&&s=="ready");FeatureCapabilityV1{task:task.into(),status:if ready.is_some(){"experimental"}else{"unavailable"}.into(),reason:if ready.is_some(){Some("Runtime-compatible task pack; broad accuracy evaluation is deferred.".into())}else{Some("Install an optional fashion pack during setup or in Settings → Local models.".into())},model_id:ready.map(|(p,_,_)|p.id.clone())}}).collect())
    }
    pub fn fashion_model_catalog(&self) -> AppResult<Vec<TaskPackV2>> {
        [
            include_str!("../../../models/task-packs/detection.json"),
            include_str!("../../../models/task-packs/hair.json"),
            include_str!("../../../models/task-packs/mask.json"),
            include_str!("../../../models/task-packs/embedding.json"),
        ]
        .into_iter()
        .map(|s| serde_json::from_str(s).map_err(error))
        .collect()
    }
    pub fn download_task_pack(&self, id: &str, reviewed: bool) -> AppResult<JobEvent> {
        if !reviewed {
            return Err(AppError::new(
                "MODEL_REVIEW_REQUIRED",
                "Review the pack size, source and license before downloading.",
            ));
        }
        let p = self
            .fashion_model_catalog()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| {
                AppError::new("TASK_PACK", "Choose a pack from the built-in catalog.")
            })?;
        let (mut event, cancel) = self.job("task_pack_download", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let _lane = lock_cancellable(&app.inner.task_install_lane, || {
                    cancel.load(Ordering::SeqCst)
                })?;
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                if let Some((installed, directory, _)) = lock(&app.inner.db)?
                    .task_packs()?
                    .into_iter()
                    .find(|(installed, directory, status)| {
                        installed.id == p.id && status == "ready" && Path::new(directory).is_dir()
                    })
                {
                    if installed.preprocessing == p.preprocessing
                        && installed.adapter == p.adapter
                        && installed
                            .artifacts
                            .iter()
                            .find(|a| a.name.ends_with(".onnx"))
                            .map(|a| &a.sha256)
                            == p.artifacts
                                .iter()
                                .find(|a| a.name.ends_with(".onnx"))
                                .map(|a| &a.sha256)
                        && validate_reference(&installed, Path::new(&directory)).is_ok()
                    {
                        event.result_ids = vec![p.id.clone()];
                        return Ok(());
                    }
                }
                let directory = app.fashion_directory("task-downloads")?.join(&p.id);
                model_recovery::prepare_directory(&directory)?;
                let total: u64 = p
                    .artifacts
                    .iter()
                    .map(|a| a.byte_size.parse::<u64>().map_err(error))
                    .collect::<AppResult<Vec<_>>>()?
                    .iter()
                    .sum();
                let mut completed = 0u64;
                event.message_code = "TASK_PACK_DOWNLOADING".into();
                event.total_units = Some(total.to_string());
                for artifact in &p.artifacts {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(AppError::new(
                            "CANCELLED",
                            "Pack download cancelled. Retry to resume.",
                        ));
                    }
                    let mut updated = Instant::now();
                    let response = app.infer_stream(
                        InferenceRequestV1::DownloadArtifact {
                            artifact: artifact.clone(),
                            directory: directory.to_string_lossy().into(),
                        },
                        || cancel.load(Ordering::SeqCst),
                        |done, _| {
                            event.completed_units = (completed + done).to_string();
                            if updated.elapsed() > Duration::from_millis(150) {
                                app.emit(&event, None)?;
                                updated = Instant::now();
                            }
                            Ok(())
                        },
                    )?;
                    let expected = directory.join(format!("{}.partial", artifact.sha256));
                    if !matches!(response, InferenceResponseV1::Downloaded { ref path } if Path::new(path) == expected)
                    {
                        return Err(error("Unexpected artifact download response"));
                    }
                    let destination = directory.join(&artifact.name);
                    if destination.exists() || destination.is_symlink() {
                        if !fs::symlink_metadata(&destination).map_err(error)?.is_file()
                            || destination.is_symlink()
                        {
                            return Err(error("Package cache must contain regular owned files"));
                        }
                        fs::remove_file(&destination).map_err(error)?;
                    }
                    // The downloader retains its verified body for retry. A link avoids
                    // another large copy while assembling the named package files.
                    if fs::hard_link(&expected, &destination).is_err() {
                        fs::copy(&expected, &destination).map_err(error)?;
                    }
                    completed += artifact.byte_size.parse::<u64>().map_err(error)?;
                    event.completed_units = completed.to_string();
                    app.emit(&event, None)?;
                }
                app.activate_task_pack(&p, &directory, &mut event, &cancel)?;
                // Only successful activation discards resumable bodies.
                let _ = fs::remove_dir_all(&directory);
                Ok(())
            })();
            app.finish_job(&mut event, result, "TASK_PACK_READY");
        });
        Ok(first)
    }
    pub fn import_task_pack(&self, manifest: &Path, reviewed: bool) -> AppResult<JobEvent> {
        if !reviewed {
            return Err(AppError::new(
                "MODEL_REVIEW_REQUIRED",
                "Review the package license, artifacts and reference report before installing.",
            ));
        }
        if manifest.metadata().map_err(error)?.len() > 128 * 1024 {
            return Err(error("Task manifest exceeds limit"));
        }
        let p: TaskPackV2 =
            serde_json::from_slice(&fs::read(manifest).map_err(error)?).map_err(error)?;
        let origin = manifest
            .canonicalize()
            .map_err(error)?
            .parent()
            .ok_or_else(|| error("Missing package directory"))?
            .to_owned();
        let (mut event, cancel) = self.job("task_pack_import", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let _lane = lock_cancellable(&app.inner.task_install_lane, || {
                    cancel.load(Ordering::SeqCst)
                })?;
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                app.activate_task_pack(&p, &origin, &mut event, &cancel)
            })();
            app.finish_job(&mut event, result, "TASK_PACK_READY");
        });
        Ok(first)
    }
    fn activate_task_pack(
        &self,
        p: &TaskPackV2,
        origin: &Path,
        event: &mut JobEvent,
        cancel: &AtomicBool,
    ) -> AppResult<()> {
        let app = self;
        event.message_code = "TASK_PACK_VALIDATING".into();
        app.emit(event, None)?;
        validate_reference(p, origin)?;
        let root = app.fashion_directory("task-packs")?;
        let staging = tempfile::Builder::new()
            .prefix("stage-")
            .tempdir_in(&root)
            .map_err(error)?;
        for a in &p.artifacts {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::new("CANCELLED", "Task pack import cancelled."));
            }
            fs::copy(origin.join(&a.name), staging.path().join(&a.name)).map_err(error)?;
        }
        fs::write(
            staging.path().join("manifest.json"),
            serde_json::to_vec(p).map_err(error)?,
        )
        .map_err(error)?;
        let validation = app.infer(
            InferenceRequestV1::RunTaskPack {
                pack: p.clone(),
                directory: staging.path().to_string_lossy().into(),
                image: None,
                rectangle: None,
                mask_output: None,
            },
            || cancel.load(Ordering::SeqCst),
        )?;
        let expected_hash = &p
            .artifacts
            .iter()
            .find(|a| a.name.ends_with(".onnx"))
            .ok_or_else(|| error("Missing graph"))?
            .sha256;
        if !matches!(validation, InferenceResponseV1::TaskResult { ref result } if result.pack_id == p.id && &result.model_hash == expected_hash)
        {
            return Err(error(
                "The model worker did not validate the expected task pack",
            ));
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::new("CANCELLED", "Task pack import cancelled."));
        }
        let destination = root.join(Uuid::new_v4().to_string());
        event.message_code = "TASK_PACK_ACTIVATING".into();
        app.emit(event, None)?;
        fs::rename(staging.path(), &destination).map_err(error)?;
        if let Err(e) =
            lock(&app.inner.db)?.save_task_pack(&p, &destination.to_string_lossy(), "ready")
        {
            let _ = fs::remove_dir_all(destination);
            return Err(e);
        }
        event.result_ids = vec![p.id.clone()];
        Ok(())
    }
    pub fn remove_task_pack(&self, id: &str) -> AppResult<()> {
        let _lane = lock(&self.inner.task_install_lane)?;
        let entries = lock(&self.inner.db)?.task_packs()?;
        let (p, path, _) = entries
            .iter()
            .find(|(p, _, _)| p.id == id)
            .ok_or_else(|| error("Task pack unavailable"))?;
        let path = PathBuf::from(path);
        let root = self
            .fashion_directory("task-packs")?
            .canonicalize()
            .map_err(error)?;
        if path.is_symlink()
            || path.parent().and_then(|p| p.canonicalize().ok()).as_deref() != Some(root.as_path())
        {
            return Err(error("Task package path is not owned by FrameIQ"));
        }
        *lock(&self.inner.ai)? = None;
        let mut db = lock(&self.inner.db)?;
        db.save_task_pack(p, &path.to_string_lossy(), "removed")?;
        db.invalidate_task_pack(id)?;
        drop(db);
        if path.exists() {
            fs::remove_dir_all(path).map_err(error)?;
        }
        Ok(())
    }
    pub(super) fn run_task_pack_sync(
        &self,
        task: &str,
        image: &str,
        rectangle: Option<RegionRectV1>,
        mask: Option<String>,
        cancel: &AtomicBool,
    ) -> AppResult<TaskResultV2> {
        let packs = lock(&self.inner.db)?.task_packs()?;
        let (p, d, _) = packs
            .into_iter()
            .find(|(p, _, s)| p.task == task && s == "ready")
            .ok_or_else(|| {
                AppError::new(
                    "CAPABILITY_UNAVAILABLE",
                    format!("Install a reviewed {task} task pack first."),
                )
            })?;
        let expected_pack = p.id.clone();
        let expected_hash = p
            .artifacts
            .iter()
            .find(|a| a.name.ends_with(".onnx"))
            .map(|a| a.sha256.clone())
            .ok_or_else(|| error("Missing task graph"))?;
        let expected_mask = mask.clone();
        let key = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    crate::fashion::hash(Path::new(image))?,
                    &rectangle,
                    &p,
                    "task-adapter-cache-v2"
                ))
                .map_err(error)?
            )
        );
        if let Some(r) = lock(&self.inner.db)?.task_result(&key)? {
            if r.mask
                .as_ref()
                .is_none_or(|a| crate::fashion::check_asset(a).is_ok())
            {
                return Ok(r);
            }
        }
        match self.infer(
            InferenceRequestV1::RunTaskPack {
                pack: p,
                directory: d,
                image: Some(image.into()),
                rectangle,
                mask_output: mask,
            },
            || cancel.load(Ordering::SeqCst),
        )? {
            InferenceResponseV1::TaskResult { mut result } => {
                if result.pack_id != expected_pack
                    || result.model_hash != expected_hash
                    || result
                        .mask
                        .as_ref()
                        .is_some_and(|a| Some(&a.path) != expected_mask.as_ref())
                    || result.embedding.len() > 4096
                    || result.embedding.iter().any(|v| !v.is_finite())
                {
                    return Err(AppError::new(
                        "TASK_OUTPUT",
                        "The task result did not match its reviewed model or output path.",
                    ));
                }
                for d in &result.detections {
                    frameiq_analysis::presets::validate_rectangle(&d.rectangle)?;
                    if !d.score.is_finite() || !(0.0..=1.0).contains(&d.score) {
                        return Err(error("Invalid detection score"));
                    }
                }
                if let Some(a) = &mut result.mask {
                    let target = self
                        .fashion_directory("task-masks")?
                        .join(format!("{}.png", Uuid::new_v4()));
                    fs::rename(&a.path, &target).map_err(error)?;
                    a.path = target.to_string_lossy().into();
                }
                lock(&self.inner.db)?.save_task_result(&key, &result)?;
                Ok(result)
            }
            _ => Err(error("Task worker returned no result")),
        }
    }
    pub fn detect_fashion_local(&self, project: &str, media: MediaRefV1) -> AppResult<JobEvent> {
        let (mut event, cancel) = self.job("fashion_detection", None)?;
        let first = event.clone();
        let app = self.clone();
        let project = project.to_string();
        std::thread::spawn(move || {
            let result = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let a = app.media_asset_task(&project, &media, &cancel)?;
                let result = app.run_task_pack_sync("detection", &a.path, None, None, &cancel)?;
                let existing = lock(&app.inner.db)?.fashion_items_for_analysis(&project)?;
                event.total_units = Some(result.detections.len().to_string());
                for d in result.detections {
                    if !CATEGORIES.contains(&d.category.as_str()) {
                        continue;
                    }
                    if cancel.load(Ordering::SeqCst) {
                        return Err(AppError::new(
                            "CANCELLED",
                            "Detection cancelled; completed items remain.",
                        ));
                    }
                    if existing.iter().any(|i| {
                        i.corrected
                            && serde_json::to_string(&i.media).ok()
                                == serde_json::to_string(&media).ok()
                            && frameiq_analysis::tracking::iou(&i.rectangle, &d.rectangle) > 0.75
                    }) {
                        continue;
                    }
                    let old = existing.iter().find(|i| {
                        serde_json::to_string(&i.media).ok() == serde_json::to_string(&media).ok()
                            && i.category == d.category
                            && frameiq_analysis::tracking::iou(&i.rectangle, &d.rectangle) > 0.75
                    });
                    if old.is_some_and(|i| i.corrected) {
                        continue;
                    }
                    let path = app
                        .fashion_directory("items")?
                        .join(format!("{}.png", Uuid::new_v4()));
                    let mut crop = None;
                    app.process(
                        WorkerRequest::CropPhoto {
                            path: a.path.clone(),
                            output: path.to_string_lossy().into(),
                            mask_output: None,
                            rectangle: d.rectangle.clone(),
                            polygon: vec![],
                        },
                        Duration::from_secs(60),
                        || cancel.load(Ordering::SeqCst),
                        |m| {
                            if let WorkerMessage::PhotoAssets { crop: a, .. } = m {
                                crop = Some(a);
                            }
                            Ok(())
                        },
                    )?;
                    let item = FashionItemV1 {
                        id: old
                            .map(|i| i.id.clone())
                            .unwrap_or_else(|| Uuid::new_v4().to_string()),
                        project_id: project.clone(),
                        media: media.clone(),
                        revision: old.map(|i| i.revision).unwrap_or(0),
                        category: d.category,
                        rectangle: d.rectangle,
                        polygon: vec![],
                        mask: None,
                        crop,
                        attributes: vec![],
                        suppressed_attributes: vec![],
                        detection_score: Some(d.score),
                        provenance: "local_detection_experimental".into(),
                        model_hash: Some(result.model_hash.clone()),
                        corrected: false,
                        deleted: false,
                    };
                    let i = lock(&app.inner.db)?.save_fashion_item(item, false)?;
                    event.result_ids.push(i.id);
                    event.completed_units = event.result_ids.len().to_string();
                    app.emit(&event, None)?;
                }
                Ok(())
            })();
            app.finish_job(&mut event, result, "FASHION_DETECTED");
        });
        Ok(first)
    }
    pub fn refine_item_mask(&self, id: &str) -> AppResult<JobEvent> {
        let item = lock(&self.inner.db)?.fashion_item(id)?;
        let (mut event, cancel) = self.job("item_mask", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let image = app.media_asset_task(&item.project_id, &item.media, &cancel)?;
                let out = app
                    .fashion_directory("items")?
                    .join(format!("{}.png", Uuid::new_v4()));
                let result = app.run_task_pack_sync(
                    if item.category == "hair" {
                        "hair"
                    } else {
                        "mask"
                    },
                    &image.path,
                    Some(item.rectangle.clone()),
                    Some(out.to_string_lossy().into()),
                    &cancel,
                )?;
                let mut latest = lock(&app.inner.db)?.fashion_item(&item.id)?;
                if latest.revision != item.revision {
                    let _ = fs::remove_file(out);
                    return Err(AppError::new(
                        "STALE_ITEM",
                        "The item changed while generating its mask. Review and try again.",
                    ));
                }
                latest.mask = result.mask;
                latest.model_hash = Some(result.model_hash);
                latest.polygon.clear();
                if let Some(mask) = &latest.mask {
                    let output = app
                        .fashion_directory("items")?
                        .join(format!("{}.png", Uuid::new_v4()));
                    let mut crop = None;
                    app.process(
                        WorkerRequest::ApplyPhotoMask {
                            path: image.path.clone(),
                            mask: mask.clone(),
                            output: output.to_string_lossy().into(),
                            rectangle: latest.rectangle.clone(),
                        },
                        Duration::from_secs(60),
                        || cancel.load(Ordering::SeqCst),
                        |m| {
                            if let WorkerMessage::PhotoAssets { crop: c, .. } = m {
                                crop = Some(c);
                            }
                            Ok(())
                        },
                    )?;
                    latest.crop = crop;
                }
                lock(&app.inner.db)?.save_fashion_item(latest, true)?;
                event.result_ids = vec![item.id.clone()];
                Ok(())
            })();
            app.finish_job(&mut event, result, "MASK_READY");
        });
        Ok(first)
    }
}

fn validate_reference(p: &TaskPackV2, origin: &Path) -> AppResult<()> {
    if !matches!(
        (p.task.as_str(), p.adapter.as_str()),
        ("detection", "grounding_dino_v1")
            | ("hair", "hair_segmenter_v1")
            | ("mask", "sam2_image_v1")
            | ("mask", "sam_image_v1")
            | ("embedding", "siglip2_image_v1")
    ) {
        return Err(AppError::new(
            "TASK_PACK",
            "The task and adapter do not match.",
        ));
    }
    frameiq_inference::task_packs::validate(p, origin)?;
    if p.qualification != "reference_checked" {
        return Err(AppError::new(
            "MODEL_REFERENCE",
            "Run the task-package reference qualification tool before activating this export.",
        ));
    }
    let report = p
        .artifacts
        .iter()
        .find(|a| a.name == "qualification.json")
        .ok_or_else(|| error("Include the hashed qualification.json artifact"))?;
    let evidence: serde_json::Value =
        serde_json::from_slice(&fs::read(origin.join(&report.name)).map_err(error)?)
            .map_err(error)?;
    if evidence["preprocessingSha256"]
        != format!("{:x}", Sha256::digest(p.preprocessing.as_bytes()))
    {
        return Err(AppError::new(
            "MODEL_REFERENCE",
            "The reference report does not match this preprocessing profile.",
        ));
    }
    if evidence["passed"] != true
        || evidence["adapter"] != p.adapter
        || evidence["modelSha256"]
            != p.artifacts
                .iter()
                .find(|a| a.name.ends_with(".onnx"))
                .unwrap()
                .sha256
        || evidence["cases"]
            .as_array()
            .is_none_or(|c| c.is_empty() || c.iter().any(|c| c["passed"] != true))
        || evidence["nativeCases"]
            .as_array()
            .is_none_or(|c| c.is_empty() || c.iter().any(|c| c["passed"] != true))
    {
        return Err(AppError::new(
            "MODEL_REFERENCE",
            "The reference report does not qualify this exported model.",
        ));
    }
    Ok(())
}

/// Under the instance lock, discard only recognizable abandoned installations.
/// Partial download bodies remain available for an explicit retry.
pub(super) fn recover(cache: &Path, db: &Database) -> AppResult<()> {
    let fashion = cache.join("fashion");
    if fashion.is_symlink() || !fashion.is_dir() {
        return Ok(());
    }
    let root = fashion.join("task-packs");
    let packs = db.task_packs()?;
    if !root.is_symlink() && root.is_dir() {
        for entry in fs::read_dir(&root).map_err(error)?.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let owned = Uuid::parse_str(&name).is_ok()
                || name
                    .strip_prefix("stage-")
                    .is_some_and(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_alphanumeric()));
            if !owned
                || path.is_symlink()
                || !path.is_dir()
                || packs
                    .iter()
                    .any(|(_, directory, status)| status == "ready" && Path::new(directory) == path)
            {
                continue;
            }
            let Ok(entries) =
                fs::read_dir(&path).and_then(|entries| entries.collect::<Result<Vec<_>, _>>())
            else {
                continue;
            };
            if entries.iter().all(|e| {
                !e.path().is_symlink()
                    && e.path().is_file()
                    && [
                        "model.onnx",
                        "qualification.json",
                        "manifest.json",
                        "LICENSE.txt",
                        "NOTICE.txt",
                    ]
                    .contains(&e.file_name().to_string_lossy().as_ref())
            }) {
                for entry in entries {
                    let _ = fs::remove_file(entry.path());
                }
                let _ = fs::remove_dir(&path);
            }
        }
    }
    let downloads = fashion.join("task-downloads");
    if !downloads.is_symlink() && downloads.is_dir() {
        for entry in fs::read_dir(downloads).map_err(error)?.flatten() {
            let path = entry.path();
            if path.is_symlink() || !path.is_dir() {
                continue;
            }
            frameiq_inference::download::cleanup_owned_checkpoints(&path);
            if let Some((pack, installed, _)) = packs.iter().find(|(p, _, status)| {
                status == "ready" && p.id == entry.file_name().to_string_lossy()
            }) {
                if validate_reference(pack, Path::new(installed)).is_ok() {
                    let _ = fs::remove_dir_all(path);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn task_recovery_preserves_partial_downloads_unknown_files_and_symlink_targets() {
        let temp = tempfile::tempdir().unwrap();
        let db = Database::open(&temp.path().join("frameiq.db")).unwrap();
        let cache = temp.path().join("cache");
        let root = cache.join("fashion/task-packs");
        let abandoned = root.join("stage-ABC123");
        fs::create_dir_all(&abandoned).unwrap();
        fs::write(abandoned.join("model.onnx"), b"incomplete").unwrap();
        let unknown = root.join(Uuid::new_v4().to_string());
        fs::create_dir_all(&unknown).unwrap();
        fs::write(unknown.join("user-notes.txt"), b"retain").unwrap();
        let partial = cache.join("fashion/task-downloads/reviewed-pack");
        fs::create_dir_all(&partial).unwrap();
        fs::write(partial.join("retained.partial"), b"resume").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&unknown, root.join("stage-DEF456")).unwrap();
        recover(&cache, &db).unwrap();
        assert!(!abandoned.exists());
        assert!(unknown.join("user-notes.txt").is_file());
        assert!(partial.join("retained.partial").is_file());
        #[cfg(unix)]
        assert!(root.join("stage-DEF456").is_symlink());
    }
}
