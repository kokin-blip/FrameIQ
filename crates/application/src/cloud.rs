use super::*;
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType};
pub(super) struct Prepared {
    pub review: CloudReviewV1,
    _directory: tempfile::TempDir,
    created: Instant,
}
const TTL: Duration = Duration::from_secs(600);
fn cancelled(cancel: &AtomicBool) -> AppResult<()> {
    if cancel.load(Ordering::SeqCst) {
        Err(AppError::new("CANCELLED", "Cloud preparation cancelled."))
    } else {
        Ok(())
    }
}
fn rectangle(r: &RegionRectV1) -> AppResult<()> {
    if [r.x, r.y, r.width, r.height].iter().any(|x| !x.is_finite())
        || r.x < 0.
        || r.y < 0.
        || r.x >= 1.
        || r.y >= 1.
        || r.width <= 0.
        || r.height <= 0.
        || r.x + r.width > 1.000001
        || r.y + r.height > 1.000001
    {
        return Err(AppError::new(
            "INVALID_REGION",
            "Keep the region within the image.",
        ));
    }
    Ok(())
}
impl Application {
    fn validate_cloud_frames(&self, images: &[CloudImageInputV1]) -> AppResult<()> {
        let db = lock(&self.inner.db)?;
        let mut project = None;
        let mut unique = std::collections::HashSet::new();
        for input in images {
            let s = db.source(&input.frame.source_id)?;
            db.validate_source(&s)?;
            let f = db.frame(
                &s.index_generation,
                &input.frame.frame_ref.presentation_ordinal,
            )?;
            if serde_json::to_value(db.frame_ref(&s, &f)?).map_err(error)?
                != serde_json::to_value(&input.frame.frame_ref).map_err(error)?
            {
                return Err(AppError::new(
                    "SOURCE_CHANGED",
                    "Prepare the current indexed frame again.",
                ));
            }
            if project.as_ref().is_some_and(|p| p != &s.project_id)
                || !unique.insert((
                    s.id.clone(),
                    input.frame.frame_ref.presentation_ordinal.clone(),
                ))
            {
                return Err(AppError::new(
                    "CLOUD_INPUT",
                    "Choose distinct frames from one project.",
                ));
            }
            project = Some(s.project_id);
            if let Some(r) = &input.region {
                rectangle(r)?;
            }
        }
        Ok(())
    }
    pub fn prepare_cloud_review(&self, request: CloudReviewRequestV1) -> AppResult<JobEvent> {
        frameiq_inference::cloud_vision::validate_request(
            &request.model,
            request.task,
            &request.goal,
            request.images.len(),
        )?;
        self.validate_cloud_frames(&request.images)?;
        let source = request.images[0].frame.source_id.clone();
        let (mut event, cancel) = self.job("cloud_preparing", Some(&source))?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let outcome = (|| {
                let _lane =
                    lock_cancellable(&app.inner.cloud_lane, || cancel.load(Ordering::SeqCst))?;
                cancelled(&cancel)?;
                {
                    let mut reviews = lock(&app.inner.cloud_reviews)?;
                    reviews.retain(|_, r| r.created.elapsed() < TTL);
                    if reviews.len() >= 4 {
                        return Err(AppError::new(
                            "REVIEW_LIMIT",
                            "Discard an earlier cloud review before preparing another.",
                        ));
                    }
                }
                event.status = JobStatus::Running;
                event.total_units = Some(request.images.len().to_string());
                app.emit(&event, Some(&source))?;
                let directory = tempfile::Builder::new()
                    .prefix("cloud-review-")
                    .tempdir_in(&app.inner.cache)
                    .map_err(error)?;
                let mut images = Vec::new();
                fs::write(
                    directory.path().join("FrameIQ-owned-review"),
                    b"frameiq-cloud-review-v1",
                )
                .map_err(error)?;
                for (index, input) in request.images.into_iter().enumerate() {
                    cancelled(&cancel)?;
                    let view = app.render_task(
                        &input.frame.source_id,
                        &input.frame.frame_ref.presentation_ordinal,
                        &cancel,
                    )?;
                    let mut image = image::open(&view.image_path).map_err(error)?;
                    if let Some(r) = &input.region {
                        let x = (r.x * image.width() as f64).floor() as u32;
                        let y = (r.y * image.height() as f64).floor() as u32;
                        let width = ((r.width * image.width() as f64).ceil() as u32)
                            .max(1)
                            .min(image.width() - x);
                        let height = ((r.height * image.height() as f64).ceil() as u32)
                            .max(1)
                            .min(image.height() - y);
                        image = image.crop_imm(x, y, width, height);
                    }
                    if image.width() > 1024 || image.height() > 1024 {
                        image = image.resize(1024, 1024, FilterType::Triangle);
                    }
                    let id = format!("image_{}", index + 1);
                    let path = directory.path().join(format!("{id}.jpg"));
                    let mut bytes = Vec::new();
                    JpegEncoder::new_with_quality(&mut bytes, 85)
                        .encode_image(&image)
                        .map_err(error)?;
                    if bytes.len() > 1024 * 1024 {
                        return Err(AppError::new(
                            "CLOUD_INPUT",
                            "A preview exceeds the upload limit. Use a smaller region.",
                        ));
                    }
                    fs::write(&path, &bytes).map_err(error)?;
                    images.push(CloudReviewImageV1 {
                        id,
                        input: CloudImageInputV1 {
                            frame: Selection {
                                source_id: input.frame.source_id,
                                frame_ref: view.frame_ref,
                                time_seconds: view.time_seconds,
                            },
                            region: input.region,
                        },
                        path: path.to_string_lossy().into(),
                        sha256: format!("{:x}", Sha256::digest(&bytes)),
                        width: image.width(),
                        height: image.height(),
                        bytes: bytes.len() as u32,
                    });
                    event.completed_units = (index + 1).to_string();
                    app.emit(&event, Some(&source))?;
                }
                cancelled(&cancel)?;
                app.validate_cloud_frames(
                    &images.iter().map(|i| i.input.clone()).collect::<Vec<_>>(),
                )?;
                let review = CloudReviewV1 {
                    id: event.job_id.clone(),
                    provider: request.provider,
                    model: request.model,
                    task: request.task,
                    goal: request.goal,
                    images,
                };
                lock(&app.inner.cloud_reviews)?.insert(
                    review.id.clone(),
                    Prepared {
                        review,
                        _directory: directory,
                        created: Instant::now(),
                    },
                );
                event.result_ids = vec![event.job_id.clone()];
                Ok(())
            })();
            app.finish_job(&mut event, outcome, "CLOUD_REVIEW_READY");
        });
        Ok(first)
    }
    pub fn cloud_review(&self, id: &str) -> AppResult<CloudReviewV1> {
        let mut reviews = lock(&self.inner.cloud_reviews)?;
        reviews.retain(|_, r| r.created.elapsed() < TTL);
        reviews.get(id).map(|r| r.review.clone()).ok_or_else(|| {
            AppError::new(
                "REVIEW_EXPIRED",
                "Prepare the images again; the review expired or was already used.",
            )
        })
    }
    pub fn discard_cloud_review(&self, id: &str) -> AppResult<()> {
        lock(&self.inner.cloud_reviews)?.remove(id);
        Ok(())
    }
    pub fn analyze_cloud_review(
        &self,
        id: &str,
        key: String,
        consent: bool,
    ) -> AppResult<JobEvent> {
        if !consent {
            return Err(AppError::new(
                "CONSENT_REQUIRED",
                "Review the images and approve this provider request first.",
            ));
        }
        frameiq_inference::cloud::validate_key(&key)?;
        self.cloud_review(id)?;
        let prepared = lock(&self.inner.cloud_reviews)?
            .remove(id)
            .ok_or_else(|| AppError::new("REVIEW_EXPIRED", "This review was already used."))?;
        let inputs = prepared
            .review
            .images
            .iter()
            .map(|i| i.input.clone())
            .collect::<Vec<_>>();
        self.validate_cloud_frames(&inputs)?;
        let source = inputs[0].frame.source_id.clone();
        let (mut event, cancel) = self.job("cloud_analysis", Some(&source))?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let outcome = (|| {
                let _lane =
                    lock_cancellable(&app.inner.cloud_lane, || cancel.load(Ordering::SeqCst))?;
                cancelled(&cancel)?;
                if prepared.created.elapsed() >= TTL {
                    return Err(AppError::new(
                        "REVIEW_EXPIRED",
                        "Prepare the images again; the review expired.",
                    ));
                }
                app.validate_cloud_frames(&inputs)?;
                let mut images = Vec::new();
                for input in &prepared.review.images {
                    let meta = fs::symlink_metadata(&input.path).map_err(error)?;
                    if !meta.is_file()
                        || meta.len() != input.bytes as u64
                        || meta.len() > 1024 * 1024
                    {
                        return Err(AppError::new(
                            "REVIEW_CHANGED",
                            "A reviewed image changed. Prepare it again.",
                        ));
                    }
                    let bytes = fs::read(&input.path).map_err(error)?;
                    if format!("{:x}", Sha256::digest(&bytes)) != input.sha256 {
                        return Err(AppError::new(
                            "REVIEW_CHANGED",
                            "A reviewed image changed. Prepare it again.",
                        ));
                    }
                    images.push((input.id.clone(), bytes));
                }
                event.status = JobStatus::Running;
                event.message_code = "CLOUD_REQUEST_PENDING".into();
                app.emit(&event, Some(&source))?;
                let review = &prepared.review;
                let response = frameiq_inference::cloud_vision::analyze(
                    review.provider,
                    &key,
                    &review.model,
                    review.task,
                    &review.goal,
                    &images,
                    || cancel.load(Ordering::SeqCst),
                )?;
                cancelled(&cancel)?;
                app.validate_cloud_frames(&inputs)?;
                let mut review = prepared.review.clone();
                let final_directory = app
                    .inner
                    .cache
                    .join(format!("cloud-result-images-{}", event.job_id));
                fs::rename(prepared._directory.path(), &final_directory).map_err(error)?;
                let _ = prepared._directory.keep();
                for image in &mut review.images {
                    image.path = final_directory
                        .join(format!("{}.jpg", image.id))
                        .to_string_lossy()
                        .into();
                }
                let result = CloudAnalysisResultV1 {
                    job_id: event.job_id.clone(),
                    review,
                    assessments: response.assessments,
                    actual_model: response.model,
                    input_tokens: response.input_tokens,
                    output_tokens: response.output_tokens,
                };
                let path = app
                    .inner
                    .cache
                    .join(format!("cloud-result-{}.json", event.job_id));
                let mut file = tempfile::NamedTempFile::new_in(&app.inner.cache).map_err(error)?;
                file.write_all(&serde_json::to_vec(&result).map_err(error)?)
                    .map_err(error)?;
                file.as_file().sync_all().map_err(error)?;
                file.persist_noclobber(path).map_err(error)?;
                event.result_ids = vec![event.job_id.clone()];
                prune_results(&app.inner.cache)?;
                Ok(())
            })();
            app.finish_job(&mut event, outcome, "CLOUD_SUGGESTIONS_READY");
        });
        Ok(first)
    }
    pub fn cloud_result(&self, id: &str) -> AppResult<Option<CloudAnalysisResultV1>> {
        Uuid::parse_str(id).map_err(|_| error("Invalid job ID"))?;
        let path = self.inner.cache.join(format!("cloud-result-{id}.json"));
        if !path.exists() {
            return Ok(None);
        }
        let meta = fs::symlink_metadata(&path).map_err(error)?;
        if !meta.is_file() || meta.len() > 2 * 1024 * 1024 {
            return Err(error("Invalid cloud result cache"));
        }
        let result: CloudAnalysisResultV1 =
            serde_json::from_slice(&fs::read(path).map_err(error)?).map_err(error)?;
        if result.job_id != id {
            return Err(error("Cloud result ID mismatch"));
        }
        frameiq_inference::cloud_vision::validate_output(
            &serde_json::json!({"assessments":result.assessments}).to_string(),
            &result
                .review
                .images
                .iter()
                .map(|i| i.id.clone())
                .collect::<Vec<_>>(),
        )?;
        self.validate_cloud_frames(
            &result
                .review
                .images
                .iter()
                .map(|i| i.input.clone())
                .collect::<Vec<_>>(),
        )?;
        Ok(Some(result))
    }
}

fn owned_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
        && fs::symlink_metadata(path.join("FrameIQ-owned-review")).is_ok_and(|m| m.is_file())
        && fs::read(path.join("FrameIQ-owned-review"))
            .is_ok_and(|b| b == b"frameiq-cloud-review-v1")
}
fn prune_results(cache: &Path) -> AppResult<()> {
    let mut results = Vec::new();
    for entry in fs::read_dir(cache).map_err(error)? {
        let e = entry.map_err(error)?;
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(id) = name.strip_prefix("cloud-result-images-") {
            if Uuid::parse_str(id).is_ok() && owned_directory(&e.path()) {
                let json = cache.join(format!("cloud-result-{id}.json"));
                if !json.is_file() {
                    fs::remove_dir_all(e.path()).map_err(error)?;
                } else {
                    results.push((
                        e.metadata().map_err(error)?.modified().map_err(error)?,
                        e.path(),
                        json,
                    ));
                }
            }
        }
    }
    results.sort_by_key(|r| r.0);
    let excess = results.len().saturating_sub(16);
    for (_, dir, json) in results.into_iter().take(excess) {
        fs::remove_file(json).map_err(error)?;
        fs::remove_dir_all(dir).map_err(error)?;
    }
    Ok(())
}
/// Startup runs under the app's instance lock and recognizes only marked app-owned directories.
pub(super) fn recover(cache: &Path) -> AppResult<()> {
    for entry in fs::read_dir(cache).map_err(error)? {
        let e = entry.map_err(error)?;
        if e.file_name().to_string_lossy().starts_with("cloud-review-")
            && owned_directory(&e.path())
        {
            fs::remove_dir_all(e.path()).map_err(error)?;
        }
    }
    prune_results(cache)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_preserves_unknown_files_and_limits_owned_result_directories() {
        let dir = tempfile::tempdir().unwrap();
        let unknown = dir.path().join("cloud-review-unknown");
        fs::create_dir(&unknown).unwrap();
        fs::write(unknown.join("user-file"), b"preserve").unwrap();
        let pending = dir.path().join("cloud-review-owned");
        fs::create_dir(&pending).unwrap();
        fs::write(
            pending.join("FrameIQ-owned-review"),
            b"frameiq-cloud-review-v1",
        )
        .unwrap();
        for _ in 0..20 {
            let id = Uuid::new_v4();
            let folder = dir.path().join(format!("cloud-result-images-{id}"));
            fs::create_dir(&folder).unwrap();
            fs::write(
                folder.join("FrameIQ-owned-review"),
                b"frameiq-cloud-review-v1",
            )
            .unwrap();
            fs::write(dir.path().join(format!("cloud-result-{id}.json")), b"{}").unwrap();
        }
        recover(dir.path()).unwrap();
        assert!(unknown.join("user-file").exists());
        assert!(!pending.exists());
        assert_eq!(
            fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e
                    .file_name()
                    .to_string_lossy()
                    .starts_with("cloud-result-images-"))
                .count(),
            16
        );
    }
}
