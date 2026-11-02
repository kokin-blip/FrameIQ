use frameiq_application::Application;
use frameiq_contracts::*;
use std::path::PathBuf;
use tauri::State;
#[tauri::command]
pub fn photos(app: State<Application>, project: String) -> AppResult<Vec<PhotoSourceV1>> {
    app.photos(&project)
}
#[tauri::command]
pub fn import_photo(
    app: State<Application>,
    project: String,
    path: PathBuf,
) -> AppResult<JobEvent> {
    app.import_photo(&project, &path)
}
#[tauri::command]
pub async fn fashion_media(
    app: State<'_, Application>,
    project: String,
    media: MediaRefV1,
) -> AppResult<AssetRefV1> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.media_asset(&project, &media))
        .await
        .map_err(|_| AppError::new("MEDIA_ERROR", "Media loading interrupted."))?
}
#[tauri::command]
pub fn fashion_items(app: State<Application>, project: String) -> AppResult<Vec<FashionItemV1>> {
    app.fashion_items(&project)
}
#[tauri::command]
pub fn save_fashion_item(app: State<Application>, item: FashionItemV1) -> AppResult<JobEvent> {
    app.save_fashion_item(item)
}
#[tauri::command]
pub fn delete_fashion_item(app: State<Application>, id: String) -> AppResult<()> {
    app.delete_fashion_item(&id)
}
#[tauri::command]
pub fn undo_fashion_item(app: State<Application>, id: String) -> AppResult<FashionItemV1> {
    app.undo_fashion_item(&id)
}
#[tauri::command]
pub fn garment_groups(app: State<Application>, project: String) -> AppResult<Vec<GarmentGroupV1>> {
    app.garment_groups(&project)
}
#[tauri::command]
pub fn save_garment_group(
    app: State<Application>,
    group: GarmentGroupV1,
) -> AppResult<GarmentGroupV1> {
    app.save_garment_group(group)
}
#[tauri::command]
pub async fn associate_garments(
    app: State<'_, Application>,
    project: String,
) -> AppResult<JobEvent> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.associate_garments(&project))
        .await
        .map_err(|_| AppError::new("ASSOCIATION", "Association interrupted."))?
}
#[tauri::command]
pub fn product_candidates(
    app: State<Application>,
    group: String,
) -> AppResult<Vec<ProductCandidateV1>> {
    app.candidates(&group)
}
#[tauri::command]
pub fn confirm_product_candidate(
    app: State<Application>,
    group: String,
    id: String,
    identifier: String,
    source_url: String,
    authoritative: bool,
) -> AppResult<ProductCandidateV1> {
    app.confirm_candidate(&group, &id, &identifier, &source_url, authoritative)
}
#[tauri::command]
pub fn saved_looks(app: State<Application>, project: String) -> AppResult<Vec<SavedLookV1>> {
    app.looks(&project)
}
#[tauri::command]
pub fn save_look(app: State<Application>, look: SavedLookV1) -> AppResult<SavedLookV1> {
    app.save_look(look)
}
#[tauri::command]
pub fn delete_look(app: State<Application>, id: String) -> AppResult<()> {
    app.delete_look(&id)
}
#[tauri::command]
pub fn fashion_capabilities(app: State<Application>) -> AppResult<Vec<FeatureCapabilityV1>> {
    app.fashion_capabilities()
}
#[tauri::command]
pub fn fashion_model_catalog(app: State<Application>) -> AppResult<Vec<TaskPackV2>> {
    app.fashion_model_catalog()
}
#[tauri::command]
pub fn download_task_pack(app: State<Application>, id: String, reviewed: bool) -> AppResult<JobEvent> {
    app.download_task_pack(&id, reviewed)
}
#[tauri::command]
pub fn detect_fashion_local(
    app: State<Application>,
    project: String,
    media: MediaRefV1,
) -> AppResult<JobEvent> {
    app.detect_fashion_local(&project, media)
}
#[tauri::command]
pub fn refine_item_mask(app: State<Application>, id: String) -> AppResult<JobEvent> {
    app.refine_item_mask(&id)
}
#[tauri::command]
pub fn catalog_products(app: State<Application>) -> AppResult<Vec<ProductV1>> {
    app.catalog()
}
#[tauri::command]
pub async fn import_catalog(app: State<'_, Application>, path: PathBuf) -> AppResult<JobEvent> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.import_catalog(&path))
        .await
        .map_err(|_| AppError::new("CATALOG_IMPORT", "Catalog reading interrupted."))?
}
#[tauri::command]
pub async fn search_catalog(
    app: State<'_, Application>,
    group: String,
    query: String,
) -> AppResult<Vec<ProductCandidateV1>> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.search_catalog(&group, &query))
        .await
        .map_err(|_| AppError::new("CATALOG_SEARCH", "Search interrupted."))?
}
#[tauri::command]
pub async fn import_task_pack(
    app: State<'_, Application>,
    manifest: PathBuf,
    reviewed: bool,
) -> AppResult<JobEvent> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.import_task_pack(&manifest, reviewed))
        .await
        .map_err(|_| AppError::new("TASK_PACK", "Package validation interrupted."))?
}
#[tauri::command]
pub async fn remove_task_pack(app: State<'_, Application>, id: String) -> AppResult<()> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.remove_task_pack(&id))
        .await
        .map_err(|_| AppError::new("TASK_PACK", "Package removal interrupted."))?
}

#[tauri::command]
pub async fn read_task_pack(path: PathBuf) -> AppResult<TaskPackV2> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| AppError::new("TASK_PACK", "Choose a package manifest."))?
            .take(128 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| AppError::new("TASK_PACK", "Cannot read manifest."))?;
        if bytes.len() > 128 * 1024 {
            return Err(AppError::new("TASK_PACK", "Manifest exceeds limit."));
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| AppError::new("TASK_PACK", "Invalid task-package manifest."))
    })
    .await
    .map_err(|_| AppError::new("TASK_PACK", "Manifest reading interrupted."))?
}
#[tauri::command]
pub async fn prepare_fashion_review(
    app: State<'_, Application>,
    bridge: State<'_, crate::subscription::Bridge>,
    mut request: FashionReviewRequestV1,
) -> AppResult<JobEvent> {
    let app = app.inner().clone();
    let bridge = bridge.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        request.account_id =
            if request.runtime.is_none() && request.shopping_provider.as_deref() != Some("ebay") {
                Some(bridge.account_id()?)
            } else {
                None
            };
        app.prepare_fashion_review(request)
    })
    .await
    .map_err(|_| AppError::new("FASHION_REVIEW", "Preparation interrupted."))?
}
#[tauri::command]
pub fn fashion_review(app: State<Application>, id: String) -> AppResult<FashionReviewV1> {
    app.fashion_review(&id)
}
#[tauri::command]
pub fn discard_fashion_review(app: State<Application>, id: String) -> AppResult<()> {
    app.discard_fashion_review(&id)
}
#[tauri::command]
pub async fn run_fashion_review(
    app: State<'_, Application>,
    bridge: State<'_, crate::subscription::Bridge>,
    id: String,
    consent: bool,
) -> AppResult<JobEvent> {
    if !consent {
        return Err(AppError::new(
            "CONSENT_REQUIRED",
            "Review and approve the exact images first.",
        ));
    }
    let app = app.inner().clone();
    let bridge = bridge.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let review = app.fashion_review(&id)?;
        let key = if review.request.shopping_provider.as_deref() == Some("ebay") {
            Some(crate::shopping::credentials()?.ok_or_else(|| {
                AppError::new(
                    "SHOPPING_AUTH",
                    "Configure your optional eBay production credentials in Settings.",
                )
            })?)
        } else if let Some(r) = review.request.runtime.as_deref() {
            crate::runtime_token(r)?
        } else {
            let account = review.request.account_id.as_deref().ok_or_else(|| {
                AppError::new(
                    "ACCOUNT_CHANGED",
                    "Review these images with the current ChatGPT account.",
                )
            })?;
            Some(bridge.access_token(Some(account))?)
        };
        app.run_fashion_review(&id, key, true)
    })
    .await
    .map_err(|_| AppError::new("FASHION_REVIEW", "Analysis interrupted."))?
}

#[tauri::command]
pub fn fashion_exports(
    app: State<Application>,
    project: String,
) -> AppResult<Vec<FashionExportV1>> {
    app.fashion_exports(&project)
}
#[tauri::command]
pub fn export_look(
    app: State<Application>,
    project: String,
    id: String,
    destination: PathBuf,
    format: String,
) -> AppResult<JobEvent> {
    app.export_look(&project, &id, &destination, &format)
}
#[tauri::command]
pub fn resume_look_export(app: State<Application>, id: String) -> AppResult<JobEvent> {
    app.resume_look_export(&id)
}

#[tauri::command]
pub fn fashion_history(
    app: State<Application>,
    project: String,
) -> AppResult<Vec<serde_json::Value>> {
    app.fashion_history(&project)
}
