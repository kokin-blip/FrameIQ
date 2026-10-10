//! YuNet 2023 decoder, following OpenCV FaceDetectorYN's stride/score/landmark contract.
//! Reference: https://github.com/opencv/opencv/blob/4.x/modules/objdetect/src/face_detect.cpp
//! OpenCV implementation is Apache-2.0; weights have their own manifest license.
use frameiq_contracts::*;
use std::path::Path;
pub const ALGORITHM: &str = "yunet-2023-letterbox-v1";
const SIZE: u32 = 640;
const SCORE: f32 = 0.6;
const NMS: f32 = 0.3;
#[derive(Debug)]
pub struct Geometry {
    pub width: u32,
    pub height: u32,
    pub resized_width: u32,
    pub resized_height: u32,
}
fn error(message: impl std::fmt::Display) -> AppError {
    AppError::new("FACE_MODEL_INCOMPATIBLE", message.to_string())
}
pub fn validate_template(m: &ModelManifestV1) -> AppResult<()> {
    if m.input_layout.as_deref().unwrap_or("nchw") != "nchw"
        || m.task != "yunet"
        || m.input.shape != [1, 3, SIZE, SIZE]
        || m.channel_order != "bgr"
        || m.scale != 1.
        || m.mean != [0.; 3]
        || m.outputs.len() != 12
    {
        return Err(error(
            "YuNet requires the reviewed 640×640 BGR float32 template.",
        ));
    }
    for stride in [8, 16, 32] {
        let cells = (SIZE / stride).pow(2);
        for (prefix, channels) in [("cls", 1), ("obj", 1), ("bbox", 4), ("kps", 10)] {
            if !m
                .outputs
                .iter()
                .any(|o| o.name == format!("{prefix}_{stride}") && o.shape == [1, cells, channels])
            {
                return Err(error(
                    "YuNet stride outputs differ from the reviewed template.",
                ));
            }
        }
    }
    Ok(())
}
pub fn prepare(path: &Path) -> AppResult<(image::RgbImage, Geometry)> {
    let mut reader = image::ImageReader::open(path)
        .map_err(error)?
        .with_guessed_format()
        .map_err(error)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let source = reader.decode().map_err(error)?.to_rgb8();
    prepare_rgb(&source)
}
pub fn prepare_rgb(source: &image::RgbImage) -> AppResult<(image::RgbImage, Geometry)> {
    let (width, height) = source.dimensions();
    if width == 0 || height == 0 {
        return Err(error("The image is empty."));
    }
    let scale = SIZE as f64 / width.max(height) as f64;
    let resized_width = ((width as f64 * scale).round() as u32).clamp(1, SIZE);
    let resized_height = ((height as f64 * scale).round() as u32).clamp(1, SIZE);
    let resized = image::imageops::resize(
        source,
        resized_width,
        resized_height,
        image::imageops::FilterType::Triangle,
    );
    let mut padded = image::RgbImage::new(SIZE, SIZE);
    image::imageops::replace(&mut padded, &resized, 0, 0);
    Ok((
        padded,
        Geometry {
            width,
            height,
            resized_width,
            resized_height,
        },
    ))
}
struct Candidate {
    box_xywh: [f32; 4],
    landmarks: [[f32; 2]; 5],
    score: f32,
}
fn tensor<'a>(
    outputs: &'a [TensorOutput],
    name: &str,
    cells: usize,
    channels: usize,
) -> AppResult<&'a [f32]> {
    let t = outputs
        .iter()
        .find(|o| o.name == name)
        .ok_or_else(|| error("Missing YuNet output."))?;
    if t.shape != [1, cells as i64, channels as i64]
        || t.values.len() != cells * channels
        || t.values.iter().any(|v| !v.is_finite())
    {
        return Err(error("YuNet output shape or values are invalid."));
    }
    Ok(&t.values)
}
// OpenCV FaceDetectorYN applies NMS to truncation-rounded integer rectangles.
fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let a = a.map(|v| v as i64 as f64);
    let b = b.map(|v| v as i64 as f64);
    let intersection = ((a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0])).max(0.)
        * ((a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1])).max(0.);
    let union = a[2] * a[3] + b[2] * b[3] - intersection;
    if union <= 0. {
        0.
    } else {
        (intersection / union) as f32
    }
}
pub fn decode(outputs: &[TensorOutput], g: &Geometry) -> AppResult<Vec<FaceDetectionV1>> {
    if g.width == 0
        || g.height == 0
        || g.resized_width == 0
        || g.resized_height == 0
        || g.resized_width > SIZE
        || g.resized_height > SIZE
    {
        return Err(error("Invalid face-image transform."));
    }
    let mut candidates = Vec::new();
    for stride in [8usize, 16, 32] {
        let cols = SIZE as usize / stride;
        let cells = cols * cols;
        let cls = tensor(outputs, &format!("cls_{stride}"), cells, 1)?;
        let obj = tensor(outputs, &format!("obj_{stride}"), cells, 1)?;
        let bbox = tensor(outputs, &format!("bbox_{stride}"), cells, 4)?;
        let kps = tensor(outputs, &format!("kps_{stride}"), cells, 10)?;
        for i in 0..cells {
            let score = (cls[i].clamp(0., 1.) * obj[i].clamp(0., 1.)).sqrt();
            if score < SCORE {
                continue;
            }
            let col = (i % cols) as f32;
            let row = (i / cols) as f32;
            let s = stride as f32;
            let cx = (col + bbox[i * 4]) * s;
            let cy = (row + bbox[i * 4 + 1]) * s;
            let w = bbox[i * 4 + 2].exp() * s;
            let h = bbox[i * 4 + 3].exp() * s;
            if !w.is_finite() || !h.is_finite() || w < 1. || h < 1. || w > 1e6 || h > 1e6 {
                continue;
            }
            let box_xywh = [cx - w / 2., cy - h / 2., w, h];
            let landmarks = std::array::from_fn(|n| {
                [
                    (col + kps[i * 10 + 2 * n]) * s,
                    (row + kps[i * 10 + 2 * n + 1]) * s,
                ]
            });
            if landmarks.iter().flatten().any(|v| !v.is_finite()) {
                continue;
            }
            candidates.push(Candidate {
                box_xywh,
                landmarks,
                score,
            });
        }
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    candidates.truncate(5000);
    let mut selected: Vec<Candidate> = Vec::new();
    for c in candidates {
        if !selected
            .iter()
            .any(|other| iou(&c.box_xywh, &other.box_xywh) > NMS)
        {
            selected.push(c);
        }
    }
    let rw = g.resized_width as f64;
    let rh = g.resized_height as f64;
    Ok(selected
        .into_iter()
        .filter_map(|c| {
            let [x, y, w, h] = c.box_xywh.map(f64::from);
            let x1 = x.max(0.).min(rw);
            let y1 = y.max(0.).min(rh);
            let x2 = (x + w).max(0.).min(rw);
            let y2 = (y + h).max(0.).min(rh);
            if x2 <= x1 || y2 <= y1 {
                return None;
            }
            let landmarks = c.landmarks.map(|p| LandmarkV1 {
                x: p[0] as f64 / rw,
                y: p[1] as f64 / rh,
            });
            let all_landmarks_in_frame = landmarks
                .iter()
                .all(|p| (0. ..=1.).contains(&p.x) && (0. ..=1.).contains(&p.y));
            Some(FaceDetectionV1 {
                rectangle: RegionRectV1 {
                    x: x1 / rw,
                    y: y1 / rh,
                    width: (x2 - x1) / rw,
                    height: (y2 - y1) / rh,
                },
                detection_score: c.score as f64,
                landmarks,
                in_frame_fraction: (((x2 - x1) * (y2 - y1)) / (w * h)).clamp(0., 1.),
                all_landmarks_in_frame,
            })
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn outputs() -> Vec<TensorOutput> {
        let manifest: ModelManifestV1 =
            serde_json::from_str(include_str!("../../../models/manifests/yunet-2023mar.json"))
                .unwrap();
        manifest
            .outputs
            .into_iter()
            .map(|o| TensorOutput {
                name: o.name,
                values: vec![0.; o.shape.iter().product::<u32>() as usize],
                shape: o.shape.into_iter().map(i64::from).collect(),
            })
            .collect()
    }
    #[test]
    fn rectangular_images_are_padded_without_stretching() {
        let source = image::RgbImage::from_pixel(1920, 1080, image::Rgb([50, 90, 120]));
        let (prepared, g) = prepare_rgb(&source).unwrap();
        assert_eq!((g.resized_width, g.resized_height), (640, 360));
        assert_eq!(prepared.get_pixel(10, 359).0, [50, 90, 120]);
        assert_eq!(prepared.get_pixel(10, 360).0, [0, 0, 0]);
    }
    #[test]
    fn decoded_landmarks_map_to_source_and_invalid_shapes_are_rejected() {
        let mut out = outputs();
        let i = 20 * 80 + 30;
        for name in ["cls_8", "obj_8"] {
            out.iter_mut().find(|o| o.name == name).unwrap().values[i] = 0.9;
        }
        let bbox = &mut out.iter_mut().find(|o| o.name == "bbox_8").unwrap().values;
        bbox[i * 4..i * 4 + 4].copy_from_slice(&[0., 0., 10f32.ln(), 10f32.ln()]);
        let g = Geometry {
            width: 1920,
            height: 1080,
            resized_width: 640,
            resized_height: 360,
        };
        let result = decode(&out, &g).unwrap();
        assert_eq!(result.len(), 1);
        assert!((result[0].rectangle.x - 200. / 640.).abs() < 1e-6);
        assert!((result[0].landmarks[0].y - 160. / 360.).abs() < 1e-6);
        assert!(result[0].all_landmarks_in_frame);
        out[0].shape[1] = 3;
        assert!(decode(&out, &g).is_err());
    }
    #[test]
    fn duplicate_boxes_are_suppressed_and_padding_only_faces_are_removed() {
        let mut out = outputs();
        for (i, dx) in [(20 * 80 + 30, 0.), (20 * 80 + 31, -1.), (70 * 80 + 30, 0.)] {
            for name in ["cls_8", "obj_8"] {
                out.iter_mut().find(|o| o.name == name).unwrap().values[i] = 0.9;
            }
            out.iter_mut().find(|o| o.name == "bbox_8").unwrap().values[i * 4..i * 4 + 4]
                .copy_from_slice(&[dx, 0., 10f32.ln(), 10f32.ln()]);
        }
        let g = Geometry {
            width: 1920,
            height: 1080,
            resized_width: 640,
            resized_height: 360,
        };
        assert_eq!(decode(&out, &g).unwrap().len(), 1);
    }
}
