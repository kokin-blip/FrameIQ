use frameiq_contracts::*;
fn entry(provider: CloudProviderV1) -> AppResult<keyring::Entry> {
    let name = match provider {
        CloudProviderV1::Openai => "openai",
        CloudProviderV1::Gemini => "gemini",
    };
    keyring::Entry::new("com.frameiq.cloud", name).map_err(|_| store_error())
}
fn store_error() -> AppError {
    AppError::new(
        "CREDENTIAL_STORE",
        "Unlock OS credential storage and try again.",
    )
}
fn key(provider: CloudProviderV1) -> AppResult<Option<String>> {
    match entry(provider)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(store_error()),
    }
}
#[tauri::command]
pub async fn cloud_key_status(provider: CloudProviderV1) -> AppResult<CloudCredentialStatusV1> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(CloudCredentialStatusV1 {
            provider,
            saved: key(provider)?.is_some(),
        })
    })
    .await
    .map_err(|_| store_error())?
}
#[tauri::command]
pub async fn save_cloud_key(provider: CloudProviderV1, api_key: String) -> AppResult<()> {
    frameiq_inference::cloud::validate_key(&api_key)?;
    tauri::async_runtime::spawn_blocking(move || {
        entry(provider)?
            .set_password(&api_key)
            .map_err(|_| store_error())
    })
    .await
    .map_err(|_| store_error())?
}
#[tauri::command]
pub async fn remove_cloud_key(provider: CloudProviderV1) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(store_error()),
    })
    .await
    .map_err(|_| store_error())?
}
#[tauri::command]
pub async fn test_cloud_connection(provider: CloudProviderV1) -> AppResult<CloudConnectionV1> {
    tauri::async_runtime::spawn_blocking(move || {
        let key = key(provider)?
            .ok_or_else(|| AppError::new("AUTH_REQUIRED", "Save an API key first."))?;
        frameiq_inference::cloud::test_connection(provider, &key)
    })
    .await
    .map_err(|_| AppError::new("CLOUD_CONNECTION", "Connection test interrupted."))?
}

#[tauri::command]
pub fn prepare_cloud_review(
    app: tauri::State<frameiq_application::Application>,
    request: CloudReviewRequestV1,
) -> AppResult<JobEvent> {
    app.prepare_cloud_review(request)
}
#[tauri::command]
pub fn cloud_review(
    app: tauri::State<frameiq_application::Application>,
    id: String,
) -> AppResult<CloudReviewV1> {
    app.cloud_review(&id)
}
#[tauri::command]
pub fn discard_cloud_review(
    app: tauri::State<frameiq_application::Application>,
    id: String,
) -> AppResult<()> {
    app.discard_cloud_review(&id)
}
#[tauri::command]
pub async fn analyze_cloud_review(
    app: tauri::State<'_, frameiq_application::Application>,
    id: String,
    consent: bool,
) -> AppResult<JobEvent> {
    if !consent {
        return Err(AppError::new(
            "CONSENT_REQUIRED",
            "Approve this provider request first.",
        ));
    }
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let review = app.cloud_review(&id)?;
        let key = key(review.provider)?.ok_or_else(|| {
            AppError::new("AUTH_REQUIRED", "Save a provider key in Settings first.")
        })?;
        app.analyze_cloud_review(&id, key, true)
    })
    .await
    .map_err(|_| store_error())?
}
#[tauri::command]
pub fn cloud_result(
    app: tauri::State<frameiq_application::Application>,
    id: String,
) -> AppResult<Option<CloudAnalysisResultV1>> {
    app.cloud_result(&id)
}
