//! Startup recovery under the application instance lock; optional AI storage never
//! blocks the offline extractor when a filesystem entry cannot be cleaned safely.
use super::*;

pub(super) const STAGING_PREFIX: &str = ".frameiq-install-";

pub(super) fn prepare_directory(path: &Path) -> AppResult<()> {
    if path.is_symlink() {
        return Err(AppError::new(
            "MODEL_STORAGE",
            "Model storage must be an application-owned directory.",
        ));
    }
    fs::create_dir_all(path).map_err(error)?;
    if !real_directory(path) {
        return Err(AppError::new(
            "MODEL_STORAGE",
            "Model storage must be an application-owned directory.",
        ));
    }
    Ok(())
}
pub(super) fn remove_installation(path: &Path) -> AppResult<()> {
    if !real_directory(path) {
        return Err(AppError::new(
            "MODEL_STORAGE",
            "Model removal requires an application-owned directory.",
        ));
    }
    let model = path.join("model.onnx");
    if real_file(&model) {
        fs::remove_file(model).map_err(error)?;
    } else if model.exists() || model.is_symlink() {
        return Err(AppError::new(
            "MODEL_STORAGE",
            "Model removal cannot delete an unknown installation entry.",
        ));
    }
    // Unknown files are retained; only an empty directory can be removed.
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(e) => Err(error(e)),
    }
}

#[cfg(feature = "fault-injection")]
pub(super) fn checkpoint(name: &str) {
    if std::env::var("FRAMEIQ_TEST_MODEL_CRASH").as_deref() == Ok(name) {
        std::process::exit(86);
    }
}
#[cfg(not(feature = "fault-injection"))]
pub(super) fn checkpoint(_: &str) {}

fn owned_name(name: &str) -> bool {
    let candidate = if let Some(stage) = name.strip_prefix(STAGING_PREFIX) {
        let Some(id) = stage.get(..36) else {
            return false;
        };
        id
    } else {
        name
    };
    Uuid::parse_str(candidate).is_ok_and(|u| {
        u.get_version_num() == 4
            && u.to_string() == candidate
            && (name == candidate
                || name
                    .strip_prefix(&format!("{STAGING_PREFIX}{candidate}-"))
                    .is_some_and(|tail| {
                        tail.len() == 6 && tail.bytes().all(|b| b.is_ascii_alphanumeric())
                    }))
    })
}

fn real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}
fn real_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
}
fn verified_copy(path: &Path, manifest: &ModelManifestV1) -> bool {
    real_file(path)
        && fs::metadata(path).is_ok_and(|m| Some(m.len()) == manifest.byte_size.parse::<u64>().ok())
        && hash_file(path, || false).is_ok_and(|hash| hash == manifest.sha256)
}

pub(super) fn recover(data: &Path, db: &Database) -> AppResult<()> {
    let models = db.model_installations()?;
    let directory = data.join("models");
    if real_directory(&directory) {
        if let Ok(entries) = fs::read_dir(&directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name();
                if !name.to_str().is_some_and(owned_name)
                    || !real_directory(&path)
                    || models
                        .iter()
                        .any(|m| Path::new(&m.path) == path.join("model.onnx"))
                {
                    continue;
                }
                // Do not recursively delete unknown files or follow child symlinks.
                let Ok(children) =
                    fs::read_dir(&path).and_then(|r| r.collect::<Result<Vec<_>, _>>())
                else {
                    continue;
                };
                if children
                    .iter()
                    .any(|c| c.file_name() != "model.onnx" || !real_file(&c.path()))
                {
                    continue;
                }
                if children.is_empty() || fs::remove_file(path.join("model.onnx")).is_ok() {
                    let _ = fs::remove_dir(&path);
                }
            }
        }
    }
    let downloads = data.join("model-downloads");
    if real_directory(&downloads) {
        for model in models.iter().filter(|m| m.status == "ready") {
            if frameiq_inference::validate_manifest(&model.manifest).is_err()
                || model.manifest.sha256.len() != 64
                || !model
                    .manifest
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                continue;
            }
            // Retain interrupted/uninstalled downloads for resume. Only a verified
            // installed duplicate is redundant; corrupt or unrelated files stay put.
            let partial = downloads.join(format!("{}.partial", model.manifest.sha256));
            let metadata = partial.with_extension("json");
            if real_file(&partial)
                && (real_file(&metadata) || !metadata.exists() && !metadata.is_symlink())
                && verified_copy(Path::new(&model.path), &model.manifest)
                && verified_copy(&partial, &model.manifest)
                && fs::remove_file(&partial).is_ok()
            {
                let _ = fs::remove_file(&metadata);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save_model(db: &Database, data: &Path, bytes: &[u8]) -> ModelInstallationV1 {
        let id = Uuid::new_v4().to_string();
        let path = data.join("models").join(&id).join("model.onnx");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        let mut manifest: ModelManifestV1 =
            serde_json::from_str(include_str!("../../../models/manifests/yunet-2023mar.json"))
                .unwrap();
        manifest.id = id.clone();
        manifest.byte_size = bytes.len().to_string();
        manifest.sha256 = format!("{:x}", Sha256::digest(bytes));
        let model = ModelInstallationV1 {
            id,
            manifest,
            status: "ready".into(),
            path: path.to_string_lossy().into(),
            error: None,
        };
        db.save_model(&model).unwrap();
        model
    }

    #[test]
    fn restart_retains_committed_models_and_resumable_downloads() {
        let data = tempfile::tempdir().unwrap();
        let db = Database::open(&data.path().join("frameiq.db")).unwrap();
        let installed = save_model(&db, data.path(), b"validated fixture");
        let models = data.path().join("models");
        let orphan = models.join(Uuid::new_v4().to_string());
        fs::create_dir(&orphan).unwrap();
        fs::write(orphan.join("model.onnx"), b"uncommitted activation").unwrap();
        let stage = tempfile::Builder::new()
            .prefix(&format!("{STAGING_PREFIX}{}-", Uuid::new_v4()))
            .tempdir_in(&models)
            .unwrap()
            .keep();
        fs::write(stage.join("model.onnx"), b"interrupted copy").unwrap();
        let unknown = models.join(Uuid::new_v4().to_string());
        fs::create_dir(&unknown).unwrap();
        fs::write(unknown.join("original.txt"), b"preserve unknown content").unwrap();
        let legacy = models.join(".tmpLegacy");
        fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join("model.onnx"), b"unrecognized name").unwrap();
        let downloads = data.path().join("model-downloads");
        fs::create_dir(&downloads).unwrap();
        let duplicate = downloads.join(format!("{}.partial", installed.manifest.sha256));
        fs::write(&duplicate, b"validated fixture").unwrap();
        fs::write(duplicate.with_extension("json"), b"retained metadata").unwrap();
        let resumable = downloads.join(format!("{}.partial", "a".repeat(64)));
        fs::write(&resumable, b"cancelled download").unwrap();
        recover(data.path(), &db).unwrap();
        assert!(Path::new(&installed.path).exists());
        assert!(!orphan.exists());
        assert!(!stage.exists());
        assert!(unknown.join("original.txt").exists());
        assert!(legacy.exists());
        assert!(!duplicate.exists());
        assert!(!duplicate.with_extension("json").exists());
        assert!(resumable.exists());
        assert_eq!(db.model_installations().unwrap().len(), 1);
        // A damaged installation is never grounds for discarding its retained copy.
        fs::write(&duplicate, b"validated fixture").unwrap();
        fs::write(&installed.path, b"damaged installation").unwrap();
        recover(data.path(), &db).unwrap();
        assert!(duplicate.exists());
    }

    #[cfg(unix)]
    #[test]
    fn recovery_preserves_symlink_targets_and_redirected_ai_directories() {
        use std::os::unix::fs::symlink;
        let data = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let db = Database::open(&data.path().join("frameiq.db")).unwrap();
        fs::write(outside.path().join("original.onnx"), b"imported original").unwrap();
        let models = data.path().join("models");
        fs::create_dir(&models).unwrap();
        let linked = models.join(Uuid::new_v4().to_string());
        symlink(outside.path(), &linked).unwrap();
        let child = models.join(Uuid::new_v4().to_string());
        fs::create_dir(&child).unwrap();
        symlink(
            outside.path().join("original.onnx"),
            child.join("model.onnx"),
        )
        .unwrap();
        recover(data.path(), &db).unwrap();
        assert!(linked.is_symlink());
        assert!(child.join("model.onnx").is_symlink());
        fs::remove_dir_all(&models).unwrap();
        symlink(outside.path(), &models).unwrap();
        symlink(outside.path(), data.path().join("model-downloads")).unwrap();
        recover(data.path(), &db).unwrap();
        assert_eq!(
            fs::read(outside.path().join("original.onnx")).unwrap(),
            b"imported original"
        );
    }
}
