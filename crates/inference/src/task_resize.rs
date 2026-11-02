//! RGB bilinear compatibility with Pillow 11.3's separable 22-bit rounding.
//! Profile: pillow_bilinear_v1. Independent Rust implementation; Python is not used at runtime.
//! Algorithm reference: python-pillow/Pillow, src/libImaging/Resample.c (11.3.0).
use image::RgbImage;
fn coefficients(input: u32, output: u32) -> Vec<(u32, Vec<i64>)> {
    let scale = input as f64 / output as f64;
    let filter = scale.max(1.);
    let support = filter;
    (0..output)
        .map(|i| {
            let center = (i as f64 + 0.5) * scale;
            let start = ((center - support + 0.5) as i64).max(0) as u32;
            let end = ((center + support + 0.5) as i64).clamp(0, input as i64) as u32;
            let weights = (start..end)
                .map(|x| (1. - ((x as f64 - center + 0.5) / filter).abs()).max(0.))
                .collect::<Vec<_>>();
            let sum = weights.iter().sum::<f64>();
            (
                start,
                weights
                    .into_iter()
                    .map(|w| (w / sum * 4194304. + 0.5).floor() as i64)
                    .collect(),
            )
        })
        .collect()
}
pub(crate) fn bilinear(source: &RgbImage, width: u32, height: u32) -> RgbImage {
    let horizontal = if width == source.width() {
        source.clone()
    } else {
        let xs = coefficients(source.width(), width);
        RgbImage::from_fn(width, source.height(), |x, y| {
            let (start, weights) = &xs[x as usize];
            let mut sums = [2097152i64; 3];
            for (offset, weight) in weights.iter().enumerate() {
                let p = source.get_pixel(*start + offset as u32, y);
                for c in 0..3 {
                    sums[c] += p[c] as i64 * weight;
                }
            }
            image::Rgb(sums.map(|s| (s >> 22).clamp(0, 255) as u8))
        })
    };
    if height == source.height() {
        return horizontal;
    }
    let ys = coefficients(source.height(), height);
    RgbImage::from_fn(width, height, |x, y| {
        let (start, weights) = &ys[y as usize];
        let mut sums = [2097152i64; 3];
        for (offset, weight) in weights.iter().enumerate() {
            let p = horizontal.get_pixel(x, *start + offset as u32);
            for c in 0..3 {
                sums[c] += p[c] as i64 * weight;
            }
        }
        image::Rgb(sums.map(|s| (s >> 22).clamp(0, 255) as u8))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_rounding_matches_independent_pillow_fixture() {
        let source = RgbImage::from_fn(2, 2, |x, y| {
            image::Rgb([[0, 100, 200, 255][(y * 2 + x) as usize]; 3])
        });
        let resized = bilinear(&source, 3, 3);
        assert_eq!(
            resized.pixels().map(|p| p[0]).collect::<Vec<_>>(),
            [0, 50, 100, 100, 139, 178, 200, 228, 255]
        );
        assert_eq!(bilinear(&source, 2, 2), source);
    }
}
