use super::*;
fn json<T: serde::Serialize>(v: &T) -> AppResult<String> {
    serde_json::to_string(v).map_err(err)
}
impl Database {
    pub fn reserve_fashion_request(&mut self, provider: &str, scope: &str) -> AppResult<()> {
        if !["chatgpt", "ebay", "ollama", "lm_studio"].contains(&provider) {
            return Err(AppError::new(
                "ZERO_BUDGET",
                "This feature permits no paid-provider fallback.",
            ));
        }
        let day = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(err)?
            .as_secs()
            / 86400;
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute(
            "INSERT OR IGNORE INTO fashion_provider_usage VALUES(?,?,?,0)",
            params![provider, scope, day as i64],
        )
        .map_err(err)?;
        if tx.execute("UPDATE fashion_provider_usage SET requests=requests+1 WHERE provider=? AND scope=? AND day=? AND requests<100",params![provider,scope,day as i64]).map_err(err)?==0{return Err(AppError::new("SHOPPING_QUOTA","The local limit of 100 reviewed requests per provider/account per UTC day was reached. No fallback request was sent."));}
        tx.commit().map_err(err)
    }
    pub fn save_fashion_run(
        &self,
        id: &str,
        project: &str,
        group: Option<&str>,
        revision: Option<u32>,
        payload: &serde_json::Value,
    ) -> AppResult<()> {
        self.conn
            .execute(
                "INSERT INTO fashion_research_runs VALUES(?,?,?,?,?)",
                params![id, project, group, revision, json(payload)?],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn fashion_runs(&self, project: &str) -> AppResult<Vec<serde_json::Value>> {
        self.conn.prepare("SELECT payload FROM fashion_research_runs WHERE project_id=? ORDER BY rowid DESC LIMIT 16").map_err(err)?.query_map([project],|r|r.get::<_,String>(0)).map_err(err)?.map(|r|serde_json::from_str(&r.map_err(err)?).map_err(err)).collect()
    }
    pub fn save_target_features(&self, region: &str, f: &TargetFeaturesV2) -> AppResult<()> {
        self.conn.execute("INSERT INTO preset_target_features VALUES(?,?) ON CONFLICT(region_id) DO UPDATE SET payload=excluded.payload",params![region,json(f)?]).map_err(err)?;
        Ok(())
    }
    pub fn target_features(&self, region: &str) -> AppResult<Option<TargetFeaturesV2>> {
        let s: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM preset_target_features WHERE region_id=?",
                [region],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        s.map(|s| serde_json::from_str(&s).map_err(err)).transpose()
    }
    pub fn track_task_dependencies(&self, track: &str, ids: &[String]) -> AppResult<()> {
        for id in ids {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO task_track_dependencies VALUES(?,?)",
                    params![track, id],
                )
                .map_err(err)?;
        }
        Ok(())
    }
    pub fn fashion_exports(&self, project: &str) -> AppResult<Vec<FashionExportV1>> {
        self.conn.prepare("SELECT e.payload FROM fashion_exports e WHERE e.project_id=? ORDER BY e.rowid DESC").map_err(err)?.query_map([project],|r|r.get::<_,String>(0)).map_err(err)?.map(|r|serde_json::from_str(&r.map_err(err)?).map_err(err)).collect()
    }
    pub fn invalidate_task_pack(&mut self, id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM fashion_task_results WHERE pack_id=?", [id])
            .map_err(err)?;
        let packs = self.task_packs()?;
        let hashes = packs
            .iter()
            .filter(|(p, _, _)| p.id == id)
            .flat_map(|(p, _, _)| {
                p.artifacts
                    .iter()
                    .filter(|a| a.name.ends_with(".onnx"))
                    .map(|a| a.sha256.clone())
            })
            .collect::<Vec<_>>();
        for h in hashes {
            self.conn.execute("UPDATE product_candidates SET stale=1 WHERE group_id IN(SELECT g.group_id FROM garment_group_items g JOIN fashion_items i ON i.id=g.item_id WHERE json_extract(i.payload,'$.modelHash')=?)",[&h]).map_err(err)?;
            self.conn.execute("UPDATE fashion_items SET deleted=1,payload=json_set(payload,'$.deleted',json('true')) WHERE corrected=0 AND json_extract(payload,'$.modelHash')=?",[&h]).map_err(err)?;
        }
        self.conn.execute("DELETE FROM target_regions WHERE id IN(SELECT o.region_id FROM track_observations o JOIN task_track_dependencies d ON d.track_id=o.track_id WHERE d.pack_id=?)",[id]).map_err(err)?;
        self.conn.execute("DELETE FROM subject_tracks WHERE id IN(SELECT track_id FROM task_track_dependencies WHERE pack_id=?)",[id]).map_err(err)?;
        Ok(())
    }
    pub fn save_photo(
        &mut self,
        project: &str,
        path: &Path,
        hash: &str,
        asset: &AssetRefV1,
    ) -> AppResult<PhotoSourceV1> {
        let path = path.canonicalize().map_err(err)?;
        let (bytes, modified) = file_stamp(&path)?;
        let existing:Option<(String,String)>=self.conn.query_row("SELECT s.id,r.id FROM sources s JOIN source_revisions r ON r.source_id=s.id WHERE s.project_id=? AND s.path=? AND s.kind='image' ORDER BY r.rowid DESC LIMIT 1",params![project,path.to_string_lossy()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
        let id = existing
            .as_ref()
            .map(|x| x.0.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let revision = Uuid::new_v4().to_string();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let same: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM source_revisions WHERE source_id=? AND sha256=?",
                params![id, hash],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        let revision = same.unwrap_or(revision);
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute(
            "INSERT OR IGNORE INTO sources(id,project_id,kind,name,path) VALUES(?,?,'image',?,?)",
            params![id, project, name, path.to_string_lossy()],
        )
        .map_err(err)?;
        tx.execute("INSERT INTO source_revisions(id,source_id,sha256,bytes,modified_ns) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET bytes=excluded.bytes,modified_ns=excluded.modified_ns",params![revision,id,hash,bytes as i64,modified]).map_err(err)?;
        tx.execute("INSERT INTO photo_assets VALUES(?,?,?,?,?,?) ON CONFLICT(revision_id) DO UPDATE SET image_path=excluded.image_path,image_sha256=excluded.image_sha256",params![revision,id,asset.width,asset.height,asset.path,asset.sha256]).map_err(err)?;
        tx.execute("INSERT INTO photo_heads VALUES(?,?) ON CONFLICT(source_id) DO UPDATE SET revision_id=excluded.revision_id",params![id,revision]).map_err(err)?;
        tx.commit().map_err(err)?;
        self.photo(&revision)
    }
    pub fn photo(&self, revision: &str) -> AppResult<PhotoSourceV1> {
        self.conn.query_row("SELECT s.id,s.project_id,r.id,s.name,s.path,r.sha256,p.width,p.height,p.image_path,p.image_sha256 FROM photo_assets p JOIN sources s ON s.id=p.source_id JOIN source_revisions r ON r.id=p.revision_id WHERE r.id=?",[revision],|r|Ok(PhotoSourceV1{id:r.get(0)?,project_id:r.get(1)?,revision:r.get(2)?,name:r.get(3)?,path:r.get(4)?,sha256:r.get(5)?,width:r.get(6)?,height:r.get(7)?,image_path:r.get(8)?,image_sha256:r.get(9)?})).map_err(err)
    }
    pub fn photos(&self, project: &str) -> AppResult<Vec<PhotoSourceV1>> {
        let ids=self.conn.prepare("SELECT p.revision_id FROM photo_assets p JOIN sources s ON s.id=p.source_id JOIN photo_heads h ON h.revision_id=p.revision_id WHERE s.project_id=? ORDER BY p.rowid DESC").map_err(err)?.query_map([project],|r|r.get::<_,String>(0)).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
        ids.iter().map(|id| self.photo(id)).collect()
    }
    pub fn fashion_items_for_analysis(&self, project: &str) -> AppResult<Vec<FashionItemV1>> {
        self.conn
            .prepare("SELECT payload FROM fashion_items WHERE project_id=? ORDER BY rowid")
            .map_err(err)?
            .query_map([project], |r| r.get::<_, String>(0))
            .map_err(err)?
            .map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn research_cached(
        &self,
        request: &FashionReviewRequestV1,
        revision: Option<u32>,
    ) -> AppResult<bool> {
        if !matches!(request.task, FashionTaskV1::Research) {
            return Ok(false);
        }
        self.conn.query_row("SELECT EXISTS(SELECT 1 FROM fashion_research_runs WHERE group_id=? AND group_revision=? AND json_extract(payload,'$.review.request.task')='research' AND json_extract(payload,'$.review.request.model')=? AND coalesce(json_extract(payload,'$.review.request.shoppingProvider'),'chatgpt')=?)",params![request.group_id,revision,request.model,request.shopping_provider.as_deref().unwrap_or("chatgpt")],|r|r.get(0)).map_err(err)
    }
    pub fn fashion_item(&self, id: &str) -> AppResult<FashionItemV1> {
        let s: String = self
            .conn
            .query_row("SELECT payload FROM fashion_items WHERE id=?", [id], |r| {
                r.get(0)
            })
            .map_err(err)?;
        serde_json::from_str(&s).map_err(err)
    }
    pub fn fashion_items(&self, project: &str) -> AppResult<Vec<FashionItemV1>> {
        self.conn
            .prepare(
                "SELECT payload FROM fashion_items WHERE project_id=? AND deleted=0 ORDER BY rowid",
            )
            .map_err(err)?
            .query_map([project], |r| r.get::<_, String>(0))
            .map_err(err)?
            .map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn save_fashion_item(
        &mut self,
        mut item: FashionItemV1,
        user: bool,
    ) -> AppResult<FashionItemV1> {
        let old: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM fashion_items WHERE id=?",
                [&item.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        if let Some(s) = old.as_ref() {
            let before: FashionItemV1 = serde_json::from_str(s).map_err(err)?;
            if before.project_id != item.project_id || json(&before.media)? != json(&item.media)? {
                return Err(AppError::new(
                    "ITEM_SCOPE",
                    "An item cannot move to a different source.",
                ));
            }
            if !user && before.corrected {
                return Ok(before);
            }
            if before.revision != item.revision {
                return Err(AppError::new(
                    "STALE_ITEM",
                    "Reload this item before saving another correction.",
                ));
            }
            item.revision += 1;
        } else {
            item.revision = 1;
        }
        if user {
            item.corrected = true;
        }
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute("INSERT INTO fashion_items VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,category=excluded.category,corrected=excluded.corrected,deleted=excluded.deleted,payload=excluded.payload",params![item.id,item.project_id,json(&item.media)?,item.revision,item.category,item.corrected,item.deleted,json(&item)?]).map_err(err)?;
        if let Some(old) = old {
            let v: FashionItemV1 = serde_json::from_str(&old).map_err(err)?;
            tx.execute("INSERT OR IGNORE INTO fashion_item_history(item_id,revision,payload) VALUES(?,?,?)",params![item.id,v.revision,old]).map_err(err)?;
        }
        tx.execute("UPDATE garment_groups SET revision=revision+1 WHERE id IN(SELECT group_id FROM garment_group_items WHERE item_id=?)",[&item.id]).map_err(err)?;
        tx.execute("UPDATE product_candidates SET stale=1 WHERE group_id IN(SELECT group_id FROM garment_group_items WHERE item_id=?)",[&item.id]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(item)
    }
    pub fn undo_fashion_item(&mut self, id: &str) -> AppResult<FashionItemV1> {
        let current = self.fashion_item(id)?;
        let (s,target):(String,u32)=self.conn.query_row("SELECT payload,revision FROM fashion_item_history WHERE item_id=? AND revision<? AND undoable=1 ORDER BY revision DESC LIMIT 1",params![id,current.revision],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
        let mut item: FashionItemV1 = serde_json::from_str(&s).map_err(err)?;
        item.revision = current.revision;
        let saved = self.save_fashion_item(item, true)?;
        self.conn
            .execute(
                "UPDATE fashion_item_history SET undoable=0 WHERE item_id=? AND revision IN(?,?)",
                params![id, target, current.revision],
            )
            .map_err(err)?;
        Ok(saved)
    }
    pub fn garment_group(&self, id: &str) -> AppResult<GarmentGroupV1> {
        let (s, rev): (String, u32) = self
            .conn
            .query_row(
                "SELECT payload,revision FROM garment_groups WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(err)?;
        let mut g: GarmentGroupV1 = serde_json::from_str(&s).map_err(err)?;
        g.revision = rev;
        g.item_ids = self.group_item_ids(id)?;
        Ok(g)
    }
    pub fn garment_groups(&self, project: &str) -> AppResult<Vec<GarmentGroupV1>> {
        let ids = self
            .conn
            .prepare("SELECT id FROM garment_groups WHERE project_id=? ORDER BY rowid")
            .map_err(err)?
            .query_map([project], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        ids.iter().map(|id| self.garment_group(id)).collect()
    }
    pub fn save_garment_group(&mut self, mut g: GarmentGroupV1) -> AppResult<GarmentGroupV1> {
        let old = self.garment_group(&g.id).ok();
        if let Some(old) = old {
            if old.project_id != g.project_id || old.revision != g.revision {
                return Err(AppError::new(
                    "STALE_GROUP",
                    "Reload the group before changing its observations.",
                ));
            }
            g.revision += 1;
        } else {
            g.revision = 1;
        }
        let unique: std::collections::HashSet<_> = g.item_ids.iter().collect();
        if unique.len() != g.item_ids.len() || g.item_ids.is_empty() {
            return Err(AppError::new(
                "GROUP_ITEMS",
                "Choose distinct observations for this garment.",
            ));
        }
        for id in &g.item_ids {
            let i = self.fashion_item(id)?;
            if i.project_id != g.project_id || i.deleted {
                return Err(AppError::new(
                    "GROUP_SCOPE",
                    "Choose available items from this project.",
                ));
            }
        }
        let tx = self.conn.transaction().map_err(err)?;
        // Reassignment invalidates the old group as well as the new one.
        for id in &g.item_ids {
            tx.execute("UPDATE product_candidates SET stale=1 WHERE group_id IN(SELECT group_id FROM garment_group_items WHERE item_id=?)",[id]).map_err(err)?;
            tx.execute("UPDATE garment_groups SET revision=revision+1 WHERE id IN(SELECT group_id FROM garment_group_items WHERE item_id=?) AND id<>?",params![id,g.id]).map_err(err)?;
            tx.execute("DELETE FROM garment_group_items WHERE item_id=?", [id])
                .map_err(err)?;
        }
        tx.execute("INSERT INTO garment_groups VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,name=excluded.name,payload=excluded.payload",params![g.id,g.project_id,g.revision,g.name,json(&g)?]).map_err(err)?;
        tx.execute("DELETE FROM garment_group_items WHERE group_id=?", [&g.id])
            .map_err(err)?;
        for id in &g.item_ids {
            tx.execute(
                "INSERT INTO garment_group_items VALUES(?,?)",
                params![g.id, id],
            )
            .map_err(err)?;
        }
        tx.execute(
            "UPDATE product_candidates SET stale=1 WHERE group_id=?",
            [&g.id],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(g)
    }
    pub fn group_item_ids(&self, id: &str) -> AppResult<Vec<String>> {
        self.conn
            .prepare("SELECT item_id FROM garment_group_items WHERE group_id=? ORDER BY rowid")
            .map_err(err)?
            .query_map([id], |r| r.get(0))
            .map_err(err)?
            .collect::<Result<_, _>>()
            .map_err(err)
    }
    pub fn save_product(&self, p: &ProductV1, catalog: bool) -> AppResult<()> {
        self.conn.execute("INSERT INTO fashion_products VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET catalog=max(fashion_products.catalog,excluded.catalog),name=excluded.name,brand=excluded.brand,category=excluded.category,payload=excluded.payload",params![p.id,catalog,p.name,p.brand,p.category,json(p)?]).map_err(err)?;
        self.conn
            .execute("DELETE FROM fashion_product_search WHERE id=?", [&p.id])
            .map_err(err)?;
        self.conn
            .execute(
                "INSERT INTO fashion_product_search VALUES(?,?,?,?)",
                params![p.id, p.name, p.brand, p.category],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn catalog(&self) -> AppResult<Vec<ProductV1>> {
        self.conn
            .prepare(
                "SELECT payload FROM fashion_products WHERE catalog=1 ORDER BY name LIMIT 10000",
            )
            .map_err(err)?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?
            .map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn save_candidate(&mut self, c: &ProductCandidateV1) -> AppResult<()> {
        self.save_product(&c.product, false)?;
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute("INSERT INTO product_candidates VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET group_revision=excluded.group_revision,product_id=excluded.product_id,stale=excluded.stale,payload=excluded.payload",params![c.id,c.group_id,c.group_revision,c.product.id,c.stale,json(c)?]).map_err(err)?;
        for e in &c.evidence {
            tx.execute("INSERT INTO product_evidence VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",params![e.id,e.url,e.independence_group,json(e)?]).map_err(err)?;
            tx.execute(
                "INSERT OR IGNORE INTO candidate_evidence VALUES(?,?)",
                params![c.id, e.id],
            )
            .map_err(err)?;
        }
        for l in &c.listings {
            tx.execute("INSERT INTO retailer_listings VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET product_id=excluded.product_id,provider=excluded.provider,url=excluded.url,payload=excluded.payload",params![l.id,c.product.id,l.provider,l.url,json(l)?]).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(())
    }
    pub fn candidates(&self, group: &str) -> AppResult<Vec<ProductCandidateV1>> {
        self.conn
            .prepare(
                "SELECT payload,stale FROM product_candidates WHERE group_id=? ORDER BY rowid DESC",
            )
            .map_err(err)?
            .query_map([group], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?))
            })
            .map_err(err)?
            .map(|r| {
                let (s, stale) = r.map_err(err)?;
                let mut c: ProductCandidateV1 = serde_json::from_str(&s).map_err(err)?;
                c.stale = stale;
                Ok(c)
            })
            .collect()
    }
    pub fn save_look(&mut self, mut l: SavedLookV1) -> AppResult<SavedLookV1> {
        let owner: Option<String> = self
            .conn
            .query_row(
                "SELECT project_id FROM saved_looks WHERE id=?",
                [&l.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        if owner.as_deref().is_some_and(|p| p != l.project_id) {
            return Err(AppError::new(
                "LOOK_SCOPE",
                "A saved look cannot move to another project.",
            ));
        }
        if let Some(old) = self.looks(&l.project_id)?.iter().find(|x| x.id == l.id) {
            if old.revision != l.revision {
                return Err(AppError::new(
                    "STALE_LOOK",
                    "Reload this look before saving.",
                ));
            }
            l.revision += 1;
        } else {
            l.revision = 1;
        }
        for i in &l.items {
            if self.garment_group(&i.group_id)?.project_id != l.project_id {
                return Err(AppError::new(
                    "LOOK_SCOPE",
                    "Look items must belong to this project.",
                ));
            }
            let cs = self.candidates(&i.group_id)?;
            for id in i.candidate_id.iter().chain(i.alternative_ids.iter()) {
                if !cs.iter().any(|c| &c.id == id && !c.stale) {
                    return Err(AppError::new(
                        "STALE_CANDIDATE",
                        "Review current product evidence before saving.",
                    ));
                }
            }
        }
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute("INSERT INTO saved_looks VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,name=excluded.name,payload=excluded.payload",params![l.id,l.project_id,l.revision,l.name,json(&l)?]).map_err(err)?;
        tx.execute("DELETE FROM look_entries WHERE look_id=?", [&l.id])
            .map_err(err)?;
        for i in &l.items {
            tx.execute(
                "INSERT INTO look_entries VALUES(?,?,?)",
                params![l.id, i.group_id, i.candidate_id],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(l)
    }
    pub fn looks(&self, project: &str) -> AppResult<Vec<SavedLookV1>> {
        self.conn
            .prepare("SELECT payload FROM saved_looks WHERE project_id=? ORDER BY rowid DESC")
            .map_err(err)?
            .query_map([project], |r| r.get::<_, String>(0))
            .map_err(err)?
            .map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn delete_look(&self, id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM saved_looks WHERE id=?", [id])
            .map_err(err)?;
        Ok(())
    }
    pub fn save_fashion_export(&self, e: &FashionExportV1) -> AppResult<()> {
        self.conn.execute("INSERT INTO fashion_exports VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",params![e.id,e.look.project_id,e.look.id,json(e)?]).map_err(err)?;
        Ok(())
    }
    pub fn fashion_export(&self, id: &str) -> AppResult<FashionExportV1> {
        let s: String = self
            .conn
            .query_row(
                "SELECT payload FROM fashion_exports WHERE id=?",
                [id],
                |r| r.get(0),
            )
            .map_err(err)?;
        serde_json::from_str(&s).map_err(err)
    }
    pub fn save_task_pack(&self, p: &TaskPackV2, directory: &str, status: &str) -> AppResult<()> {
        self.conn.execute("INSERT INTO task_packs VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET manifest=excluded.manifest,directory=excluded.directory,status=excluded.status",params![p.id,json(p)?,directory,status]).map_err(err)?;
        Ok(())
    }
    pub fn task_packs(&self) -> AppResult<Vec<(TaskPackV2, String, String)>> {
        self.conn
            .prepare("SELECT manifest,directory,status FROM task_packs ORDER BY id")
            .map_err(err)?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?)))
            .map_err(err)?
            .map(|r| {
                let (s, d, t) = r.map_err(err)?;
                Ok((serde_json::from_str(&s).map_err(err)?, d, t))
            })
            .collect()
    }
}

impl Database {
    pub fn track_pose_dependencies(&self, track: &str, models: &[String]) -> AppResult<()> {
        for model in models {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO pose_track_dependencies(track_id,model_id) VALUES(?,?)",
                    params![track, model],
                )
                .map_err(err)?;
        }
        Ok(())
    }
}
impl Database {
    /// Tracking is a suggestion only. A unique containing track is required before
    /// using appearance to associate an item across frames in a crowded source.
    pub fn item_subject_tracks(&self, item: &FashionItemV1) -> AppResult<Vec<String>> {
        let MediaRefV1::Frame { selection } = &item.media else {
            return Ok(vec![]);
        };
        let mut stmt=self.conn.prepare("SELECT DISTINCT o.track_id,r.region_json FROM track_observations o JOIN target_regions r ON r.id=o.region_id WHERE r.source_id=? AND r.index_id=? AND r.ordinal=?").map_err(err)?;
        let rows = stmt
            .query_map(
                params![
                    selection.source_id,
                    selection.frame_ref.index_generation,
                    selection.frame_ref.presentation_ordinal
                ],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(err)?;
        let mut ids = Vec::new();
        for row in rows {
            let (id, payload) = row.map_err(err)?;
            let r: GuidedRegionV1 = serde_json::from_str(&payload).map_err(err)?;
            let x = item.rectangle.x + item.rectangle.width / 2.;
            let y = item.rectangle.y + item.rectangle.height / 2.;
            if x >= r.rectangle.x
                && x <= r.rectangle.x + r.rectangle.width
                && y >= r.rectangle.y
                && y <= r.rectangle.y + r.rectangle.height
                && !ids.contains(&id)
            {
                ids.push(id);
            }
        }
        Ok(ids)
    }
}

impl Database {
    pub fn save_listing(&self, l: &RetailerListingV1) -> AppResult<()> {
        self.conn.execute("INSERT INTO retailer_listings VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET product_id=excluded.product_id,provider=excluded.provider,url=excluded.url,payload=excluded.payload",params![l.id,l.product_id,l.provider,l.url,json(l)?]).map_err(err)?;
        Ok(())
    }
    pub fn product_listings(&self, id: &str) -> AppResult<Vec<RetailerListingV1>> {
        self.conn
            .prepare("SELECT payload FROM retailer_listings WHERE product_id=?")
            .map_err(err)?
            .query_map([id], |r| r.get::<_, String>(0))
            .map_err(err)?
            .map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
}

impl Database {
    pub fn task_result(&self, key: &str) -> AppResult<Option<TaskResultV2>> {
        let v: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM fashion_task_results WHERE cache_key=?",
                [key],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        v.map(|v| serde_json::from_str(&v).map_err(err)).transpose()
    }
    pub fn save_task_result(&self, key: &str, result: &TaskResultV2) -> AppResult<()> {
        self.conn.execute("INSERT INTO fashion_task_results VALUES(?,?,?) ON CONFLICT(cache_key) DO UPDATE SET payload=excluded.payload",params![key,result.pack_id,json(result)?]).map_err(err)?;
        Ok(())
    }
}
#[cfg(test)]
mod fashion_tests {
    use super::*;
    #[test]
    fn reviewed_request_quota_is_scoped_and_never_falls_back_to_paid_providers() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open(&dir.path().join("quota.db")).unwrap();
        for _ in 0..100 {
            db.reserve_fashion_request("chatgpt", "account-a").unwrap();
        }
        assert_eq!(
            db.reserve_fashion_request("chatgpt", "account-a")
                .unwrap_err()
                .code,
            "SHOPPING_QUOTA"
        );
        db.reserve_fashion_request("chatgpt", "account-b").unwrap();
        assert_eq!(
            db.reserve_fashion_request("paid_api", "account-a")
                .unwrap_err()
                .code,
            "ZERO_BUDGET"
        );
    }
}
impl Database {
    pub fn fashion_run(&self, id: &str) -> AppResult<Option<serde_json::Value>> {
        let payload: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM fashion_research_runs WHERE id=?",
                [id],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        payload
            .map(|p| serde_json::from_str(&p).map_err(err))
            .transpose()
    }
}
