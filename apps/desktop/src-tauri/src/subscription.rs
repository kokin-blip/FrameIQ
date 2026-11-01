use frameiq_contracts::*;
use frameiq_inference::subscription::{Account, SignIn};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, MutexGuard,
};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

#[derive(Default, Clone)]
pub struct Bridge(Arc<Mutex<Inner>>);
#[derive(Default)]
struct Inner {
    pending: Option<Arc<AtomicBool>>,
    error: Option<AppError>,
}
#[derive(Serialize, Deserialize)]
struct Store {
    host_id: String,
    accounts: Vec<Account>,
    active: Option<String>,
}
fn storage_error() -> AppError {
    AppError::new(
        "CREDENTIAL_STORE",
        "Unlock OS credential storage and try again.",
    )
}
fn entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new("com.frameiq.subscription", "chatgpt-v1").map_err(|_| storage_error())
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    generation: String,
    chunks: usize,
}
fn chunk_entry(manifest: &Manifest, index: usize) -> AppResult<keyring::Entry> {
    keyring::Entry::new(
        "com.frameiq.subscription",
        &format!("chatgpt-{}-{index}", manifest.generation),
    )
    .map_err(|_| storage_error())
}
fn manifest() -> AppResult<Option<Manifest>> {
    match entry()?.get_password() {
        Ok(value) => {
            let value: Manifest = serde_json::from_str(&value).map_err(|_| storage_error())?;
            if uuid::Uuid::parse_str(&value.generation).is_err()
                || value.chunks == 0
                || value.chunks > 1024
            {
                return Err(storage_error());
            }
            Ok(Some(value))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(storage_error()),
    }
}
fn load() -> AppResult<Store> {
    if let Some(manifest) = manifest()? {
        let mut bytes = Vec::new();
        for index in 0..manifest.chunks {
            bytes.extend(
                chunk_entry(&manifest, index)?
                    .get_secret()
                    .map_err(|_| storage_error())?,
            );
        }
        serde_json::from_slice(&bytes).map_err(|_| storage_error())
    } else {
        Ok(Store {
            host_id: format!("urn:uuid:{}", uuid::Uuid::new_v4()),
            accounts: Vec::new(),
            active: None,
        })
    }
}
fn save(store: &Store) -> AppResult<()> {
    // Windows vault entries have a small blob limit. Stage bounded binary chunks,
    // then atomically replace the manifest so refresh rotation cannot tear a record.
    let bytes = serde_json::to_vec(store).map_err(|_| storage_error())?;
    let old = manifest()?;
    let next = Manifest {
        generation: uuid::Uuid::new_v4().to_string(),
        chunks: bytes.len().div_ceil(2000),
    };
    if next.chunks > 1024 {
        return Err(storage_error());
    }
    let outcome = (|| {
        for (index, chunk) in bytes.chunks(2000).enumerate() {
            chunk_entry(&next, index)?
                .set_secret(chunk)
                .map_err(|_| storage_error())?;
        }
        entry()?
            .set_password(&serde_json::to_string(&next).map_err(|_| storage_error())?)
            .map_err(|_| storage_error())
    })();
    let obsolete = if outcome.is_ok() {
        old.as_ref()
    } else {
        Some(&next)
    };
    if let Some(manifest) = obsolete {
        for index in 0..manifest.chunks {
            if let Ok(entry) = chunk_entry(manifest, index) {
                let _ = entry.delete_credential();
            }
        }
    }
    outcome
}
impl Bridge {
    fn lock(&self) -> AppResult<MutexGuard<'_, Inner>> {
        self.0.lock().map_err(|_| storage_error())
    }
    fn status(&self) -> AppResult<SubscriptionStatusV1> {
        let inner = self.lock()?;
        let store = load()?;
        Ok(SubscriptionStatusV1 {
            accounts: store.accounts.iter().map(Account::public).collect(),
            active_account_id: store.active,
            signing_in: inner.pending.is_some(),
            error: inner.error.clone(),
        })
    }
    pub fn account_id(&self) -> AppResult<String> {
        let _guard = self.lock()?;
        let store = load()?;
        store
            .active
            .filter(|id| {
                store
                    .accounts
                    .iter()
                    .any(|a| &a.id == id && a.tokens.is_some())
            })
            .ok_or_else(|| {
                AppError::new("AUTH_REQUIRED", "Continue with ChatGPT in Settings first.")
            })
    }
    /// Serialize refresh-token rotation, including durable storage, across native requests.
    pub fn access_token(&self, expected_account: Option<&str>) -> AppResult<String> {
        let _guard = self.lock()?;
        let mut store = load()?;
        if expected_account.is_some_and(|id| store.active.as_deref() != Some(id)) {
            return Err(AppError::new(
                "ACCOUNT_CHANGED",
                "The ChatGPT account changed. Review the images again before sending.",
            ));
        }
        let active = store.active.clone();
        let account = store
            .accounts
            .iter_mut()
            .find(|a| Some(&a.id) == active.as_ref())
            .ok_or_else(|| {
                AppError::new("AUTH_REQUIRED", "Continue with ChatGPT in Settings first.")
            })?;
        let result = account.refresh_if_needed();
        let token = account.tokens.as_ref().map(|t| t.access_token.clone());
        save(&store)?;
        result?;
        token.ok_or_else(|| AppError::new("AUTH_REQUIRED", "Continue with ChatGPT again."))
    }
}

#[tauri::command]
pub async fn subscription_status(
    bridge: tauri::State<'_, Bridge>,
) -> AppResult<SubscriptionStatusV1> {
    let bridge = bridge.inner().clone();
    tauri::async_runtime::spawn_blocking(move || bridge.status())
        .await
        .map_err(|_| storage_error())?
}
#[tauri::command]
pub async fn start_subscription_login(
    app: tauri::AppHandle,
    account_id: Option<String>,
) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let bridge = app.state::<Bridge>().inner().clone();
        let mut inner = bridge.lock()?;
        if inner.pending.is_some() {
            return Err(AppError::new(
                "OAUTH_PENDING",
                "Complete or cancel the current ChatGPT sign-in first.",
            ));
        }
        let store = load()?;
        if store.accounts.len() >= 16 && account_id.is_none() {
            return Err(AppError::new(
                "ACCOUNT_LIMIT",
                "Reconnect an existing ChatGPT account.",
            ));
        }
        let previous = match account_id {
            Some(id) => Some(
                store
                    .accounts
                    .iter()
                    .find(|a| a.id == id)
                    .cloned()
                    .ok_or_else(storage_error)?,
            ),
            None => None,
        };
        let attempt = SignIn::new(previous)?;
        save(&store)?; // Persist the stable host identifier before opening the browser.
        let cancelled = Arc::new(AtomicBool::new(false));
        // Open the fixed OpenAI URL natively. Hints and OAuth transaction values never enter the webview.
        app.opener()
            .open_url(attempt.authorization_url(&store.host_id), None::<&str>)
            .map_err(|_| {
                AppError::new(
                    "OAUTH_BROWSER",
                    "Could not open your browser. Check the default browser and try again.",
                )
            })?;
        inner.pending = Some(cancelled.clone());
        inner.error = None;
        drop(inner);
        std::thread::spawn(move || {
            let outcome = attempt.complete(&cancelled);
            if let Ok(mut inner) = bridge.lock() {
                // A cancelled older attempt must not overwrite a newer login or its error.
                if !inner
                    .pending
                    .as_ref()
                    .is_some_and(|p| Arc::ptr_eq(p, &cancelled))
                {
                    return;
                }
                let result = if cancelled.load(Ordering::SeqCst) {
                    Ok(())
                } else {
                    outcome.and_then(|account| {
                        let mut store = load()?;
                        let id = account.id.clone();
                        // The issued client and verified subject identify a registration, never the email.
                        if let Some(existing) = store.accounts.iter_mut().find(|a| {
                            a.client_id == account.client_id && a.subject == account.subject
                        }) {
                            let existing_id = existing.id.clone();
                            *existing = Account {
                                id: existing_id.clone(),
                                ..account
                            };
                            store.active = Some(existing_id);
                        } else {
                            store.accounts.push(account);
                            store.active = Some(id);
                        }
                        save(&store)
                    })
                };
                inner.pending = None;
                inner.error = result.err();
            }
        });
        Ok(())
    })
    .await
    .map_err(|_| storage_error())?
}
#[tauri::command]
pub fn cancel_subscription_login(bridge: tauri::State<Bridge>) -> AppResult<()> {
    let mut inner = bridge.lock()?;
    if let Some(pending) = inner.pending.take() {
        pending.store(true, Ordering::SeqCst);
    }
    inner.error = None;
    Ok(())
}
#[tauri::command]
pub async fn select_subscription_account(
    bridge: tauri::State<'_, Bridge>,
    account_id: String,
) -> AppResult<()> {
    let bridge = bridge.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut inner = bridge.lock()?;
        if inner.pending.is_some() {
            return Err(AppError::new(
                "OAUTH_PENDING",
                "Complete or cancel ChatGPT sign-in before switching accounts.",
            ));
        }
        let mut store = load()?;
        if !store.accounts.iter().any(|a| a.id == account_id) {
            return Err(storage_error());
        }
        store.active = Some(account_id);
        save(&store)?;
        inner.error = None;
        Ok(())
    })
    .await
    .map_err(|_| storage_error())?
}
#[tauri::command]
pub async fn disconnect_subscription(bridge: tauri::State<'_, Bridge>) -> AppResult<()> {
    let bridge = bridge.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut inner = bridge.lock()?;
        if let Some(pending) = inner.pending.take() {
            pending.store(true, Ordering::SeqCst);
        }
        inner.error = None;
        let mut store = load()?;
        let active = store.active.clone();
        let result = store
            .accounts
            .iter_mut()
            .find(|a| Some(&a.id) == active.as_ref())
            .map(Account::revoke)
            .unwrap_or(Ok(()));
        save(&store)?;
        result
    })
    .await
    .map_err(|_| storage_error())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyring::credential::{Credential, CredentialApi, CredentialBuilderApi};
    use std::{any::Any, collections::HashMap};

    #[derive(Clone, Default)]
    struct Vault {
        values: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        fail_commit: Arc<AtomicBool>,
    }
    struct VaultEntry {
        vault: Vault,
        name: String,
    }
    impl CredentialBuilderApi for Vault {
        fn build(&self, _: Option<&str>, _: &str, user: &str) -> keyring::Result<Box<Credential>> {
            Ok(Box::new(VaultEntry {
                vault: self.clone(),
                name: user.into(),
            }))
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }
    impl CredentialApi for VaultEntry {
        fn set_secret(&self, value: &[u8]) -> keyring::Result<()> {
            assert!(value.len() <= 2000);
            if self.name == "chatgpt-v1" && self.vault.fail_commit.swap(false, Ordering::SeqCst) {
                return Err(keyring::Error::NoEntry);
            }
            self.vault
                .values
                .lock()
                .unwrap()
                .insert(self.name.clone(), value.to_vec());
            Ok(())
        }
        fn get_secret(&self) -> keyring::Result<Vec<u8>> {
            self.vault
                .values
                .lock()
                .unwrap()
                .get(&self.name)
                .cloned()
                .ok_or(keyring::Error::NoEntry)
        }
        fn delete_credential(&self) -> keyring::Result<()> {
            self.vault.values.lock().unwrap().remove(&self.name);
            Ok(())
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }
    #[test]
    fn vault_commits_large_sessions_atomically_and_binds_reviews_to_accounts() {
        let vault = Vault::default();
        keyring::set_default_credential_builder(Box::new(vault.clone()));
        let mut store = Store {
            host_id: "urn:uuid:stable".into(),
            active: Some("registration".into()),
            accounts: vec![Account {
                id: "registration".into(),
                client_id: "oaiapp_saved".into(),
                subject: "verified-user".into(),
                email: Some("test@example.com".into()),
                tokens: Some(frameiq_inference::subscription::Tokens {
                    access_token: "access".repeat(1000),
                    refresh_token: "refresh".repeat(1000),
                    id_token: "id-token".repeat(1000),
                    expires_at: u64::MAX,
                    scopes: vec!["chatgpt.tokens.use.direct".into()],
                }),
            }],
        };
        save(&store).unwrap();
        let old = manifest().unwrap().unwrap();
        assert!(old.chunks > 1);
        assert_eq!(load().unwrap().host_id, store.host_id);
        let before = load().unwrap().accounts[0]
            .tokens
            .as_ref()
            .unwrap()
            .access_token
            .clone();
        store.accounts[0].tokens.as_mut().unwrap().access_token = "rotated-access".into();
        vault.fail_commit.store(true, Ordering::SeqCst);
        assert!(save(&store).is_err());
        assert_eq!(
            load().unwrap().accounts[0]
                .tokens
                .as_ref()
                .unwrap()
                .access_token,
            before
        );
        assert_eq!(vault.values.lock().unwrap().len(), old.chunks + 1);
        save(&store).unwrap();
        assert_eq!(
            load().unwrap().accounts[0]
                .tokens
                .as_ref()
                .unwrap()
                .access_token,
            "rotated-access"
        );
        assert!(!vault
            .values
            .lock()
            .unwrap()
            .keys()
            .any(|key| key.contains(&old.generation)));
        let bridge = Bridge::default();
        assert_eq!(
            bridge
                .access_token(Some("different-account"))
                .err()
                .unwrap()
                .code,
            "ACCOUNT_CHANGED"
        );
        assert_eq!(
            bridge.access_token(Some("registration")).unwrap(),
            "rotated-access"
        );
        let public = serde_json::to_string(&bridge.status().unwrap()).unwrap();
        for secret in [
            "rotated-access",
            "refresh_token",
            "access_token",
            "id_token",
        ] {
            assert!(!public.contains(secret));
        }
        store.accounts[0].tokens = None;
        save(&store).unwrap();
        let loaded = load().unwrap();
        assert_eq!(loaded.host_id, "urn:uuid:stable");
        assert_eq!(loaded.accounts[0].client_id, "oaiapp_saved");
        assert!(!loaded.accounts[0].public().connected);
        keyring::set_default_credential_builder(keyring::default::default_credential_builder());
    }
}
