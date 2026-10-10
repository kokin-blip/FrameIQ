//! Experimental shot-scoped execution. Numerical model parity is not human accuracy validation.
use super::*;
use frameiq_analysis::tracking::{self, Association, Candidate};
const TRACKING: &str = "shot-region-v1";
impl Application {
    /// Poll child jobs without detaching them when the parent is cancelled.
    fn wait_preset_child(&self, child: JobEvent, cancel: &AtomicBool) -> AppResult<()> {
        let started = Instant::now();
        loop {
            if cancel.load(Ordering::SeqCst) {
                if let Err(e) = self.cancel(&child.job_id) {
                    if e.code != "JOB_NOT_RUNNING" {
                        return Err(e);
                    }
                }
            }
            let event = self.get_job(&child.job_id)?;
            match event.status {
                JobStatus::Succeeded => {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(AppError::new("CANCELLED", "Preset execution cancelled."));
                    }
                    return Ok(());
                }
                JobStatus::Failed | JobStatus::Cancelled => {
                    return Err(event.error.unwrap_or_else(|| {
                        AppError::new("CANCELLED", "Preset execution cancelled.")
                    }))
                }
                _ => {}
            }
            if started.elapsed() > Duration::from_secs(3600) {
                self.cancel(&child.job_id)?;
                return Err(AppError::new("TIMEOUT", "Preset stage timed out."));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    pub fn preset_result(&self, job: &str) -> AppResult<Option<PresetResultV1>> {
        let db = lock(&self.inner.db)?;
        let result = db.preset_result(job)?;
        if let Some(r) = &result {
            let source = db.source(&r.track.source_id)?;
            db.validate_source(&source)?;
            if source.index_generation != r.track.index_generation {
                return Err(AppError::new(
                    "SOURCE_CHANGED",
                    "Preset results belong to an earlier source index.",
                ));
            }
        }
        Ok(result)
    }
    pub fn execute_preset(&self, request: PresetExecutionV1) -> AppResult<JobEvent> {
        frameiq_analysis::presets::validate(&request.preset)?;
        if request.preset.weights[0] + request.preset.weights[1] <= 0. {
            return Err(AppError::new("INVALID_RANKING","Give sharpness or lighting a positive weight; other features are not validated yet."));
        }
        if !request.experimental {
            return Err(AppError::new("CAPABILITY_UNVALIDATED","Automatic tracking requires explicit experimental opt-in; human accuracy gates remain open."));
        }
        let kind = request.preset.required_capabilities.as_slice();
        let supported = kind == ["person", "pose", "person_mask"]
            || kind == ["face_landmarks"]
            || kind == ["region_tracking"];
        if !supported {
            return Err(AppError::new("CAPABILITY_UNAVAILABLE","This preset requires a hair, detailed-region, or orientation capability that has not passed its model gate."));
        }
        let custom = kind == ["region_tracking"];
        let makeup = kind == ["face_landmarks"];
        if custom {
            frameiq_analysis::presets::validate_rectangle(
                request.custom_rectangle.as_ref().ok_or_else(|| {
                    AppError::new("REGION_REQUIRED", "Draw a region on the anchor frame.")
                })?,
            )?;
        }
        let models = self.models()?;
        let person = models
            .iter()
            .find(|m| m.id == request.person_model_id && m.status == "ready")
            .ok_or_else(|| AppError::new("MODEL_MISSING", "Install a person detector."))?;
        frameiq_inference::person::validate_template(&person.manifest)?;
        let face = if makeup {
            let m = models
                .iter()
                .find(|m| Some(&m.id) == request.face_model_id.as_ref() && m.status == "ready")
                .ok_or_else(|| {
                    AppError::new(
                        "MODEL_MISSING",
                        "Install a face detector for Makeup Detail.",
                    )
                })?;
            frameiq_inference::face::validate_template(&m.manifest)?;
            Some(m)
        } else {
            None
        };
        let source = {
            let db = lock(&self.inner.db)?;
            let s = db.source(&request.source_id)?;
            db.validate_source(&s)?;
            db.frame(&s.index_generation, &request.anchor_ordinal)?;
            s
        };
        if source.status != "ready" {
            return Err(AppError::new(
                "INDEX_PENDING",
                "Finish indexing before running a preset.",
            ));
        }
        // Weight/count changes intentionally do not participate in the feature-cache key.
        let key=format!("{:x}",Sha256::digest(serde_json::to_vec(&serde_json::json!({"algorithm":TRACKING,"quality":frameiq_analysis::ALGORITHM,"index":source.index_generation,"anchor":request.anchor_ordinal,"subject":request.person_index,"person":person.manifest.sha256,"personId":person.id,"face":face.map(|m|&m.manifest.sha256),"faceId":face.map(|m|&m.id),"kind":kind,"rectangle":request.custom_rectangle,"color":"reference_preview_sdr"})).map_err(error)?));
        let (mut event, cancel) = self.job("preset_execution", Some(&source.id))?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let outcome = (|| {
                let _lane =
                    lock_cancellable(&app.inner.presets_lane, || cancel.load(Ordering::SeqCst))?;
                event.status = JobStatus::Running;
                app.emit(&event, Some(&source.id))?;
                let cached = {
                    let mut db = lock(&app.inner.db)?;
                    let cached = db.subject_track(&source, &key)?;
                    if let Some(t) = &cached {
                        if matches!(
                            t.stop_reason.as_deref(),
                            Some("running" | "cancelled" | "stage_failed")
                        ) {
                            db.detach_interrupted_track(&t.id)?;
                            None
                        } else {
                            cached
                        }
                    } else {
                        None
                    }
                };
                if let Some(track) = cached {
                    app.rank_preset_track(&request.preset, &track, &event.job_id)?;
                    event.result_ids = vec![track.id];
                    return Ok(());
                }
                app.wait_preset_child(app.analyze(&source.id)?, &cancel)?;
                let features = app.quality_measurements(&source.id)?;
                let anchor = request.anchor_ordinal.parse::<u64>().map_err(error)?;
                // Use exact indexed identities from adaptive analysis, including refinement candidates.
                let mut frames: Vec<_> = features
                    .into_iter()
                    .filter(|q| {
                        q.frame
                            .frame_ref
                            .presentation_ordinal
                            .parse::<u64>()
                            .is_ok_and(|n| n >= anchor)
                    })
                    .collect();
                frames.sort_by_key(|q| {
                    q.frame
                        .frame_ref
                        .presentation_ordinal
                        .parse::<u64>()
                        .unwrap_or(u64::MAX)
                });
                if !frames
                    .iter()
                    .any(|q| q.frame.frame_ref.presentation_ordinal == request.anchor_ordinal)
                {
                    let view = app.render_task(&source.id, &request.anchor_ordinal, &cancel)?;
                    frames.insert(
                        0,
                        QualityFrameV1 {
                            frame: Selection {
                                source_id: source.id.clone(),
                                frame_ref: view.frame_ref,
                                time_seconds: view.time_seconds,
                            },
                            metrics: frameiq_analysis::measure(
                                &image::open(view.image_path).map_err(error)?,
                            ),
                        },
                    );
                }
                event.total_units = Some(frames.len().to_string());
                app.emit(&event, None)?;
                let mut track = SubjectTrackV1 {
                    id: Uuid::new_v4().to_string(),
                    source_id: source.id.clone(),
                    index_generation: source.index_generation.clone(),
                    algorithm: TRACKING.into(),
                    cache_key: key,
                    observations: vec![],
                    coverage_complete: false,
                    stop_reason: Some("running".into()),
                };
                let retained_regions = app.regions(&source.id)?;
                let mut previous: Option<(Candidate, QualityFrameV1)> = None;
                let mut relative: Option<RegionRectV1> = None;
                let result = (|| {
                    for q in &frames {
                        if cancel.load(Ordering::SeqCst) {
                            return Err(AppError::new(
                                "CANCELLED",
                                "Completed preset observations are retained.",
                            ));
                        }
                        let n = &q.frame.frame_ref.presentation_ordinal;
                        app.wait_preset_child(
                            app.detect_persons(&source.id, n, &request.person_model_id)?,
                            &cancel,
                        )?;
                        let people = app
                            .person_feature(&source.id, n, &request.person_model_id)?
                            .ok_or_else(|| error("Person features missing"))?;
                        let view = app.render_task(&source.id, n, &cancel)?;
                        let image = image::open(&view.image_path).map_err(error)?;
                        if people
                            .result
                            .persons
                            .iter()
                            .any(|p| p.landmarks.iter().flatten().any(|v| !v.is_finite()))
                        {
                            return Err(error("Non-finite subject geometry"));
                        }
                        let candidates: Vec<_> = people
                            .result
                            .persons
                            .iter()
                            .map(|p| {
                                let r = body_rectangle(p, view.width, view.height);
                                Candidate {
                                    appearance: tracking::appearance(&image, &r),
                                    rectangle: r,
                                }
                            })
                            .collect();
                        let index = if let Some((prior, prior_q)) = &previous {
                            match tracking::associate(
                                prior,
                                &candidates,
                                frameiq_analysis::similarity(
                                    &prior_q.metrics.structure,
                                    &q.metrics.structure,
                                ),
                                q.frame.time_seconds - prior_q.frame.time_seconds,
                            ) {
                                Association::Matched(i) => i,
                                Association::Stopped(reason) => {
                                    track.stop_reason = Some(reason.into());
                                    break;
                                }
                            }
                        } else {
                            request.person_index as usize
                        };
                        let candidate = candidates
                            .get(index)
                            .ok_or_else(|| {
                                AppError::new(
                                    "INVALID_SUBJECT",
                                    "Choose a detected person on the anchor frame.",
                                )
                            })?
                            .clone();
                        let rect = if custom {
                            let rel = relative.get_or_insert_with(|| {
                                let r = request.custom_rectangle.as_ref().unwrap();
                                let b = &candidate.rectangle;
                                RegionRectV1 {
                                    x: (r.x - b.x) / b.width,
                                    y: (r.y - b.y) / b.height,
                                    width: r.width / b.width,
                                    height: r.height / b.height,
                                }
                            });
                            let b = &candidate.rectangle;
                            let r = RegionRectV1 {
                                x: b.x + rel.x * b.width,
                                y: b.y + rel.y * b.height,
                                width: rel.width * b.width,
                                height: rel.height * b.height,
                            };
                            if frameiq_analysis::presets::validate_rectangle(&r).is_err() {
                                track.stop_reason = Some("region_out_of_frame".into());
                                break;
                            }
                            r
                        } else if makeup {
                            let model = request.face_model_id.as_ref().unwrap();
                            app.wait_preset_child(
                                app.detect_faces(&source.id, n, model)?,
                                &cancel,
                            )?;
                            let faces = app
                                .face_feature(&source.id, n, model)?
                                .ok_or_else(|| error("Face features missing"))?;
                            let inside: Vec<_> = faces
                                .result
                                .faces
                                .iter()
                                .filter(|f| {
                                    let x = f.rectangle.x + f.rectangle.width / 2.;
                                    let y = f.rectangle.y + f.rectangle.height / 2.;
                                    let b = &candidate.rectangle;
                                    x >= b.x
                                        && x <= b.x + b.width
                                        && y >= b.y
                                        && y <= b.y + b.height
                                        && f.all_landmarks_in_frame
                                })
                                .collect();
                            if inside.len() != 1 {
                                track.stop_reason = Some("face_missing_or_ambiguous".into());
                                break;
                            }
                            inside[0].rectangle.clone()
                        } else {
                            candidate.rectangle.clone()
                        };
                        let x = (rect.x * view.width as f64).floor() as u32;
                        let y = (rect.y * view.height as f64).floor() as u32;
                        let w = (rect.width * view.width as f64).round() as u32;
                        let h = (rect.height * view.height as f64).round() as u32;
                        let (w, h) = (w.min(view.width - x), h.min(view.height - y));
                        if w < 16 || h < 16 {
                            track.stop_reason = Some("region_too_small".into());
                            break;
                        }
                        let subject = SubjectReferenceV1 {
                            person_feature_id: people.id,
                            person_index: index as u32,
                        };
                        let retained = retained_regions.iter().find(|r| {
                            r.origin == "experimental_shot_tracking"
                                && r.preset_id == request.preset.id
                                && r.frame.frame_ref.index_generation
                                    == q.frame.frame_ref.index_generation
                                && r.frame.frame_ref.presentation_ordinal
                                    == q.frame.frame_ref.presentation_ordinal
                                && r.algorithm == frameiq_analysis::ALGORITHM
                                && r.color_policy == "reference_preview_sdr"
                                && r.subject.as_ref().is_some_and(|s| {
                                    s.person_feature_id == subject.person_feature_id
                                        && s.person_index == subject.person_index
                                })
                                && r.rectangle.x == rect.x
                                && r.rectangle.y == rect.y
                                && r.rectangle.width == rect.width
                                && r.rectangle.height == rect.height
                        });
                        let reused = retained.is_some();
                        let region = retained.cloned().unwrap_or_else(|| GuidedRegionV1 {
                            id: Uuid::new_v4().to_string(),
                            subject: Some(subject),
                            preset_id: request.preset.id.clone(),
                            frame: q.frame.clone(),
                            rectangle: rect.clone(),
                            metrics: frameiq_analysis::measure(&image.crop_imm(x, y, w, h).resize(
                                640,
                                640,
                                image::imageops::FilterType::Triangle,
                            )),
                            origin: "experimental_shot_tracking".into(),
                            algorithm: frameiq_analysis::ALGORITHM.into(),
                            color_policy: "reference_preview_sdr".into(),
                        });
                        if cancel.load(Ordering::SeqCst) {
                            return Err(AppError::new("CANCELLED", "Preset execution cancelled."));
                        }
                        {
                            let db = lock(&app.inner.db)?;
                            let current = db.source(&source.id)?;
                            db.validate_source(&current)?;
                            if current.index_generation != source.index_generation {
                                return Err(AppError::new(
                                    "SOURCE_CHANGED",
                                    "Source index changed during tracking.",
                                ));
                            }
                            if !reused {
                                db.save_region(&region)?;
                            }
                        }
                        track.observations.push(TrackObservationV1 {
                            frame: q.frame.clone(),
                            subject: region.subject.unwrap(),
                            rectangle: rect,
                            region_id: region.id,
                        });
                        lock(&app.inner.db)?.save_subject_track(
                            &track,
                            &request.person_model_id,
                            request.face_model_id.as_deref(),
                        )?;
                        previous = Some((candidate, q.clone()));
                        event.completed_units = track.observations.len().to_string();
                        app.emit(&event, None)?;
                    }
                    Ok(())
                })();
                if result.is_err() {
                    track.stop_reason = Some(
                        if cancel.load(Ordering::SeqCst) {
                            "cancelled"
                        } else {
                            "stage_failed"
                        }
                        .into(),
                    );
                } else if track.stop_reason.as_deref() == Some("running") {
                    track.stop_reason = if anchor == 0 {
                        None
                    } else {
                        Some("before_anchor_not_analyzed".into())
                    };
                    track.coverage_complete = anchor == 0;
                }
                if !track.observations.is_empty() {
                    lock(&app.inner.db)?.save_subject_track(
                        &track,
                        &request.person_model_id,
                        request.face_model_id.as_deref(),
                    )?;
                    app.rank_preset_track(&request.preset, &track, &event.job_id)?;
                    event.result_ids = vec![track.id];
                }
                result
            })();
            app.finish_job(&mut event, outcome, "PRESET_RESULTS_READY");
        });
        Ok(first)
    }
    fn rank_preset_track(
        &self,
        preset: &PresetV1,
        track: &SubjectTrackV1,
        job: &str,
    ) -> AppResult<()> {
        let regions = self.regions(&track.source_id)?;
        let frames = track
            .observations
            .iter()
            .filter_map(|o| {
                regions
                    .iter()
                    .find(|r| r.id == o.region_id)
                    .map(|r| QualityFrameV1 {
                        frame: r.frame.clone(),
                        metrics: r.metrics.clone(),
                    })
            })
            .collect::<Vec<_>>();
        let result=PresetResultV1{job_id:job.into(),track:track.clone(),preset:preset.clone(),frames:frameiq_analysis::rank(&frames,&RankingConfigV1{count:preset.count,sharpness_weight:preset.weights[0],exposure_weight:preset.weights[1],blur_weight:0.})?,validation_status:"experimental_unvalidated".into(),limitations:vec!["Tracking thresholds have not passed held-out human evaluation.".into(),"Body regions use detector extent geometry, not validated clothing masks or visibility decisions. Visibility, coverage and composition weights are not scored; available quality weights are renormalized.".into(),"Tracks stop at cuts, long sampling gaps and ambiguity. Correct the subject on a new anchor to start another track.".into()]};
        lock(&self.inner.db)?.save_preset_result(&result)
    }
}
fn body_rectangle(p: &PersonDetectionV1, w: u32, h: u32) -> RegionRectV1 {
    let hip = p.landmarks[0];
    let extent = p.landmarks[1];
    let radius = ((hip[0] - extent[0]).powi(2) + (hip[1] - extent[1]).powi(2))
        .sqrt()
        .max(1.);
    let x = ((hip[0] - radius) / w as f64).clamp(0., 1. - 1. / w as f64);
    let y = ((hip[1] - radius) / h as f64).clamp(0., 1. - 1. / h as f64);
    RegionRectV1 {
        x,
        y,
        width: ((hip[0] + radius) / w as f64).clamp(x + 1. / w as f64, 1.) - x,
        height: ((hip[1] + radius) / h as f64).clamp(y + 1. / h as f64, 1.) - y,
    }
}
