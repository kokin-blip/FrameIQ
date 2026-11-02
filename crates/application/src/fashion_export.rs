use super::*;
use crate::fashion::{check_asset, hash};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn rewrite_assets(v: &mut Value, files: &[FashionExportFileV1]) {
    match v {
        Value::Object(o) => {
            if let Some(Value::String(p)) = o.get_mut("path") {
                if let Some(f) = files
                    .iter()
                    .find(|f| f.source.as_ref().is_some_and(|a| &a.path == p))
                {
                    *p = f.name.clone();
                } else {
                    *p = String::new();
                }
            }
            for v in o.values_mut() {
                rewrite_assets(v, files);
            }
        }
        Value::Array(xs) => {
            for v in xs {
                rewrite_assets(v, files)
            }
        }
        _ => {}
    }
}
impl Application {
    pub fn fashion_exports(&self, project: &str) -> AppResult<Vec<FashionExportV1>> {
        lock(&self.inner.db)?.fashion_exports(project)
    }
    pub fn export_look(
        &self,
        project: &str,
        id: &str,
        destination: &Path,
        format: &str,
    ) -> AppResult<JobEvent> {
        if !["json", "html", "reference"].contains(&format) {
            return Err(error("Choose JSON, HTML or reference pack"));
        }
        let look = lock(&self.inner.db)?
            .looks(project)?
            .into_iter()
            .find(|l| l.id == id)
            .ok_or_else(|| error("Look unavailable"))?;
        let parent = destination.canonicalize().map_err(error)?;
        if !parent.is_dir() {
            return Err(error("Choose an existing destination folder"));
        }
        let (mut event, cancel) = self.job("look_export", None)?;
        let first = event.clone();
        let app = self.clone();
        let format = format.to_string();
        std::thread::spawn(move || {
            let outcome = (|| {
                let _lane =
                    lock_cancellable(&app.inner.background, || cancel.load(Ordering::SeqCst))?;
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let mut groups = Vec::new();
                let mut items = Vec::new();
                let mut candidates = Vec::new();
                let mut assets = Vec::new();
                {
                    let db = lock(&app.inner.db)?;
                    for e in &look.items {
                        let g = db.garment_group(&e.group_id)?;
                        for id in &g.item_ids {
                            let i = db.fashion_item(id)?;
                            if i.deleted {
                                continue;
                            }
                            for a in i.crop.iter().chain(i.mask.iter()) {
                                assets.push(a.clone());
                            }
                            items.push(i);
                        }
                        let cs = db.candidates(&e.group_id)?;
                        for id in e.candidate_id.iter().chain(e.alternative_ids.iter()) {
                            let c = cs
                                .iter()
                                .find(|c| &c.id == id)
                                .ok_or_else(|| error("Saved product evidence unavailable"))?;
                            if matches!(c.product.image_rights.as_str(), "owned" | "copy_allowed") {
                                assets.extend(c.product.image.iter().cloned());
                            }
                            let mut c = c.clone();
                            if !matches!(c.product.image_rights.as_str(), "owned" | "copy_allowed")
                            {
                                c.product.image = None;
                            }
                            c.stale |= c.group_revision != g.revision;
                            candidates.push(c);
                        }
                        groups.push(g);
                    }
                }
                for i in &items {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(AppError::new("CANCELLED", "Look export cancelled."));
                    }
                    assets.push(app.media_asset_task(&look.project_id, &i.media, &cancel)?);
                }
                let mut files = Vec::new();
                for a in assets {
                    check_asset(&a)?;
                    let name = format!("assets/{}.png", a.sha256);
                    if !files.iter().any(|f: &FashionExportFileV1| f.name == name) {
                        files.push(FashionExportFileV1 {
                            name,
                            sha256: a.sha256.clone(),
                            source: Some(a),
                        });
                    }
                }
                let mut analysis = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for i in &items {
                    for id in i
                        .attributes
                        .iter()
                        .flat_map(|a| &a.evidence_ids)
                        .filter_map(|id| id.strip_prefix("analysis:"))
                    {
                        if !seen.insert(id.to_string()) {
                            continue;
                        }
                        if let Some(run) = lock(&app.inner.db)?.fashion_run(id)? {
                            analysis.push(json!({"id":id,"adapterVersion":run["adapterVersion"],"model":run["outcome"]["model"],"requestedModel":run["review"]["request"]["model"],"runtime":run["review"]["request"]["runtime"],"task":run["review"]["request"]["task"],"sources":run["sources"],"searchEvents":run["searchEvents"],"imageHashes":run["review"]["images"].as_array().map(|a|a.iter().map(|i|i["sha256"].clone()).collect::<Vec<_>>())}));
                        } else {
                            analysis.push(json!({"id":id,"unavailable":true}));
                        }
                    }
                }
                let snapshot = json!({"analysis":analysis,"schemaVersion":1,"look":look,"groups":groups,"items":items,"candidates":candidates,"assets":files.iter().filter_map(|f|f.source.as_ref()).collect::<Vec<_>>()});
                let mut plan = FashionExportV1 {
                    id: event.job_id.clone(),
                    look: look.clone(),
                    destination: parent
                        .join(format!("FrameIQ-look-{}", event.job_id))
                        .to_string_lossy()
                        .into(),
                    format,
                    completed: vec![],
                    status: "pending".into(),
                    snapshot,
                    files,
                };
                let content = app.look_documents(&plan)?;
                let frozen = app.fashion_directory("exports")?.join(&event.job_id);
                fs::create_dir(&frozen).map_err(error)?;
                for (name, bytes) in &content {
                    let target = frozen.join(name);
                    let mut f = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&target)
                        .map_err(error)?;
                    f.write_all(bytes).map_err(error)?;
                    f.sync_all().map_err(error)?;
                    let sha256 = format!("{:x}", Sha256::digest(bytes));
                    plan.files.push(FashionExportFileV1 {
                        name: name.clone(),
                        sha256: sha256.clone(),
                        source: Some(AssetRefV1 {
                            path: target.to_string_lossy().into(),
                            sha256,
                            width: 0,
                            height: 0,
                        }),
                    });
                }
                lock(&app.inner.db)?.save_fashion_export(&plan)?;
                app.write_look_export(&mut plan, &mut event, &cancel)
            })();
            app.finish_job(&mut event, outcome, "LOOK_EXPORTED");
        });
        Ok(first)
    }
    pub fn resume_look_export(&self, id: &str) -> AppResult<JobEvent> {
        let mut plan = lock(&self.inner.db)?.fashion_export(id)?;
        let (mut event, cancel) = self.job("look_export", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let _lane =
                    lock_cancellable(&app.inner.background, || cancel.load(Ordering::SeqCst))?;
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                app.write_look_export(&mut plan, &mut event, &cancel)
            })();
            app.finish_job(&mut event, result, "LOOK_EXPORTED");
        });
        Ok(first)
    }
    fn look_documents(&self, plan: &FashionExportV1) -> AppResult<Vec<(String, Vec<u8>)>> {
        let mut snapshot = plan.snapshot.clone();
        rewrite_assets(&mut snapshot, &plan.files);
        let data = serde_json::to_vec_pretty(&snapshot).map_err(error)?;
        let mut result = vec![("look.json".into(), data)];
        if plan.format == "json" {
            return Ok(result);
        }
        let mut html=format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>{}</title><style>body{{font:16px/1.6 system-ui;max-width:1100px;margin:3rem auto;padding:0 2rem;color:#202020}}section{{margin:3rem 0;border-top:1px solid #bbb}}img{{max-width:360px;max-height:460px;object-fit:contain}}a{{color:#005a9e}}pre{{white-space:pre-wrap}}</style><h1>{}</h1><p>{}</p>",escape(&plan.look.name),escape(&plan.look.name),escape(&plan.look.notes));
        for group in plan.snapshot["groups"].as_array().into_iter().flatten() {
            let id = group["id"].as_str().unwrap_or_default();
            html.push_str(&format!(
                "<section><h2>{}</h2><p>{}</p>",
                escape(group["name"].as_str().unwrap_or("Item")),
                escape(group["notes"].as_str().unwrap_or_default())
            ));
            for i in plan.snapshot["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|i| {
                    group["itemIds"]
                        .as_array()
                        .is_some_and(|ids| ids.contains(&i["id"]))
                })
            {
                if let Ok(a) = serde_json::from_value::<AssetRefV1>(i["crop"].clone()) {
                    check_asset(&a)?;
                    let src = if plan.format == "html" {
                        if html.len() as u64
                            + fs::metadata(&a.path)
                                .map_err(error)?
                                .len()
                                .saturating_mul(4)
                                / 3
                            > 64 * 1024 * 1024
                        {
                            return Err(AppError::new(
                                "EXPORT_LIMIT",
                                "This inline lookbook exceeds 64 MiB. Choose a reference pack.",
                            ));
                        }
                        format!(
                            "data:image/png;base64,{}",
                            STANDARD.encode(fs::read(&a.path).map_err(error)?)
                        )
                    } else {
                        format!("assets/{}.png", a.sha256)
                    };
                    html.push_str(&format!(
                        "<img alt=\"{} observation\" src=\"{}\">",
                        escape(i["category"].as_str().unwrap_or("Item")),
                        src
                    ));
                }
                html.push_str("<ul>");
                for a in i["attributes"].as_array().into_iter().flatten() {
                    html.push_str(&format!(
                        "<li>{}: {} ({})</li>",
                        escape(a["name"].as_str().unwrap_or_default()),
                        escape(a["value"].as_str().unwrap_or_default()),
                        escape(a["origin"].as_str().unwrap_or_default())
                    ));
                }
                html.push_str("</ul>");
            }
            let cs = plan.snapshot["candidates"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["groupId"] == id)
                .collect::<Vec<_>>();
            if cs.is_empty() {
                html.push_str("<p>Unidentified item — no product has been saved.</p>");
            }
            for c in cs {
                html.push_str(&format!(
                    "<h3>{}</h3><p>{} {}</p>",
                    escape(c["product"]["name"].as_str().unwrap_or("Product candidate")),
                    escape(c["classification"].as_str().unwrap_or("unidentified")),
                    if c["stale"] == true {
                        "— evidence needs review"
                    } else {
                        ""
                    }
                ));
                if let Ok(a) = serde_json::from_value::<AssetRefV1>(c["product"]["image"].clone()) {
                    if matches!(
                        c["product"]["imageRights"].as_str(),
                        Some("owned" | "copy_allowed")
                    ) {
                        check_asset(&a)?;
                        let src = if plan.format == "html" {
                            if html.len() as u64
                                + fs::metadata(&a.path)
                                    .map_err(error)?
                                    .len()
                                    .saturating_mul(4)
                                    / 3
                                > 64 * 1024 * 1024
                            {
                                return Err(AppError::new(
                                    "EXPORT_LIMIT",
                                    "This inline lookbook exceeds 64 MiB. Choose a reference pack.",
                                ));
                            }
                            format!(
                                "data:image/png;base64,{}",
                                STANDARD.encode(fs::read(&a.path).map_err(error)?)
                            )
                        } else {
                            format!("assets/{}.png", a.sha256)
                        };
                        html.push_str(&format!(
                            "<img alt=\"Permitted product image\" src=\"{}\">",
                            src
                        ));
                    }
                }
                if let Some(attribution) = c["product"]["imageAttribution"].as_str() {
                    html.push_str(&format!(
                        "<p>Image attribution: {}</p>",
                        escape(attribution)
                    ));
                }
                if let Some(url) = c["product"]["imageLicenseUrl"]
                    .as_str()
                    .filter(|u| frameiq_inference::fashion::valid_url(u))
                {
                    html.push_str(&format!(
                        "<p><a href=\"{}\">Image license</a></p>",
                        escape(url)
                    ));
                }
                for listing in c["listings"].as_array().into_iter().flatten() {
                    if let Some(url) = listing["url"]
                        .as_str()
                        .filter(|u| frameiq_inference::fashion::valid_url(u))
                    {
                        let price = listing["price"]
                            .as_str()
                            .zip(listing["currency"].as_str())
                            .map(|(p, c)| format!("{c} {p}"))
                            .unwrap_or_else(|| "Price unknown".into());
                        html.push_str(&format!("<p><a href=\"{}\" rel=\"noreferrer\">{}</a> · {} · {} · observed {}</p>",escape(url),escape(listing["retailer"].as_str().unwrap_or("Retailer source")),escape(&price),escape(listing["availability"].as_str().unwrap_or("Availability unverified")),escape(listing["observedAt"].as_str().unwrap_or("unknown"))));
                    }
                }
                for e in c["evidence"].as_array().into_iter().flatten() {
                    if let Some(u) = e["url"]
                        .as_str()
                        .filter(|u| frameiq_inference::fashion::valid_url(u))
                    {
                        html.push_str(&format!("<p><a href=\"{}\" rel=\"noreferrer\">{}</a></p><blockquote>{}</blockquote>",escape(u),escape(e["title"].as_str().unwrap_or(u)),escape(e["excerpt"].as_str().unwrap_or_default())));
                    }
                }
            }
            html.push_str("</section>");
            if html.len() > 64 * 1024 * 1024 {
                return Err(AppError::new(
                    "EXPORT_LIMIT",
                    "This inline lookbook exceeds 64 MiB. Choose a reference pack.",
                ));
            }
        }
        html.push_str("</html>");
        result.push(("index.html".into(), html.into_bytes()));
        Ok(result)
    }
    fn write_look_export(
        &self,
        plan: &mut FashionExportV1,
        event: &mut JobEvent,
        cancel: &AtomicBool,
    ) -> AppResult<()> {
        let destination = PathBuf::from(&plan.destination);
        if destination.is_symlink() {
            return Err(error("The export folder cannot be a symbolic link"));
        }
        let existed = destination.exists();
        fs::create_dir_all(&destination).map_err(error)?;
        let marker = destination.join("FrameIQ-look-owner.json");
        if existed {
            if marker.is_symlink()
                || fs::read_to_string(&marker).map_err(|_| {
                    AppError::new(
                        "OUTPUT_CONFLICT",
                        "The destination has no matching export ownership record.",
                    )
                })? != plan.id
            {
                return Err(AppError::new(
                    "OUTPUT_CONFLICT",
                    "The export destination belongs to another operation.",
                ));
            }
        } else {
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker)
                .map_err(error)?;
            f.write_all(plan.id.as_bytes()).map_err(error)?;
            f.sync_all().map_err(error)?;
        }
        let assets = destination.join("assets");
        if assets.is_symlink() {
            return Err(error("The export assets directory cannot be redirected"));
        }
        fs::create_dir_all(&assets).map_err(error)?;
        event.total_units = Some(plan.files.len().to_string());
        plan.status = "running".into();
        lock(&self.inner.db)?.save_fashion_export(plan)?;
        let outcome = (|| {
            for f in plan.files.clone() {
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new(
                        "CANCELLED",
                        "Look export cancelled. Resume the saved export to continue.",
                    ));
                }
                let target = destination.join(&f.name);
                if target.is_symlink() {
                    return Err(AppError::new(
                        "OUTPUT_CONFLICT",
                        "A destination file was redirected.",
                    ));
                }
                if target.exists() {
                    if hash(&target)? != f.sha256 {
                        return Err(AppError::new(
                            "OUTPUT_CONFLICT",
                            "A saved output was modified. It was preserved.",
                        ));
                    }
                } else {
                    let bytes = if let Some(a) = &f.source {
                        check_asset(a)?;
                        fs::read(&a.path).map_err(error)?
                    } else {
                        return Err(error("Export snapshot document is missing"));
                    };
                    if format!("{:x}", Sha256::digest(&bytes)) != f.sha256 {
                        return Err(AppError::new(
                            "EXPORT_CHANGED",
                            "The immutable export content changed.",
                        ));
                    }
                    let mut temp =
                        tempfile::NamedTempFile::new_in(target.parent().unwrap()).map_err(error)?;
                    temp.write_all(&bytes).map_err(error)?;
                    temp.as_file().sync_all().map_err(error)?;
                    temp.persist_noclobber(&target).map_err(|_| {
                        AppError::new(
                            "OUTPUT_CONFLICT",
                            "The output appeared while writing. It was preserved.",
                        )
                    })?;
                }
                if !plan.completed.contains(&f.name) {
                    plan.completed.push(f.name);
                }
                event.completed_units = plan.completed.len().to_string();
                lock(&self.inner.db)?.save_fashion_export(plan)?;
                self.emit(event, None)?;
            }
            Ok(())
        })();
        plan.status = match &outcome {
            Ok(_) => "succeeded",
            Err(e) if e.code == "CANCELLED" => "cancelled",
            Err(_) => "failed",
        }
        .into();
        lock(&self.inner.db)?.save_fashion_export(plan)?;
        event.result_ids = vec![plan.id.clone()];
        outcome
    }
}
