use frameiq_contracts::*;
pub fn credentials() -> AppResult<Option<String>> {
    let e = keyring::Entry::new("com.frameiq.shopping", "ebay").map_err(|_| error())?;
    match e.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(error()),
    }
}
fn error() -> AppError {
    AppError::new(
        "CREDENTIAL_STORE",
        "Unlock OS credential storage and try again.",
    )
}
#[tauri::command]
pub async fn ebay_status() -> AppResult<bool> {
    tauri::async_runtime::spawn_blocking(|| Ok(credentials()?.is_some()))
        .await
        .map_err(|_| error())?
}
#[tauri::command]
pub async fn save_ebay_credentials(
    client_id: String,
    client_secret: String,
    approved: bool,
) -> AppResult<()> {
    if !approved
        || client_id.trim().is_empty()
        || client_secret.trim().is_empty()
        || client_id.len() > 4096
        || client_secret.len() > 4096
    {
        return Err(AppError::new(
            "SHOPPING_ACCESS",
            "Confirm approved production access and enter your own eBay application credentials.",
        ));
    }
    tauri::async_runtime::spawn_blocking(move || {
        keyring::Entry::new("com.frameiq.shopping", "ebay")
            .map_err(|_| error())?
            .set_password(
                &serde_json::json!({"clientId":client_id,"clientSecret":client_secret}).to_string(),
            )
            .map_err(|_| error())
    })
    .await
    .map_err(|_| error())?
}
#[tauri::command]
pub async fn remove_ebay_credentials() -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(|| {
        match keyring::Entry::new("com.frameiq.shopping", "ebay")
            .map_err(|_| error())?
            .delete_credential()
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(error()),
        }
    })
    .await
    .map_err(|_| error())?
}
