use super::*;
use crate::fashion::check_asset;
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogRow {
    name: String,
    category: String,
    source_url: String,
    #[serde(default)]
    brand: Option<String>,
    #[serde(default)]
    style_id: Option<String>,
    #[serde(default)]
    image_path: Option<String>,
    #[serde(default)]
    image_rights: Option<String>,
    #[serde(default)]
    image_attribution: Option<String>,
    #[serde(default)]
    image_license_url: Option<String>,
    #[serde(default)]
    price: Option<String>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    availability: Option<String>,
    #[serde(default)]
    observed_at: Option<String>,
    #[serde(default)]
    retailer: Option<String>,
}
impl Application {
    pub fn catalog(&self) -> AppResult<Vec<ProductV1>> {
        lock(&self.inner.db)?.catalog()
    }
    pub fn import_catalog(&self, path: &Path) -> AppResult<JobEvent> {
        let path = path.canonicalize().map_err(error)?;
        if path.metadata().map_err(error)?.len() > 8 * 1024 * 1024 {
            return Err(error("Catalog exceeds eight MiB"));
        }
        let rows: Vec<CatalogRow> = if path.extension().is_some_and(|e| e == "csv") {
            csv::Reader::from_path(&path)
                .map_err(error)?
                .deserialize()
                .collect::<Result<_, _>>()
                .map_err(error)?
        } else {
            serde_json::from_slice(&fs::read(&path).map_err(error)?).map_err(error)?
        };
        if rows.len() > 10000 {
            return Err(error("Import at most 10,000 products per catalog"));
        }
        for r in &rows {
            if r.image_attribution.as_ref().is_some_and(|s| s.len() > 1000)
                || r.image_license_url
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .is_some_and(|s| !frameiq_inference::fashion::valid_url(s))
                || r.name.trim().is_empty()
                || r.name.len() > 500
                || !frameiq_inference::fashion::valid_url(&r.source_url)
                || r.category.len() > 80
                || r.brand.as_ref().is_some_and(|s| s.len() > 120)
                || r.style_id.as_ref().is_some_and(|s| s.len() > 120)
                || r.price.as_ref().filter(|s| !s.is_empty()).is_some_and(|s| {
                    s.len() > 32 || !s.parse::<f64>().is_ok_and(|n| n.is_finite() && n >= 0.)
                })
                || r.currency
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .is_some_and(|s| s.len() != 3 || !s.bytes().all(|b| b.is_ascii_uppercase()))
                || r.observed_at.as_ref().is_some_and(|s| s.len() > 80)
                || r.retailer.as_ref().is_some_and(|s| s.len() > 200)
                || r.availability.as_ref().is_some_and(|s| s.len() > 80)
                || !matches!(
                    r.image_rights.as_deref(),
                    None | Some("owned" | "copy_allowed" | "link_only")
                )
            {
                return Err(AppError::new("CATALOG_FORMAT","Catalog records need a name, category, genuine HTTPS source and permitted image-rights value."));
            }
        }
        let (mut e, cancel) = self.job("catalog_import", None)?;
        let first = e.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                e.status = JobStatus::Running;
                e.total_units = Some(rows.len().to_string());
                app.emit(&e, None)?;
                let base = path.parent().unwrap();
                let root = app.fashion_directory("catalog")?;
                for r in rows {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(AppError::new(
                            "CANCELLED",
                            "Catalog import cancelled; imported products are retained.",
                        ));
                    }
                    let id = format!("{:x}", Sha256::digest(r.source_url.as_bytes()));
                    let mut image = None;
                    if let Some(relative) = r.image_path.filter(|_| {
                        matches!(r.image_rights.as_deref(), Some("owned" | "copy_allowed"))
                    }) {
                        let input = base.join(relative).canonicalize().map_err(error)?;
                        if !input.starts_with(base) {
                            return Err(AppError::new(
                                "CATALOG_ASSET",
                                "Catalog images must be inside the catalog directory.",
                            ));
                        }
                        let output = root.join(format!("{}.png", Uuid::new_v4()));
                        app.process(
                            WorkerRequest::DecodePhoto {
                                path: input.to_string_lossy().into(),
                                output: output.to_string_lossy().into(),
                                max_width: Some(1024),
                            },
                            Duration::from_secs(60),
                            || cancel.load(Ordering::SeqCst),
                            |m| {
                                if let WorkerMessage::Photo { asset, .. } = m {
                                    image = Some(asset);
                                }
                                Ok(())
                            },
                        )?;
                    }
                    let mut product = ProductV1 {
                        id,
                        name: r.name,
                        brand: r.brand,
                        style_id: r.style_id,
                        category: r.category,
                        source_url: r.source_url,
                        image,
                        image_url: None,
                        image_rights: r.image_rights.unwrap_or_else(|| "link_only".into()),
                        image_attribution: r.image_attribution.filter(|s| !s.is_empty()),
                        image_license_url: r.image_license_url.filter(|s| !s.is_empty()),
                        attributes: vec![],
                        embedding: vec![],
                        embedding_model: None,
                    };
                    if let Some(a) = &product.image {
                        if let Ok(result) =
                            app.run_task_pack_sync("embedding", &a.path, None, None, &cancel)
                        {
                            product.embedding = result.embedding;
                            product.embedding_model = Some(result.model_hash);
                        }
                    }
                    lock(&app.inner.db)?.save_product(&product, true)?;
                    if r.price.as_ref().is_some_and(|s| !s.is_empty())
                        || r.availability.as_ref().is_some_and(|s| !s.is_empty())
                    {
                        if r.observed_at.as_ref().is_none_or(|s| s.is_empty()) {
                            return Err(AppError::new(
                                "CATALOG_FORMAT",
                                "Prices and availability require observedAt source timestamps.",
                            ));
                        }
                        let price = r.price.filter(|s| !s.is_empty());
                        let currency = r.currency.filter(|s| !s.is_empty());
                        if price.is_some() && currency.is_none() {
                            return Err(AppError::new(
                                "CATALOG_FORMAT",
                                "Catalog prices require a currency.",
                            ));
                        }
                        lock(&app.inner.db)?.save_listing(&RetailerListingV1 {
                            id: format!("catalog:{}", product.id),
                            product_id: product.id.clone(),
                            provider: "local_catalog".into(),
                            retailer: r.retailer.unwrap_or_else(|| "Imported catalog".into()),
                            url: product.source_url.clone(),
                            price,
                            currency,
                            availability: r.availability.filter(|s| !s.is_empty()),
                            observed_at: r.observed_at.unwrap(),
                            evidence_ids: vec![],
                        })?;
                    }
                    e.result_ids.push(product.id);
                    e.completed_units = e.result_ids.len().to_string();
                    app.emit(&e, None)?;
                }
                Ok(())
            })();
            app.finish_job(&mut e, result, "CATALOG_READY");
        });
        Ok(first)
    }
    pub fn search_catalog(&self, group: &str, query: &str) -> AppResult<Vec<ProductCandidateV1>> {
        if query.len() > 1000 {
            return Err(error("Search query too long"));
        }
        let db = lock(&self.inner.db)?;
        let g = db.garment_group(group)?;
        let items = db
            .group_item_ids(group)?
            .iter()
            .map(|id| db.fashion_item(id))
            .collect::<AppResult<Vec<_>>>()?;
        let products = db.catalog()?;
        drop(db);
        let mut query_embedding = None;
        if let Some(a) = items.first().and_then(|i| i.crop.as_ref()) {
            check_asset(a)?;
            if let Ok(r) =
                self.run_task_pack_sync("embedding", &a.path, None, None, &AtomicBool::new(false))
            {
                query_embedding = Some((r.embedding, r.model_hash));
            }
        }
        let words = query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let category = items
            .first()
            .map(|i| i.category.as_str())
            .unwrap_or("unknown");
        let mut scored = Vec::new();
        for p in products {
            let text = format!(
                "{} {} {}",
                p.name,
                p.brand.as_deref().unwrap_or(""),
                p.category
            )
            .to_lowercase();
            let text_score = if words.is_empty() {
                0.0
            } else {
                words.iter().filter(|w| text.contains(w.as_str())).count() as f64
                    / words.len() as f64
            };
            let visual = query_embedding.as_ref().and_then(|(q, m)| {
                if p.embedding_model.as_ref() == Some(m)
                    && p.embedding.len() == q.len()
                    && !q.is_empty()
                {
                    Some(
                        q.iter()
                            .zip(&p.embedding)
                            .map(|(a, b)| *a as f64 * *b as f64)
                            .sum::<f64>(),
                    )
                } else {
                    None
                }
            });
            if text_score == 0.0 && visual.is_none() && category != p.category {
                continue;
            }
            let score = visual.unwrap_or(text_score);
            scored.push((p, score, visual));
        }
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut result = Vec::new();
        for (p, _, visual) in scored.into_iter().take(20) {
            let c=ProductCandidateV1{id:format!("{:x}",Sha256::digest(format!("catalog:{group}:{}:{}",g.revision,p.id))),group_id:group.into(),group_revision:g.revision,evidence:vec![ProductEvidenceV1{id:Uuid::new_v4().to_string(),url:p.source_url.clone(),title:p.name.clone(),excerpt:format!("User-imported catalog record. Identifier: {}",p.style_id.as_deref().unwrap_or("unknown")),independence_group:p.source_url.clone(),identifier:p.style_id.clone(),authoritative:false,observed_at:String::new()}],listings:lock(&self.inner.db)?.product_listings(&p.id)?,product:p,classification:MatchClassV1::VisuallySimilar,similarity:visual,reason:if visual.is_some(){"Local image embedding retrieval; visual similarity does not establish product identity."}else{"Local catalog text/category retrieval; no image similarity was measured."}.into(),user_confirmed:false,stale:false};
            lock(&self.inner.db)?.save_candidate(&c)?;
            result.push(c);
        }
        Ok(result)
    }
    pub fn associate_garments(&self, project: &str) -> AppResult<JobEvent> {
        let project = project.to_string();
        let (mut event, cancel) = self.job("garment_association", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let groups = app.associate_garments_task(&project, &cancel)?;
                event.result_ids = groups.iter().map(|g| g.id.clone()).collect();
                event.completed_units = event.result_ids.len().to_string();
                Ok(())
            })();
            app.finish_job(&mut event, result, "GARMENTS_GROUPED");
        });
        Ok(first)
    }
    fn associate_garments_task(
        &self,
        project: &str,
        cancel: &AtomicBool,
    ) -> AppResult<Vec<GarmentGroupV1>> {
        let db = lock(&self.inner.db)?;
        let items = db.fashion_items(project)?;
        if items.len() > 500 {
            return Err(AppError::new(
                "ASSOCIATION_LIMIT",
                "Choose and group observations manually for projects with more than 500 items.",
            ));
        }
        let existing = db.garment_groups(project)?;
        let assigned = existing
            .iter()
            .flat_map(|g| g.item_ids.iter().cloned())
            .collect::<std::collections::HashSet<_>>();
        let hints = items
            .iter()
            .map(|i| db.item_subject_tracks(i))
            .collect::<AppResult<Vec<_>>>()?;
        drop(db);
        let mut embeddings = std::collections::HashMap::new();
        for i in &items {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::new("CANCELLED", "Garment association cancelled."));
            }
            if assigned.contains(&i.id) {
                continue;
            }
            if let Some(a) = &i.crop {
                check_asset(a)?;
                if let Ok(r) = self.run_task_pack_sync("embedding", &a.path, None, None, cancel) {
                    embeddings.insert(i.id.clone(), (r.embedding, r.model_hash));
                }
            }
        }
        let mut buckets: Vec<Vec<usize>> = Vec::new();
        for (position, i) in items.iter().enumerate() {
            if assigned.contains(&i.id) {
                continue;
            }
            let mut possible = Vec::new();
            for (index, bucket) in buckets.iter().enumerate() {
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new("CANCELLED", "Garment association cancelled."));
                }
                let prev = &items[bucket[0]];
                if i.category != prev.category {
                    continue;
                }
                let same_source = match (&i.media, &prev.media) {
                    (MediaRefV1::Frame { selection: a }, MediaRefV1::Frame { selection: b }) => {
                        a.source_id == b.source_id
                            && a.frame_ref.index_generation == b.frame_ref.index_generation
                            && a.frame_ref.presentation_ordinal != b.frame_ref.presentation_ordinal
                            && (a.time_seconds - b.time_seconds).abs() <= 10.
                    }
                    _ => false,
                };
                if !same_source
                    || hints[position].len() != 1
                    || hints[bucket[0]].len() != 1
                    || hints[position] != hints[bucket[0]]
                {
                    continue;
                }
                let (Some(a), Some(b)) = (&i.crop, &prev.crop) else {
                    continue;
                };
                let qa = frameiq_analysis::measure(&image::open(&a.path).map_err(error)?);
                let qb = frameiq_analysis::measure(&image::open(&b.path).map_err(error)?);
                let structural = frameiq_analysis::similarity(&qa.structure, &qb.structure);
                let visual = embeddings
                    .get(&i.id)
                    .zip(embeddings.get(&prev.id))
                    .filter(|((a, am), (b, bm))| am == bm && a.len() == b.len())
                    .map(|((a, _), (b, _))| {
                        a.iter()
                            .zip(b)
                            .map(|(a, b)| *a as f64 * *b as f64)
                            .sum::<f64>()
                    });
                let color = i
                    .attributes
                    .iter()
                    .find(|a| a.name == "color")
                    .zip(prev.attributes.iter().find(|a| a.name == "color"))
                    .is_some_and(|(a, b)| a.value.eq_ignore_ascii_case(&b.value));
                if color
                    && ((visual.is_some_and(|v| v >= 0.97) && structural > 0.75)
                        || (structural > 0.94
                            && frameiq_analysis::tracking::iou(&i.rectangle, &prev.rectangle)
                                > 0.5))
                {
                    possible.push(index);
                }
            }
            if possible.len() == 1 {
                buckets[possible[0]].push(position);
            } else {
                buckets.push(vec![position]);
            }
        }
        let mut results = Vec::new();
        for bucket in buckets {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::new(
                    "CANCELLED",
                    "Association cancelled; completed groups are retained.",
                ));
            }
            let g=GarmentGroupV1{id:Uuid::new_v4().to_string(),project_id:project.into(),name:items[bucket[0]].category.clone(),revision:0,item_ids:bucket.iter().map(|n|items[*n].id.clone()).collect(),notes:if bucket.len()>1{"Suggested using a unique subject track, category, geometry and appearance; inspect and split when ambiguous."}else{"Unidentified association: manually collect complementary views. Different or ambiguous subject tracks are kept separate."}.into()};
            results.push(self.save_garment_group(g)?);
        }
        Ok(results)
    }
}
