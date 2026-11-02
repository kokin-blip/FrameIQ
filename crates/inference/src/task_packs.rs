//! Fixed, reviewed task adapters. The legacy user-import validator remains unchanged.
use super::*;
use ort::session::SessionInputValue;
use ort::value::TensorElementType;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;
pub(crate) struct CachedTaskSession {
    key: String,
    session: Session,
    stamps: Vec<(std::path::PathBuf, u64, std::time::SystemTime)>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    #[serde(default)]
    resampling: Option<String>,
    image_input: String,
    width: u32,
    height: u32,
    layout: String,
    scale: f32,
    mean: [f32; 3],
    std: [f32; 3],
    #[serde(default)]
    letterbox: bool,
    #[serde(default)]
    constants: HashMap<String, Constant>,
    output: String,
    #[serde(default)]
    boxes: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    token_ranges: Vec<[usize; 2]>,
    #[serde(default = "threshold")]
    threshold: f32,
}
fn threshold() -> f32 {
    0.35
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Constant {
    #[serde(default)]
    values: Vec<f64>,
    #[serde(default)]
    fill: Option<f64>,
    #[serde(default)]
    from: Option<String>,
}
fn bad(message: impl std::fmt::Display) -> AppError {
    AppError::new("TASK_PACK", message.to_string())
}
fn digest(p: &Path) -> AppResult<String> {
    let mut f = std::fs::File::open(p).map_err(bad)?;
    let mut h = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b).map_err(bad)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn count(s: &[u32]) -> AppResult<usize> {
    s.iter()
        .try_fold(1usize, |a, n| a.checked_mul(*n as usize))
        .filter(|n| *n > 0 && *n <= 32 * 1024 * 1024)
        .ok_or_else(|| bad("Task tensors must have bounded nonzero dimensions."))
}
pub fn validate(pack: &TaskPackV2, directory: &Path) -> AppResult<String> {
    if pack.schema_version != 2
        || !matches!(
            pack.adapter.as_str(),
            "grounding_dino_v1"
                | "sam2_image_v1"
                | "sam_image_v1"
                | "hair_segmenter_v1"
                | "siglip2_image_v1"
        )
        || pack.inputs.is_empty()
        || pack.inputs.len() > 16
        || pack.outputs.is_empty()
        || pack.outputs.len() > 16
        || pack.artifacts.is_empty()
        || pack.artifacts.len() > 16
        || pack.license.is_empty()
        || !crate::fashion::valid_url(&pack.license_url)
        || pack.preprocessing.len() > 65536
    {
        return Err(bad("Unsupported or incomplete task package."));
    }
    let config: Config = serde_json::from_str(&pack.preprocessing).map_err(bad)?;
    if config
        .resampling
        .as_deref()
        .is_some_and(|s| !["pillow_bilinear_v1", "triangle_v1"].contains(&s))
    {
        return Err(bad("Unknown resize profile"));
    }
    if config.width == 0
        || config.height == 0
        || config.width > 2048
        || config.height > 2048
        || !["nchw", "nhwc"].contains(&config.layout.as_str())
        || !config.scale.is_finite()
        || config
            .mean
            .iter()
            .chain(config.std.iter())
            .any(|n| !n.is_finite())
        || config.std.contains(&0.)
        || !config.threshold.is_finite()
        || !(0.0..=1.0).contains(&config.threshold)
    {
        return Err(bad("Invalid preprocessing profile."));
    }
    let mut names = std::collections::HashSet::new();
    for t in pack.inputs.iter().chain(pack.outputs.iter()) {
        count(&t.shape)?;
        if !["float32", "int64", "bool"].contains(&t.dtype.as_str())
            || !names.insert((&t.name, pack.inputs.iter().any(|i| std::ptr::eq(i, t))))
        {
            return Err(bad("Invalid typed tensor profile."));
        }
    }
    let root = directory.canonicalize().map_err(bad)?;
    let mut total = 0u64;
    let mut graph = None;
    for a in &pack.artifacts {
        if a.name.is_empty()
            || Path::new(&a.name).components().count() != 1
            || a.name == "."
            || a.name == ".."
            || a.sha256.len() != 64
            || !a.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(bad("Artifact names and hashes must be canonical."));
        }
        let p = root.join(&a.name);
        if p.is_symlink() {
            return Err(bad("Task artifacts cannot be symlinks."));
        }
        let m = p.metadata().map_err(bad)?;
        let size = a.byte_size.parse::<u64>().map_err(bad)?;
        total = total
            .checked_add(size)
            .ok_or_else(|| bad("Package size overflow"))?;
        if !m.is_file()
            || m.len() != size
            || total > 2 * 1024 * 1024 * 1024
            || digest(&p)? != a.sha256
        {
            return Err(bad("Task artifact size/checksum mismatch."));
        }
        if a.name.ends_with(".onnx") {
            if graph.is_some() {
                return Err(bad(
                    "This profile requires one self-contained exported graph.",
                ));
            }
            let bytes = std::fs::read(&p).map_err(bad)?;
            crate::graph::validate_reviewed(&bytes)?;
            graph = Some(a.sha256.clone());
        }
    }
    graph.ok_or_else(|| bad("The package needs a self-contained ONNX graph."))
}
impl Engine {
    pub fn task_pack(
        &mut self,
        pack: &TaskPackV2,
        directory: &Path,
        image_path: Option<&str>,
        rect: Option<&RegionRectV1>,
        mask_output: Option<&str>,
    ) -> AppResult<TaskResultV2> {
        let fingerprint = pack
            .artifacts
            .iter()
            .find(|a| a.name.ends_with(".onnx"))
            .map(|a| a.sha256.clone())
            .ok_or_else(|| bad("Missing graph"))?;
        let key = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(pack, directory.to_string_lossy())).map_err(bad)?)
        );
        let profile: Config = serde_json::from_str(&pack.preprocessing).map_err(bad)?;
        // Release a different task before admission, so its retained session does
        // not prevent a bounded replacement from satisfying the same reserve.
        if self.task_session.as_ref().is_some_and(|s| s.key != key) {
            self.task_session = None;
        }
        self.session = None;
        let h = hardware();
        let total = pack
            .artifacts
            .iter()
            .map(|a| a.byte_size.parse::<u64>().unwrap_or(u64::MAX))
            .try_fold(0u64, |a, b| a.checked_add(b))
            .ok_or_else(|| bad("Package too large"))?;
        if total.saturating_mul(4) > h.ai_budget_bytes.parse::<u64>().map_err(bad)?
            || h.available_memory_bytes.parse::<u64>().map_err(bad)?
                < h.total_memory_bytes.parse::<u64>().map_err(bad)? * 15 / 100
        {
            return Err(AppError::new(
                "MEMORY_LIMIT",
                "This task pack exceeds the managed memory budget or system reserve.",
            ));
        }
        self.session = None;
        initialize_runtime()?;
        let valid = self.task_session.as_ref().is_some_and(|cached| {
            cached.key == key
                && cached.stamps.iter().all(|(path, size, time)| {
                    !path.is_symlink()
                        && path.metadata().is_ok_and(|m| {
                            m.len() == *size && m.modified().is_ok_and(|t| t == *time)
                        })
                })
        });
        if !valid {
            self.task_session = None;
            validate(pack, directory)?;
            let graph = pack
                .artifacts
                .iter()
                .find(|a| a.name.ends_with(".onnx"))
                .unwrap();
            let session = Session::builder()
                .map_err(bad)?
                .with_intra_threads(1)
                .map_err(bad)?
                .with_inter_threads(1)
                .map_err(bad)?
                .commit_from_file(directory.join(&graph.name))
                .map_err(bad)?;
            let stamps = pack
                .artifacts
                .iter()
                .map(|a| {
                    let path = directory.join(&a.name);
                    let m = path.metadata().map_err(bad)?;
                    Ok((path, m.len(), m.modified().map_err(bad)?))
                })
                .collect::<AppResult<Vec<_>>>()?;
            self.task_session = Some(CachedTaskSession {
                key,
                session,
                stamps,
            });
        }
        let session = &mut self.task_session.as_mut().unwrap().session;
        if session.inputs().len() != pack.inputs.len()
            || session.outputs().len() != pack.outputs.len()
        {
            return Err(bad("Graph tensors do not match the reviewed profile."));
        }
        for (actual, reviewed) in session
            .inputs()
            .iter()
            .map(|i| (i, &pack.inputs))
            .chain(session.outputs().iter().map(|o| (o, &pack.outputs)))
        {
            let t = reviewed
                .iter()
                .find(|t| t.name == actual.name())
                .ok_or_else(|| bad("Unexpected graph tensor."))?;
            let dtype = match t.dtype.as_str() {
                "float32" => TensorElementType::Float32,
                "int64" => TensorElementType::Int64,
                "bool" => TensorElementType::Bool,
                _ => return Err(bad("Unsupported tensor type")),
            };
            let dims = t.shape.iter().map(|n| *n as i64).collect::<Vec<_>>();
            if actual.dtype().tensor_type() != Some(dtype)
                || actual
                    .dtype()
                    .tensor_shape()
                    .is_none_or(|s| s.as_ref() != dims.as_slice())
            {
                return Err(bad(
                    "Graph tensor types and bounded dimensions must match the reviewed manifest.",
                ));
            }
        }
        let mut result = TaskResultV2 {
            pack_id: pack.id.clone(),
            model_hash: fingerprint,
            detections: vec![],
            mask: None,
            embedding: vec![],
        };
        let Some(path) = image_path else {
            return Ok(result);
        };
        let mut reader = image::ImageReader::open(path)
            .map_err(bad)?
            .with_guessed_format()
            .map_err(bad)?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode().map_err(bad)?;
        let (w, h) = (image.width(), image.height());
        let region = rect.cloned().unwrap_or(RegionRectV1 {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        });
        frameiq_analysis::presets::validate_rectangle(&region)?;
        // HairSegmenter is a close-up, single-person model. Infer only the reviewed
        // head ROI, then project the result onto the immutable complete source.
        let (roi_x, roi_y, roi_w, roi_h) = if pack.adapter == "hair_segmenter_v1" {
            let x = (region.x * w as f64).floor() as u32;
            let y = (region.y * h as f64).floor() as u32;
            (
                x,
                y,
                ((region.width * w as f64).ceil() as u32).max(1).min(w - x),
                ((region.height * h as f64).ceil() as u32).max(1).min(h - y),
            )
        } else {
            (0, 0, w, h)
        };
        let inference_image = image.crop_imm(roi_x, roi_y, roi_w, roi_h);
        let (rw, rh) = if profile.letterbox {
            let scale =
                (profile.width as f64 / roi_w as f64).min(profile.height as f64 / roi_h as f64);
            (
                ((roi_w as f64 * scale).round() as u32).max(1),
                ((roi_h as f64 * scale).round() as u32).max(1),
            )
        } else {
            (profile.width, profile.height)
        };
        let resized = if profile.resampling.as_deref() == Some("pillow_bilinear_v1") {
            crate::task_resize::bilinear(&inference_image.to_rgb8(), rw, rh)
        } else {
            inference_image
                .resize_exact(rw, rh, image::imageops::FilterType::Triangle)
                .to_rgb8()
        };
        let (ox, oy) = if pack.adapter == "grounding_dino_v1" {
            (0, 0)
        } else {
            ((profile.width - rw) / 2, (profile.height - rh) / 2)
        };
        let mut pixels = vec![0f32; (profile.width * profile.height * 3) as usize];
        for y in 0..profile.height {
            for x in 0..profile.width {
                let p = if x >= ox && x < ox + rw && y >= oy && y < oy + rh {
                    resized.get_pixel(x - ox, y - oy).0
                } else {
                    [0, 0, 0]
                };
                for c in 0..3 {
                    let v = if x >= ox && x < ox + rw && y >= oy && y < oy + rh {
                        (p[c] as f32 * profile.scale - profile.mean[c]) / profile.std[c]
                    } else {
                        0.
                    };
                    let idx = if profile.layout == "nchw" {
                        (c as u32 * profile.width * profile.height + y * profile.width + x) as usize
                    } else {
                        ((y * profile.width + x) * 3 + c as u32) as usize
                    };
                    pixels[idx] = v;
                }
            }
        }
        let mut inputs: Vec<(String, SessionInputValue<'static>)> = Vec::new();
        for t in &pack.inputs {
            let dims = t.shape.iter().map(|n| *n as i64).collect::<Vec<_>>();
            let n = count(&t.shape)?;
            if t.name == profile.image_input {
                if t.dtype != "float32" || n != pixels.len() {
                    return Err(bad("Image tensor shape does not match preprocessing."));
                }
                inputs.push((
                    t.name.clone(),
                    Tensor::from_array((dims, pixels.clone()))
                        .map_err(bad)?
                        .into(),
                ));
                continue;
            }
            let c = profile
                .constants
                .get(&t.name)
                .ok_or_else(|| bad(format!("Missing bounded input {}", t.name)))?;
            let vals = match c.from.as_deref() {
                Some("box") => vec![
                    (region.x * rw as f64 + ox as f64),
                    (region.y * rh as f64 + oy as f64),
                    ((region.x + region.width) * rw as f64 + ox as f64),
                    ((region.y + region.height) * rh as f64 + oy as f64),
                ],
                Some("image_size") => vec![h as f64, w as f64],
                Some("pixel_mask") => (0..profile.height)
                    .flat_map(|y| {
                        (0..profile.width).map(move |x| {
                            if x >= ox && x < ox + rw && y >= oy && y < oy + rh {
                                1.
                            } else {
                                0.
                            }
                        })
                    })
                    .collect(),
                Some(_) => return Err(bad("Unknown input generator")),
                None => {
                    if let Some(fill) = c.fill {
                        vec![fill; n]
                    } else {
                        c.values.clone()
                    }
                }
            };
            if vals.len() != n || vals.iter().any(|v| !v.is_finite()) {
                return Err(bad(
                    "Constant tensor does not match its reviewed dimensions.",
                ));
            }
            let v: SessionInputValue<'static> = match t.dtype.as_str() {
                "float32" => {
                    Tensor::from_array((dims, vals.iter().map(|n| *n as f32).collect::<Vec<_>>()))
                        .map_err(bad)?
                        .into()
                }
                "int64" => {
                    if vals
                        .iter()
                        .any(|n| n.fract() != 0. || *n < i64::MIN as f64 || *n >= i64::MAX as f64)
                    {
                        return Err(bad("Invalid integer input"));
                    }
                    Tensor::from_array((dims, vals.iter().map(|n| *n as i64).collect::<Vec<_>>()))
                        .map_err(bad)?
                        .into()
                }
                "bool" => {
                    Tensor::from_array((dims, vals.iter().map(|n| *n != 0.).collect::<Vec<_>>()))
                        .map_err(bad)?
                        .into()
                }
                _ => return Err(bad("Unsupported dtype")),
            };
            inputs.push((t.name.clone(), v));
        }
        let outputs = session.run(inputs).map_err(bad)?;
        let tensor = outputs
            .get(&profile.output)
            .ok_or_else(|| bad("Missing named result tensor"))?;
        let (shape, data) = tensor.try_extract_tensor::<f32>().map_err(bad)?;
        if data.iter().any(|n| {
            !n.is_finite() && !(pack.adapter == "grounding_dino_v1" && *n == f32::NEG_INFINITY)
        }) || data.len() > 32 * 1024 * 1024
        {
            return Err(bad("Invalid result tensor."));
        }
        match pack.adapter.as_str() {
            "siglip2_image_v1" => {
                if data.len() > 4096 {
                    return Err(bad("Embedding exceeds dimensions."));
                }
                let norm = data.iter().map(|v| v * v).sum::<f32>().sqrt();
                if norm <= 1e-8 {
                    return Err(bad("Empty embedding."));
                }
                result.embedding = data.iter().map(|v| v / norm).collect();
            }
            "grounding_dino_v1" => {
                let boxes = outputs
                    .get(
                        profile
                            .boxes
                            .as_deref()
                            .ok_or_else(|| bad("Missing boxes profile"))?,
                    )
                    .ok_or_else(|| bad("Missing boxes tensor"))?;
                let (_, boxes) = boxes.try_extract_tensor::<f32>().map_err(bad)?;
                if boxes.iter().any(|n| !n.is_finite())
                    || shape.len() != 3
                    || shape[0] != 1
                    || boxes.len() != shape[1] as usize * 4
                    || profile.labels.len() != profile.token_ranges.len()
                {
                    return Err(bad("Invalid detector output shape."));
                }
                let tokens = shape[2] as usize;
                let mut ds = Vec::new();
                for (q, b) in boxes.chunks_exact(4).enumerate() {
                    let mut best = None;
                    for (i, range) in profile.token_ranges.iter().enumerate() {
                        if range[0] >= range[1] || range[1] > tokens {
                            return Err(bad("Invalid prompt token mapping"));
                        }
                        let score = data[q * tokens + range[0]..q * tokens + range[1]]
                            .iter()
                            .map(|v| 1. / (1. + (-v).exp()))
                            .fold(0f32, f32::max);
                        if score >= profile.threshold && best.is_none_or(|(_, s)| score > s) {
                            best = Some((i, score));
                        }
                    }
                    if let Some((i, score)) = best {
                        let x = ((b[0] - b[2] / 2.) as f64 * profile.width as f64 - ox as f64)
                            / rw as f64;
                        let y = ((b[1] - b[3] / 2.) as f64 * profile.height as f64 - oy as f64)
                            / rh as f64;
                        let right =
                            (x + b[2] as f64 * profile.width as f64 / rw as f64).clamp(0., 1.);
                        let bottom =
                            (y + b[3] as f64 * profile.height as f64 / rh as f64).clamp(0., 1.);
                        let x = x.clamp(0., 1.);
                        let y = y.clamp(0., 1.);
                        if right > x && bottom > y {
                            ds.push(TaskDetectionV2 {
                                category: profile.labels[i].clone(),
                                rectangle: RegionRectV1 {
                                    x,
                                    y,
                                    width: right - x,
                                    height: bottom - y,
                                },
                                score: score as f64,
                            });
                        }
                    }
                }
                ds.sort_by(|a, b| b.score.total_cmp(&a.score));
                for d in ds {
                    if result.detections.len() >= 64 {
                        break;
                    }
                    if !result.detections.iter().any(|old| {
                        old.category == d.category
                            && frameiq_analysis::tracking::iou(&old.rectangle, &d.rectangle) > 0.5
                    }) {
                        result.detections.push(d);
                    }
                }
            }
            "hair_segmenter_v1" | "sam2_image_v1" | "sam_image_v1" => {
                let out =
                    mask_output.ok_or_else(|| bad("Choose an unused owned mask output path."))?;
                if Path::new(out).exists() || Path::new(out).is_symlink() {
                    return Err(bad("Mask output already exists."));
                }
                let (mw, mh) = if shape.len() >= 2 {
                    (shape[shape.len() - 1] as u32, shape[shape.len() - 2] as u32)
                } else {
                    return Err(bad("Mask output needs spatial dimensions."));
                };
                if mw == 0 || mh == 0 || data.len() < (mw * mh) as usize {
                    return Err(bad("Invalid mask shape."));
                }
                let raw = image::GrayImage::from_fn(mw, mh, |x, y| {
                    image::Luma([
                        if data[(y * mw + x) as usize]
                            > if matches!(pack.adapter.as_str(), "sam2_image_v1" | "sam_image_v1") {
                                0.
                            } else {
                                profile.threshold
                            }
                        {
                            255
                        } else {
                            0
                        },
                    ])
                });
                let resized = image::imageops::resize(
                    &raw,
                    profile.width,
                    profile.height,
                    image::imageops::FilterType::Nearest,
                );
                let crop = image::imageops::crop_imm(&resized, ox, oy, rw, rh).to_image();
                let roi_mask = image::imageops::resize(
                    &crop,
                    roi_w,
                    roi_h,
                    image::imageops::FilterType::Nearest,
                );
                let mut mask = image::GrayImage::new(w, h);
                image::imageops::replace(&mut mask, &roi_mask, roi_x as i64, roi_y as i64);
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(out)
                    .map_err(bad)?;
                let mut writer = std::io::BufWriter::new(&mut f);
                image::DynamicImage::ImageLuma8(mask)
                    .write_to(&mut writer, image::ImageFormat::Png)
                    .map_err(bad)?;
                writer.flush().map_err(bad)?;
                drop(writer);
                f.sync_all().map_err(bad)?;
                result.mask = Some(AssetRefV1 {
                    path: out.into(),
                    sha256: digest(Path::new(out))?,
                    width: w,
                    height: h,
                });
            }
            _ => return Err(bad("Unsupported task adapter")),
        }
        Ok(result)
    }
}
