//! Experimental shot-scoped execution. Numerical model parity is not human accuracy validation.
use super::*;
use frameiq_analysis::tracking::{self, Association, Candidate};
const TRACKING: &str = "shot-region-v3";
impl Application {
    /// Poll child jobs without detaching them when the parent is cancelled.
    pub(super) fn wait_preset_child(&self, child: JobEvent, cancel: &AtomicBool) -> AppResult<()> {
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
        self.execute_preset_mode(request, false)
    }
    pub fn select_preset_frames(&self, request: PresetExecutionV1) -> AppResult<JobEvent> {
        self.execute_preset_mode(request, true)
    }
    fn execute_preset_mode(
        &self,
        request: PresetExecutionV1,
        select_results: bool,
    ) -> AppResult<JobEvent> {
        frameiq_analysis::presets::validate(&request.preset)?;
        if !request.experimental {
            return Err(AppError::new("CAPABILITY_UNVALIDATED","Automatic tracking requires explicit experimental opt-in; human accuracy gates remain open."));
        }
        let kind = request.preset.required_capabilities.as_slice();
        let supported = kind == ["person", "pose", "person_mask"]
            || kind == ["face_landmarks"]
            || kind == ["region_tracking"]
            || kind == ["head_region", "hair_mask"]
            || kind == ["accessory_region"]
            || kind == ["foot_landmarks", "footwear_region"]
            || kind == ["person", "pose", "view_orientation"];
        if !supported {
            return Err(AppError::new("CAPABILITY_UNAVAILABLE","This preset requires a hair, detailed-region, or orientation capability that has not passed its model gate."));
        }
        let required_task = if kind == ["head_region", "hair_mask"] {
            Some("hair")
        } else if kind == ["accessory_region"] || kind == ["foot_landmarks", "footwear_region"] {
            Some("detection")
        } else {
            None
        };
        let pack_dependencies = lock(&self.inner.db)?
            .task_packs()?
            .into_iter()
            .filter(|(p, _, s)| {
                s == "ready"
                    && (required_task == Some(p.task.as_str())
                        || (kind == ["person", "pose", "view_orientation"]
                            && p.task == "detection"))
            })
            .collect::<Vec<_>>();
        if required_task.is_some_and(|t| !pack_dependencies.iter().any(|(p, _, _)| p.task == t)) {
            return Err(AppError::new(
                "CAPABILITY_UNAVAILABLE",
                "Install the required hair or detailed-detection task pack in Settings.",
            ));
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
        if kind == ["person", "pose", "view_orientation"]
            && !models.iter().any(|m| {
                m.status == "ready"
                    && frameiq_inference::pose::validate_template(&m.manifest).is_ok()
            })
        {
            return Err(AppError::new(
                "CAPABILITY_UNAVAILABLE",
                "Complete Look needs a ready pose model to distinguish views.",
            ));
        }
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
        let key=format!("{:x}",Sha256::digest(serde_json::to_vec(&serde_json::json!({"algorithm":TRACKING,"quality":frameiq_analysis::ALGORITHM,"index":source.index_generation,"anchor":request.anchor_ordinal,"subject":request.person_index,"person":person.manifest.sha256,"personId":person.id,"face":face.map(|m|&m.manifest.sha256),"faceId":face.map(|m|&m.id),"kind":kind,"rectangle":request.custom_rectangle,"packs":pack_dependencies.iter().map(|(p,_,_)|(&p.id,&p.artifacts)).collect::<Vec<_>>(),"poseModels":models.iter().filter(|m|frameiq_inference::pose::validate_template(&m.manifest).is_ok()).map(|m|(&m.id,&m.manifest.sha256)).collect::<Vec<_>>(),"color":"reference_preview_sdr"})).map_err(error)?));
        let (mut event, cancel) = self.job("preset_execution", Some(&source.id))?;
        let first = event.clone();
        let app = self.clone();
        let capabilities = request.preset.required_capabilities.clone();
        std::thread::spawn(move || {
            let kind = capabilities.as_slice();
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
                    app.rank_preset_track(&request.preset, &track, &event.job_id, &cancel)?;
                    event.result_ids = vec![track.id];
                    return Ok(());
                }
                app.wait_preset_child(app.analyze(&source.id)?, &cancel)?;
                let features = app.quality_measurements(&source.id)?;
                let anchor = request.anchor_ordinal.parse::<u64>().map_err(error)?;
                // Use exact indexed identities from adaptive analysis, including refinement candidates.
                let mut frames: Vec<_> = features.into_iter().collect();
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
                let mut forward = frames
                    .iter()
                    .filter(|q| {
                        q.frame
                            .frame_ref
                            .presentation_ordinal
                            .parse::<u64>()
                            .is_ok_and(|n| n >= anchor)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let mut backward = frames
                    .iter()
                    .filter(|q| {
                        q.frame
                            .frame_ref
                            .presentation_ordinal
                            .parse::<u64>()
                            .is_ok_and(|n| n <= anchor)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                forward.sort_by_key(|q| {
                    q.frame
                        .frame_ref
                        .presentation_ordinal
                        .parse::<u64>()
                        .unwrap_or(0)
                });
                backward.sort_by_key(|q| {
                    std::cmp::Reverse(
                        q.frame
                            .frame_ref
                            .presentation_ordinal
                            .parse::<u64>()
                            .unwrap_or(0),
                    )
                });
                if backward.len() == 1 {
                    backward.clear();
                }
                let ordered_frames = forward
                    .into_iter()
                    .map(|q| (0usize, q))
                    .chain(backward.into_iter().map(|q| (1usize, q)))
                    .collect::<Vec<_>>();
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
                let mut stopped = [false, false];
                let mut active_direction = 0usize;
                let mut prior_pose: Vec<[f64; 2]> = vec![];
                let result = (|| {
                    for (direction, q) in &ordered_frames {
                        let direction = *direction;
                        if stopped[direction] {
                            continue;
                        }
                        if active_direction != direction {
                            previous = None;
                            relative = None;
                            prior_pose.clear();
                            active_direction = direction;
                        }
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
                                (q.frame.time_seconds - prior_q.frame.time_seconds).abs(),
                            ) {
                                Association::Matched(i) => i,
                                Association::Stopped(reason) => {
                                    track.stop_reason = Some(reason.into());
                                    stopped[direction] = true;
                                    continue;
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
                                stopped[direction] = true;
                                continue;
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
                                stopped[direction] = true;
                                continue;
                            }
                            inside[0].rectangle.clone()
                        } else {
                            candidate.rectangle.clone()
                        };
                        let (rect, target) = match app.preset_target(
                            kind,
                            rect,
                            &candidate.rectangle,
                            &people.id,
                            index as u32,
                            &view,
                            &cancel,
                        ) {
                            Ok(v) => v,
                            Err(e) if e.code == "TARGET_ABSTAIN" => {
                                track.stop_reason = Some("partial_target_visibility".into());
                                previous = Some((candidate, q.clone()));
                                continue;
                            }
                            Err(e) => return Err(e),
                        };
                        if !prior_pose.is_empty() && prior_pose.len() == target.pose_signature.len()
                        {
                            let delta = prior_pose
                                .iter()
                                .zip(&target.pose_signature)
                                .map(|(a, b)| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2))
                                .sum::<f64>()
                                / prior_pose.len() as f64;
                            if delta > 0.8 {
                                track.stop_reason = Some("pose_continuity_ambiguous".into());
                                stopped[direction] = true;
                                continue;
                            }
                        }
                        prior_pose = target.pose_signature.clone();
                        let x = (rect.x * view.width as f64).floor() as u32;
                        let y = (rect.y * view.height as f64).floor() as u32;
                        let w = (rect.width * view.width as f64).round() as u32;
                        let h = (rect.height * view.height as f64).round() as u32;
                        let (w, h) = (w.min(view.width - x), h.min(view.height - y));
                        if w < 16 || h < 16 {
                            track.stop_reason = Some("region_too_small".into());
                            stopped[direction] = true;
                            continue;
                        }
                        let subject = SubjectReferenceV1 {
                            person_feature_id: people.id,
                            person_index: index as u32,
                        };
                        let retained = retained_regions.iter().find(|r| {
                            r.origin == "experimental_shot_tracking_v2"
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
                            origin: "experimental_shot_tracking_v2".into(),
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
                            db.save_target_features(&region.id, &target)?;
                        }
                        if !track.observations.iter().any(|o| {
                            o.frame.frame_ref.presentation_ordinal
                                == q.frame.frame_ref.presentation_ordinal
                        }) {
                            track.observations.push(TrackObservationV1 {
                                frame: q.frame.clone(),
                                subject: region.subject.unwrap(),
                                rectangle: rect,
                                region_id: region.id,
                            });
                        }
                        lock(&app.inner.db)?.save_subject_track(
                            &track,
                            &request.person_model_id,
                            request.face_model_id.as_deref(),
                        )?;
                        lock(&app.inner.db)?.track_task_dependencies(
                            &track.id,
                            &pack_dependencies
                                .iter()
                                .map(|(p, _, _)| p.id.clone())
                                .collect::<Vec<_>>(),
                        )?;
                        let pose_dependencies = app
                            .models()?
                            .into_iter()
                            .filter(|m| {
                                m.status == "ready"
                                    && frameiq_inference::pose::validate_template(&m.manifest)
                                        .is_ok()
                            })
                            .map(|m| m.id)
                            .collect::<Vec<_>>();
                        lock(&app.inner.db)?
                            .track_pose_dependencies(&track.id, &pose_dependencies)?;
                        previous = Some((candidate, q.clone()));
                        event.completed_units = track.observations.len().to_string();
                        app.emit(&event, None)?;
                    }
                    app.refine_preset_regions(&request, &source, &mut track, &mut event, &cancel)?;
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
                    track.stop_reason = None;
                    track.coverage_complete = !stopped.iter().any(|s| *s);
                }
                if !track.observations.is_empty() {
                    lock(&app.inner.db)?.save_subject_track(
                        &track,
                        &request.person_model_id,
                        request.face_model_id.as_deref(),
                    )?;
                    app.rank_preset_track(&request.preset, &track, &event.job_id, &cancel)?;
                    event.result_ids = vec![track.id];
                }
                result
            })();
            // Selection belongs to the background job, so closing a panel does not lose it.
            let outcome = outcome.and_then(|()| {
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new("CANCELLED", "Preset selection cancelled."));
                }
                if select_results {
                    let db = lock(&app.inner.db)?;
                    let current = db.source(&source.id)?;
                    db.validate_source(&current)?;
                    if let Some(result) = db.preset_result(&event.job_id)? {
                        let frames = result
                            .frames
                            .into_iter()
                            .map(|f| f.quality.frame)
                            .collect::<Vec<_>>();
                        if cancel.load(Ordering::SeqCst) {
                            return Err(AppError::new("CANCELLED", "Preset selection cancelled."));
                        }
                        db.add_selections(&source.project_id, &frames)?;
                    }
                }
                Ok(())
            });
            app.finish_job(&mut event, outcome, "PRESET_RESULTS_READY");
        });
        Ok(first)
    }
    fn rank_preset_track(
        &self,
        preset: &PresetV1,
        track: &SubjectTrackV1,
        job: &str,
        cancel: &AtomicBool,
    ) -> AppResult<()> {
        let regions = self.regions(&track.source_id)?;
        let db = lock(&self.inner.db)?;
        let mut frames = Vec::new();
        for o in &track.observations {
            if let Some(r) = regions.iter().find(|r| r.id == o.region_id) {
                let features = db.target_features(&r.id)?.ok_or_else(|| {
                    AppError::new(
                        "TARGET_FEATURES",
                        "Required target measurements are missing. Run analysis again.",
                    )
                })?;
                frames.push((
                    QualityFrameV1 {
                        frame: r.frame.clone(),
                        metrics: r.metrics.clone(),
                    },
                    features,
                ));
            }
        }
        drop(db);
        let ranked = frameiq_analysis::target_ranking::rank(&frames, preset)?;
        let details = self.complete_look_details(preset, track, &ranked, cancel)?;
        let result=PresetResultV1{details:Some(details),job_id:job.into(),track:track.clone(),preset:preset.clone(),frames:ranked,validation_status:"experimental_unvalidated".into(),limitations:vec!["Visibility, occlusion, view orientation and tracking are experimental signals, not calibrated probabilities.".into(),"All five weights are scored. Without a compatible pose model, visibility/coverage use explicitly labeled tracked-region geometry proxies.".into(),"Tracking runs in both directions within shot boundaries and stops on subject ambiguity. Complete Look returns only supported distinct view groups.".into()]};
        lock(&self.inner.db)?.save_preset_result(&result)
    }
}
pub(super) fn body_rectangle(p: &PersonDetectionV1, w: u32, h: u32) -> RegionRectV1 {
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
