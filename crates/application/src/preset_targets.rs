use super::*;
impl Application {
    pub(super) fn preset_target(
        &self,
        kind: &[String],
        mut rect: RegionRectV1,
        body: &RegionRectV1,
        parent: &str,
        index: u32,
        view: &FrameView,
        cancel: &AtomicBool,
    ) -> AppResult<(RegionRectV1, TargetFeaturesV2)> {
        let hair = kind == ["head_region", "hair_mask"];
        let accessories = kind == ["accessory_region"];
        let footwear = kind == ["foot_landmarks", "footwear_region"];
        let mut mask_density = None;
        let mut provenance = "tracked_region_geometry_proxy".to_string();
        if hair {
            let path = self
                .fashion_directory("target-masks")?
                .join(format!("{}.png", Uuid::new_v4()));
            let result = self.run_task_pack_sync(
                "hair",
                &view.image_path,
                Some(RegionRectV1 {
                    x: body.x,
                    y: body.y,
                    width: body.width,
                    height: body.height * 0.55,
                }),
                Some(path.to_string_lossy().into()),
                cancel,
            )?;
            let a = result
                .mask
                .ok_or_else(|| error("Hair adapter returned no mask"))?;
            let mask = image::open(&a.path).map_err(error)?.to_luma8();
            let x0 = (body.x * view.width as f64) as u32;
            let y0 = (body.y * view.height as f64) as u32;
            let x1 = ((body.x + body.width) * view.width as f64).ceil() as u32;
            let y1 = ((body.y + body.height * 0.6) * view.height as f64).ceil() as u32;
            let mut bounds = [view.width, view.height, 0, 0];
            let mut count = 0u64;
            for y in y0..y1.min(view.height) {
                for x in x0..x1.min(view.width) {
                    if mask.get_pixel(x, y)[0] > 127 {
                        bounds[0] = bounds[0].min(x);
                        bounds[1] = bounds[1].min(y);
                        bounds[2] = bounds[2].max(x + 1);
                        bounds[3] = bounds[3].max(y + 1);
                        count += 1;
                    }
                }
            }
            let _ = fs::remove_file(path);
            if count < 64 || bounds[2] <= bounds[0] || bounds[3] <= bounds[1] {
                return Err(AppError::new(
                    "TARGET_ABSTAIN",
                    "No usable hair mask for the selected subject.",
                ));
            }
            rect = RegionRectV1 {
                x: bounds[0] as f64 / view.width as f64,
                y: bounds[1] as f64 / view.height as f64,
                width: (bounds[2] - bounds[0]) as f64 / view.width as f64,
                height: (bounds[3] - bounds[1]) as f64 / view.height as f64,
            };
            mask_density = Some(
                count as f64 / ((bounds[2] - bounds[0]) as f64 * (bounds[3] - bounds[1]) as f64),
            );
            provenance = "hair_mask_experimental".into();
        }
        if accessories || footwear {
            let result =
                self.run_task_pack_sync("detection", &view.image_path, None, None, cancel)?;
            let matches = result
                .detections
                .into_iter()
                .filter(|d| {
                    let category = if footwear {
                        d.category == "footwear"
                    } else {
                        [
                            "bag",
                            "jewelry",
                            "sunglasses",
                            "hat",
                            "hair_accessory",
                            "belt",
                            "accessory",
                        ]
                        .contains(&d.category.as_str())
                    };
                    let x = d.rectangle.x + d.rectangle.width / 2.;
                    let y = d.rectangle.y + d.rectangle.height / 2.;
                    category
                        && x >= body.x
                        && x <= body.x + body.width
                        && y >= body.y
                        && y <= body.y + body.height
                })
                .collect::<Vec<_>>();
            let Some(first) = matches.iter().max_by(|a, b| {
                (a.rectangle.width * a.rectangle.height)
                    .total_cmp(&(b.rectangle.width * b.rectangle.height))
            }) else {
                return Err(AppError::new(
                    "TARGET_ABSTAIN",
                    "The detailed detector found no supported region for this subject.",
                ));
            };
            rect = first.rectangle.clone();
            if footwear && matches.len() == 2 {
                let other = &matches[if std::ptr::eq(first, &matches[0]) {
                    1
                } else {
                    0
                }]
                .rectangle;
                let x = rect.x.min(other.x);
                let y = rect.y.min(other.y);
                let right = (rect.x + rect.width).max(other.x + other.width);
                let bottom = (rect.y + rect.height).max(other.y + other.height);
                rect = RegionRectV1 {
                    x,
                    y,
                    width: right - x,
                    height: bottom - y,
                };
            }
            provenance = "detailed_detector_region_experimental".into();
        }
        frameiq_analysis::presets::validate_rectangle(&rect)?;
        let center = ((rect.x + rect.width / 2. - 0.5).powi(2)
            + (rect.y + rect.height / 2. - 0.5).powi(2))
        .sqrt();
        let margin = rect
            .x
            .min(rect.y)
            .min(1. - rect.x - rect.width)
            .min(1. - rect.y - rect.height)
            .max(0.);
        let mut f = TargetFeaturesV2 {
            visibility: 1.,
            coverage: ((rect.width * view.width as f64).min(rect.height * view.height as f64)
                / 128.)
                .clamp(0., 1.),
            composition: ((1. - center / 0.707) * 0.8 + (margin / 0.05).min(1.) * 0.2)
                .clamp(0., 1.),
            occlusion: 0.,
            orientation: "unknown".into(),
            orientation_confidence: 0.,
            provenance,
            pose_signature: vec![],
        };
        if let Some(density) = mask_density {
            f.visibility = density.clamp(0., 1.);
        }
        let pose_model = self.models()?.into_iter().find(|m| {
            m.status == "ready" && frameiq_inference::pose::validate_template(&m.manifest).is_ok()
        });
        if let Some(model) = pose_model {
            self.wait_target_pose(parent, index, &model.id, cancel)?;
            if let Some(pose) = self
                .pose_feature(parent, index, &model.id)?
                .and_then(|p| p.result.pose)
            {
                let indices: &[usize] = if footwear {
                    &[27, 28, 29, 30, 31, 32]
                } else if kind == ["face_landmarks"] || hair {
                    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
                } else {
                    &[11, 12, 13, 14, 15, 16, 23, 24, 25, 26, 27, 28]
                };
                let visible = indices
                    .iter()
                    .filter_map(|i| pose.landmarks.get(*i))
                    .map(|p| {
                        if p.x >= 0.
                            && p.x < view.width as f64
                            && p.y >= 0.
                            && p.y < view.height as f64
                        {
                            (p.visibility * p.presence).clamp(0., 1.)
                        } else {
                            0.
                        }
                    })
                    .collect::<Vec<_>>();
                if !visible.is_empty() {
                    let observed = visible.iter().sum::<f64>() / visible.len() as f64;
                    f.visibility *= observed;
                    f.occlusion = 1. - observed;
                    f.coverage *=
                        visible.iter().filter(|v| **v >= 0.5).count() as f64 / visible.len() as f64;
                    f.provenance.push_str("+pose_visibility_signals");
                }
                for i in [11, 12, 23, 24, 27, 28] {
                    if let Some(p) = pose.landmarks.get(i) {
                        f.pose_signature.push([
                            (p.x / view.width as f64 - body.x) / body.width,
                            (p.y / view.height as f64 - body.y) / body.height,
                        ]);
                    }
                }
                if let (Some(left), Some(right)) =
                    (pose.world_landmarks.get(11), pose.world_landmarks.get(12))
                {
                    let dx = (left[0] - right[0]).abs();
                    let dz = left[2] - right[2];
                    let nose = pose
                        .landmarks
                        .first()
                        .map(|p| p.visibility * p.presence)
                        .unwrap_or(0.);
                    if dx < dz.abs() * 0.8 {
                        f.orientation = if dz > 0. { "side_left" } else { "side_right" }.into();
                        f.orientation_confidence = 0.65;
                    } else if nose >= 0.65 {
                        f.orientation = "front".into();
                        f.orientation_confidence = nose.clamp(0., 1.);
                    } else if nose <= 0.2 && f.visibility >= 0.6 {
                        f.orientation = "back".into();
                        f.orientation_confidence = 0.6;
                    }
                }
            }
        }
        Ok((rect, f))
    }
    fn wait_target_pose(
        &self,
        parent: &str,
        index: u32,
        model: &str,
        cancel: &AtomicBool,
    ) -> AppResult<()> {
        let job = self.estimate_pose(parent, index, model)?;
        loop {
            if cancel.load(Ordering::SeqCst) {
                let _ = self.cancel(&job.job_id);
            }
            let e = self.get_job(&job.job_id)?;
            match e.status {
                JobStatus::Succeeded => return Ok(()),
                JobStatus::Failed | JobStatus::Cancelled => {
                    return Err(e
                        .error
                        .unwrap_or_else(|| error("Pose estimation interrupted")))
                }
                _ => std::thread::sleep(Duration::from_millis(25)),
            }
        }
    }
}
