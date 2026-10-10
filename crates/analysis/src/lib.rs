use frameiq_contracts::*;
use image::{imageops::FilterType, DynamicImage};
pub const ALGORITHM: &str = "quality-v2-sdr640";
/// Quality estimates, not probabilities. Compare sharpness within one source/region class.
pub fn measure(image: &DynamicImage) -> QualityMetricsV1 {
    let gray = image.to_luma8();
    let (w, h) = gray.dimensions();
    let mut top = 0;
    let mut bottom = h;
    // Exclude uniform near-black letterbox rows, retaining at least half the image.
    while top < h / 4 && (0..w).all(|x| gray.get_pixel(x, top)[0] < 5) {
        top += 1;
    }
    while bottom > 3 * h / 4 && (0..w).all(|x| gray.get_pixel(x, bottom - 1)[0] < 5) {
        bottom -= 1;
    }
    let crop = image::imageops::crop_imm(&gray, 0, top, w, (bottom - top).max(1)).to_image();
    let (w, h) = crop.dimensions();
    let mut sum = 0.;
    let mut sum2 = 0.;
    let mut gx = 0.;
    let mut gy = 0.;
    let mut n = 0.;
    let mut dark = 0.;
    let mut bright = 0.;
    let mut histogram = [0u64; 256];
    for p in crop.pixels() {
        histogram[p[0] as usize] += 1;
        if p[0] <= 5 {
            dark += 1.;
        }
        if p[0] >= 250 {
            bright += 1.;
        }
    }
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let c = crop.get_pixel(x, y)[0] as f64;
            let a = crop.get_pixel(x - 1, y)[0] as f64;
            let b = crop.get_pixel(x + 1, y)[0] as f64;
            let d = crop.get_pixel(x, y - 1)[0] as f64;
            let e = crop.get_pixel(x, y + 1)[0] as f64;
            let lap = a + b + d + e - 4. * c;
            sum += lap;
            sum2 += lap * lap;
            gx += (a - b).powi(2);
            gy += (d - e).powi(2);
            n += 1.;
        }
    }
    let sharp = if n > 0. {
        (sum2 / n - (sum / n).powi(2)).max(0.)
    } else {
        0.
    };
    let anisotropy = (gx - gy).abs() / (gx + gy + 1.);
    let blur = (anisotropy * (1. - (sharp / 1000.).min(1.))).clamp(0., 1.);
    let area = (w as f64 * h as f64).max(1.);
    dark /= area;
    bright /= area;
    let small = image::imageops::resize(&crop, 32, 32, FilterType::Triangle);
    // Low-frequency DCT pHash, excluding DC from the threshold median.
    static COS: std::sync::OnceLock<[[f64; 32]; 8]> = std::sync::OnceLock::new();
    let cos = COS.get_or_init(|| {
        std::array::from_fn(|u| {
            std::array::from_fn(|x| {
                ((2 * x + 1) as f64 * u as f64 * std::f64::consts::PI / 64.).cos()
            })
        })
    });
    let mut horizontal = [[0.; 8]; 32];
    for y in 0..32 {
        for u in 0..8 {
            horizontal[y][u] = (0..32)
                .map(|x| small.get_pixel(x as u32, y as u32)[0] as f64 * cos[u][x])
                .sum();
        }
    }
    let mut coefficients = [0.; 64];
    for v in 0..8 {
        for u in 0..8 {
            coefficients[v * 8 + u] = (0..32).map(|y| horizontal[y][u] * cos[v][y]).sum();
        }
    }
    let percentile = |fraction: f64| {
        let target = (w as f64 * h as f64 * fraction).ceil() as u64;
        let mut n = 0;
        for (i, count) in histogram.iter().enumerate() {
            n += count;
            if n >= target {
                return i as f64 / 255.;
            }
        }
        1.
    };
    let mut sorted = coefficients[1..].to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[31];
    let mut hash = 0u64;
    for (i, c) in coefficients.iter().enumerate().skip(1) {
        if *c > median {
            hash |= 1 << i;
        }
    }
    let structure = image::imageops::resize(&crop, 16, 16, FilterType::Triangle)
        .pixels()
        .map(|p| p[0] as f64 / 255.)
        .collect();
    QualityMetricsV1 {
        sharpness: sharp,
        blur_estimate: blur,
        exposure: (1. - dark - bright).clamp(0., 1.),
        dark_fraction: dark,
        luminance_p05: percentile(0.05),
        luminance_p50: percentile(0.5),
        luminance_p95: percentile(0.95),
        bright_fraction: bright,
        perceptual_hash: format!("{hash:016x}"),
        structure,
    }
}
pub fn similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.;
    }
    let n = a.len() as f64;
    let ma = a.iter().sum::<f64>() / n;
    let mb = b.iter().sum::<f64>() / n;
    let va = a.iter().map(|x| (x - ma).powi(2)).sum::<f64>() / n;
    let vb = b.iter().map(|x| (x - mb).powi(2)).sum::<f64>() / n;
    let cov = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - ma) * (y - mb))
        .sum::<f64>()
        / n;
    ((2. * ma * mb + 0.0001) * (2. * cov + 0.0009)
        / ((ma * ma + mb * mb + 0.0001) * (va + vb + 0.0009)))
        .clamp(-1., 1.)
}
pub fn rank(
    frames: &[QualityFrameV1],
    config: &RankingConfigV1,
) -> AppResult<Vec<RecommendationV1>> {
    let weights = [
        config.sharpness_weight,
        config.exposure_weight,
        config.blur_weight,
    ];
    let total = weights.iter().sum::<f64>();
    if !(1..=50).contains(&config.count)
        || !total.is_finite()
        || total <= 0.
        || weights.iter().any(|x| !x.is_finite() || *x < 0.)
    {
        return Err(AppError::new(
            "INVALID_RANKING",
            "Choose 1–50 frames and finite, nonnegative weights with a positive total.",
        ));
    }
    let mut sharp: Vec<f64> = frames.iter().map(|f| f.metrics.sharpness).collect();
    sharp.sort_by(f64::total_cmp);
    let max = sharp
        .get(sharp.len().saturating_sub(1) * 95 / 100)
        .copied()
        .unwrap_or(1.)
        .max(1.);
    let mut candidates: Vec<_> = frames
        .iter()
        .map(|q| RecommendationV1 {
            quality: q.clone(),
            score: 100.
                * (q.metrics.sharpness.min(max) / max * weights[0]
                    + q.metrics.exposure * weights[1]
                    + (1. - q.metrics.blur_estimate) * weights[2])
                / total,
            reasons: vec![
                format!("Sharpness {:.0}", q.metrics.sharpness),
                format!("Unclipped pixels {:.0}%", q.metrics.exposure * 100.),
                format!("Estimated blur {:.0}%", q.metrics.blur_estimate * 100.),
            ],
        })
        .collect();
    candidates.sort_by(|a, b| {
        b.score.total_cmp(&a.score).then_with(|| {
            a.quality
                .frame
                .time_seconds
                .total_cmp(&b.quality.frame.time_seconds)
        })
    });
    let mut selected: Vec<RecommendationV1> = Vec::new();
    for candidate in candidates {
        let hash = u64::from_str_radix(&candidate.quality.metrics.perceptual_hash, 16).unwrap_or(0);
        if selected.iter().any(|s| {
            let other = u64::from_str_radix(&s.quality.metrics.perceptual_hash, 16).unwrap_or(0);
            (hash ^ other).count_ones() <= 6
                && similarity(
                    &candidate.quality.metrics.structure,
                    &s.quality.metrics.structure,
                ) >= 0.96
        }) {
            continue;
        }
        selected.push(candidate);
        if selected.len() == config.count as usize {
            break;
        }
    }
    Ok(selected)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exposed_flat_and_sharp_images_are_distinguished() {
        let flat = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            64,
            64,
            image::Rgb([128, 128, 128]),
        ));
        let black = DynamicImage::ImageRgb8(image::RgbImage::new(64, 64));
        let mut edges = image::RgbImage::new(64, 64);
        for (x, y, p) in edges.enumerate_pixels_mut() {
            let v = if (x / 4 + y / 4) % 2 == 0 { 30 } else { 220 };
            *p = image::Rgb([v, v, v]);
        }
        assert!(measure(&DynamicImage::ImageRgb8(edges)).sharpness > measure(&flat).sharpness);
        assert!(measure(&flat).exposure > measure(&black).exposure);
        assert_eq!(similarity(&[0.2, 0.8], &[0.2, 0.8]), 1.);
    }
}

pub mod presets;
pub mod tracking;
