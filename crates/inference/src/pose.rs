//! Detector-to-pose ROI geometry, matching the pinned OpenCV Zoo preprocessing.
//! Raster sampling and decoding follow the pinned OpenCV Zoo reference.
use crate::person::Person;
use frameiq_contracts::*;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Roi {
    pub origin: [f64; 2],
    pub size: [u32; 2],
    pub angle_radians: f64,
    pub hip_in_crop: [f64; 2],
    pub source_size: [u32; 2],
}
fn bad() -> AppError {
    AppError::new(
        "POSE_ROI_INVALID",
        "Person landmarks do not define a usable bounded pose region",
    )
}
pub const ALGORITHM: &str = "mediapipe-pose-roi-v1";
pub fn validate_template(m: &ModelManifestV1) -> AppResult<()> {
    let shapes = [
        ("Identity", vec![1, 195]),
        ("Identity_1", vec![1, 1]),
        ("Identity_2", vec![1, 256, 256, 1]),
        ("Identity_3", vec![1, 64, 64, 39]),
        ("Identity_4", vec![1, 117]),
    ];
    if m.task != "pose"
        || m.input.shape != [1, 256, 256, 3]
        || m.input_layout.as_deref() != Some("nhwc")
        || m.channel_order != "rgb"
        || (m.scale - 1. / 255.).abs() > 1e-9
        || m.mean != [0.; 3]
        || m.outputs.len() != 5
        || shapes.iter().any(|(name, shape)| {
            !m.outputs
                .iter()
                .any(|o| o.name == *name && o.shape == *shape)
        })
    {
        return Err(AppError::new(
            "POSE_MODEL_INCOMPATIBLE",
            "Use the reviewed MediaPipe NHWC FP32 pose template",
        ));
    }
    Ok(())
}
// OpenCV warpAffine uses 10-bit affine deltas and 5-bit interpolation fractions.
fn warp_point(m: &[[f64; 3]; 2], x: u32, y: u32) -> (i64, i64, f32, f32) {
    let q = |row: usize| {
        (((m[row][1] * y as f64 + m[row][2]) * 1024.).round_ties_even() as i64
            + 16
            + (m[row][0] * x as f64 * 1024.).round_ties_even() as i64)
            >> 5
    };
    let (qx, qy) = (q(0), q(1));
    (
        qx >> 5,
        qy >> 5,
        (qx & 31) as f32 / 32.,
        (qy & 31) as f32 / 32.,
    )
}
pub fn prepare(source: &image::RgbImage, r: &Roi) -> AppResult<Vec<f32>> {
    r.validate()?;
    if source.dimensions() != (r.source_size[0], r.source_size[1])
        || r.size[0] as u64 * r.size[1] as u64 > 16_777_216
    {
        return Err(bad());
    }
    let (c, s) = (r.angle_radians.cos(), r.angle_radians.sin());
    let hip = r.hip_in_crop.map(|v| v as f32 as f64);
    let inverse = [
        [c, -s, (1. - c) * hip[0] + s * hip[1]],
        [s, c, -s * hip[0] + (1. - c) * hip[1]],
    ];
    let mut rotated = image::RgbImage::new(r.size[0], r.size[1]);
    for y in 0..r.size[1] {
        for x in 0..r.size[0] {
            let (ix, iy, fx, fy) = warp_point(&inverse, x, y);
            let pixel = |a: i64, b: i64, channel: usize| -> f32 {
                if a < 0 || b < 0 || a >= r.size[0] as i64 || b >= r.size[1] as i64 {
                    return 0.;
                }
                let sx = a + r.origin[0] as i64;
                let sy = b + r.origin[1] as i64;
                if sx < 0 || sy < 0 || sx >= source.width() as i64 || sy >= source.height() as i64 {
                    0.
                } else {
                    source.get_pixel(sx as u32, sy as u32)[channel] as f32
                }
            };
            let mut p = [0; 3];
            for channel in 0..3 {
                let v = (pixel(ix, iy, channel) * (1. - fx) + pixel(ix + 1, iy, channel) * fx)
                    * (1. - fy)
                    + (pixel(ix, iy + 1, channel) * (1. - fx)
                        + pixel(ix + 1, iy + 1, channel) * fx)
                        * fy;
                p[channel] = (v + 0.5).floor().clamp(0., 255.) as u8;
            }
            rotated.put_pixel(x, y, image::Rgb(p));
        }
    }
    // INTER_AREA on an RGB8 image rounds the area-averaged value before normalization.
    let mut data = vec![0.; 256 * 256 * 3];
    for y in 0..256 {
        for x in 0..256 {
            for channel in 0..3 {
                let value = area_pixel(&rotated, x, y, channel);
                data[(y * 256 + x) * 3 + channel] =
                    value.round_ties_even().clamp(0., 255.) as f32 / 255.;
            }
        }
    }
    Ok(data)
}
fn area_pixel(image: &image::RgbImage, x: usize, y: usize, c: usize) -> f64 {
    let sx = image.width() as f64 / 256.;
    let sy = image.height() as f64 / 256.;
    let (left, right, top, bottom) = (
        x as f64 * sx,
        (x + 1) as f64 * sx,
        y as f64 * sy,
        (y + 1) as f64 * sy,
    );
    let mut sum = 0.;
    for iy in top.floor() as u32..(bottom.ceil() as u32).min(image.height()) {
        for ix in left.floor() as u32..(right.ceil() as u32).min(image.width()) {
            let weight = (right.min(ix as f64 + 1.) - left.max(ix as f64))
                * (bottom.min(iy as f64 + 1.) - top.max(iy as f64));
            sum += image.get_pixel(ix, iy)[c] as f64 * weight;
        }
    }
    sum / (sx * sy)
}
pub type Landmark = PoseLandmarkV1;
pub type Pose = PoseDetectionV1;
fn tensor<'a>(t: &'a [TensorOutput], name: &str, shape: &[i64]) -> AppResult<&'a [f32]> {
    let t = t.iter().find(|o| o.name == name).ok_or_else(bad)?;
    if t.shape != shape
        || t.values.len() != shape.iter().product::<i64>() as usize
        || t.values.iter().any(|v| !v.is_finite())
    {
        return Err(bad());
    }
    Ok(&t.values)
}
pub fn decode(t: &[TensorOutput], r: &Roi) -> AppResult<Option<Pose>> {
    r.validate()?;
    let raw = tensor(t, "Identity", &[1, 195])?;
    let score = tensor(t, "Identity_1", &[1, 1])?[0] as f64;
    tensor(t, "Identity_2", &[1, 256, 256, 1])?;
    tensor(t, "Identity_3", &[1, 64, 64, 39])?;
    let world = tensor(t, "Identity_4", &[1, 117])?;
    if !(0.0..=1.).contains(&score) {
        return Err(bad());
    }
    if score < 0.5 {
        return Ok(None);
    }
    let sigmoid = |v: f32| 1. / (1. + (-(v as f64).clamp(-100., 100.)).exp());
    let landmarks: Vec<_> = raw
        .chunks_exact(5)
        .map(|p| {
            let xy = r.tensor_to_source([p[0] as f64, p[1] as f64]);
            Landmark {
                x: xy[0],
                y: xy[1],
                z: p[2] as f64 * r.size[0].max(r.size[1]) as f64 / 256.,
                visibility: sigmoid(p[3]),
                presence: sigmoid(p[4]),
            }
        })
        .collect();
    let (c, s) = (r.angle_radians.cos(), r.angle_radians.sin());
    let world_landmarks = world
        .chunks_exact(3)
        .map(|p| {
            [
                c * p[0] as f64 - s * p[1] as f64,
                s * p[0] as f64 + c * p[1] as f64,
                p[2] as f64,
            ]
        })
        .collect();
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in &landmarks {
        bounds[0] = bounds[0].min(p.x);
        bounds[1] = bounds[1].min(p.y);
        bounds[2] = bounds[2].max(p.x);
        bounds[3] = bounds[3].max(p.y);
    }
    for axis in 0..2 {
        let center = (bounds[axis] + bounds[axis + 2]) / 2.;
        let half = (bounds[axis + 2] - bounds[axis]) * 1.25 / 2.;
        bounds[axis] = center - half;
        bounds[axis + 2] = center + half;
    }
    Ok(Some(Pose {
        score,
        box_xyxy: bounds,
        landmarks,
        world_landmarks,
    }))
}
/// Source-sized binary person mask. Model logits are not garment segmentation.
pub fn mask(t: &[TensorOutput], r: &Roi) -> AppResult<image::GrayImage> {
    r.validate()?;
    let raw = tensor(t, "Identity_2", &[1, 256, 256, 1])?;
    if r.source_size[0] as u64 * r.source_size[1] as u64 > 67_108_864 {
        return Err(bad());
    }
    let (c, s) = (r.angle_radians.cos(), r.angle_radians.sin());
    let inverse = [
        [c, s, (1. - c) * 128. - s * 128.],
        [-s, c, s * 128. + (1. - c) * 128.],
    ];
    let mut unrotated = vec![0.; 256 * 256];
    for y in 0..256 {
        for x in 0..256 {
            let (ix, iy, fx, fy) = warp_point(&inverse, x, y);
            let p = |a: i64, b: i64| {
                if (0..256).contains(&a) && (0..256).contains(&b) {
                    raw[b as usize * 256 + a as usize]
                } else {
                    0.
                }
            };
            unrotated[y as usize * 256 + x as usize] = (p(ix, iy) * (1. - fx) + p(ix + 1, iy) * fx)
                * (1. - fy)
                + (p(ix, iy + 1) * (1. - fx) + p(ix + 1, iy + 1) * fx) * fy;
        }
    }
    let mut image = image::GrayImage::new(r.source_size[0], r.source_size[1]);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let cx = x as f64 - r.origin[0];
            let cy = y as f64 - r.origin[1];
            if cx < 0. || cy < 0. || cx >= r.size[0] as f64 || cy >= r.size[1] as f64 {
                continue;
            }
            let sx = ((cx + 0.5) * 256. / r.size[0] as f64 - 0.5).max(0.) as f32;
            let sy = ((cy + 0.5) * 256. / r.size[1] as f64 - 0.5).max(0.) as f32;
            let ix = sx.floor() as usize;
            let iy = sy.floor() as usize;
            let fx = sx - ix as f32;
            let fy = sy - iy as f32;
            let p = |a: usize, b: usize| unrotated[b.min(255) * 256 + a.min(255)];
            let v = (p(ix, iy) * (1. - fx) + p(ix + 1, iy) * fx) * (1. - fy)
                + (p(ix, iy + 1) * (1. - fx) + p(ix + 1, iy + 1) * fx) * fy;
            if v > 0. {
                image.put_pixel(x, y, image::Luma([255]));
            }
        }
    }
    Ok(image)
}
impl Roi {
    fn validate(&self) -> AppResult<()> {
        if self.size.iter().any(|v| *v == 0 || *v > 8192)
            || self.source_size.iter().any(|v| *v == 0 || *v > 16384)
            || !self.angle_radians.is_finite()
            || self
                .origin
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 32768. || v.fract() != 0.)
            || self
                .hip_in_crop
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 8192.)
        {
            return Err(bad());
        }
        Ok(())
    }
    pub fn from_person(p: &Person, width: u32, height: u32) -> AppResult<Self> {
        if width == 0
            || height == 0
            || width > 16384
            || height > 16384
            || p.landmarks.iter().flatten().any(|v| !v.is_finite())
        {
            return Err(bad());
        }
        let hip = p.landmarks[0];
        let extent = p.landmarks[1];
        let dx = extent[0] - hip[0];
        let dy = extent[1] - hip[1];
        let radius = dx.hypot(dy);
        if !(1.0..=4096.).contains(&radius) {
            return Err(bad());
        }
        let origin = [(hip[0] - radius).trunc(), (hip[1] - radius).trunc()];
        let end = [(hip[0] + radius).trunc(), (hip[1] + radius).trunc()];
        let size = [(end[0] - origin[0]) as u32, (end[1] - origin[1]) as u32];
        if size.contains(&0)
            || end[0] <= 0.
            || end[1] <= 0.
            || origin[0] >= width as f64
            || origin[1] >= height as f64
        {
            return Err(bad());
        }
        let raw = std::f64::consts::FRAC_PI_2 - (-dy).atan2(dx);
        let angle =
            (raw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        Ok(Self {
            origin,
            size,
            angle_radians: angle,
            hip_in_crop: [hip[0] - origin[0], hip[1] - origin[1]],
            source_size: [width, height],
        })
    }
    /// Source pixels -> rotated 256×256 pose tensor coordinates, including padded pixels.
    pub fn source_to_tensor(&self, point: [f64; 2]) -> [f64; 2] {
        let x = point[0] - self.origin[0] - self.hip_in_crop[0];
        let y = point[1] - self.origin[1] - self.hip_in_crop[1];
        let (c, s) = (self.angle_radians.cos(), self.angle_radians.sin());
        [
            (c * x + s * y + self.hip_in_crop[0]) * 256. / self.size[0] as f64,
            (-s * x + c * y + self.hip_in_crop[1]) * 256. / self.size[1] as f64,
        ]
    }
    /// Inverse geometry; retain out-of-frame coordinates instead of falsely clipping visibility.
    pub fn tensor_to_source(&self, point: [f64; 2]) -> [f64; 2] {
        let x = point[0] * self.size[0] as f64 / 256. - self.hip_in_crop[0];
        let y = point[1] * self.size[1] as f64 / 256. - self.hip_in_crop[1];
        let (c, s) = (self.angle_radians.cos(), self.angle_radians.sin());
        [
            c * x - s * y + self.hip_in_crop[0] + self.origin[0],
            s * x + c * y + self.hip_in_crop[1] + self.origin[1],
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn person(hip: [f64; 2], extent: [f64; 2]) -> Person {
        Person {
            detector_box: [0.; 4],
            landmarks: [hip, extent, [0.; 2], [0.; 2]],
            score: 0.9,
        }
    }
    #[test]
    fn upright_and_rotated_roi_round_trip_with_padding() {
        for extent in [[50., -30.], [130., 50.], [-30., 50.], [80., -10.]] {
            let r = Roi::from_person(&person([50., 50.], extent), 100, 120).unwrap();
            assert!(r.origin[0] < 0.);
            assert!(r.origin[1] < 0.);
            for p in [[0., 0.], [50., 50.], [-20., 150.], [99., 119.]] {
                let q = r.tensor_to_source(r.source_to_tensor(p));
                assert!((q[0] - p[0]).abs() < 1e-9);
                assert!((q[1] - p[1]).abs() < 1e-9);
            }
            let hip = r.source_to_tensor([50., 50.]);
            let top = r.source_to_tensor(extent);
            assert!((hip[0] - top[0]).abs() < 1e-9);
            assert!(top[1] < hip[1]);
        }
    }
    #[test]
    fn invalid_or_disjoint_regions_abstain() {
        assert!(Roi::from_person(&person([50., 50.], [50., 50.]), 100, 100).is_err());
        assert!(Roi::from_person(&person([10000., 10000.], [10001., 10000.]), 100, 100).is_err());
        assert!(Roi::from_person(&person([f64::NAN, 0.], [1., 1.]), 100, 100).is_err());
    }
    fn tensors(score: f32) -> Vec<TensorOutput> {
        vec![
            TensorOutput {
                name: "Identity".into(),
                shape: vec![1, 195],
                values: [128., 128., 1., 0., 0.].repeat(39),
            },
            TensorOutput {
                name: "Identity_1".into(),
                shape: vec![1, 1],
                values: vec![score],
            },
            TensorOutput {
                name: "Identity_2".into(),
                shape: vec![1, 256, 256, 1],
                values: vec![1.; 65536],
            },
            TensorOutput {
                name: "Identity_3".into(),
                shape: vec![1, 64, 64, 39],
                values: vec![0.; 64 * 64 * 39],
            },
            TensorOutput {
                name: "Identity_4".into(),
                shape: vec![1, 117],
                values: vec![0.; 117],
            },
        ]
    }
    #[test]
    fn upright_preprocessing_landmarks_and_mask_preserve_source_coordinates() {
        let roi = Roi::from_person(&person([128., 128.], [128., 0.]), 256, 256).unwrap();
        let source = image::RgbImage::from_pixel(256, 256, image::Rgb([255, 0, 0]));
        let input = prepare(&source, &roi).unwrap();
        assert!(input.chunks_exact(3).all(|p| p == [1., 0., 0.]));
        let t = tensors(0.9);
        let p = decode(&t, &roi).unwrap().unwrap();
        assert_eq!(p.landmarks.len(), 39);
        assert_eq!(
            (p.landmarks[0].x, p.landmarks[0].y, p.landmarks[0].z),
            (128., 128., 1.)
        );
        assert_eq!(p.landmarks[0].visibility, 0.5);
        assert!(mask(&t, &roi).unwrap().pixels().all(|p| p[0] == 255));
        assert!(decode(&tensors(0.4), &roi).unwrap().is_none());
    }
    #[test]
    fn invalid_tensors_and_unbounded_rasters_are_rejected() {
        let mut roi = Roi::from_person(&person([128., 128.], [128., 0.]), 256, 256).unwrap();
        let mut t = tensors(0.9);
        t[0].values[0] = f32::NAN;
        assert!(decode(&t, &roi).is_err());
        roi.size = [0, 256];
        assert!(mask(&tensors(0.9), &roi).is_err());
        roi.size = [8192, 8192];
        assert!(prepare(&image::RgbImage::new(256, 256), &roi).is_err());
    }
}
