use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaRefV1 {
    Frame { selection: Selection },
    Photo { revision: String },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PhotoSourceV1 {
    pub id: String,
    pub project_id: String,
    pub revision: String,
    pub name: String,
    pub path: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub image_path: String,
    pub image_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AssetRefV1 {
    pub path: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FashionAttributeV1 {
    pub name: String,
    pub value: String,
    /// observed, inferred, or user. Scores are model signals, never identity probabilities.
    pub origin: String,
    pub confidence: Option<f64>,
    pub evidence_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FashionItemV1 {
    pub id: String,
    pub project_id: String,
    pub media: MediaRefV1,
    pub revision: u32,
    pub category: String,
    pub rectangle: RegionRectV1,
    pub polygon: Vec<[f64; 2]>,
    pub mask: Option<AssetRefV1>,
    pub crop: Option<AssetRefV1>,
    pub attributes: Vec<FashionAttributeV1>,
    #[serde(default)]
    pub suppressed_attributes: Vec<String>,
    pub detection_score: Option<f64>,
    pub provenance: String,
    pub model_hash: Option<String>,
    pub corrected: bool,
    pub deleted: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GarmentGroupV1 {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub revision: u32,
    pub item_ids: Vec<String>,
    pub notes: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductEvidenceV1 {
    pub id: String,
    pub url: String,
    pub title: String,
    pub excerpt: String,
    pub independence_group: String,
    pub identifier: Option<String>,
    pub authoritative: bool,
    pub observed_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductV1 {
    pub id: String,
    pub name: String,
    pub brand: Option<String>,
    pub style_id: Option<String>,
    pub category: String,
    pub source_url: String,
    pub image: Option<AssetRefV1>,
    pub image_url: Option<String>,
    pub image_rights: String,
    #[serde(default)]
    pub image_attribution: Option<String>,
    #[serde(default)]
    pub image_license_url: Option<String>,
    pub attributes: Vec<FashionAttributeV1>,
    pub embedding: Vec<f32>,
    pub embedding_model: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RetailerListingV1 {
    pub id: String,
    pub product_id: String,
    pub provider: String,
    pub retailer: String,
    pub url: String,
    pub price: Option<String>,
    pub currency: Option<String>,
    pub availability: Option<String>,
    pub observed_at: String,
    pub evidence_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MatchClassV1 {
    VerifiedExact,
    ProbableExact,
    VisuallySimilar,
    Unidentified,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductCandidateV1 {
    pub id: String,
    pub group_id: String,
    pub group_revision: u32,
    pub product: ProductV1,
    pub listings: Vec<RetailerListingV1>,
    pub evidence: Vec<ProductEvidenceV1>,
    pub classification: MatchClassV1,
    pub similarity: Option<f64>,
    pub reason: String,
    pub user_confirmed: bool,
    pub stale: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LookItemV1 {
    pub group_id: String,
    pub candidate_id: Option<String>,
    pub alternative_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SavedLookV1 {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub notes: String,
    pub items: Vec<LookItemV1>,
    pub revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FashionTaskV1 {
    Detect,
    Attributes,
    Research,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FashionReviewRequestV1 {
    pub project_id: String,
    pub media: Vec<MediaRefV1>,
    pub item_ids: Vec<String>,
    pub group_id: Option<String>,
    pub task: FashionTaskV1,
    pub model: String,
    pub account_id: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub shopping_provider: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FashionReviewV1 {
    pub id: String,
    pub request: FashionReviewRequestV1,
    pub images: Vec<AssetRefV1>,
    pub group_revision: Option<u32>,
    pub item_revisions: Vec<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FeatureCapabilityV1 {
    pub task: String,
    pub status: String,
    pub reason: Option<String>,
    pub model_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskTensorV2 {
    pub name: String,
    pub shape: Vec<u32>,
    pub dtype: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactV2 {
    pub name: String,
    pub sha256: String,
    pub byte_size: String,
    pub source_url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskPackV2 {
    pub schema_version: u32,
    pub id: String,
    pub task: String,
    pub adapter: String,
    pub inputs: Vec<TaskTensorV2>,
    pub outputs: Vec<TaskTensorV2>,
    pub artifacts: Vec<TaskArtifactV2>,
    pub license: String,
    pub license_url: String,
    pub preprocessing: String,
    pub qualification: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FashionExportV1 {
    pub id: String,
    pub look: SavedLookV1,
    pub destination: String,
    pub format: String,
    pub completed: Vec<String>,
    pub status: String,
    #[serde(default)]
    #[ts(type = "unknown")]
    pub snapshot: serde_json::Value,
    #[serde(default)]
    pub files: Vec<FashionExportFileV1>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FashionExportFileV1 {
    pub name: String,
    pub sha256: String,
    pub source: Option<AssetRefV1>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetectionV2 {
    pub category: String,
    pub rectangle: RegionRectV1,
    pub score: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskResultV2 {
    pub pack_id: String,
    pub model_hash: String,
    pub detections: Vec<TaskDetectionV2>,
    pub mask: Option<AssetRefV1>,
    pub embedding: Vec<f32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TargetFeaturesV2 {
    pub visibility: f64,
    pub coverage: f64,
    pub composition: f64,
    pub occlusion: f64,
    pub orientation: String,
    pub orientation_confidence: f64,
    pub provenance: String,
    #[serde(default)]
    pub pose_signature: Vec<[f64; 2]>,
}
