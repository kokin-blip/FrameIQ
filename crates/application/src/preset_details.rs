//! Optional Complete Look detail crops, kept separate from complementary views.
use super::*;
impl Application {
    pub(super) fn complete_look_details(
        &self,
        preset: &PresetV1,
        track: &SubjectTrackV1,
        frames: &[RecommendationV1],
        cancel: &AtomicBool,
    ) -> AppResult<Vec<FashionItemV1>> {
        if cancel.load(Ordering::SeqCst) {
            return Ok(vec![]);
        }
        if !preset
            .required_capabilities
            .iter()
            .any(|c| c == "view_orientation")
        {
            return Ok(vec![]);
        }
        if !lock(&self.inner.db)?
            .task_packs()?
            .iter()
            .any(|(p, _, s)| p.task == "detection" && s == "ready")
        {
            return Ok(vec![]);
        }
        let source = lock(&self.inner.db)?.source(&track.source_id)?;
        let mut details = Vec::new();
        let mut categories = std::collections::HashSet::new();
        for frame in frames.iter().take(3) {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::new(
                    "CANCELLED",
                    "Detail crop generation cancelled.",
                ));
            }
            let Some(observation) = track.observations.iter().find(|o| {
                o.frame.frame_ref.presentation_ordinal
                    == frame.quality.frame.frame_ref.presentation_ordinal
            }) else {
                continue;
            };
            let media = MediaRefV1::Frame {
                selection: frame.quality.frame.clone(),
            };
            let image = self.media_asset_task(&source.project_id, &media, cancel)?;
            let result = self.run_task_pack_sync("detection", &image.path, None, None, cancel)?;
            for d in result.detections {
                let r = &d.rectangle;
                let body = &observation.rectangle;
                let x = r.x + r.width / 2.;
                let y = r.y + r.height / 2.;
                if x < body.x
                    || x > body.x + body.width
                    || y < body.y
                    || y > body.y + body.height
                    || r.width * r.height > body.width * body.height * 0.35
                    || !crate::fashion::CATEGORIES.contains(&d.category.as_str())
                    || !categories.insert(d.category.clone())
                {
                    continue;
                }
                let id = format!(
                    "{:x}",
                    Sha256::digest(
                        serde_json::to_vec(&(
                            &media,
                            &d.category,
                            &d.rectangle,
                            &result.model_hash,
                            "complete-look-detail-v1"
                        ))
                        .map_err(error)?
                    )
                );
                if let Ok(item) = lock(&self.inner.db)?.fashion_item(&id) {
                    if !item.deleted {
                        details.push(item);
                    }
                    continue;
                }
                let output = self
                    .fashion_directory("items")?
                    .join(format!("{}.png", Uuid::new_v4()));
                let mut crop = None;
                self.process(
                    WorkerRequest::CropPhoto {
                        path: image.path.clone(),
                        output: output.to_string_lossy().into(),
                        mask_output: None,
                        rectangle: d.rectangle.clone(),
                        polygon: vec![],
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
                let item = FashionItemV1 {
                    id,
                    project_id: source.project_id.clone(),
                    media: media.clone(),
                    revision: 0,
                    category: d.category,
                    rectangle: d.rectangle,
                    polygon: vec![],
                    crop,
                    mask: None,
                    attributes: vec![],
                    suppressed_attributes: vec![],
                    detection_score: Some(d.score),
                    provenance: "complete_look_detail_experimental".into(),
                    model_hash: Some(result.model_hash.clone()),
                    corrected: false,
                    deleted: false,
                };
                let item = lock(&self.inner.db)?.save_fashion_item(item, false)?;
                details.push(item);
                if details.len() >= 4 {
                    return Ok(details);
                }
            }
        }
        Ok(details)
    }
}
