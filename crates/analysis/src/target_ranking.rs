use frameiq_contracts::*;
/// All five displayed weights have a corresponding measurement. Signals remain experimental.
pub fn rank(
    candidates: &[(QualityFrameV1, TargetFeaturesV2)],
    preset: &PresetV1,
) -> AppResult<Vec<RecommendationV1>> {
    crate::presets::validate(preset)?;
    let mut sharp = candidates
        .iter()
        .map(|(q, _)| q.metrics.sharpness)
        .collect::<Vec<_>>();
    sharp.sort_by(f64::total_cmp);
    let max = sharp
        .get(sharp.len().saturating_sub(1) * 95 / 100)
        .copied()
        .unwrap_or(1.)
        .max(1.);
    let total = preset.weights.iter().sum::<f64>();
    let mut ranked = Vec::new();
    for (q, f) in candidates {
        if [
            f.visibility,
            f.coverage,
            f.composition,
            f.occlusion,
            f.orientation_confidence,
        ]
        .iter()
        .any(|n| !n.is_finite() || !(0.0..=1.0).contains(n))
        {
            return Err(AppError::new(
                "TARGET_FEATURES",
                "Required target features are invalid.",
            ));
        }
        let values = [
            q.metrics.sharpness.min(max) / max * (1. - q.metrics.blur_estimate),
            q.metrics.exposure,
            f.visibility * (1. - f.occlusion),
            f.coverage,
            f.composition,
        ];
        let score = 100.
            * values
                .iter()
                .zip(preset.weights)
                .map(|(v, w)| v * w)
                .sum::<f64>()
            / total;
        ranked.push((
            RecommendationV1 {
                quality: q.clone(),
                score,
                reasons: vec![
                    format!("Sharpness/blur {:.0}%", values[0] * 100.),
                    format!("Lighting {:.0}%", values[1] * 100.),
                    format!("Target visibility signal {:.0}%", values[2] * 100.),
                    format!("Coverage signal {:.0}%", values[3] * 100.),
                    format!("Composition {:.0}%", values[4] * 100.),
                    format!(
                        "View: {} ({:.0}% model signal)",
                        f.orientation,
                        f.orientation_confidence * 100.
                    ),
                    format!("Experimental measurement: {}", f.provenance),
                ],
            },
            f,
        ));
    }
    ranked.sort_by(|a, b| b.0.score.total_cmp(&a.0.score));
    let mut result: Vec<RecommendationV1> = Vec::new();
    let mut views = std::collections::HashSet::new();
    let complementary = preset
        .required_capabilities
        .iter()
        .any(|c| c == "view_orientation");
    for (r, f) in ranked {
        let hash = u64::from_str_radix(&r.quality.metrics.perceptual_hash, 16).unwrap_or(0);
        if result.iter().any(|old| {
            let other =
                u64::from_str_radix(&old.quality.metrics.perceptual_hash, 16).unwrap_or(u64::MAX);
            (hash ^ other).count_ones() <= 8
                && crate::similarity(&old.quality.metrics.structure, &r.quality.metrics.structure)
                    > 0.94
        }) {
            continue;
        }
        if complementary {
            let key = if f.orientation_confidence >= 0.6 {
                if f.coverage < 0.75 {
                    format!("detail_{}", f.orientation)
                } else {
                    f.orientation.clone()
                }
            } else {
                "unknown".into()
            };
            if !views.insert(key) {
                continue;
            }
        }
        result.push(r);
        if result.len() >= preset.count as usize {
            break;
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_weight_changes_the_order() {
        fn f(n: &str, values: [f64; 5]) -> (QualityFrameV1, TargetFeaturesV2) {
            (
                QualityFrameV1 {
                    frame: Selection {
                        source_id: "source".into(),
                        frame_ref: FrameRefV1 {
                            source_revision_id: "r".into(),
                            stream_index: 0,
                            index_generation: "i".into(),
                            presentation_ordinal: n.into(),
                            pts_ticks: None,
                            time_base_numerator: 1,
                            time_base_denominator: 1,
                            timestamp_origin: TimestampOrigin::Source,
                        },
                        time_seconds: n.parse().unwrap(),
                    },
                    metrics: QualityMetricsV1 {
                        sharpness: values[0],
                        exposure: values[1],
                        blur_estimate: 0.,
                        dark_fraction: 0.,
                        bright_fraction: 0.,
                        luminance_p05: 0.,
                        luminance_p50: 0.5,
                        luminance_p95: 1.,
                        perceptual_hash: if n == "0" {
                            "0000000000000000"
                        } else {
                            "ffffffffffffffff"
                        }
                        .into(),
                        structure: vec![0.1, 0.9],
                    },
                },
                TargetFeaturesV2 {
                    visibility: values[2],
                    coverage: values[3],
                    composition: values[4],
                    occlusion: 0.,
                    orientation: "unknown".into(),
                    orientation_confidence: 0.,
                    provenance: "fixture".into(),
                    pose_signature: vec![],
                },
            )
        }
        for k in 0..5 {
            let mut high = [0.1; 5];
            high[k] = 0.9;
            let mut low = [0.8; 5];
            low[k] = 0.2;
            let mut p = crate::presets::builtins().remove(0);
            p.weights = [0.; 5];
            p.weights[k] = 100.;
            let r = rank(&[f("0", high), f("1", low)], &p).unwrap();
            assert_eq!(
                r[0].quality.frame.frame_ref.presentation_ordinal, "0",
                "weight {k}"
            );
        }
    }
}
