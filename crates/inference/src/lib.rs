pub mod download;
pub mod face;
pub mod graph;
mod hardware;
pub mod local;
pub mod person;
pub mod pose;
use frameiq_contracts::*;
use ort::{session::Session, value::Tensor};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path, time::Instant};
pub const RUNTIME_VERSION: &str = "1.22.0";
fn error(e: impl std::fmt::Display) -> AppError {
    AppError::new("MODEL_INCOMPATIBLE", e.to_string())
}
pub fn hardware() -> HardwareProfileV1 {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    let total = system.total_memory();
    let available = hardware::available(system.available_memory()).min(total);
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1) as u32;
    HardwareProfileV1 {
        os: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
        logical_cores: cores,
        total_memory_bytes: total.to_string(),
        available_memory_bytes: available.to_string(),
        ai_budget_bytes: ai_budget(total).to_string(),
        worker_threads: (cores / 2).clamp(1, 4),
    }
}
fn ai_budget(total: u64) -> u64 {
    (total / 4).min(4 * 1024 * 1024 * 1024)
}
fn input_dimensions(m: &ModelManifestV1) -> AppResult<(u32, u32)> {
    let s = &m.input.shape;
    let layout = m.input_layout.as_deref().unwrap_or("nchw");
    if s.len() != 4 || s[0] != 1 || !["nchw", "nhwc"].contains(&layout) {
        return Err(error(
            "Use a bounded single-image NCHW or NHWC float32 input.",
        ));
    }
    let (channels, width, height) = if layout == "nhwc" {
        (s[3], s[2], s[1])
    } else {
        (s[1], s[3], s[2])
    };
    if channels != 3 || width == 0 || height == 0 || width > 1024 || height > 1024 {
        return Err(error(
            "Input dimensions exceed the supported image template.",
        ));
    }
    Ok((width, height))
}
fn image_tensor(m: &ModelManifestV1, image: &image::RgbImage) -> AppResult<Vec<f32>> {
    let (w, h) = input_dimensions(m)?;
    if image.dimensions() != (w, h) {
        return Err(error(
            "Preprocessed image dimensions differ from the manifest.",
        ));
    }
    let plane = w as usize * h as usize;
    let mut data = vec![0.; 3 * plane];
    for (i, p) in image.pixels().enumerate() {
        for c in 0..3 {
            let channel = if m.channel_order == "bgr" { 2 - c } else { c };
            let index = if m.input_layout.as_deref() == Some("nhwc") {
                i * 3 + c
            } else {
                c * plane + i
            };
            data[index] = (p[channel] as f32 - m.mean[c]) * m.scale;
        }
    }
    Ok(data)
}
pub fn validate_manifest(m: &ModelManifestV1) -> AppResult<()> {
    if m.schema_version != 1
        || ![
            "yunet",
            "person_detector",
            "pose",
            "hair",
            "region_detector",
            "vision_tensor",
        ]
        .contains(&m.task.as_str())
        || m.runtime_version != RUNTIME_VERSION
        || m.license.trim().is_empty()
        || m.license_url.trim().is_empty()
        || m.source_url.trim().is_empty()
        || m.sha256.len() != 64
        || !m.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || m.byte_size
            .parse::<u64>()
            .ok()
            .is_none_or(|n| n == 0 || n > 512 * 1024 * 1024)
        || input_dimensions(m).is_err()
        || !["rgb", "bgr"].contains(&m.channel_order.as_str())
        || !m.scale.is_finite()
        || m.mean.iter().any(|n| !n.is_finite())
        || m.outputs.is_empty()
        || m.outputs.len() > 32
        || m.outputs.iter().any(|o| {
            o.name.is_empty()
                || o.shape.iter().any(|n| *n == 0)
                || o.shape
                    .iter()
                    .try_fold(1u64, |a, n| a.checked_mul(*n as u64))
                    .is_none_or(|n| n > 200000)
        })
    {
        return Err(AppError::new(
            "INVALID_MODEL_MANIFEST",
            "Use a licensed, bounded NCHW/NHWC float32 task manifest for the pinned runtime.",
        ));
    }
    Ok(())
}
pub fn verify_model(path: &Path, manifest: &ModelManifestV1) -> AppResult<Vec<u8>> {
    validate_manifest(manifest)?;
    let file = std::fs::File::open(path).map_err(error)?;
    let length = file.metadata().map_err(error)?.len();
    if length.to_string() != manifest.byte_size {
        return Err(AppError::new(
            "MODEL_CHECKSUM",
            "Model size differs from its manifest.",
        ));
    }
    let mut bytes = Vec::new();
    file.take(512 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if format!("{:x}", Sha256::digest(&bytes)) != manifest.sha256.to_lowercase() {
        return Err(AppError::new(
            "MODEL_CHECKSUM",
            "Model checksum differs from its manifest.",
        ));
    }
    graph::validate(&bytes)?;
    Ok(bytes)
}
pub fn initialize_runtime() -> AppResult<()> {
    let path = if let Some(path) = std::env::var_os("FRAMEIQ_ORT_LIBRARY") {
        std::path::PathBuf::from(path)
    } else {
        let exe = std::env::current_exe().map_err(error)?;
        #[cfg(target_os = "macos")]
        let path = exe
            .parent()
            .unwrap()
            .join("../Resources/native/libonnxruntime.1.22.0.dylib");
        #[cfg(target_os = "windows")]
        let path = exe.parent().unwrap().join("onnxruntime.dll");
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let path = exe.parent().unwrap().join("libonnxruntime.so.1.22.0");
        path
    };
    if !path.is_file() {
        return Err(AppError::new(
            "RUNTIME_MISSING",
            "The bundled ONNX Runtime library is missing.",
        ));
    }
    ort::init_from(path).map_err(error)?.commit();
    Ok(())
}
fn check_memory(m: &ModelManifestV1, h: &HardwareProfileV1) -> AppResult<()> {
    let total = h.total_memory_bytes.parse::<u64>().map_err(error)?;
    let available = h.available_memory_bytes.parse::<u64>().map_err(error)?;
    let size = m.byte_size.parse::<u64>().map_err(error)?;
    let budget = h.ai_budget_bytes.parse::<u64>().map_err(error)?;
    if total == 0 {
        return Err(AppError::new(
            "HARDWARE_UNAVAILABLE",
            "The operating system did not report physical memory.",
        ));
    }
    if available < total * 15 / 100 || size.saturating_mul(4) > budget {
        return Err(AppError::new("MEMORY_LIMIT", format!(
            "Available memory: {:.2} GiB; required system reserve: {:.2} GiB. Model load estimate: {:.2} GiB; managed budget: {:.2} GiB.",
            available as f64 / 1073741824., total as f64 * 0.15 / 1073741824., size.saturating_mul(4) as f64 / 1073741824., budget as f64 / 1073741824.
        )));
    }
    Ok(())
}
pub struct Engine {
    session: Option<(String, Session)>,
}
impl Default for Engine {
    fn default() -> Self {
        Self { session: None }
    }
}
impl Engine {
    pub fn load(&mut self, m: &ModelManifestV1, path: &Path) -> AppResult<f64> {
        validate_manifest(m)?;
        let h = hardware();
        if let Err(e) = check_memory(m, &h) {
            self.session = None;
            return Err(e);
        }
        if self
            .session
            .as_ref()
            .is_some_and(|(hash, _)| hash == &m.sha256)
        {
            return Ok(0.);
        }
        self.session = None;
        initialize_runtime()?;
        let bytes = verify_model(path, m)?;
        let started = Instant::now();
        let session = Session::builder()
            .map_err(error)?
            .with_intra_threads(
                std::env::var("FRAMEIQ_CPU_THREADS")
                    .ok()
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(h.worker_threads as usize)
                    .clamp(1, h.worker_threads as usize),
            )
            .map_err(error)?
            .with_inter_threads(1)
            .map_err(error)?
            .commit_from_memory(&bytes)
            .map_err(error)?;
        if session.inputs().len() != 1
            || session.inputs()[0].name() != m.input.name
            || session.outputs().len() != m.outputs.len()
            || session
                .outputs()
                .iter()
                .any(|o| !m.outputs.iter().any(|s| s.name == o.name()))
        {
            return Err(error("Tensor names differ from the manifest."));
        }
        self.session = Some((m.sha256.clone(), session));
        Ok(started.elapsed().as_secs_f64() * 1000.)
    }
    pub fn run(
        &mut self,
        m: &ModelManifestV1,
        path: &Path,
        image: Option<&Path>,
    ) -> AppResult<Vec<TensorOutput>> {
        self.load(m, path)?;
        let (w, h) = input_dimensions(m)?;
        let image = if let Some(path) = image {
            let reader = image::ImageReader::open(path)
                .map_err(error)?
                .with_guessed_format()
                .map_err(error)?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(16384);
            limits.max_image_height = Some(16384);
            limits.max_alloc = Some(512 * 1024 * 1024);
            let mut reader = reader;
            reader.limits(limits);
            reader
                .decode()
                .map_err(error)?
                .resize_exact(w, h, image::imageops::FilterType::Triangle)
                .to_rgb8()
        } else {
            image::RgbImage::from_fn(w, h, |x, y| {
                image::Rgb([((x + y) % 256) as u8, (x % 256) as u8, (y % 256) as u8])
            })
        };
        self.run_rgb(m, &image)
    }
    fn run_rgb(
        &mut self,
        m: &ModelManifestV1,
        image: &image::RgbImage,
    ) -> AppResult<Vec<TensorOutput>> {
        let data = image_tensor(m, image)?;
        self.execute_tensor(m, data)
    }
    /// Task adapters may supply normalized/padded pixels that cannot be represented as RGB8.
    pub fn run_prepared(
        &mut self,
        m: &ModelManifestV1,
        path: &Path,
        data: Vec<f32>,
    ) -> AppResult<Vec<TensorOutput>> {
        self.load(m, path)?;
        self.execute_tensor(m, data)
    }
    fn execute_tensor(
        &mut self,
        m: &ModelManifestV1,
        data: Vec<f32>,
    ) -> AppResult<Vec<TensorOutput>> {
        if self.session.is_none()
            || data.len() != m.input.shape.iter().map(|v| *v as usize).product::<usize>()
            || data.iter().any(|v| !v.is_finite())
        {
            return Err(error("Invalid task input or unloaded model session"));
        }
        let input = Tensor::from_array((
            m.input
                .shape
                .iter()
                .map(|n| *n as usize)
                .collect::<Vec<_>>(),
            data,
        ))
        .map_err(error)?;
        let outputs = self
            .session
            .as_mut()
            .unwrap()
            .1
            .run(ort::inputs![m.input.name.as_str()=>input])
            .map_err(error)?;
        let mut tensors = Vec::new();
        for (name, value) in outputs.iter() {
            let (shape, values) = value.try_extract_tensor::<f32>().map_err(error)?;
            let expected = m
                .outputs
                .iter()
                .find(|o| o.name == name)
                .ok_or_else(|| error("Unexpected output"))?;
            if values.len() > 200000
                || shape.iter().copied().collect::<Vec<_>>()
                    != expected.shape.iter().map(|n| *n as i64).collect::<Vec<_>>()
                || values.iter().any(|v| !v.is_finite())
            {
                return Err(error(
                    "Output tensor shape or values differ from the manifest.",
                ));
            }
            tensors.push(TensorOutput {
                name: name.into(),
                shape: shape.iter().copied().collect(),
                values: values.to_vec(),
            });
        }
        Ok(tensors)
    }
    pub fn handle(&mut self, request: InferenceRequestV1) -> InferenceResponseV1 {
        let result = (|| -> AppResult<InferenceResponseV1> {
            match request {
                InferenceRequestV1::Download { .. }
                | InferenceRequestV1::Discover { .. }
                | InferenceRequestV1::Probe { .. } => Err(AppError::new(
                    "AI_PROTOCOL",
                    "Network operations require the supervised worker.",
                )),
                InferenceRequestV1::Unload => {
                    self.session = None;
                    Ok(InferenceResponseV1::Unloaded)
                }
                InferenceRequestV1::Validate { manifest, path } => {
                    self.run(&manifest, Path::new(&path), None)?;
                    Ok(InferenceResponseV1::Validated)
                }
                InferenceRequestV1::DetectFaces {
                    manifest,
                    path,
                    image,
                } => {
                    face::validate_template(&manifest)?;
                    self.load(&manifest, Path::new(&path))?;
                    let (prepared, geometry) = face::prepare(Path::new(&image))?;
                    let tensors = self.run_rgb(&manifest, &prepared)?;
                    Ok(InferenceResponseV1::Faces {
                        result: FaceResultV1 {
                            width: geometry.width,
                            height: geometry.height,
                            algorithm: face::ALGORITHM.into(),
                            model_hash: manifest.sha256,
                            faces: face::decode(&tensors, &geometry)?,
                        },
                    })
                }
                InferenceRequestV1::Run {
                    manifest,
                    path,
                    image,
                } => Ok(InferenceResponseV1::Outputs {
                    tensors: self.run(&manifest, Path::new(&path), Some(Path::new(&image)))?,
                }),
                InferenceRequestV1::DetectPersons {
                    manifest,
                    path,
                    image,
                } => {
                    person::validate_template(&manifest)?;
                    let source = person::read_image(Path::new(&image))?;
                    let (data, geometry) = person::prepare(&source)?;
                    let tensors = self.run_prepared(&manifest, Path::new(&path), data)?;
                    Ok(InferenceResponseV1::Persons {
                        result: PersonResultV1 {
                            width: geometry.width,
                            height: geometry.height,
                            algorithm: person::ALGORITHM.into(),
                            model_hash: manifest.sha256,
                            persons: person::decode(&tensors, &geometry)?,
                        },
                    })
                }
                InferenceRequestV1::EstimatePose {
                    manifest,
                    path,
                    image,
                    person,
                    mask_output,
                } => {
                    pose::validate_template(&manifest)?;
                    let destination = Path::new(&mask_output);
                    if destination.extension().is_none_or(|e| e != "png")
                        || destination.exists()
                        || destination.is_symlink()
                    {
                        return Err(AppError::new(
                            "OUTPUT_CONFLICT",
                            "Pose masks require an unused PNG asset path",
                        ));
                    }
                    self.load(&manifest, Path::new(&path))?;
                    let source = person::read_image(Path::new(&image))?;
                    let roi = pose::Roi::from_person(&person, source.width(), source.height())?;
                    let data = pose::prepare(&source, &roi)?;
                    let tensors = self.run_prepared(&manifest, Path::new(&path), data)?;
                    let detection = pose::decode(&tensors, &roi)?;
                    let mut mask_path = None;
                    let mut mask_sha256 = None;
                    if detection.is_some() {
                        let mask = pose::mask(&tensors, &roi)?;
                        let mut file = tempfile::NamedTempFile::new_in(
                            destination
                                .parent()
                                .ok_or_else(|| error("Missing asset directory"))?,
                        )
                        .map_err(error)?;
                        image::DynamicImage::ImageLuma8(mask)
                            .write_to(file.as_file_mut(), image::ImageFormat::Png)
                            .map_err(error)?;
                        file.as_file().sync_all().map_err(error)?;
                        let hash = format!(
                            "{:x}",
                            Sha256::digest(std::fs::read(file.path()).map_err(error)?)
                        );
                        file.persist_noclobber(destination).map_err(error)?;
                        mask_path = Some(mask_output);
                        mask_sha256 = Some(hash);
                    }
                    Ok(InferenceResponseV1::Pose {
                        result: PoseResultV1 {
                            width: source.width(),
                            height: source.height(),
                            algorithm: pose::ALGORITHM.into(),
                            model_hash: manifest.sha256,
                            pose: detection,
                            mask_path,
                            mask_sha256,
                        },
                    })
                }
                InferenceRequestV1::Benchmark { manifest, path } => {
                    self.session = None;
                    let cold = self.load(&manifest, Path::new(&path))?;
                    self.run(&manifest, Path::new(&path), None)?;
                    let mut latency = Vec::new();
                    let mut peak = 0;
                    for _ in 0..5 {
                        let start = Instant::now();
                        self.run(&manifest, Path::new(&path), None)?;
                        latency.push(start.elapsed().as_secs_f64() * 1000.);
                        let mut sys = sysinfo::System::new_all();
                        sys.refresh_processes(
                            sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(
                                std::process::id(),
                            )]),
                            true,
                        );
                        peak = peak.max(
                            sys.process(sysinfo::Pid::from_u32(std::process::id()))
                                .map(|p| p.memory())
                                .unwrap_or(0),
                        );
                    }
                    latency.sort_by(f64::total_cmp);
                    Ok(InferenceResponseV1::Benchmark {
                        result: ModelBenchmarkV1 {
                            model_id: manifest.id,
                            model_hash: manifest.sha256,
                            runtime_version: RUNTIME_VERSION.into(),
                            provider: "cpu".into(),
                            cold_load_ms: cold,
                            warm_latency_ms: latency[2],
                            peak_observed_memory_bytes: peak.to_string(),
                            hardware: hardware(),
                        },
                    })
                }
            }
        })();
        result.unwrap_or_else(|error| InferenceResponseV1::Error { error })
    }
}

#[cfg(test)]
mod tensor_tests {
    use super::*;
    fn manifest() -> ModelManifestV1 {
        serde_json::from_str(include_str!("../../../models/manifests/yunet-2023mar.json")).unwrap()
    }
    #[test]
    fn managed_budget_is_one_quarter_capped_at_four_gib() {
        let gib = 1024 * 1024 * 1024;
        assert_eq!(ai_budget(0), 0);
        assert_eq!(ai_budget(8 * gib), 2 * gib);
        assert_eq!(ai_budget(16 * gib), 4 * gib);
        assert_eq!(ai_budget(24 * gib), 4 * gib);
        assert_eq!(ai_budget(u64::MAX), 4 * gib);
    }
    #[test]
    fn memory_admission_rejects_low_reserve_and_oversized_loads() {
        let m = manifest();
        let mut h = hardware();
        h.total_memory_bytes = "10000000000".into();
        h.available_memory_bytes = "1499999999".into();
        h.ai_budget_bytes = "2500000000".into();
        assert_eq!(check_memory(&m, &h).unwrap_err().code, "MEMORY_LIMIT");
        h.available_memory_bytes = "1500000000".into();
        assert!(check_memory(&m, &h).is_ok());
        h.ai_budget_bytes = "100".into();
        assert_eq!(check_memory(&m, &h).unwrap_err().code, "MEMORY_LIMIT");
        h.total_memory_bytes = "0".into();
        assert_eq!(
            check_memory(&m, &h).unwrap_err().code,
            "HARDWARE_UNAVAILABLE"
        );
    }
    #[test]
    fn layouts_preserve_channel_order_and_normalization() {
        let mut m = manifest();
        m.task = "vision_tensor".into();
        let image = image::RgbImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60]).unwrap();
        m.input.shape = vec![1, 3, 1, 2];
        m.mean = [1., 2., 3.];
        m.scale = 0.5;
        assert_eq!(
            image_tensor(&m, &image).unwrap(),
            vec![14.5, 29.5, 9., 24., 3.5, 18.5]
        );
        m.input_layout = Some("nhwc".into());
        m.input.shape = vec![1, 1, 2, 3];
        assert_eq!(
            image_tensor(&m, &image).unwrap(),
            vec![14.5, 9., 3.5, 29.5, 24., 18.5]
        );
        validate_manifest(&m).unwrap();
        m.input.shape = vec![1, 0, 2, 3];
        assert!(validate_manifest(&m).is_err());
        m.input.shape = vec![2, 1, 2, 3];
        assert!(validate_manifest(&m).is_err());
        m.input_layout = Some("unknown".into());
        assert!(validate_manifest(&m).is_err());
    }
}
