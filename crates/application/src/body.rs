//! Indexed candidate-region jobs. Automatic preset eligibility remains separately gated.
use super::*;
const COLOR: &str = "reference_preview_sdr";
impl Application {
    pub fn choose_subject(&self, person: &str, index: u32) -> AppResult<SubjectChoiceV1> {
        lock(&self.inner.db)?.choose_subject(&SubjectReferenceV1 {
            person_feature_id: person.into(),
            person_index: index,
        })
    }
    pub fn subject_choice(&self, source: &str, n: &str) -> AppResult<Option<SubjectChoiceV1>> {
        lock(&self.inner.db)?.subject_choice(source, n)
    }
    pub fn clear_subject(&self, source: &str, n: &str) -> AppResult<()> {
        lock(&self.inner.db)?.clear_subject(source, n)
    }

    fn body_model(&self, id: &str, pose: bool) -> AppResult<ModelInstallationV1> {
        let model = self
            .models()?
            .into_iter()
            .find(|m| m.id == id && m.status == "ready")
            .ok_or_else(|| {
                AppError::new("MODEL_MISSING", "Install a compatible region model first.")
            })?;
        if pose {
            frameiq_inference::pose::validate_template(&model.manifest)?;
        } else {
            frameiq_inference::person::validate_template(&model.manifest)?;
        }
        Ok(model)
    }
    pub fn person_feature(
        &self,
        source: &str,
        n: &str,
        model: &str,
    ) -> AppResult<Option<PersonFrameV1>> {
        let model = self.body_model(model, false)?;
        let db = lock(&self.inner.db)?;
        let source = db.source(source)?;
        db.validate_source(&source)?;
        db.person_feature(
            &source,
            n,
            &model,
            frameiq_inference::person::ALGORITHM,
            COLOR,
        )
    }
    pub fn detect_persons(&self, source: &str, n: &str, model: &str) -> AppResult<JobEvent> {
        let model = self.body_model(model, false)?;
        let source = source.to_owned();
        let n = n.to_owned();
        {
            let db = lock(&self.inner.db)?;
            let s = db.source(&source)?;
            db.validate_source(&s)?;
            db.frame(&s.index_generation, &n)?;
        }
        let (mut event, cancel) = self.job("person_detection", Some(&source))?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let outcome = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, Some(&source))?;
                let _lane = lock_cancellable(&app.inner.body, || cancel.load(Ordering::SeqCst))?;
                if let Some(cached) = app.person_feature(&source, &n, &model.id)? {
                    event.result_ids = vec![cached.id];
                    return Ok(());
                }
                let view = app.render_task(&source, &n, &cancel)?;
                let response = app.infer(
                    InferenceRequestV1::DetectPersons {
                        manifest: model.manifest.clone(),
                        path: model.path,
                        image: view.image_path,
                    },
                    || cancel.load(Ordering::SeqCst),
                )?;
                let result = match response {
                    InferenceResponseV1::Persons { result } => result,
                    _ => return Err(error("Unexpected person detector response")),
                };
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new("CANCELLED", "Person detection cancelled."));
                }
                if result.model_hash != model.manifest.sha256
                    || result.algorithm != frameiq_inference::person::ALGORITHM
                    || result.width != view.width
                    || result.height != view.height
                {
                    return Err(error("Person result does not match its indexed input"));
                }
                let feature = PersonFrameV1 {
                    id: Uuid::new_v4().to_string(),
                    model_id: model.id,
                    frame: Selection {
                        source_id: source.clone(),
                        frame_ref: view.frame_ref,
                        time_seconds: view.time_seconds,
                    },
                    color_policy: COLOR.into(),
                    result,
                };
                let saved = lock(&app.inner.db)?.save_person_feature(&feature)?;
                event.result_ids = vec![saved.id];
                event.completed_units = "1".into();
                event.total_units = Some("1".into());
                Ok(())
            })();
            app.finish_job(&mut event, outcome, "PERSON_CANDIDATES_READY");
        });
        Ok(first)
    }
    pub fn pose_feature(
        &self,
        person: &str,
        index: u32,
        model: &str,
    ) -> AppResult<Option<PoseFrameV1>> {
        let model = self.body_model(model, true)?;
        let feature = {
            let db = lock(&self.inner.db)?;
            let parent = db.person_feature_by_id(person)?;
            if index as usize >= parent.result.persons.len() {
                return Err(AppError::new(
                    "INVALID_SUBJECT",
                    "Choose a detected person on this frame.",
                ));
            }
            db.pose_feature(
                person,
                index,
                &model,
                frameiq_inference::pose::ALGORITHM,
                COLOR,
            )?
        };
        if let Some(feature) = feature {
            let valid = match (
                &feature.result.pose,
                &feature.result.mask_path,
                &feature.result.mask_sha256,
            ) {
                (None, None, None) => true,
                (Some(_), Some(path), Some(hash)) => self.valid_body_mask(path, hash),
                _ => false,
            };
            if valid {
                return Ok(Some(feature));
            }
            lock(&self.inner.db)?.delete_pose_feature(&feature.id)?;
            if let Some(path) = &feature.result.mask_path {
                self.remove_body_mask(path);
            }
        }
        Ok(None)
    }
    fn valid_body_mask(&self, path: &str, checksum: &str) -> bool {
        let p = Path::new(path);
        if !self.owns_body_mask(p) {
            return false;
        }
        let Ok(meta) = fs::symlink_metadata(p) else {
            return false;
        };
        meta.is_file()
            && meta.len() <= 128 * 1024 * 1024
            && hash_file(p, || false).is_ok_and(|h| h == checksum)
    }
    fn owns_body_mask(&self, p: &Path) -> bool {
        let directory = self.inner.cache.join("pose-masks");
        p.parent() == Some(directory.as_path())
            && fs::symlink_metadata(&directory).is_ok_and(|m| m.is_dir())
    }
    pub(super) fn remove_body_mask(&self, path: &str) {
        let p = Path::new(path);
        if self.owns_body_mask(p) {
            let _ = fs::remove_file(p);
        }
    }
    pub fn estimate_pose(&self, person: &str, index: u32, model: &str) -> AppResult<JobEvent> {
        let model = self.body_model(model, true)?;
        let parent = lock(&self.inner.db)?.person_feature_by_id(person)?;
        let subject = parent
            .result
            .persons
            .get(index as usize)
            .cloned()
            .ok_or_else(|| {
                AppError::new("INVALID_SUBJECT", "Choose a detected person on this frame.")
            })?;
        let source = parent.frame.source_id.clone();
        let (mut event, cancel) = self.job("pose_estimation", Some(&source))?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let outcome = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, Some(&source))?;
                let _lane = lock_cancellable(&app.inner.body, || cancel.load(Ordering::SeqCst))?;
                if let Some(cached) = app.pose_feature(&parent.id, index, &model.id)? {
                    event.result_ids = vec![cached.id];
                    return Ok(());
                }
                let view = app.render_task(
                    &source,
                    &parent.frame.frame_ref.presentation_ordinal,
                    &cancel,
                )?;
                let directory = app.inner.cache.join("pose-masks");
                fs::create_dir_all(&directory).map_err(error)?;
                if !fs::symlink_metadata(&directory).map_err(error)?.is_dir() {
                    return Err(error("Mask cache directory must not be a symbolic link"));
                }
                let output = directory.join(format!("{}.png", Uuid::new_v4()));
                let path = output.to_string_lossy().into_owned();
                let saved = (|| {
                    let response = app.infer(
                        InferenceRequestV1::EstimatePose {
                            manifest: model.manifest.clone(),
                            path: model.path,
                            image: view.image_path,
                            person: subject,
                            mask_output: path.clone(),
                        },
                        || cancel.load(Ordering::SeqCst),
                    )?;
                    let result = match response {
                        InferenceResponseV1::Pose { result } => result,
                        _ => return Err(error("Unexpected pose response")),
                    };
                    if cancel.load(Ordering::SeqCst) {
                        return Err(AppError::new("CANCELLED", "Pose estimation cancelled."));
                    }
                    if result.model_hash != model.manifest.sha256
                        || result.algorithm != frameiq_inference::pose::ALGORITHM
                        || result.width != view.width
                        || result.height != view.height
                    {
                        return Err(error("Pose result does not match its indexed input"));
                    }
                    match (&result.pose, &result.mask_path, &result.mask_sha256) {
                        (None, None, None) if !output.exists() => {}
                        (Some(_), Some(p), Some(h)) if p == &path && app.valid_body_mask(p, h) => {}
                        _ => return Err(error("Pose mask failed asset validation")),
                    }
                    let feature = PoseFrameV1 {
                        id: Uuid::new_v4().to_string(),
                        model_id: model.id,
                        person_feature_id: parent.id,
                        person_index: index,
                        frame: Selection {
                            source_id: source.clone(),
                            frame_ref: view.frame_ref,
                            time_seconds: view.time_seconds,
                        },
                        color_policy: COLOR.into(),
                        result,
                    };
                    lock(&app.inner.db)?.save_pose_feature(&feature)?;
                    Ok(feature.id)
                })();
                if saved.is_err() {
                    app.remove_body_mask(&path);
                }
                event.result_ids = vec![saved?];
                event.completed_units = "1".into();
                event.total_units = Some("1".into());
                Ok(())
            })();
            app.finish_job(&mut event, outcome, "POSE_CANDIDATES_READY");
        });
        Ok(first)
    }
}

// Only called under the exclusive application lock before any job can start.
// Retain committed masks, unknown names, directories and symbolic links. Never
// follow a redirected cache directory or inspect original imported model files.
pub(super) fn recover_mask_orphans(cache: &Path, db: &Database) -> AppResult<()> {
    let directory = cache.join("pose-masks");
    if !directory.exists() && !directory.is_symlink() {
        return Ok(());
    }
    let Ok(metadata) = fs::symlink_metadata(&directory) else {
        return Ok(());
    };
    if !metadata.is_dir() {
        // A bad optional AI cache must not prevent offline extraction. Pose jobs
        // independently reject this directory before writing any mask.
        return Ok(());
    }
    let retained: std::collections::HashSet<PathBuf> = db
        .retained_mask_paths()?
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let Ok(entries) = fs::read_dir(&directory) else {
        return Ok(());
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if retained.contains(&path)
            || !entry.file_type().is_ok_and(|kind| kind.is_file())
            || path.extension() != Some(std::ffi::OsStr::new("png"))
        {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(id) = Uuid::parse_str(stem) else {
            continue;
        };
        if id.get_version() == Some(uuid::Version::Random) && id.to_string() == stem {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}
