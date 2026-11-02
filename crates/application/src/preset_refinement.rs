//! Bounded target-driven refinement around the strongest tracked regions.
use super::*;
use frameiq_analysis::tracking::{self, Association, Candidate};
impl Application {
    pub(super) fn refine_preset_regions(
        &self,
        request: &PresetExecutionV1,
        source: &SourceSummary,
        track: &mut SubjectTrackV1,
        event: &mut JobEvent,
        cancel: &AtomicBool,
    ) -> AppResult<()> {
        let regions = self.regions(&source.id)?;
        let db = lock(&self.inner.db)?;
        let features = track
            .observations
            .iter()
            .filter_map(|o| regions.iter().find(|r| r.id == o.region_id))
            .map(|r| {
                Ok((
                    QualityFrameV1 {
                        frame: r.frame.clone(),
                        metrics: r.metrics.clone(),
                    },
                    db.target_features(&r.id)?
                        .ok_or_else(|| error("Target features missing"))?,
                ))
            })
            .collect::<AppResult<Vec<_>>>()?;
        drop(db);
        let mut preset = request.preset.clone();
        preset.count = 8;
        let seeds = frameiq_analysis::target_ranking::rank(&features, &preset)?;
        let mut seen = track
            .observations
            .iter()
            .map(|o| o.frame.frame_ref.presentation_ordinal.clone())
            .collect::<std::collections::HashSet<_>>();
        // Four neighboring presentation ordinals per seed; never synthesize timestamps.
        event.total_units = Some((track.observations.len() + seeds.len() * 4).to_string());
        self.emit(event, None)?;
        for seed in seeds {
            let Some(parent) = track
                .observations
                .iter()
                .find(|o| {
                    o.frame.frame_ref.presentation_ordinal
                        == seed.quality.frame.frame_ref.presentation_ordinal
                })
                .cloned()
            else {
                continue;
            };
            let n = parent
                .frame
                .frame_ref
                .presentation_ordinal
                .parse::<i64>()
                .map_err(error)?;
            let view = self.render_task(&source.id, &n.to_string(), cancel)?;
            let parent_image = image::open(&view.image_path).map_err(error)?;
            let parent_quality = frameiq_analysis::measure(&parent_image);
            let people = self
                .person_feature(&source.id, &n.to_string(), &request.person_model_id)?
                .ok_or_else(|| error("Tracked person missing"))?;
            let Some(person) = people
                .result
                .persons
                .get(parent.subject.person_index as usize)
            else {
                continue;
            };
            let body = super::preset_execution::body_rectangle(person, view.width, view.height);
            let prior = Candidate {
                appearance: tracking::appearance(&parent_image, &body),
                rectangle: body.clone(),
            };
            for offset in [-2i64, -1, 1, 2] {
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new(
                        "CANCELLED",
                        "Refinement cancelled; completed tracked regions are retained.",
                    ));
                }
                let k = n + offset;
                if k < 0 || !seen.insert(k.to_string()) {
                    continue;
                }
                // Out-of-range neighbors are a normal end-of-source condition.
                if lock(&self.inner.db)?
                    .frame(&source.index_generation, &k.to_string())
                    .is_err()
                {
                    continue;
                }
                let view = self.render_task(&source.id, &k.to_string(), cancel)?;
                let image = image::open(&view.image_path).map_err(error)?;
                let quality = frameiq_analysis::measure(&image);
                let structure =
                    frameiq_analysis::similarity(&parent_quality.structure, &quality.structure);
                if structure < 0.40 {
                    continue;
                }
                self.wait_preset_child(
                    self.detect_persons(&source.id, &k.to_string(), &request.person_model_id)?,
                    cancel,
                )?;
                let people = self
                    .person_feature(&source.id, &k.to_string(), &request.person_model_id)?
                    .ok_or_else(|| error("Refinement person features missing"))?;
                let candidates = people
                    .result
                    .persons
                    .iter()
                    .map(|p| {
                        let rectangle =
                            super::preset_execution::body_rectangle(p, view.width, view.height);
                        Candidate {
                            appearance: tracking::appearance(&image, &rectangle),
                            rectangle,
                        }
                    })
                    .collect::<Vec<_>>();
                let Association::Matched(index) = tracking::associate(
                    &prior,
                    &candidates,
                    structure,
                    (view.time_seconds - parent.frame.time_seconds).abs(),
                ) else {
                    continue;
                };
                let selected = &candidates[index];
                let kind = &request.preset.required_capabilities;
                let mut rect = selected.rectangle.clone();
                if kind == &["region_tracking"] {
                    rect = RegionRectV1 {
                        x: rect.x + (parent.rectangle.x - body.x) / body.width * rect.width,
                        y: rect.y + (parent.rectangle.y - body.y) / body.height * rect.height,
                        width: parent.rectangle.width / body.width * rect.width,
                        height: parent.rectangle.height / body.height * rect.height,
                    };
                }
                if kind == &["face_landmarks"] {
                    let model = request
                        .face_model_id
                        .as_deref()
                        .ok_or_else(|| error("Face model missing"))?;
                    self.wait_preset_child(
                        self.detect_faces(&source.id, &k.to_string(), model)?,
                        cancel,
                    )?;
                    let faces = self
                        .face_feature(&source.id, &k.to_string(), model)?
                        .ok_or_else(|| error("Face features missing"))?;
                    let matches = faces
                        .result
                        .faces
                        .iter()
                        .filter(|f| {
                            let x = f.rectangle.x + f.rectangle.width / 2.;
                            let y = f.rectangle.y + f.rectangle.height / 2.;
                            x >= rect.x
                                && x <= rect.x + rect.width
                                && y >= rect.y
                                && y <= rect.y + rect.height
                                && f.all_landmarks_in_frame
                        })
                        .collect::<Vec<_>>();
                    if matches.len() != 1 {
                        continue;
                    }
                    rect = matches[0].rectangle.clone();
                }
                if frameiq_analysis::presets::validate_rectangle(&rect).is_err() {
                    continue;
                }
                let (rect, target) = match self.preset_target(
                    kind,
                    rect,
                    &selected.rectangle,
                    &people.id,
                    index as u32,
                    &view,
                    cancel,
                ) {
                    Ok(v) => v,
                    Err(e) if e.code == "TARGET_ABSTAIN" => continue,
                    Err(e) => return Err(e),
                };
                let x = (rect.x * view.width as f64).floor() as u32;
                let y = (rect.y * view.height as f64).floor() as u32;
                let w = ((rect.width * view.width as f64).round() as u32).min(view.width - x);
                let h = ((rect.height * view.height as f64).round() as u32).min(view.height - y);
                if w < 16 || h < 16 {
                    continue;
                }
                let frame = Selection {
                    source_id: source.id.clone(),
                    frame_ref: view.frame_ref,
                    time_seconds: view.time_seconds,
                };
                let subject = SubjectReferenceV1 {
                    person_feature_id: people.id,
                    person_index: index as u32,
                };
                let region = GuidedRegionV1 {
                    id: Uuid::new_v4().to_string(),
                    subject: Some(subject.clone()),
                    preset_id: request.preset.id.clone(),
                    frame: frame.clone(),
                    rectangle: rect.clone(),
                    metrics: frameiq_analysis::measure(&image.crop_imm(x, y, w, h).resize(
                        640,
                        640,
                        image::imageops::FilterType::Triangle,
                    )),
                    origin: "experimental_shot_tracking_v2".into(),
                    algorithm: frameiq_analysis::ALGORITHM.into(),
                    color_policy: "reference_preview_sdr".into(),
                };
                {
                    let db = lock(&self.inner.db)?;
                    let current = db.source(&source.id)?;
                    db.validate_source(&current)?;
                    if current.index_generation != source.index_generation {
                        return Err(AppError::new(
                            "SOURCE_CHANGED",
                            "The index changed during refinement.",
                        ));
                    }
                    db.save_region(&region)?;
                    db.save_target_features(&region.id, &target)?;
                }
                track.observations.push(TrackObservationV1 {
                    frame,
                    subject,
                    rectangle: rect,
                    region_id: region.id,
                });
                lock(&self.inner.db)?.save_subject_track(
                    track,
                    &request.person_model_id,
                    request.face_model_id.as_deref(),
                )?;
                event.completed_units = track.observations.len().to_string();
                self.emit(event, None)?;
            }
        }
        track.observations.sort_by_key(|o| {
            o.frame
                .frame_ref
                .presentation_ordinal
                .parse::<u64>()
                .unwrap_or(u64::MAX)
        });
        Ok(())
    }
}
