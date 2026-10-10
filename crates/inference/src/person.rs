//! MediaPipe person detector geometry. OpenCV Zoo reference is Apache-2.0;
//! each model's weight license remains separate. Boxes are detector regions, not body masks.
use frameiq_contracts::*;
use serde::{Deserialize, Serialize};
pub const ALGORITHM: &str = "mediapipe-person-224-v1";
const SIZE: usize = 224;
const CELLS: usize = 2254;
fn bad(message: &str) -> AppError {
    AppError::new("PERSON_MODEL_INCOMPATIBLE", message)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Geometry {
    pub width: u32,
    pub height: u32,
    pub resized_width: u32,
    pub resized_height: u32,
    pub pad_x: u32,
    pub pad_y: u32,
}
pub type Person = PersonDetectionV1;
pub fn read_image(path: &std::path::Path) -> AppResult<image::RgbImage> {
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| bad(&e.to_string()))?
        .with_guessed_format()
        .map_err(|e| bad(&e.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode().map_err(|e| bad(&e.to_string()))?.to_rgb8())
}
pub fn validate_template(m: &ModelManifestV1) -> AppResult<()> {
    if m.task != "person_detector"
        || m.input.shape != [1, 3, 224, 224]
        || m.input_layout.as_deref().unwrap_or("nchw") != "nchw"
        || m.outputs.len() != 2
        || !m
            .outputs
            .iter()
            .any(|o| o.name == "Identity:0" && o.shape == [1, 2254, 12])
        || !m
            .outputs
            .iter()
            .any(|o| o.name == "Identity_1:0" && o.shape == [1, 2254, 1])
        || m.channel_order != "rgb"
        || (m.scale - 2. / 255.).abs() > 1e-9
        || m.mean != [127.5; 3]
    {
        return Err(bad("Use the reviewed MediaPipe person FP32 template"));
    }
    Ok(())
}
pub fn prepare(image: &image::RgbImage) -> AppResult<(Vec<f32>, Geometry)> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err(bad("Image dimensions are unsupported"));
    }
    let ratio = SIZE as f64 / width.max(height) as f64;
    let rw = (width as f64 * ratio) as usize;
    let rh = (height as f64 * ratio) as usize;
    if rw == 0 || rh == 0 {
        return Err(bad("Image aspect ratio is too extreme"));
    }
    let px = (SIZE - rw) / 2;
    let py = (SIZE - rh) / 2;
    let mut data = vec![0.; 3 * SIZE * SIZE]; // padding is zero AFTER normalization
    for y in 0..rh {
        for x in 0..rw {
            // OpenCV's linear resize stores the pixel coordinate as float32 before
            // extracting its fractional coefficient; preserve that rounding order.
            let sx = (((x as f64 + 0.5) * width as f64 / rw as f64 - 0.5) as f32).max(0.);
            let sy = (((y as f64 + 0.5) * height as f64 / rh as f64 - 0.5) as f32).max(0.);
            let x0 = sx.floor() as u32;
            let y0 = sy.floor() as u32;
            let x1 = (x0 + 1).min(width - 1);
            let y1 = (y0 + 1).min(height - 1);
            let fx = sx - x0 as f32;
            let fy = sy - y0 as f32;
            for c in 0..3 {
                let p = |a, b| image.get_pixel(a, b)[c] as f32 / 255. * 2. - 1.;
                let v = (p(x0, y0) * (1. - fx) + p(x1, y0) * fx) * (1. - fy)
                    + (p(x0, y1) * (1. - fx) + p(x1, y1) * fx) * fy;
                data[c * SIZE * SIZE + (y + py) * SIZE + x + px] = v;
            }
        }
    }
    Ok((
        data,
        Geometry {
            width,
            height,
            resized_width: rw as u32,
            resized_height: rh as u32,
            pad_x: px as u32,
            pad_y: py as u32,
        },
    ))
}
pub fn anchors() -> Vec<[f64; 2]> {
    let mut a = Vec::with_capacity(CELLS);
    for (side, repeats) in [(28, 2), (14, 2), (7, 6)] {
        for y in 0..side {
            for x in 0..side {
                for _ in 0..repeats {
                    a.push([
                        (x as f64 + 0.5) / side as f64,
                        (y as f64 + 0.5) / side as f64,
                    ]);
                }
            }
        }
    }
    a
}
fn tensor<'a>(t: &'a [TensorOutput], name: &str, shape: &[i64]) -> AppResult<&'a [f32]> {
    let t = t
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| bad("Missing person output"))?;
    if t.shape != shape
        || t.values.len() != shape.iter().product::<i64>() as usize
        || t.values.iter().any(|v| !v.is_finite())
    {
        return Err(bad("Invalid person output"));
    }
    Ok(&t.values)
}
pub fn decode(t: &[TensorOutput], g: &Geometry) -> AppResult<Vec<Person>> {
    if g.width == 0 || g.height == 0 || g.resized_width == 0 || g.resized_height == 0 {
        return Err(bad("Invalid source geometry"));
    }
    let boxes = tensor(t, "Identity:0", &[1, 2254, 12])?;
    let scores = tensor(t, "Identity_1:0", &[1, 2254, 1])?;
    let scale = g.width.max(g.height) as f64;
    let ratio = 224. / scale;
    // Reference truncates source-space padding to integral pixels.
    let bias = [
        (g.pad_x as f64 / ratio).trunc(),
        (g.pad_y as f64 / ratio).trunc(),
    ];
    let mut candidates = Vec::new();
    for (i, a) in anchors().iter().enumerate() {
        let score = 1. / (1. + (-(scores[i] as f64).clamp(-100., 100.)).exp());
        if score <= 0.5 {
            continue;
        }
        let b = &boxes[i * 12..(i + 1) * 12];
        if b[2] <= 0. || b[3] <= 0. {
            continue;
        }
        let mut rect = [0.; 4];
        let mut landmarks = [[0.; 2]; 4];
        for c in 0..2 {
            rect[c] = (a[c] + (b[c] as f64 - b[c + 2] as f64 / 2.) / 224.) * scale - bias[c];
            rect[c + 2] = (a[c] + (b[c] as f64 + b[c + 2] as f64 / 2.) / 224.) * scale - bias[c];
            for k in 0..4 {
                landmarks[k][c] = (a[c] + b[4 + k * 2 + c] as f64 / 224.) * scale - bias[c];
            }
        }
        candidates.push(Person {
            detector_box: rect,
            landmarks,
            score,
        });
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Person> = Vec::new();
    for p in candidates {
        if kept
            .iter()
            .all(|k| reference_iou(&p.detector_box, &k.detector_box) <= 0.3)
        {
            kept.push(p);
        }
    }
    Ok(kept)
}
// Preserve the pinned reference's NMS behavior: it passes xyxy values to NMSBoxes
// as Rect2d fields. Do not silently substitute xywh and claim reference parity.
fn reference_iou(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    let area = |r: &[f64; 4]| r[2].max(0.) * r[3].max(0.);
    let intersection = ((a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0])).max(0.)
        * ((a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1])).max(0.);
    let union = area(a) + area(b) - intersection;
    if union <= 0. {
        0.
    } else {
        intersection / union
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padding_is_neutral_in_normalized_tensor() {
        let image = image::RgbImage::from_pixel(400, 200, image::Rgb([255, 0, 128]));
        let (t, g) = prepare(&image).unwrap();
        assert_eq!((g.pad_x, g.pad_y), (0, 56));
        assert_eq!(t[0], 0.);
        assert_eq!(t[56 * 224], 1.);
        assert_eq!(t[224 * 224 + 56 * 224], -1.);
    }
    #[test]
    fn anchors_cover_all_reviewed_strides() {
        let a = anchors();
        assert_eq!(a.len(), 2254);
        assert!((a[0][0] - 1. / 56.).abs() < 1e-9);
        assert!((a[2253][0] - 13. / 14.).abs() < 1e-9);
    }
    #[test]
    fn malformed_outputs_and_extreme_aspects_are_rejected() {
        assert!(prepare(&image::RgbImage::new(16384, 1)).is_err());
        let (_, g) = prepare(&image::RgbImage::new(224, 224)).unwrap();
        assert!(decode(&[], &g).is_err());
    }
    #[test]
    fn logits_landmarks_and_reference_duplicate_suppression_are_decoded() {
        let (_, g) = prepare(&image::RgbImage::new(224, 224)).unwrap();
        let mut boxes = vec![0.; 2254 * 12];
        let mut scores = vec![-100.; 2254];
        for i in 0..2 {
            boxes[i * 12..i * 12 + 12]
                .copy_from_slice(&[100., 100., 20., 10., 50., 60., 50., 20., 50., 40., 50., 30.]);
            scores[i] = 2.;
        }
        let mut tensors = vec![
            TensorOutput {
                name: "Identity:0".into(),
                shape: vec![1, 2254, 12],
                values: boxes,
            },
            TensorOutput {
                name: "Identity_1:0".into(),
                shape: vec![1, 2254, 1],
                values: scores,
            },
        ];
        let result = decode(&tensors, &g).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].detector_box, [94., 99., 114., 109.]);
        assert_eq!(result[0].landmarks[0], [54., 64.]);
        assert!((result[0].score - 0.8807970779).abs() < 1e-9);
        tensors[0].values[0] = f32::NAN;
        assert!(decode(&tensors, &g).is_err());
    }
}
