use super::*;
use crate::fashion::{check_asset, PreparedFashion, CATEGORIES};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use frameiq_inference::product_providers::ProductDiscoveryProvider;
use image::codecs::jpeg::JpegEncoder;
impl Application {
    pub fn fashion_history(&self, project: &str) -> AppResult<Vec<serde_json::Value>> {
        lock(&self.inner.db)?.fashion_runs(project)
    }
    pub fn prepare_fashion_review(
        &self,
        mut request: FashionReviewRequestV1,
    ) -> AppResult<JobEvent> {
        if request.model.is_empty()
            || request.model.len() > 128
            || request.item_ids.len() > 8
            || request.media.len() > 8
        {
            return Err(AppError::new(
                "FASHION_INPUT",
                "Choose a model and at most eight item views.",
            ));
        }
        if request
            .runtime
            .as_deref()
            .is_some_and(|r| !["ollama", "lm_studio"].contains(&r))
        {
            return Err(AppError::new(
                "LOCAL_RUNTIME",
                "Choose a supported local runtime.",
            ));
        }
        if matches!(request.task, FashionTaskV1::Research) && request.runtime.is_some() {
            return Err(AppError::new(
                "SEARCH_CAPABILITY",
                "Use ChatGPT for online product research.",
            ));
        }
        if request
            .shopping_provider
            .as_deref()
            .is_some_and(|p| p != "ebay" && p != "chatgpt")
        {
            return Err(AppError::new(
                "ZERO_BUDGET",
                "Choose ChatGPT or the optional authorized eBay adapter.",
            ));
        }
        if request.shopping_provider.as_deref() == Some("ebay")
            && (!matches!(request.task, FashionTaskV1::Research) || request.runtime.is_some())
        {
            return Err(AppError::new(
                "SEARCH_CAPABILITY",
                "eBay supports reviewed product research only.",
            ));
        }
        let db = lock(&self.inner.db)?;
        let mut revisions = Vec::new();
        if let Some(id) = &request.group_id {
            let g = db.garment_group(id)?;
            if g.project_id != request.project_id {
                return Err(AppError::new(
                    "GROUP_SCOPE",
                    "Choose a garment from this project.",
                ));
            }
            let mut observations = db
                .group_item_ids(id)?
                .iter()
                .map(|id| db.fashion_item(id))
                .collect::<AppResult<Vec<_>>>()?;
            observations.sort_by_key(|i| {
                i.attributes
                    .iter()
                    .find(|a| a.name == "view_orientation")
                    .map(|a| match a.value.as_str() {
                        "front" => 0,
                        "back" => 1,
                        "side_left" | "side_right" | "side" => 2,
                        "detail" => 3,
                        _ => 4,
                    })
                    .unwrap_or(4)
            });
            let mut hashes = std::collections::HashSet::new();
            let mut views = std::collections::HashSet::new();
            let mut selected = Vec::new();
            let mut remainder = Vec::new();
            for i in observations {
                if i.deleted {
                    continue;
                }
                if let Some(c) = &i.crop {
                    if !hashes.insert(c.sha256.clone()) {
                        continue;
                    }
                }
                let view = i
                    .attributes
                    .iter()
                    .find(|a| a.name == "view_orientation")
                    .map(|a| a.value.clone())
                    .unwrap_or_else(|| "unknown".into());
                if views.insert(view) {
                    selected.push(i.id);
                } else {
                    remainder.push(i.id);
                }
            }
            selected.extend(remainder);
            request.item_ids = selected;
            request
                .item_ids
                .truncate(if request.shopping_provider.as_deref() == Some("ebay") {
                    1
                } else {
                    8
                });
        }
        for id in &request.item_ids {
            let i = db.fashion_item(id)?;
            if i.deleted || i.project_id != request.project_id {
                return Err(AppError::new(
                    "ITEM_SCOPE",
                    "Choose available items from this project.",
                ));
            }
            revisions.push(i.revision);
        }
        let group_revision = request
            .group_id
            .as_ref()
            .map(|id| db.garment_group(id).map(|g| g.revision))
            .transpose()?;
        drop(db);
        if matches!(request.task, FashionTaskV1::Detect)
            && (!request.item_ids.is_empty()
                || request.media.len() != 1
                || request.group_id.is_some())
        {
            return Err(AppError::new(
                "FASHION_INPUT",
                "Detection needs one complete source image.",
            ));
        }
        if matches!(request.task, FashionTaskV1::Attributes)
            && (request.item_ids.len() != 1
                || !request.media.is_empty()
                || request.group_id.is_some())
        {
            return Err(AppError::new(
                "FASHION_INPUT",
                "Select one item for attribute analysis.",
            ));
        }
        let count = if request.item_ids.is_empty() {
            request.media.len()
        } else {
            request.item_ids.len()
        };
        if count == 0 || count > 8 || matches!(request.task, FashionTaskV1::Detect) && count != 1 {
            return Err(AppError::new("FASHION_INPUT","Detection needs one image; attribute and research tasks accept one to eight views."));
        }
        if matches!(request.task, FashionTaskV1::Research) && request.group_id.is_none() {
            return Err(AppError::new(
                "GROUP_REQUIRED",
                "Group the observations before researching this garment.",
            ));
        }
        let (mut event, cancel) = self.job("fashion_preparing", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let _lane =
                    lock_cancellable(&app.inner.cloud_lane, || cancel.load(Ordering::SeqCst))?;
                {
                    let mut reviews = lock(&app.inner.fashion_reviews)?;
                    reviews.retain(|_, p| p.created.elapsed() < Duration::from_secs(600));
                    if reviews.len() >= 4 {
                        return Err(AppError::new(
                            "REVIEW_LIMIT",
                            "Discard an earlier image review first.",
                        ));
                    }
                }
                let directory = tempfile::Builder::new()
                    .prefix("review-")
                    .tempdir_in(app.fashion_directory("reviews")?)
                    .map_err(error)?;
                event.status = JobStatus::Running;
                event.total_units = Some(count.to_string());
                app.emit(&event, None)?;
                let mut images = Vec::new();
                for index in 0..count {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(AppError::new("CANCELLED", "Image preparation cancelled."));
                    }
                    let source = if request.item_ids.is_empty() {
                        app.media_asset_task(&request.project_id, &request.media[index], &cancel)?
                    } else {
                        let item = lock(&app.inner.db)?.fashion_item(&request.item_ids[index])?;
                        app.media_asset_task(&request.project_id, &item.media, &cancel)?;
                        let a = item.crop.ok_or_else(|| {
                            error("Save the item region before requesting analysis")
                        })?;
                        check_asset(&a)?;
                        a
                    };
                    let mut img = image::open(&source.path).map_err(error)?;
                    if img.width() > 1024 || img.height() > 1024 {
                        img = img.resize(1024, 1024, image::imageops::FilterType::Triangle);
                    }
                    let mut bytes = Vec::new();
                    JpegEncoder::new_with_quality(&mut bytes, 85)
                        .encode_image(&img)
                        .map_err(error)?;
                    if bytes.len() > 1024 * 1024 {
                        return Err(error("Reviewed image exceeds one MiB"));
                    }
                    let path = directory.path().join(format!("{index}.jpg"));
                    fs::write(&path, &bytes).map_err(error)?;
                    images.push(AssetRefV1 {
                        path: path.to_string_lossy().into(),
                        sha256: format!("{:x}", Sha256::digest(&bytes)),
                        width: img.width(),
                        height: img.height(),
                    });
                    event.completed_units = (index + 1).to_string();
                    app.emit(&event, None)?;
                }
                let review = FashionReviewV1 {
                    id: event.job_id.clone(),
                    request,
                    images,
                    group_revision,
                    item_revisions: revisions,
                };
                app.validate_fashion_review(&review)?;
                lock(&app.inner.fashion_reviews)?.insert(
                    review.id.clone(),
                    PreparedFashion {
                        review,
                        created: Instant::now(),
                        _directory: directory,
                    },
                );
                event.result_ids = vec![event.job_id.clone()];
                Ok(())
            })();
            app.finish_job(&mut event, result, "FASHION_REVIEW_READY");
        });
        Ok(first)
    }
    pub fn fashion_review(&self, id: &str) -> AppResult<FashionReviewV1> {
        let mut rs = lock(&self.inner.fashion_reviews)?;
        rs.retain(|_, p| p.created.elapsed() < Duration::from_secs(600));
        rs.get(id).map(|p| p.review.clone()).ok_or_else(|| {
            AppError::new(
                "REVIEW_EXPIRED",
                "Prepare the images again; this review expired or was used.",
            )
        })
    }
    pub fn discard_fashion_review(&self, id: &str) -> AppResult<()> {
        lock(&self.inner.fashion_reviews)?.remove(id);
        Ok(())
    }
    fn validate_fashion_review(&self, r: &FashionReviewV1) -> AppResult<()> {
        for (i, id) in r.request.item_ids.iter().enumerate() {
            let item = lock(&self.inner.db)?.fashion_item(id)?;
            if item.deleted || Some(&item.revision) != r.item_revisions.get(i) {
                return Err(AppError::new(
                    "STALE_ITEM",
                    "The item changed. Review the current crop again.",
                ));
            }
            self.media_asset(&r.request.project_id, &item.media)?;
        }
        for m in &r.request.media {
            self.media_asset(&r.request.project_id, m)?;
        }
        if let Some(id) = &r.request.group_id {
            if Some(lock(&self.inner.db)?.garment_group(id)?.revision) != r.group_revision {
                return Err(AppError::new(
                    "STALE_GROUP",
                    "The garment views changed. Prepare another review.",
                ));
            }
        }
        for a in &r.images {
            check_asset(a)?;
        }
        Ok(())
    }
    pub fn run_fashion_review(
        &self,
        id: &str,
        credential: Option<String>,
        consent: bool,
    ) -> AppResult<JobEvent> {
        if !consent {
            return Err(AppError::new(
                "CONSENT_REQUIRED",
                "Review the exact images and approve this task.",
            ));
        }
        let review = self.fashion_review(id)?;
        self.validate_fashion_review(&review)?;
        let prepared = lock(&self.inner.fashion_reviews)?
            .remove(id)
            .ok_or_else(|| AppError::new("REVIEW_EXPIRED", "This review was already used."))?;
        let (mut event, cancel) = self.job("fashion_analysis", None)?;
        let first = event.clone();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let _lane =
                    lock_cancellable(&app.inner.cloud_lane, || cancel.load(Ordering::SeqCst))?;
                app.validate_fashion_review(&review)?;
                event.status = JobStatus::Running;
                app.emit(&event, None)?;
                let images = review
                    .images
                    .iter()
                    .map(|a| fs::read(&a.path).map_err(error))
                    .collect::<AppResult<Vec<_>>>()?;
                let context = {
                    let db = lock(&app.inner.db)?;
                    let items = review
                        .request
                        .item_ids
                        .iter()
                        .map(|id| db.fashion_item(id))
                        .collect::<AppResult<Vec<_>>>()?;
                    serde_json::to_string(&items.iter().map(|i|serde_json::json!({"category":i.category,"attributes":i.attributes})).collect::<Vec<_>>()).map_err(error)?
                };
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new(
                        "CANCELLED",
                        "Request cancelled before dispatch.",
                    ));
                }
                if lock(&app.inner.db)?.research_cached(&review.request, review.group_revision)? {
                    return Err(AppError::new("RESEARCH_CACHED","This garment revision already has completed research with this provider/model. Open Product candidates or analysis history; no duplicate search was sent."));
                }
                let provider = review.request.runtime.as_deref().unwrap_or(
                    review
                        .request
                        .shopping_provider
                        .as_deref()
                        .unwrap_or("chatgpt"),
                );
                lock(&app.inner.db)?.reserve_fashion_request(
                    provider,
                    review.request.account_id.as_deref().unwrap_or("local"),
                )?;
                let outcome = if review.request.shopping_provider.as_deref() == Some("ebay") {
                    frameiq_inference::product_providers::EbaySearch {
                        credentials: credential.as_deref().ok_or_else(|| {
                            AppError::new(
                                "SHOPPING_AUTH",
                                "Configure the optional eBay credentials first.",
                            )
                        })?,
                    }
                    .search("", &images, &context, &|| cancel.load(Ordering::SeqCst))?
                } else if let Some(runtime) = &review.request.runtime {
                    match app.infer(
                        InferenceRequestV1::FashionLocal {
                            runtime: runtime.clone(),
                            port: review.request.port.unwrap_or(if runtime == "ollama" {
                                11434
                            } else {
                                1234
                            }),
                            model: review.request.model.clone(),
                            token: credential,
                            images: images.iter().map(|b| STANDARD.encode(b)).collect(),
                            context,
                            task: review.request.task.clone(),
                        },
                        || cancel.load(Ordering::SeqCst),
                    )? {
                        InferenceResponseV1::Fashion { result } => {
                            serde_json::from_value(result).map_err(error)?
                        }
                        _ => return Err(error("Local model returned no fashion result")),
                    }
                } else {
                    let token = credential
                        .as_deref()
                        .ok_or_else(|| AppError::new("AUTH_REQUIRED", "Connect ChatGPT first."))?;
                    if matches!(review.request.task, FashionTaskV1::Research) {
                        frameiq_inference::product_providers::ChatGptSearch { token }.search(
                            &review.request.model,
                            &images,
                            &context,
                            &|| cancel.load(Ordering::SeqCst),
                        )?
                    } else {
                        frameiq_inference::fashion::request(
                            token,
                            &review.request.model,
                            &review.request.task,
                            &images,
                            &context,
                            || cancel.load(Ordering::SeqCst),
                        )?
                    }
                };
                app.validate_fashion_review(&review)?;
                let retained = app.fashion_directory("research")?.join(&event.job_id);
                fs::create_dir(&retained).map_err(error)?;
                let mut preserved = review.clone();
                for (i, a) in preserved.images.iter_mut().enumerate() {
                    let target = retained.join(format!("{i}.jpg"));
                    fs::copy(&a.path, &target).map_err(error)?;
                    a.path = target.to_string_lossy().into();
                }
                lock(&app.inner.db)?.save_fashion_run(&event.job_id,&review.request.project_id,review.request.group_id.as_deref(),review.group_revision,&serde_json::json!({"jobId":event.job_id,"review":preserved,"outcome":outcome,"adapterVersion":"fashion-structured-v1","sources":outcome.sources,"searchEvents":outcome.search_events}))?;
                app.apply_fashion_outcome(&review, outcome, &cancel, &mut event)?;
                Ok(())
            })();
            drop(prepared);
            app.finish_job(&mut event, result, "FASHION_RESULTS_READY");
        });
        Ok(first)
    }
    fn apply_fashion_outcome(
        &self,
        r: &FashionReviewV1,
        out: frameiq_inference::fashion::Outcome,
        cancel: &AtomicBool,
        event: &mut JobEvent,
    ) -> AppResult<()> {
        match r.request.task {
            FashionTaskV1::Detect => {
                let media = r
                    .request
                    .media
                    .first()
                    .ok_or_else(|| error("Choose source media"))?;
                let source = self.media_asset_task(&r.request.project_id, media, cancel)?;
                let existing =
                    lock(&self.inner.db)?.fashion_items_for_analysis(&r.request.project_id)?;
                for prediction in out.items {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(AppError::new(
                            "CANCELLED",
                            "Detection cancelled; completed items are retained.",
                        ));
                    }
                    if existing.iter().any(|i| {
                        i.corrected
                            && serde_json::to_string(&i.media).ok()
                                == serde_json::to_string(media).ok()
                            && frameiq_analysis::tracking::iou(&i.rectangle, &prediction.rectangle)
                                > 0.75
                    }) {
                        continue;
                    }
                    let category = if CATEGORIES.contains(&prediction.category.as_str()) {
                        prediction.category
                    } else {
                        "unknown".into()
                    };
                    let old = existing.iter().find(|i| {
                        serde_json::to_string(&i.media).ok() == serde_json::to_string(media).ok()
                            && i.category == category
                            && frameiq_analysis::tracking::iou(&i.rectangle, &prediction.rectangle)
                                > 0.75
                    });
                    if old.is_some_and(|i| i.corrected) {
                        continue;
                    }
                    let root = self.fashion_directory("items")?;
                    let path = root.join(format!("{}.png", Uuid::new_v4()));
                    let mut crop = None;
                    self.process(
                        WorkerRequest::CropPhoto {
                            path: source.path.clone(),
                            output: path.to_string_lossy().into(),
                            mask_output: None,
                            rectangle: prediction.rectangle.clone(),
                            polygon: vec![],
                        },
                        Duration::from_secs(60),
                        || cancel.load(Ordering::SeqCst),
                        |m| {
                            if let WorkerMessage::PhotoAssets { crop: a, .. } = m {
                                crop = Some(a);
                            }
                            Ok(())
                        },
                    )?;
                    let i = FashionItemV1 {
                        id: old
                            .map(|i| i.id.clone())
                            .unwrap_or_else(|| Uuid::new_v4().to_string()),
                        project_id: r.request.project_id.clone(),
                        media: media.clone(),
                        revision: old.map(|i| i.revision).unwrap_or(0),
                        category,
                        rectangle: prediction.rectangle,
                        polygon: vec![],
                        mask: None,
                        crop,
                        attributes: prediction
                            .attributes
                            .into_iter()
                            .map(|mut a| {
                                a.evidence_ids = vec![format!("analysis:{}", event.job_id)];
                                a
                            })
                            .collect(),
                        suppressed_attributes: vec![],
                        detection_score: None,
                        provenance: format!(
                            "unverified_{}",
                            r.request.runtime.as_deref().unwrap_or("chatgpt")
                        ),
                        model_hash: Some(
                            out.model.clone().unwrap_or_else(|| r.request.model.clone()),
                        ),
                        corrected: false,
                        deleted: false,
                    };
                    let saved = lock(&self.inner.db)?.save_fashion_item(i, false)?;
                    event.result_ids.push(saved.id);
                }
            }
            FashionTaskV1::Attributes => {
                for id in &r.request.item_ids {
                    let mut db = lock(&self.inner.db)?;
                    let mut i = db.fashion_item(id)?;
                    let protected = i
                        .attributes
                        .iter()
                        .filter(|a| a.origin == "user")
                        .cloned()
                        .collect::<Vec<_>>();
                    i.attributes = out
                        .attributes
                        .iter()
                        .filter(|a| {
                            !protected.iter().any(|p| p.name == a.name)
                                && !i.suppressed_attributes.contains(&a.name)
                        })
                        .cloned()
                        .map(|mut a| {
                            a.evidence_ids = vec![format!("analysis:{}", event.job_id)];
                            a
                        })
                        .chain(protected.iter().cloned())
                        .collect();
                    let corrected = i.corrected;
                    let saved = db.save_fashion_item(i, corrected)?;
                    event.result_ids.push(saved.id);
                }
            }
            FashionTaskV1::Research => {
                let group = r
                    .request
                    .group_id
                    .as_ref()
                    .ok_or_else(|| error("Choose a garment group"))?;
                let revision = r
                    .group_revision
                    .ok_or_else(|| error("Missing garment revision"))?;
                for p in out.products {
                    let listing_id = format!("{:x}", Sha256::digest(p.url.as_bytes()));
                    let identity = match (&p.brand, &p.style_id) {
                        (Some(brand), Some(style)) => {
                            format!("{}:{}", brand.to_lowercase(), style.to_lowercase())
                        }
                        _ => p.url.clone(),
                    };
                    let product_id = format!("{:x}", Sha256::digest(identity.as_bytes()));
                    let evidence_id = Uuid::new_v4().to_string();
                    let observed_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(error)?
                        .as_secs()
                        .to_string();
                    let domain = reqwest::Url::parse(&p.url)
                        .map_err(error)?
                        .host_str()
                        .unwrap_or_default()
                        .to_string();
                    let title = out
                        .sources
                        .iter()
                        .find(|(u, _)| u == &p.url)
                        .map(|(_, t)| t.clone())
                        .unwrap_or_else(|| p.name.clone());
                    let observed_identifier = {
                        let db = lock(&self.inner.db)?;
                        r.request
                            .item_ids
                            .iter()
                            .filter_map(|id| db.fashion_item(id).ok())
                            .flat_map(|i| i.attributes)
                            .filter(|a| {
                                ["style_id", "product_id", "identifier"].contains(&a.name.as_str())
                                    && ["user", "observed"].contains(&a.origin.as_str())
                            })
                            .any(|a| {
                                p.identifier.as_deref() == Some(a.value.trim())
                                    && p.style_id.as_deref() == Some(a.value.trim())
                                    && p.source_excerpt.contains(a.value.trim())
                            })
                    };
                    let candidate = ProductCandidateV1 {
                        id: format!(
                            "{:x}",
                            Sha256::digest(format!("{group}:{revision}:{}", p.url))
                        ),
                        group_id: group.clone(),
                        group_revision: revision,
                        product: ProductV1 {
                            id: product_id.clone(),
                            name: p.name,
                            brand: p.brand,
                            style_id: p.style_id,
                            category: p.category,
                            source_url: p.url.clone(),
                            image: None,
                            image_url: p.image_url,
                            image_rights: "link_only".into(),
                            image_attribution: None,
                            image_license_url: None,
                            attributes: vec![],
                            embedding: vec![],
                            embedding_model: None,
                        },
                        listings: vec![RetailerListingV1 {
                            id: listing_id,
                            product_id,
                            provider: r
                                .request
                                .shopping_provider
                                .clone()
                                .unwrap_or_else(|| "chatgpt_search".into()),
                            retailer: p.retailer,
                            url: p.url.clone(),
                            price: p.price,
                            currency: p.currency,
                            availability: p.availability,
                            observed_at: observed_at.clone(),
                            evidence_ids: vec![evidence_id.clone()],
                        }],
                        evidence: vec![ProductEvidenceV1 {
                            id: evidence_id,
                            url: p.url,
                            title,
                            excerpt: p.source_excerpt,
                            independence_group: domain,
                            identifier: p.identifier,
                            authoritative: false,
                            observed_at,
                        }],
                        classification: if observed_identifier {
                            MatchClassV1::ProbableExact
                        } else {
                            MatchClassV1::VisuallySimilar
                        },
                        similarity: None,
                        reason: if observed_identifier {
                            format!("Observed identifier matches a source-reported identifier. Authoritative source review and user confirmation are still required. {}",p.reason)
                        } else {
                            p.reason
                        },
                        user_confirmed: false,
                        stale: false,
                    };
                    lock(&self.inner.db)?.save_candidate(&candidate)?;
                    event.result_ids.push(candidate.id);
                }
            }
        }
        event.completed_units = event.result_ids.len().to_string();
        Ok(())
    }
    pub fn confirm_candidate(
        &self,
        group: &str,
        id: &str,
        visible_identifier: &str,
        source_url: &str,
        authoritative_confirmed: bool,
    ) -> AppResult<ProductCandidateV1> {
        if !authoritative_confirmed {
            return Err(AppError::new("IDENTITY_EVIDENCE","Confirm that you checked an authoritative source against the visible garment identifier."));
        }
        let mut db = lock(&self.inner.db)?;
        let mut c = db
            .candidates(group)?
            .into_iter()
            .find(|c| c.id == id && !c.stale)
            .ok_or_else(|| {
                AppError::new("CANDIDATE_MISSING", "Choose a current product candidate.")
            })?;
        if visible_identifier.trim().is_empty()
            || c.product.style_id.as_deref() != Some(visible_identifier.trim())
        {
            return Err(AppError::new(
                "IDENTITY_EVIDENCE",
                "Enter the matching style/product identifier visible on the garment or its label.",
            ));
        }
        let e = c
            .evidence
            .iter_mut()
            .find(|e| {
                e.url == source_url
                    && e.identifier.as_deref() == Some(visible_identifier.trim())
                    && e.excerpt.contains(visible_identifier.trim())
            })
            .ok_or_else(|| {
                AppError::new(
                    "IDENTITY_EVIDENCE",
                    "Choose an authoritative source that records this identifier.",
                )
            })?;
        e.authoritative = true;
        c.classification = MatchClassV1::VerifiedExact;
        c.user_confirmed = true;
        db.save_candidate(&c)?;
        Ok(c)
    }
}
#[cfg(test)]
mod correction_tests {
    use super::*;
    fn wait(app: &Application, event: JobEvent) {
        let start = Instant::now();
        loop {
            let e = app.get_job(&event.job_id).unwrap();
            match e.status {
                JobStatus::Succeeded => return,
                JobStatus::Failed | JobStatus::Cancelled => panic!("{:?}", e.error),
                _ => {
                    assert!(start.elapsed() < Duration::from_secs(20));
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
    #[test]
    fn unchanged_attribute_origins_masks_and_explicit_removals_survive_reruns() {
        let temp = tempfile::tempdir().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let worker = std::env::var_os("FRAMEIQ_TEST_WORKER")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                root.join(if cfg!(windows) {
                    "target/debug/frameiq-worker.exe"
                } else {
                    "target/debug/frameiq-worker"
                })
            });
        let app = Application::new(&temp.path().join("data"), worker, Arc::new(|_| {})).unwrap();
        let project = app.create_project("Correction semantics").unwrap();
        let path = temp.path().join("source.png");
        image::RgbImage::new(80, 80).save(&path).unwrap();
        wait(&app, app.import_photo(&project.id, &path).unwrap());
        let photo = app.photos(&project.id).unwrap().remove(0);
        let media = MediaRefV1::Photo {
            revision: photo.revision,
        };
        let item = FashionItemV1 {
            id: String::new(),
            project_id: project.id.clone(),
            media: media.clone(),
            revision: 0,
            category: "top".into(),
            rectangle: RegionRectV1 {
                x: 0.1,
                y: 0.1,
                width: 0.8,
                height: 0.8,
            },
            polygon: vec![[0.1, 0.1], [0.9, 0.1], [0.7, 0.9]],
            mask: None,
            crop: None,
            attributes: vec![FashionAttributeV1 {
                name: "color".into(),
                value: "blue".into(),
                origin: "user".into(),
                confidence: None,
                evidence_ids: vec![],
            }],
            suppressed_attributes: vec![],
            detection_score: None,
            provenance: "user".into(),
            model_hash: None,
            corrected: true,
            deleted: false,
        };
        wait(&app, app.save_fashion_item(item).unwrap());
        let original = app.fashion_items(&project.id).unwrap().remove(0);
        let review = FashionReviewV1 {
            id: "test".into(),
            request: FashionReviewRequestV1 {
                project_id: project.id.clone(),
                media: vec![],
                item_ids: vec![original.id.clone()],
                group_id: None,
                task: FashionTaskV1::Attributes,
                model: "model".into(),
                account_id: None,
                runtime: None,
                port: None,
                shopping_provider: None,
            },
            images: vec![],
            group_revision: None,
            item_revisions: vec![original.revision],
        };
        let (mut event, cancel) = app.job("fashion_analysis", None).unwrap();
        let outcome = || frameiq_inference::fashion::Outcome {
            model: Some("model".into()),
            items: vec![],
            attributes: vec![
                FashionAttributeV1 {
                    name: "color".into(),
                    value: "red".into(),
                    origin: "observed".into(),
                    confidence: None,
                    evidence_ids: vec![],
                },
                FashionAttributeV1 {
                    name: "pattern".into(),
                    value: "striped".into(),
                    origin: "inferred".into(),
                    confidence: Some(0.7),
                    evidence_ids: vec![],
                },
            ],
            products: vec![],
            sources: vec![],
            search_events: vec![],
        };
        app.apply_fashion_outcome(&review, outcome(), &cancel, &mut event)
            .unwrap();
        let mut revised = app.fashion_items(&project.id).unwrap().remove(0);
        assert_eq!(revised.attributes[0].name, "pattern");
        assert_eq!(revised.attributes[1].value, "blue");
        revised.category = "shirt".into();
        wait(&app, app.save_fashion_item(revised).unwrap());
        let mut revised = app.fashion_items(&project.id).unwrap().remove(0);
        assert_eq!(revised.attributes[0].origin, "inferred");
        assert_eq!(
            revised.mask.as_ref().unwrap().sha256,
            original.mask.unwrap().sha256
        );
        revised.attributes.retain(|a| a.name != "pattern");
        wait(&app, app.save_fashion_item(revised).unwrap());
        app.apply_fashion_outcome(&review, outcome(), &cancel, &mut event)
            .unwrap();
        let revised = app.fashion_items(&project.id).unwrap().remove(0);
        assert_eq!(revised.attributes.len(), 1);
        assert_eq!(revised.attributes[0].value, "blue");
        assert_eq!(revised.suppressed_attributes, vec!["pattern"]);
    }
}
