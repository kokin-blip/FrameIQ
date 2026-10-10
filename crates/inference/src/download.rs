use frameiq_contracts::*;
use std::{io::Write, path::Path, time::Duration};
fn error(e: impl std::fmt::Display) -> AppError {
    AppError::new("MODEL_DOWNLOAD", e.to_string())
}
fn allowed(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str().is_some_and(|h| {
            [
                "github.com",
                "raw.githubusercontent.com",
                "media.githubusercontent.com",
                "huggingface.co",
            ]
            .contains(&h)
                || h.ends_with(".githubusercontent.com")
                || h.ends_with(".hf.co")
        })
}
fn retained_file(path: &Path) -> AppResult<()> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(error(
            "Retained model downloads must be regular application-owned files.",
        )),
    }
}
fn cancelled() -> AppError {
    AppError::new(
        "CANCELLED",
        "Download cancelled. Verified resume information is retained.",
    )
}
struct Transport {
    client: reqwest::Client,
    permitted: fn(&reqwest::Url) -> bool,
}
impl Transport {
    fn production() -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() > 5 || !allowed(attempt.url()) {
                    attempt.error("Unsupported model download redirect")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .map_err(error)?;
        Ok(Self {
            client,
            permitted: allowed,
        })
    }
}
pub fn download(
    manifest: &ModelManifestV1,
    directory: &Path,
    cancel: impl Fn() -> bool,
    progress: impl FnMut(u64, u64) -> AppResult<()>,
) -> AppResult<std::path::PathBuf> {
    download_with(
        manifest,
        directory,
        Transport::production()?,
        cancel,
        progress,
    )
}
fn download_with(
    manifest: &ModelManifestV1,
    directory: &Path,
    transport: Transport,
    cancel: impl Fn() -> bool,
    progress: impl FnMut(u64, u64) -> AppResult<()>,
) -> AppResult<std::path::PathBuf> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(error)?
        .block_on(download_async(
            manifest, directory, transport, cancel, progress,
        ))
}
async fn wait<T>(
    future: impl std::future::Future<Output = Result<T, reqwest::Error>>,
    cancel: &impl Fn() -> bool,
) -> AppResult<T> {
    let mut future = Box::pin(future);
    loop {
        if cancel() {
            return Err(cancelled());
        }
        match tokio::time::timeout(Duration::from_millis(50), future.as_mut()).await {
            Ok(value) => return value.map_err(error),
            Err(_) => continue,
        }
    }
}
fn strong_etag(tag: &str) -> bool {
    tag.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .is_some_and(|t| t.bytes().all(|b| b >= 0x21 && b != b'"' && b != 0x7f))
}
fn valid_range(range: &str, offset: u64, expected: u64) -> bool {
    let Some((interval, total)) = range.strip_prefix("bytes ").and_then(|s| s.split_once('/'))
    else {
        return false;
    };
    let Some((start, end)) = interval.split_once('-') else {
        return false;
    };
    let number = |s: &str| {
        if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
            s.parse::<u64>().ok()
        } else {
            None
        }
    };
    number(start) == Some(offset)
        && number(end) == expected.checked_sub(1)
        && number(total) == Some(expected)
}
async fn download_async(
    manifest: &ModelManifestV1,
    directory: &Path,
    transport: Transport,
    cancel: impl Fn() -> bool,
    mut progress: impl FnMut(u64, u64) -> AppResult<()>,
) -> AppResult<std::path::PathBuf> {
    super::validate_manifest(manifest)?;
    if cancel() {
        return Err(cancelled());
    }
    let url = reqwest::Url::parse(&manifest.source_url).map_err(error)?;
    if !(transport.permitted)(&url) {
        return Err(error(
            "Use an HTTPS model URL from GitHub or Hugging Face, or import the model locally.",
        ));
    }
    if directory.is_symlink() {
        return Err(error("Model download storage cannot be a symlink."));
    }
    std::fs::create_dir_all(directory).map_err(error)?;
    cleanup_owned_checkpoints(directory);
    let path = directory.join(format!("{}.partial", manifest.sha256));
    let metadata = path.with_extension("json");
    retained_file(&path)?;
    retained_file(&metadata)?;
    let expected = manifest.byte_size.parse::<u64>().map_err(error)?;
    let saved: serde_json::Value = std::fs::read(&metadata)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let etag = saved["etag"].as_str().filter(|s| strong_etag(s));
    let offset = if saved["url"] == manifest.source_url && etag.is_some() {
        std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or(0)
            .min(expected)
    } else {
        0
    };
    let mut request = transport
        .client
        .get(url)
        .header(reqwest::header::ACCEPT_ENCODING, "identity");
    if offset > 0 && offset < expected {
        request = request
            .header(reqwest::header::RANGE, format!("bytes={offset}-"))
            .header(reqwest::header::IF_RANGE, etag.unwrap());
    }
    if offset == expected {
        if super::verify_model(&path, manifest).is_ok() {
            return Ok(path);
        }
        std::fs::remove_file(&path).map_err(error)?;
        let _ = std::fs::remove_file(&metadata);
        return Err(AppError::new(
            "MODEL_CHECKSUM",
            "The retained download is invalid. Retry to download a fresh copy.",
        ));
    }
    if cancel() {
        return Err(cancelled());
    }
    let mut response = wait(request.send(), &cancel)
        .await?
        .error_for_status()
        .map_err(error)?;
    if ![
        reqwest::StatusCode::OK,
        reqwest::StatusCode::PARTIAL_CONTENT,
    ]
    .contains(&response.status())
    {
        return Err(error("Expected a complete or partial model response."));
    }
    let resumed = offset > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    if resumed {
        let range = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !valid_range(range, offset, expected) {
            return Err(error("Invalid resumed content range."));
        }
        if response
            .headers()
            .get(reqwest::header::ETAG)
            .is_some_and(|tag| tag.to_str().ok() != etag)
        {
            return Err(error("Resumed response changed its entity tag."));
        }
    } else if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(error(
            "Unexpected partial response without a resume request.",
        ));
    }
    let mut completed = if resumed { offset } else { 0 };
    if let Some(length) = response.content_length() {
        if length.checked_add(completed) != Some(expected) {
            return Err(error("Download size differs from manifest."));
        }
    }
    let tag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .unwrap_or(if resumed { etag.unwrap() } else { "" });
    let state = serde_json::json!({"url":manifest.source_url,"etag":tag});
    checkpoint("before_reset");
    let mut output = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(!resumed)
        .append(resumed)
        .open(&path)
        .map_err(error)?;
    // Reset and sync old bytes before publishing a new validator. A crash can
    // leave an empty file with old state, never old bytes with a new validator.
    output.sync_all().map_err(error)?;
    checkpoint("after_reset");
    let mut checkpoint_file = tempfile::Builder::new()
        .prefix(&format!(".frameiq-resume-{}-", manifest.sha256))
        .tempfile_in(directory)
        .map_err(error)?;
    serde_json::to_writer(&mut checkpoint_file, &state).map_err(error)?;
    checkpoint("during_metadata_write");
    checkpoint_file.as_file().sync_all().map_err(error)?;
    checkpoint("before_metadata_commit");
    checkpoint_file.persist(&metadata).map_err(error)?;
    checkpoint("after_metadata");
    loop {
        if cancel() {
            output.sync_all().map_err(error)?;
            return Err(cancelled());
        }
        let Some(chunk) = wait(response.chunk(), &cancel).await? else {
            break;
        };
        completed = completed
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| error("Download size overflow."))?;
        if completed > expected {
            return Err(error("Download exceeds expected size."));
        }
        output.write_all(&chunk).map_err(error)?;
        checkpoint("after_body_write");
        progress(completed, expected)?;
    }
    output.sync_all().map_err(error)?;
    if completed != expected {
        return Err(error("Download ended before its declared size."));
    }
    drop(output);
    checkpoint("before_verification");
    if let Err(e) = super::verify_model(&path, manifest) {
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&metadata);
        return Err(e);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> ModelManifestV1 {
        serde_json::from_str(include_str!("../../../models/manifests/yunet-2023mar.json")).unwrap()
    }
    #[test]
    fn cancellation_before_dispatch_preserves_resume_files_without_network() {
        let directory = tempfile::tempdir().unwrap();
        let m = manifest();
        let partial = directory.path().join(format!("{}.partial", m.sha256));
        std::fs::write(&partial, b"retained partial").unwrap();
        std::fs::write(partial.with_extension("json"), b"retained resume state").unwrap();
        let result = download(
            &m,
            directory.path(),
            || true,
            |_, _| panic!("Unexpected progress"),
        );
        assert_eq!(result.unwrap_err().code, "CANCELLED");
        assert_eq!(std::fs::read(&partial).unwrap(), b"retained partial");
        assert_eq!(
            std::fs::read(partial.with_extension("json")).unwrap(),
            b"retained resume state"
        );
    }
    #[test]
    fn corrupt_complete_download_is_removed_and_requires_explicit_retry() {
        let directory = tempfile::tempdir().unwrap();
        let mut m = manifest();
        m.byte_size = "4".into();
        let partial = directory.path().join(format!("{}.partial", m.sha256));
        std::fs::write(&partial, b"bad!").unwrap();
        std::fs::write(
            partial.with_extension("json"),
            serde_json::to_vec(&serde_json::json!({"url":m.source_url,"etag":"\"strong\""}))
                .unwrap(),
        )
        .unwrap();
        let result = download(
            &m,
            directory.path(),
            || false,
            |_, _| panic!("Unexpected progress"),
        );
        assert_eq!(result.unwrap_err().code, "MODEL_CHECKSUM");
        assert!(!partial.exists());
        assert!(!partial.with_extension("json").exists());
    }
    #[test]
    #[ignore = "Opt-in network smoke against the reviewed YuNet public model host"]
    fn reviewed_public_host_download_smoke() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = manifest();
        let path = download(&manifest, directory.path(), || false, |_, _| Ok(())).unwrap();
        super::super::verify_model(&path, &manifest).unwrap();
        assert_eq!(
            std::fs::metadata(path).unwrap().len().to_string(),
            manifest.byte_size
        );
    }
    #[cfg(unix)]
    #[test]
    fn redirected_retained_files_are_rejected_before_dispatch() {
        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let original = outside.path().join("original.onnx");
        std::fs::write(&original, b"original import").unwrap();
        let m = manifest();
        for extension in ["partial", "json"] {
            let link = directory.path().join(format!("{}.{extension}", m.sha256));
            std::os::unix::fs::symlink(&original, &link).unwrap();
            assert_eq!(
                download(
                    &m,
                    directory.path(),
                    || false,
                    |_, _| panic!("Unexpected progress")
                )
                .unwrap_err()
                .code,
                "MODEL_DOWNLOAD"
            );
            assert!(link.is_symlink());
            std::fs::remove_file(link).unwrap();
        }
        let linked_dir = directory.path().join("linked");
        std::os::unix::fs::symlink(outside.path(), &linked_dir).unwrap();
        assert_eq!(
            download(
                &m,
                &linked_dir,
                || false,
                |_, _| panic!("Unexpected progress")
            )
            .unwrap_err()
            .code,
            "MODEL_DOWNLOAD"
        );
        assert_eq!(std::fs::read(original).unwrap(), b"original import");
    }
}

#[cfg(test)]
#[path = "download_https_tests.rs"]
mod https_tests;

// Process termination injection exists only in the unit-test executable.
#[inline]
fn checkpoint(_point: &str) {
    #[cfg(test)]
    if std::env::var("FRAMEIQ_TEST_DOWNLOAD_CRASH").ok().as_deref() == Some(_point) {
        std::process::exit(91);
    }
}

/// Recover only reserved, regular checkpoint files while the caller owns the
/// application's instance/download lane. Completed and partial bodies are retained.
pub fn cleanup_owned_checkpoints(directory: &Path) {
    if directory.is_symlink() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(tail) = name
            .to_str()
            .and_then(|n| n.strip_prefix(".frameiq-resume-"))
        else {
            continue;
        };
        let (Some(hash), Some(random)) = (tail.get(..64), tail.get(65..)) else {
            continue;
        };
        if tail.as_bytes().get(64) != Some(&b'-')
            || random.len() != 6
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !random.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            continue;
        }
        if std::fs::symlink_metadata(entry.path())
            .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}
