use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SCHEMA_VERSION: u32 = 1;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FrameRefV1 {
    pub source_revision_id: String,
    pub stream_index: u32,
    pub index_generation: String,
    pub presentation_ordinal: String,
    pub pts_ticks: Option<String>,
    pub time_base_numerator: i32,
    pub time_base_denominator: i32,
    pub timestamp_origin: TimestampOrigin,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TimestampOrigin {
    Source,
    BestEffort,
    Synthetic,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IndexedFrame {
    pub ordinal: String,
    pub pts_ticks: Option<String>,
    pub time_seconds: f64,
    pub duration_seconds: Option<f64>,
    pub timestamp_origin: TimestampOrigin,
    pub key_frame: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StreamInfo {
    pub index: u32,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub time_base_numerator: i32,
    pub time_base_denominator: i32,
    pub rotation: i32,
    pub sample_aspect_ratio: String,
    pub color_space: String,
    pub color_transfer: String,
    pub color_primaries: String,
    pub duration_seconds: Option<f64>,
    #[serde(default)]
    pub bit_depth: Option<u32>,
    #[serde(default)]
    pub color_range: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub streams: Vec<StreamInfo>,
    pub selected_stream: u32,
    pub duration_seconds: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub path: String,
    pub revision_id: String,
    pub index_generation: String,
    pub status: String,
    pub frame_count: String,
    pub metadata: Option<MediaMetadata>,
    pub error: Option<AppError>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FrameView {
    pub frame_ref: FrameRefV1,
    pub time_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub image_path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    pub schema_version: u32,
    pub job_id: String,
    pub sequence: u32,
    pub status: JobStatus,
    pub stage: String,
    pub completed_units: String,
    pub total_units: Option<String>,
    pub message_code: String,
    pub result_ids: Vec<String>,
    pub error: Option<AppError>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, thiserror::Error)]
#[error("{code}: {message}")]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}
impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
pub type AppResult<T> = Result<T, AppError>;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub jpeg_quality: u8,
    pub color_mode: ColorMode,
    #[serde(default)]
    #[ts(optional)]
    pub color_policy: Option<ColorPolicyV2>,
}
impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Png,
            jpeg_quality: 95,
            color_mode: ColorMode::Sdr,
            color_policy: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ColorPolicyV2 {
    pub schema_version: u32,
    pub source_peak_nits: Option<f64>,
    #[serde(default)]
    #[ts(optional)]
    pub source_interpretation: Option<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Png,
    Jpeg,
    Webp,
}
impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    Sdr,
    Preserve16Bit,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeekAnchor {
    pub ordinal: u64,
    pub pts: i64,
    pub time_seconds: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkerRequest {
    Index {
        path: String,
        stream: Option<u32>,
    },
    AnalyzeBatch {
        path: String,
        stream: u32,
        targets: Vec<AnalysisTarget>,
    },
    Decode {
        path: String,
        stream: u32,
        ordinal: u64,
        expected_pts: Option<i64>,
        anchor: Option<SeekAnchor>,
        output: String,
        max_width: Option<u32>,
        options: ExportOptions,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkerMessage {
    Revision {
        sha256: String,
        bytes: u64,
    },
    Metadata {
        metadata: MediaMetadata,
    },
    Frames {
        frames: Vec<IndexedFrame>,
    },
    Image {
        path: String,
        width: u32,
        height: u32,
        sha256: String,
        #[serde(default)]
        conversion: serde_json::Value,
    },
    Quality {
        ordinal: String,
        metrics: QualityMetricsV1,
    },
    Progress {
        stage: String,
        completed: u64,
    },
    Complete,
    Error {
        error: AppError,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub source_id: String,
    pub frame_ref: FrameRefV1,
    pub time_seconds: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportItemV1 {
    pub source_id: String,
    pub frame: Selection,
    pub source_hash: String,
    pub image_path: String,
    pub checksum: Option<String>,
    pub sidecar_checksum: Option<String>,
    pub status: String,
    pub error: Option<AppError>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportPlanV1 {
    pub id: String,
    pub project_id: String,
    pub destination: String,
    pub options: ExportOptions,
    pub items: Vec<ExportItemV1>,
    pub active: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportAttemptSummaryV1 {
    pub job_id: String,
    pub plan_id: String,
    pub accounting_available: bool,
    pub completed: u32,
    pub skipped: u32,
    pub failed: u32,
    pub cancelled: u32,
    pub pending: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QualityMetricsV1 {
    pub sharpness: f64,
    pub blur_estimate: f64,
    pub exposure: f64,
    pub dark_fraction: f64,
    #[serde(default)]
    pub luminance_p05: f64,
    #[serde(default)]
    pub luminance_p50: f64,
    #[serde(default)]
    pub luminance_p95: f64,
    pub bright_fraction: f64,
    pub perceptual_hash: String,
    pub structure: Vec<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisTarget {
    pub ordinal: u64,
    pub expected_pts: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QualityFrameV1 {
    pub frame: Selection,
    pub metrics: QualityMetricsV1,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RankingConfigV1 {
    pub count: u32,
    pub sharpness_weight: f64,
    pub exposure_weight: f64,
    pub blur_weight: f64,
}
impl Default for RankingConfigV1 {
    fn default() -> Self {
        Self {
            count: 8,
            sharpness_weight: 50.,
            exposure_weight: 30.,
            blur_weight: 20.,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationV1 {
    pub quality: QualityFrameV1,
    pub score: f64,
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationSetV1 {
    pub id: String,
    pub source_id: String,
    pub index_generation: String,
    pub status: String,
    pub coverage_complete: bool,
    pub config: RankingConfigV1,
    pub frames: Vec<RecommendationV1>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TensorSpecV1 {
    pub name: String,
    pub shape: Vec<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelManifestV1 {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub task: String,
    pub source_url: String,
    pub license: String,
    pub license_url: String,
    pub sha256: String,
    pub byte_size: String,
    pub input: TensorSpecV1,
    pub outputs: Vec<TensorSpecV1>,
    /// Defaults to NCHW for manifests created before layout support.
    #[serde(default)]
    pub input_layout: Option<String>,
    pub channel_order: String,
    pub scale: f32,
    pub mean: [f32; 3],
    pub labels: Vec<String>,
    pub runtime_version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelInstallationV1 {
    pub id: String,
    pub manifest: ModelManifestV1,
    pub status: String,
    pub path: String,
    pub error: Option<AppError>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfileV1 {
    pub os: String,
    pub architecture: String,
    pub logical_cores: u32,
    pub total_memory_bytes: String,
    pub available_memory_bytes: String,
    pub ai_budget_bytes: String,
    pub worker_threads: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelBenchmarkV1 {
    pub model_id: String,
    pub model_hash: String,
    pub runtime_version: String,
    pub provider: String,
    pub cold_load_ms: f64,
    pub warm_latency_ms: f64,
    pub peak_observed_memory_bytes: String,
    pub hardware: HardwareProfileV1,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LocalRuntimeModelV1 {
    pub id: String,
    pub vision: bool,
    pub local: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LocalRuntimeReportV1 {
    pub runtime: String,
    pub models: Vec<LocalRuntimeModelV1>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TensorOutput {
    pub name: String,
    pub shape: Vec<i64>,
    pub values: Vec<f32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InferenceRequestV1 {
    Download {
        manifest: ModelManifestV1,
        directory: String,
    },
    Discover {
        runtime: String,
        port: u16,
        token: Option<String>,
    },
    Probe {
        runtime: String,
        port: u16,
        model: String,
        token: Option<String>,
    },
    Validate {
        manifest: ModelManifestV1,
        path: String,
    },
    DetectFaces {
        manifest: ModelManifestV1,
        path: String,
        image: String,
    },
    DetectPersons {
        manifest: ModelManifestV1,
        path: String,
        image: String,
    },
    EstimatePose {
        manifest: ModelManifestV1,
        path: String,
        image: String,
        person: PersonDetectionV1,
        mask_output: String,
    },
    Run {
        manifest: ModelManifestV1,
        path: String,
        image: String,
    },
    Benchmark {
        manifest: ModelManifestV1,
        path: String,
    },
    Unload,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InferenceResponseV1 {
    DownloadProgress { completed: u64, total: u64 },
    Downloaded { path: String },
    RuntimeReport { report: LocalRuntimeReportV1 },
    RuntimeProbe { latency_ms: f64 },
    Validated,
    Faces { result: FaceResultV1 },
    Persons { result: PersonResultV1 },
    Pose { result: PoseResultV1 },
    Outputs { tensors: Vec<TensorOutput> },
    Benchmark { result: ModelBenchmarkV1 },
    Unloaded,
    Error { error: AppError },
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamp_round_trip_preserves_large_integers() {
        let r = FrameRefV1 {
            source_revision_id: "r".into(),
            stream_index: 0,
            index_generation: "g".into(),
            presentation_ordinal: "9007199254740993".into(),
            pts_ticks: Some("9223372036854775807".into()),
            time_base_numerator: 1,
            time_base_denominator: 90000,
            timestamp_origin: TimestampOrigin::Source,
        };
        let s = serde_json::to_string(&r).unwrap();
        let d: FrameRefV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(d.pts_ticks, r.pts_ticks);
        assert_eq!(d.presentation_ordinal, r.presentation_ordinal);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PresetV1 {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub weights: [f64; 5],
    pub count: u32,
    pub required_capabilities: Vec<String>,
    pub automatic_available: bool,
    pub unavailable_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RegionRectV1 {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GuidedRegionV1 {
    #[serde(default)]
    pub subject: Option<SubjectReferenceV1>,
    pub id: String,
    pub preset_id: String,
    pub frame: Selection,
    pub rectangle: RegionRectV1,
    pub metrics: QualityMetricsV1,
    pub origin: String,
    #[serde(default)]
    pub algorithm: String,
    #[serde(default)]
    pub color_policy: String,
}

/// Coordinates are normalized to the oriented source image; landmarks may lie outside it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LandmarkV1 {
    pub x: f64,
    pub y: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FaceDetectionV1 {
    pub rectangle: RegionRectV1,
    /// Detector score, not an identity or calibrated visibility probability.
    pub detection_score: f64,
    /// Right eye, left eye, nose, right mouth corner, left mouth corner.
    pub landmarks: [LandmarkV1; 5],
    pub in_frame_fraction: f64,
    pub all_landmarks_in_frame: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FaceResultV1 {
    pub width: u32,
    pub height: u32,
    pub algorithm: String,
    pub model_hash: String,
    pub faces: Vec<FaceDetectionV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FaceFrameV1 {
    pub id: String,
    pub model_id: String,
    pub frame: Selection,
    pub color_policy: String,
    pub result: FaceResultV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersonDetectionV1 {
    /// Source pixel xyxy detector region; this is not a full-body bounding box.
    pub detector_box: [f64; 4],
    /// Hip, full-body extent, shoulder, upper-body extent in source pixel coordinates.
    pub landmarks: [[f64; 2]; 4],
    /// Model score, not calibrated visibility or identity confidence.
    pub score: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersonResultV1 {
    pub width: u32,
    pub height: u32,
    pub algorithm: String,
    pub model_hash: String,
    pub persons: Vec<PersonDetectionV1>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PoseLandmarkV1 {
    pub x: f64,
    pub y: f64,
    /// Depth in source-pixel scale relative to the hip; not physical distance.
    pub z: f64,
    /// Model signals, requiring held-out calibration before preset eligibility.
    pub visibility: f64,
    pub presence: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PoseDetectionV1 {
    pub score: f64,
    pub box_xyxy: [f64; 4],
    /// 33 body landmarks followed by six auxiliary model points, in source pixels.
    pub landmarks: Vec<PoseLandmarkV1>,
    /// Model-estimated coordinates; not verified anthropometric measurements.
    pub world_landmarks: Vec<[f64; 3]>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PoseResultV1 {
    pub width: u32,
    pub height: u32,
    pub algorithm: String,
    pub model_hash: String,
    pub pose: Option<PoseDetectionV1>,
    pub mask_path: Option<String>,
    pub mask_sha256: Option<String>,
}

/// Candidate regions tied to an indexed frame; not automatic visibility eligibility.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersonFrameV1 {
    pub id: String,
    pub model_id: String,
    pub frame: Selection,
    pub color_policy: String,
    pub result: PersonResultV1,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PoseFrameV1 {
    pub id: String,
    pub model_id: String,
    pub person_feature_id: String,
    pub person_index: u32,
    pub frame: Selection,
    pub color_policy: String,
    pub result: PoseResultV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubjectReferenceV1 {
    pub person_feature_id: String,
    pub person_index: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubjectChoiceV1 {
    pub model_id: String,
    pub subject: SubjectReferenceV1,
    pub frame: Selection,
}

/// Experimental execution is opt-in until human visibility and tracking gates pass.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PresetExecutionV1 {
    pub source_id: String,
    pub preset: PresetV1,
    pub anchor_ordinal: String,
    pub person_model_id: String,
    pub person_index: u32,
    pub face_model_id: Option<String>,
    pub custom_rectangle: Option<RegionRectV1>,
    pub experimental: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TrackObservationV1 {
    pub frame: Selection,
    pub subject: SubjectReferenceV1,
    pub rectangle: RegionRectV1,
    pub region_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubjectTrackV1 {
    pub id: String,
    pub source_id: String,
    pub index_generation: String,
    pub algorithm: String,
    pub cache_key: String,
    pub observations: Vec<TrackObservationV1>,
    pub coverage_complete: bool,
    pub stop_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PresetResultV1 {
    pub job_id: String,
    pub track: SubjectTrackV1,
    pub preset: PresetV1,
    pub frames: Vec<RecommendationV1>,
    pub validation_status: String,
    pub limitations: Vec<String>,
}
