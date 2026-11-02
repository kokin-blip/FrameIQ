use super::*;
pub(super) struct PreparedFashion {
    pub review: FashionReviewV1,
    pub created: Instant,
    pub _directory: tempfile::TempDir,
}
pub const CATEGORIES: &[&str] = &[
    "dress",
    "top",
    "shirt",
    "outerwear",
    "trousers",
    "skirt",
    "footwear",
    "bag",
    "jewelry",
    "sunglasses",
    "hat",
    "hair_accessory",
    "belt",
    "hosiery",
    "accessory",
    "hair",
    "makeup",
    "unknown",
];
fn asset(path: &Path, w: u32, h: u32) -> AppResult<AssetRefV1> {
    Ok(AssetRefV1 {
        path: path.to_string_lossy().into(),
        sha256: hash(path)?,
        width: w,
        height: h,
    })
}
pub(super) fn hash(path: &Path) -> AppResult<String> {
    let mut f = File::open(path).map_err(error)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf).map_err(error)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
pub(super) fn check_asset(a: &AssetRefV1) -> AppResult<()> {
    if hash(Path::new(&a.path))? != a.sha256 {
        return Err(AppError::new(
            "ASSET_CHANGED",
            "The image changed. Prepare it again.",
        ));
    }
    Ok(())
}
fn validate_name(s: &str) -> AppResult<()> {
    if s.trim().is_empty() || s.len() > 500 || s.chars().any(char::is_control) {
        return Err(AppError::new(
            "INVALID_NAME",
            "Enter a name of 1–500 characters.",
        ));
    }
    Ok(())
}
impl Application {
    pub fn photos(&self, project: &str) -> AppResult<Vec<PhotoSourceV1>> {
        lock(&self.inner.db)?.photos(project)
    }
    pub fn import_photo(&self, project: &str, path: &Path) -> AppResult<JobEvent> {
        let path = path.canonicalize().map_err(error)?;
        let project = project.to_string();
        let app = self.clone();
        let (mut event, cancel) = self.job("photo_import", None)?;
        let first = event.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let _lane =
                    lock_cancellable(&app.inner.background, || cancel.load(Ordering::SeqCst))?;
                let root = app.fashion_directory("photos")?;
                let out = root.join(format!("{}.png", Uuid::new_v4()));
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let mut decoded = None;
                let result = app.process(
                    WorkerRequest::DecodePhoto {
                        path: path.to_string_lossy().into(),
                        output: out.to_string_lossy().into(),
                        max_width: None,
                    },
                    Duration::from_secs(120),
                    || cancel.load(Ordering::SeqCst),
                    |m| {
                        if let WorkerMessage::Photo {
                            asset,
                            source_sha256,
                        } = m
                        {
                            decoded = Some((asset, source_sha256));
                        }
                        Ok(())
                    },
                );
                if result.is_err() {
                    let _ = fs::remove_file(&out);
                }
                result?;
                let (a, h) = decoded.ok_or_else(|| error("Photo decoder returned no result"))?;
                if cancel.load(Ordering::SeqCst) {
                    let _ = fs::remove_file(&out);
                    return Err(AppError::new("CANCELLED", "Photo import cancelled."));
                }
                let saved = lock(&app.inner.db)?.save_photo(&project, &path, &h, &a);
                match saved {
                    Ok(p) => {
                        event.result_ids = vec![p.revision];
                        event.completed_units = "1".into();
                        event.total_units = Some("1".into());
                        Ok(())
                    }
                    Err(e) => {
                        let _ = fs::remove_file(&out);
                        Err(e)
                    }
                }
            })();
            app.finish_job(&mut event, result, "PHOTO_READY");
        });
        Ok(first)
    }
    pub(super) fn fashion_directory(&self, name: &str) -> AppResult<PathBuf> {
        let root = self.inner.cache.join("fashion");
        for p in [&root, &root.join(name)] {
            if p.is_symlink() {
                return Err(AppError::new(
                    "ASSET_DIRECTORY",
                    "The fashion cache must not be a symbolic link.",
                ));
            }
            fs::create_dir_all(p).map_err(error)?;
        }
        Ok(root.join(name))
    }
    pub fn media_asset(&self, project: &str, media: &MediaRefV1) -> AppResult<AssetRefV1> {
        self.media_asset_task(project, media, &AtomicBool::new(false))
    }
    pub(super) fn media_asset_task(
        &self,
        project: &str,
        media: &MediaRefV1,
        cancel: &AtomicBool,
    ) -> AppResult<AssetRefV1> {
        match media {
            MediaRefV1::Photo { revision } => {
                let p = lock(&self.inner.db)?.photo(revision)?;
                if p.project_id != project {
                    return Err(AppError::new(
                        "MEDIA_SCOPE",
                        "Choose a photo from this project.",
                    ));
                }
                if hash(Path::new(&p.path))? != p.sha256 {
                    return Err(AppError::new(
                        "SOURCE_CHANGED",
                        "Reimport the changed photo.",
                    ));
                }
                let a = AssetRefV1 {
                    path: p.image_path,
                    sha256: p.image_sha256,
                    width: p.width,
                    height: p.height,
                };
                check_asset(&a)?;
                Ok(a)
            }
            MediaRefV1::Frame { selection } => {
                let db = lock(&self.inner.db)?;
                let s = db.source(&selection.source_id)?;
                if s.project_id != project {
                    return Err(AppError::new(
                        "MEDIA_SCOPE",
                        "Choose a frame from this project.",
                    ));
                }
                let f = db.frame(
                    &s.index_generation,
                    &selection.frame_ref.presentation_ordinal,
                )?;
                if serde_json::to_value(db.frame_ref(&s, &f)?).map_err(error)?
                    != serde_json::to_value(&selection.frame_ref).map_err(error)?
                {
                    return Err(AppError::new(
                        "SOURCE_CHANGED",
                        "Select the current exact frame.",
                    ));
                }
                drop(db);
                let v = self.render_task(
                    &selection.source_id,
                    &selection.frame_ref.presentation_ordinal,
                    cancel,
                )?;
                asset(Path::new(&v.image_path), v.width, v.height)
            }
        }
    }
    pub fn fashion_items(&self, project: &str) -> AppResult<Vec<FashionItemV1>> {
        lock(&self.inner.db)?.fashion_items(project)
    }
    pub fn save_fashion_item(&self, mut item: FashionItemV1) -> AppResult<JobEvent> {
        frameiq_analysis::presets::validate_rectangle(&item.rectangle)?;
        if !CATEGORIES.contains(&item.category.as_str())
            || item.polygon.len() > 128
            || item.attributes.len() > 48
        {
            return Err(AppError::new(
                "INVALID_ITEM",
                "Choose a supported category and a bounded region.",
            ));
        }
        if item.polygon.len() == 1
            || item.polygon.len() == 2
            || item
                .polygon
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || *v < 0. || *v > 1.)
        {
            return Err(AppError::new(
                "INVALID_REGION",
                "A polygon needs at least three points inside the image.",
            ));
        }
        let old = if item.id.is_empty() {
            item.id = Uuid::new_v4().to_string();
            None
        } else {
            let old = lock(&self.inner.db)?.fashion_item(&item.id)?;
            if old.project_id != item.project_id
                || serde_json::to_string(&old.media).map_err(error)?
                    != serde_json::to_string(&item.media).map_err(error)?
            {
                return Err(AppError::new(
                    "ITEM_SCOPE",
                    "Choose an item from this source and project.",
                ));
            }
            Some(old)
        };
        item.suppressed_attributes = old
            .as_ref()
            .map(|o| {
                o.suppressed_attributes
                    .iter()
                    .cloned()
                    .chain(
                        o.attributes
                            .iter()
                            .filter(|a| !item.attributes.iter().any(|b| a.name == b.name))
                            .map(|a| a.name.clone()),
                    )
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .filter(|name| !item.attributes.iter().any(|a| &a.name == name))
                    .collect()
            })
            .unwrap_or_default();
        for a in &mut item.attributes {
            if a.name.trim().is_empty()
                || a.name.len() > 80
                || a.value.trim().is_empty()
                || a.value.len() > 1000
                || a.name.chars().chain(a.value.chars()).any(char::is_control)
            {
                return Err(AppError::new(
                    "INVALID_ATTRIBUTE",
                    "Use an attribute name up to 80 characters and value up to 1,000 characters.",
                ));
            }
            if let Some(before) = old.as_ref().and_then(|o| {
                o.attributes
                    .iter()
                    .find(|b| b.name == a.name && b.value == a.value)
            }) {
                *a = before.clone();
            } else {
                a.origin = "user".into();
                a.confidence = None;
                a.evidence_ids.clear();
            }
        }
        let preserved_mask = old
            .as_ref()
            .filter(|o| {
                o.polygon == item.polygon
                    && serde_json::to_string(&o.rectangle).ok()
                        == serde_json::to_string(&item.rectangle).ok()
            })
            .and_then(|o| o.mask.clone());
        let (mut event, cancel) = self.job("item_correction", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let a = app.media_asset_task(&item.project_id, &item.media, &cancel)?;
                let root = app.fashion_directory("items")?;
                let output = root.join(format!("{}.png", Uuid::new_v4()));
                let mask = root.join(format!("{}.png", Uuid::new_v4()));
                let mut assets = None;
                let request = if let Some(mask) = &preserved_mask {
                    WorkerRequest::ApplyPhotoMask {
                        path: a.path.clone(),
                        mask: mask.clone(),
                        output: output.to_string_lossy().into(),
                        rectangle: item.rectangle.clone(),
                    }
                } else {
                    WorkerRequest::CropPhoto {
                        path: a.path.clone(),
                        output: output.to_string_lossy().into(),
                        mask_output: if item.polygon.is_empty() {
                            None
                        } else {
                            Some(mask.to_string_lossy().into())
                        },
                        rectangle: item.rectangle.clone(),
                        polygon: item.polygon.clone(),
                    }
                };
                let r = app.process(
                    request,
                    Duration::from_secs(90),
                    || cancel.load(Ordering::SeqCst),
                    |m| {
                        if let WorkerMessage::PhotoAssets { crop, mask } = m {
                            assets = Some((crop, mask));
                        }
                        Ok(())
                    },
                );
                if r.is_err() {
                    let _ = fs::remove_file(&output);
                    let _ = fs::remove_file(&mask);
                }
                r?;
                check_asset(&a)?;
                let (crop, mask_asset) =
                    assets.ok_or_else(|| error("No cropped image returned"))?;
                item.crop = Some(crop);
                item.mask = preserved_mask.or(mask_asset);
                item.provenance = "user_corrected".into();
                let saved = if cancel.load(Ordering::SeqCst) {
                    Err(AppError::new("CANCELLED", "Correction cancelled."))
                } else {
                    lock(&app.inner.db)?.save_fashion_item(item, true)
                };
                match saved {
                    Ok(i) => {
                        event.result_ids = vec![i.id];
                        Ok(())
                    }
                    Err(e) => {
                        let _ = fs::remove_file(output);
                        let _ = fs::remove_file(mask);
                        Err(e)
                    }
                }
            })();
            app.finish_job(&mut event, result, "ITEM_SAVED");
        });
        Ok(first)
    }
    pub fn delete_fashion_item(&self, id: &str) -> AppResult<()> {
        let mut db = lock(&self.inner.db)?;
        let mut i = db.fashion_item(id)?;
        i.deleted = true;
        db.save_fashion_item(i, true)?;
        Ok(())
    }
    pub fn undo_fashion_item(&self, id: &str) -> AppResult<FashionItemV1> {
        lock(&self.inner.db)?.undo_fashion_item(id)
    }
    pub fn garment_groups(&self, project: &str) -> AppResult<Vec<GarmentGroupV1>> {
        lock(&self.inner.db)?.garment_groups(project)
    }
    pub fn save_garment_group(&self, g: GarmentGroupV1) -> AppResult<GarmentGroupV1> {
        validate_name(&g.name)?;
        if g.notes.len() > 10000 || g.item_ids.len() > 100 {
            return Err(error("Group is too large"));
        }
        lock(&self.inner.db)?.save_garment_group(g)
    }
    pub fn candidates(&self, group: &str) -> AppResult<Vec<ProductCandidateV1>> {
        lock(&self.inner.db)?.candidates(group)
    }
    pub fn looks(&self, project: &str) -> AppResult<Vec<SavedLookV1>> {
        lock(&self.inner.db)?.looks(project)
    }
    pub fn save_look(&self, l: SavedLookV1) -> AppResult<SavedLookV1> {
        validate_name(&l.name)?;
        if l.notes.len() > 10000 || l.items.len() > 100 {
            return Err(error("Look is too large"));
        }
        lock(&self.inner.db)?.save_look(l)
    }
    pub fn delete_look(&self, id: &str) -> AppResult<()> {
        lock(&self.inner.db)?.delete_look(id)
    }
}
