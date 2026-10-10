use frameiq_contracts::*;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{path::Path, time::UNIX_EPOCH};
use uuid::Uuid;
fn err(e: impl std::fmt::Display) -> AppError {
    AppError::new("DATABASE_ERROR", e.to_string())
}
fn ordinal(s: &str) -> AppResult<i64> {
    s.parse::<i64>()
        .ok()
        .filter(|n| *n >= 0)
        .ok_or_else(|| AppError::new("INVALID_FRAME", "Invalid presentation ordinal."))
}
pub fn file_stamp(path: &Path) -> AppResult<(u64, String)> {
    let m = std::fs::metadata(path)
        .map_err(|_| AppError::new("SOURCE_MISSING", "The original source file is missing."))?;
    let n = m
        .modified()
        .map_err(err)?
        .duration_since(UNIX_EPOCH)
        .map_err(err)?
        .as_nanos()
        .to_string();
    Ok((m.len(), n))
}
pub struct Database {
    conn: Connection,
}
impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(err)?;
        }
        let mut conn = Connection::open(path).map_err(err)?;
        conn.execute_batch("PRAGMA foreign_keys=ON;PRAGMA journal_mode=WAL;PRAGMA busy_timeout=5000;CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY,checksum TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now'))) STRICT;").map_err(err)?;
        let migrations = [
            (1i64, include_str!("../../../migrations/001_core.sql")),
            (2i64, include_str!("../../../migrations/002_recovery.sql")),
            (3i64, include_str!("../../../migrations/003_analysis.sql")),
            (4i64, include_str!("../../../migrations/004_models.sql")),
            (
                5i64,
                include_str!("../../../migrations/005_detection_features.sql"),
            ),
            (
                6i64,
                include_str!("../../../migrations/006_body_features.sql"),
            ),
            (
                7i64,
                include_str!("../../../migrations/007_subject_choices.sql"),
            ),
            (
                8i64,
                include_str!("../../../migrations/008_export_attempt_items.sql"),
            ),
            (
                9i64,
                include_str!("../../../migrations/009_preset_execution.sql"),
            ),
        ];
        let max: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        if max > migrations.last().unwrap().0 {
            return Err(AppError::new(
                "NEWER_DATABASE",
                "This database was written by a newer FrameIQ version.",
            ));
        }
        for (version, sql) in migrations {
            let checksum = format!("{:x}", Sha256::digest(sql.as_bytes()));
            let old: Option<String> = conn
                .query_row(
                    "SELECT checksum FROM schema_migrations WHERE version=?",
                    [version],
                    |r| r.get(0),
                )
                .optional()
                .map_err(err)?;
            if let Some(old) = old {
                if old != checksum {
                    return Err(AppError::new(
                        "MIGRATION_MISMATCH",
                        "An applied migration checksum has changed.",
                    ));
                }
                continue;
            }
            if max > 0 {
                // SQLite backup includes committed WAL pages; copying the main file does not.
                let backup_path = path.with_extension(format!("pre-{version}.backup"));
                let mut backup_conn = Connection::open(backup_path).map_err(err)?;
                rusqlite::backup::Backup::new(&conn, &mut backup_conn)
                    .map_err(err)?
                    .run_to_completion(128, std::time::Duration::from_millis(10), None)
                    .map_err(err)?;
            }
            let tx = conn.transaction().map_err(err)?;
            tx.execute_batch(sql).map_err(err)?;
            tx.execute(
                "INSERT INTO schema_migrations(version,checksum) VALUES(?,?)",
                params![version, checksum],
            )
            .map_err(err)?;
            let violations: i64 = tx
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })
                .map_err(err)?;
            if violations != 0 {
                return Err(AppError::new(
                    "DATABASE_INTEGRITY",
                    "Migration foreign-key validation failed.",
                ));
            }
            tx.commit().map_err(err)?;
        }
        let violations: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .map_err(err)?;
        if violations != 0 {
            return Err(AppError::new(
                "DATABASE_INTEGRITY",
                "Foreign-key validation failed.",
            ));
        }
        let db = Self { conn };
        db.conn
            .execute(
                "UPDATE frame_indexes SET status='cancelled' WHERE status IN('queued','indexing')",
                [],
            )
            .map_err(err)?;
        for mut job in db.list_jobs()? {
            if matches!(
                job.status,
                JobStatus::Queued | JobStatus::Running | JobStatus::Cancelling
            ) {
                job.status = JobStatus::Cancelled;
                job.message_code = "INTERRUPTED_BY_RESTART".into();
                job.sequence += 1;
                db.save_job(&job, None)?;
            }
        }
        Ok(db)
    }
    pub fn create_project(&self, name: &str) -> AppResult<Project> {
        if name.trim().is_empty() {
            return Err(AppError::new("INVALID_PROJECT", "Enter a project name."));
        }
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO projects(id,name) VALUES(?,?)",
                params![id, name.trim()],
            )
            .map_err(err)?;
        self.conn
            .query_row(
                "SELECT id,name,created_at FROM projects WHERE id=?",
                [id],
                |r| {
                    Ok(Project {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        created_at: r.get(2)?,
                    })
                },
            )
            .map_err(err)
    }
    pub fn projects(&self) -> AppResult<Vec<Project>> {
        self.conn
            .prepare("SELECT id,name,created_at FROM projects ORDER BY created_at DESC")
            .map_err(err)?
            .query_map([], |r| {
                Ok(Project {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    created_at: r.get(2)?,
                })
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)
    }
    pub fn source_for_path(
        &self,
        project_id: &str,
        path: &Path,
    ) -> AppResult<Option<SourceSummary>> {
        let path = path
            .canonicalize()
            .map_err(|_| AppError::new("SOURCE_MISSING", "Select an existing local video."))?;
        let id: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM sources WHERE project_id=? AND path=?",
                params![project_id, path.to_string_lossy()],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        id.map(|id| self.source(&id)).transpose()
    }
    pub fn create_source(&mut self, project_id: &str, path: &Path) -> AppResult<SourceSummary> {
        let path = path
            .canonicalize()
            .map_err(|_| AppError::new("SOURCE_MISSING", "Select an existing local video."))?;
        let (bytes, modified) = file_stamp(&path)?;
        let existing = self.source_for_path(project_id, &path)?;
        if existing
            .as_ref()
            .is_some_and(|s| s.status == "queued" || s.status == "indexing")
        {
            return Err(AppError::new(
                "SOURCE_BUSY",
                "This source is already being indexed.",
            ));
        }
        let reuse_revision = existing
            .as_ref()
            .is_some_and(|s| self.validate_source(s).is_ok());
        let id = existing
            .as_ref()
            .map(|s| s.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let rev = if reuse_revision {
            existing.as_ref().unwrap().revision_id.clone()
        } else {
            Uuid::new_v4().to_string()
        };
        let index = Uuid::new_v4().to_string();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute(
            "INSERT OR IGNORE INTO sources(id,project_id,kind,name,path) VALUES(?,?,'video',?,?)",
            params![id, project_id, name, path.to_string_lossy()],
        )
        .map_err(err)?;
        tx.execute(
            "INSERT OR IGNORE INTO source_revisions(id,source_id,bytes,modified_ns) VALUES(?,?,?,?)",
            params![rev, id, bytes as i64, modified],
        )
        .map_err(err)?;
        tx.execute("DELETE FROM frame_selections WHERE source_id=?", [&id])
            .map_err(err)?;
        tx.execute("INSERT INTO frame_indexes(id,revision_id,decoder_build,status) VALUES(?,?,'ffmpeg-8/software-v1','queued')",params![index,rev]).map_err(err)?;
        tx.commit().map_err(err)?;
        self.source(&id)
    }
    pub fn face_feature(
        &self,
        source: &SourceSummary,
        n: &str,
        model: &ModelInstallationV1,
        algorithm: &str,
        color: &str,
    ) -> AppResult<Option<FaceFrameV1>> {
        let json: Option<String> = self.conn.query_row(
            "SELECT result_json FROM detection_features WHERE source_id=? AND index_id=? AND ordinal=? AND model_id=? AND model_hash=? AND algorithm=? AND color_policy=?",
            params![source.id, source.index_generation, ordinal(n)?, model.id, model.manifest.sha256, algorithm, color],
            |r| r.get(0),
        ).optional().map_err(err)?;
        json.map(|j| serde_json::from_str(&j).map_err(err))
            .transpose()
    }
    pub fn save_face_feature(&self, feature: &FaceFrameV1) -> AppResult<()> {
        let source = self.source(&feature.frame.source_id)?;
        self.validate_source(&source)?;
        if source.index_generation != feature.frame.frame_ref.index_generation {
            return Err(AppError::new(
                "SOURCE_CHANGED",
                "Frame index changed during detection.",
            ));
        }
        let active: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM model_installations WHERE id=? AND status='ready' AND json_extract(manifest_json,'$.sha256')=?)",
            params![feature.model_id, feature.result.model_hash], |r| r.get(0),
        ).map_err(err)?;
        if !active || feature.frame.frame_ref.source_revision_id != source.revision_id {
            return Err(AppError::new(
                "STALE_DETECTION",
                "The model installation or source revision changed during detection.",
            ));
        }
        self.conn.execute("INSERT INTO detection_features(id,source_id,index_id,ordinal,model_id,model_hash,algorithm,color_policy,result_json) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(index_id,ordinal,model_id,model_hash,algorithm,color_policy) DO UPDATE SET id=excluded.id,result_json=excluded.result_json",
            params![feature.id, source.id, source.index_generation, ordinal(&feature.frame.frame_ref.presentation_ordinal)?, feature.model_id, feature.result.model_hash, feature.result.algorithm, feature.color_policy, serde_json::to_string(feature).map_err(err)?]).map_err(err)?;
        Ok(())
    }
    pub fn person_feature(
        &self,
        source: &SourceSummary,
        n: &str,
        model: &ModelInstallationV1,
        algorithm: &str,
        color: &str,
    ) -> AppResult<Option<PersonFrameV1>> {
        let json: Option<String> = self.conn.query_row(
            "SELECT result_json FROM person_features WHERE source_id=? AND index_id=? AND ordinal=? AND model_id=? AND model_hash=? AND algorithm=? AND color_policy=?",
            params![source.id, source.index_generation, ordinal(n)?, model.id, model.manifest.sha256, algorithm, color], |r| r.get(0),
        ).optional().map_err(err)?;
        json.map(|j| serde_json::from_str(&j).map_err(err))
            .transpose()
    }
    fn validate_body_feature(
        &self,
        frame: &Selection,
        model: &str,
        hash: &str,
    ) -> AppResult<SourceSummary> {
        let source = self.source(&frame.source_id)?;
        self.validate_source(&source)?;
        if source.index_generation != frame.frame_ref.index_generation
            || source.revision_id != frame.frame_ref.source_revision_id
        {
            return Err(AppError::new(
                "SOURCE_CHANGED",
                "Frame index or source revision changed during detection.",
            ));
        }
        self.frame(
            &source.index_generation,
            &frame.frame_ref.presentation_ordinal,
        )?;
        let active: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM model_installations WHERE id=? AND status='ready' AND json_extract(manifest_json,'$.sha256')=?)",
            params![model, hash], |r| r.get(0),
        ).map_err(err)?;
        if !active {
            return Err(AppError::new(
                "STALE_DETECTION",
                "The model installation changed during detection.",
            ));
        }
        Ok(source)
    }
    /// Immutable parent ID: concurrent insertions return the canonical cached record.
    pub fn save_person_feature(&self, feature: &PersonFrameV1) -> AppResult<PersonFrameV1> {
        let source = self.validate_body_feature(
            &feature.frame,
            &feature.model_id,
            &feature.result.model_hash,
        )?;
        self.conn.execute("INSERT INTO person_features(id,source_id,index_id,ordinal,model_id,model_hash,algorithm,color_policy,result_json) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(index_id,ordinal,model_id,model_hash,algorithm,color_policy) DO NOTHING",
            params![feature.id, source.id, source.index_generation, ordinal(&feature.frame.frame_ref.presentation_ordinal)?, feature.model_id, feature.result.model_hash, feature.result.algorithm, feature.color_policy, serde_json::to_string(feature).map_err(err)?]).map_err(err)?;
        self.conn.query_row("SELECT result_json FROM person_features WHERE index_id=? AND ordinal=? AND model_id=? AND model_hash=? AND algorithm=? AND color_policy=?",
            params![source.index_generation, ordinal(&feature.frame.frame_ref.presentation_ordinal)?, feature.model_id, feature.result.model_hash, feature.result.algorithm, feature.color_policy], |r| r.get::<_,String>(0)).map_err(err).and_then(|j| serde_json::from_str(&j).map_err(err))
    }
    pub fn person_feature_by_id(&self, id: &str) -> AppResult<PersonFrameV1> {
        let json: String = self
            .conn
            .query_row(
                "SELECT result_json FROM person_features WHERE id=?",
                [id],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?
            .ok_or_else(|| {
                AppError::new(
                    "DETECTION_MISSING",
                    "Run person detection on this frame first.",
                )
            })?;
        let feature: PersonFrameV1 = serde_json::from_str(&json).map_err(err)?;
        self.validate_body_feature(
            &feature.frame,
            &feature.model_id,
            &feature.result.model_hash,
        )?;
        Ok(feature)
    }
    pub fn pose_feature(
        &self,
        person: &str,
        index: u32,
        model: &ModelInstallationV1,
        algorithm: &str,
        color: &str,
    ) -> AppResult<Option<PoseFrameV1>> {
        let json: Option<String> = self.conn.query_row("SELECT result_json FROM pose_features WHERE person_feature_id=? AND person_index=? AND model_id=? AND model_hash=? AND algorithm=? AND color_policy=?",
            params![person,index,model.id,model.manifest.sha256,algorithm,color], |r| r.get(0)).optional().map_err(err)?;
        json.map(|j| serde_json::from_str(&j).map_err(err))
            .transpose()
    }
    pub fn save_pose_feature(&self, feature: &PoseFrameV1) -> AppResult<()> {
        self.validate_body_feature(
            &feature.frame,
            &feature.model_id,
            &feature.result.model_hash,
        )?;
        let parent = self.person_feature_by_id(&feature.person_feature_id)?;
        if feature.person_index as usize >= parent.result.persons.len()
            || serde_json::to_value(&feature.frame).map_err(err)?
                != serde_json::to_value(&parent.frame).map_err(err)?
            || feature.color_policy != parent.color_policy
            || feature.result.width != parent.result.width
            || feature.result.height != parent.result.height
        {
            return Err(AppError::new(
                "INVALID_SUBJECT",
                "Pose must reference a detected person on the same indexed frame.",
            ));
        }
        self.conn.execute("INSERT INTO pose_features(id,person_feature_id,person_index,model_id,model_hash,algorithm,color_policy,result_json) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(person_feature_id,person_index,model_id,model_hash,algorithm,color_policy) DO UPDATE SET id=excluded.id,result_json=excluded.result_json",
            params![feature.id,feature.person_feature_id,feature.person_index,feature.model_id,feature.result.model_hash,feature.result.algorithm,feature.color_policy,serde_json::to_string(feature).map_err(err)?]).map_err(err)?;
        Ok(())
    }
    pub fn delete_pose_feature(&self, id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM pose_features WHERE id=?", [id])
            .map_err(err)?;
        Ok(())
    }
    pub fn retained_mask_paths(&self) -> AppResult<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT json_extract(result_json,'$.result.maskPath') FROM pose_features WHERE json_extract(result_json,'$.result.maskPath') IS NOT NULL").map_err(err)?;
        let rows = stmt.query_map([], |r| r.get(0)).map_err(err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(err)
    }
    pub fn model_mask_paths(&self, model: &str) -> AppResult<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT json_extract(p.result_json,'$.result.maskPath') FROM pose_features p JOIN person_features d ON d.id=p.person_feature_id WHERE (p.model_id=? OR d.model_id=?) AND json_extract(p.result_json,'$.result.maskPath') IS NOT NULL").map_err(err)?;
        let rows = stmt
            .query_map(params![model, model], |r| r.get(0))
            .map_err(err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(err)
    }
    pub fn clear_model_features(&self, model: &str) -> AppResult<()> {
        self.conn.execute("DELETE FROM target_regions WHERE json_extract(region_json,'$.origin')='experimental_shot_tracking' AND id IN (SELECT s.region_id FROM region_subjects s JOIN person_features p ON p.id=s.person_feature_id WHERE p.model_id=?)",[model]).map_err(err)?;
        self.conn.execute("DELETE FROM target_regions WHERE id IN (SELECT o.region_id FROM track_observations o JOIN subject_tracks t ON t.id=o.track_id WHERE t.person_model_id=? OR t.face_model_id=?)",params![model,model]).map_err(err)?;
        self.conn
            .execute(
                "DELETE FROM subject_tracks WHERE person_model_id=? OR face_model_id=?",
                params![model, model],
            )
            .map_err(err)?;
        self.conn
            .execute("DELETE FROM pose_features WHERE model_id=?", [model])
            .map_err(err)?;
        self.conn
            .execute("DELETE FROM person_features WHERE model_id=?", [model])
            .map_err(err)?;
        self.conn
            .execute("DELETE FROM detection_features WHERE model_id=?", [model])
            .map_err(err)?;
        Ok(())
    }
    pub fn remove_model_transaction(&self, model: &str) -> AppResult<Vec<String>> {
        let tx = self.conn.unchecked_transaction().map_err(err)?;
        let masks = self.model_mask_paths(model)?;
        self.clear_model_features(model)?;
        let changed = tx
            .execute(
                "UPDATE model_installations SET status='removed' WHERE id=? AND status<>'removed'",
                [model],
            )
            .map_err(err)?;
        if changed != 1 {
            return Err(AppError::new("MODEL_MISSING", "Model is not installed."));
        }
        tx.commit().map_err(err)?;
        Ok(masks)
    }
    pub fn save_preset(&self, preset: &PresetV1) -> AppResult<()> {
        self.conn
            .execute(
                "INSERT INTO preset_versions(id,name,config_json) VALUES(?,?,?)",
                params![
                    preset.id,
                    preset.name,
                    serde_json::to_string(preset).map_err(err)?
                ],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn presets(&self) -> AppResult<Vec<PresetV1>> {
        self.conn
            .prepare("SELECT config_json FROM preset_versions ORDER BY created_at,id")
            .map_err(err)?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?
            .map(|s| serde_json::from_str(&s.map_err(err)?).map_err(err))
            .collect()
    }
    fn validate_subject(&self, subject: &SubjectReferenceV1) -> AppResult<PersonFrameV1> {
        let parent = self.person_feature_by_id(&subject.person_feature_id)?;
        if subject.person_index as usize >= parent.result.persons.len() {
            return Err(AppError::new(
                "INVALID_SUBJECT",
                "Choose a detected person on this frame.",
            ));
        }
        Ok(parent)
    }
    pub fn choose_subject(&self, subject: &SubjectReferenceV1) -> AppResult<SubjectChoiceV1> {
        let parent = self.validate_subject(subject)?;
        self.conn.execute("INSERT INTO subject_choices(index_id,ordinal,person_feature_id,person_index) VALUES(?,?,?,?) ON CONFLICT(index_id,ordinal) DO UPDATE SET person_feature_id=excluded.person_feature_id,person_index=excluded.person_index",
            params![parent.frame.frame_ref.index_generation,ordinal(&parent.frame.frame_ref.presentation_ordinal)?,subject.person_feature_id,subject.person_index]).map_err(err)?;
        Ok(SubjectChoiceV1 {
            model_id: parent.model_id,
            subject: subject.clone(),
            frame: parent.frame,
        })
    }
    pub fn subject_choice(&self, source: &str, n: &str) -> AppResult<Option<SubjectChoiceV1>> {
        let source = self.source(source)?;
        self.validate_source(&source)?;
        let subject = self.conn.query_row("SELECT person_feature_id,person_index FROM subject_choices WHERE index_id=? AND ordinal=?",params![source.index_generation,ordinal(n)?], |r| Ok(SubjectReferenceV1 { person_feature_id:r.get(0)?,person_index:r.get(1)? })).optional().map_err(err)?;
        subject
            .map(|subject| {
                let parent = self.validate_subject(&subject)?;
                Ok(SubjectChoiceV1 {
                    model_id: parent.model_id,
                    subject,
                    frame: parent.frame,
                })
            })
            .transpose()
    }
    pub fn clear_subject(&self, source: &str, n: &str) -> AppResult<()> {
        let source = self.source(source)?;
        self.conn
            .execute(
                "DELETE FROM subject_choices WHERE index_id=? AND ordinal=?",
                params![source.index_generation, ordinal(n)?],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn save_region(&self, region: &GuidedRegionV1) -> AppResult<()> {
        if let Some(subject) = &region.subject {
            let parent = self.validate_subject(subject)?;
            if serde_json::to_value(&parent.frame).map_err(err)?
                != serde_json::to_value(&region.frame).map_err(err)?
            {
                return Err(AppError::new(
                    "INVALID_SUBJECT",
                    "Region and subject must belong to the same indexed frame.",
                ));
            }
        }
        // Subject association is normalized, so removing a detector preserves the guided measurement.
        let mut stored = region.clone();
        stored.subject = None;
        let tx = self.conn.unchecked_transaction().map_err(err)?;
        tx.execute("INSERT INTO target_regions(id,source_id,index_id,ordinal,revision,region_json) VALUES(?,?,?,?,1,?)",params![region.id,region.frame.source_id,region.frame.frame_ref.index_generation,ordinal(&region.frame.frame_ref.presentation_ordinal)?,serde_json::to_string(&stored).map_err(err)?]).map_err(err)?;
        if let Some(subject) = &region.subject {
            tx.execute("INSERT INTO region_subjects(region_id,person_feature_id,person_index) VALUES(?,?,?)",params![region.id,subject.person_feature_id,subject.person_index]).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(())
    }
    pub fn regions(&self, source: &str) -> AppResult<Vec<GuidedRegionV1>> {
        let s = self.source(source)?;
        let mut stmt = self.conn.prepare("SELECT r.region_json,s.person_feature_id,s.person_index FROM target_regions r LEFT JOIN region_subjects s ON s.region_id=r.id WHERE r.source_id=? AND r.index_id=? ORDER BY r.ordinal,r.id").map_err(err)?;
        let rows = stmt
            .query_map(params![source, s.index_generation], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<u32>>(2)?,
                ))
            })
            .map_err(err)?;
        rows.map(|row| {
            let (json, person, index) = row.map_err(err)?;
            let mut region: GuidedRegionV1 = serde_json::from_str(&json).map_err(err)?;
            region.subject =
                person
                    .zip(index)
                    .map(|(person_feature_id, person_index)| SubjectReferenceV1 {
                        person_feature_id,
                        person_index,
                    });
            Ok(region)
        })
        .collect()
    }
    pub fn remove_region(&self, id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM target_regions WHERE id=?", [id])
            .map_err(err)?;
        Ok(())
    }
    pub fn source(&self, id: &str) -> AppResult<SourceSummary> {
        self.conn.query_row("SELECT s.id,s.project_id,s.name,s.path,r.id,i.id,i.status,i.frame_count,r.metadata_json,i.error_json FROM sources s JOIN source_revisions r ON r.source_id=s.id JOIN frame_indexes i ON i.revision_id=r.id WHERE s.id=? ORDER BY i.created_at DESC,i.rowid DESC LIMIT 1",[id],|r|{
            let meta:Option<String>=r.get(8)?;let e:Option<String>=r.get(9)?;
            Ok(SourceSummary{id:r.get(0)?,project_id:r.get(1)?,name:r.get(2)?,path:r.get(3)?,revision_id:r.get(4)?,index_generation:r.get(5)?,status:r.get(6)?,frame_count:r.get::<_,i64>(7)?.to_string(),metadata:meta.and_then(|x|serde_json::from_str(&x).ok()),error:e.and_then(|x|serde_json::from_str(&x).ok())})
        }).map_err(err)
    }
    pub fn sources(&self, project: &str) -> AppResult<Vec<SourceSummary>> {
        let ids = self
            .conn
            .prepare("SELECT id FROM sources WHERE project_id=? ORDER BY rowid DESC")
            .map_err(err)?
            .query_map([project], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        ids.into_iter()
            .map(|id| {
                let mut source = self.source(&id)?;
                if source.status == "ready" {
                    source.error = self.validate_source(&source).err();
                }
                Ok(source)
            })
            .collect()
    }
    pub fn validate_source(&self, source: &SourceSummary) -> AppResult<()> {
        let (bytes, modified) = file_stamp(Path::new(&source.path))?;
        let (b, m): (i64, String) = self
            .conn
            .query_row(
                "SELECT bytes,modified_ns FROM source_revisions WHERE id=?",
                [&source.revision_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(err)?;
        if bytes != b as u64 || modified != m {
            return Err(AppError::new(
                "SOURCE_CHANGED",
                "The source changed. Re-import it before decoding or exporting.",
            ));
        }
        Ok(())
    }
    pub fn revision_hash(&self, source: &SourceSummary) -> AppResult<String> {
        self.conn
            .query_row(
                "SELECT sha256 FROM source_revisions WHERE id=?",
                [&source.revision_id],
                |r| r.get(0),
            )
            .map_err(err)
    }
    pub fn set_revision(&self, source: &SourceSummary, hash: &str, bytes: u64) -> AppResult<()> {
        self.validate_source(source)?;
        self.conn
            .execute(
                "UPDATE source_revisions SET sha256=?,bytes=? WHERE id=?",
                params![hash, bytes as i64, source.revision_id],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn set_metadata(&mut self, source: &SourceSummary, meta: &MediaMetadata) -> AppResult<()> {
        let tx = self.conn.transaction().map_err(err)?;
        let mut selected = String::new();
        for s in &meta.streams {
            let id = tx
                .query_row(
                    "SELECT id FROM video_streams WHERE revision_id=? AND stream_index=?",
                    params![source.revision_id, s.index],
                    |r| r.get::<_, String>(0),
                )
                .optional()
                .map_err(err)?
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            if s.index == meta.selected_stream {
                selected = id.clone();
            }
            tx.execute(
                "INSERT INTO video_streams VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(revision_id,stream_index) DO NOTHING",
                params![
                    id,
                    source.revision_id,
                    s.index,
                    s.codec,
                    s.width,
                    s.height,
                    s.time_base_numerator,
                    s.time_base_denominator,
                    s.rotation,
                    s.sample_aspect_ratio,
                    s.color_space,
                    s.color_transfer,
                    s.color_primaries
                ],
            )
            .map_err(err)?;
        }
        tx.execute(
            "UPDATE source_revisions SET metadata_json=? WHERE id=?",
            params![
                serde_json::to_string(meta).map_err(err)?,
                source.revision_id
            ],
        )
        .map_err(err)?;
        tx.execute(
            "UPDATE frame_indexes SET video_stream_id=?,status='indexing' WHERE id=?",
            params![selected, source.index_generation],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(())
    }
    pub fn append_frames(&mut self, index: &str, frames: &[IndexedFrame]) -> AppResult<()> {
        let tx = self.conn.transaction().map_err(err)?;
        for f in frames {
            tx.execute(
                "INSERT INTO frames VALUES(?,?,?,?,?,?,?)",
                params![
                    index,
                    ordinal(&f.ordinal)?,
                    f.pts_ticks,
                    f.time_seconds,
                    f.duration_seconds,
                    serde_json::to_value(f.timestamp_origin)
                        .map_err(err)?
                        .as_str()
                        .unwrap(),
                    f.key_frame
                ],
            )
            .map_err(err)?;
        }
        tx.execute("UPDATE frame_indexes SET frame_count=(SELECT COUNT(*) FROM frames WHERE index_id=?) WHERE id=?",params![index,index]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(())
    }
    pub fn finish_index(
        &self,
        index: &str,
        status: &str,
        error: Option<&AppError>,
    ) -> AppResult<()> {
        self.conn
            .execute(
                "UPDATE frame_indexes SET status=?,error_json=? WHERE id=?",
                params![
                    status,
                    error.map(serde_json::to_string).transpose().map_err(err)?,
                    index
                ],
            )
            .map_err(err)?;
        Ok(())
    }
    fn read_frame(r: &rusqlite::Row) -> rusqlite::Result<IndexedFrame> {
        let origin: String = r.get(4)?;
        Ok(IndexedFrame {
            ordinal: r.get::<_, i64>(0)?.to_string(),
            pts_ticks: r.get(1)?,
            time_seconds: r.get(2)?,
            duration_seconds: r.get(3)?,
            timestamp_origin: match origin.as_str() {
                "source" => TimestampOrigin::Source,
                "best_effort" => TimestampOrigin::BestEffort,
                _ => TimestampOrigin::Synthetic,
            },
            key_frame: r.get(5)?,
        })
    }
    pub fn frames(&self, index: &str, start: u64, limit: u32) -> AppResult<Vec<IndexedFrame>> {
        self.conn.prepare("SELECT ordinal,pts_ticks,time_seconds,duration_seconds,timestamp_origin,key_frame FROM frames WHERE index_id=? AND ordinal>=? ORDER BY ordinal LIMIT ?").map_err(err)?.query_map(params![index,start as i64,limit.min(256)],Self::read_frame).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)
    }
    pub fn frame(&self, index: &str, n: &str) -> AppResult<IndexedFrame> {
        self.frames(index, ordinal(n)? as u64, 1)?
            .into_iter()
            .find(|f| f.ordinal == n)
            .ok_or_else(|| {
                AppError::new("INDEX_PENDING", "The requested frame has not been indexed.")
            })
    }
    pub fn resolve(&self, index: &str, time: f64) -> AppResult<IndexedFrame> {
        if !time.is_finite() {
            return Err(AppError::new(
                "INVALID_TIMESTAMP",
                "Enter a finite timestamp.",
            ));
        }
        let columns = "ordinal,pts_ticks,time_seconds,duration_seconds,timestamp_origin,key_frame";
        let sql=format!("SELECT {columns} FROM frames WHERE index_id=? AND time_seconds<=? AND (time_seconds=? OR time_seconds+COALESCE(duration_seconds,0)>?) ORDER BY ordinal DESC LIMIT 1");
        if let Some(f) = self
            .conn
            .query_row(&sql, params![index, time, time, time], Self::read_frame)
            .optional()
            .map_err(err)?
        {
            return Ok(f);
        }
        let sql=format!("SELECT {columns} FROM frames WHERE index_id=? ORDER BY ABS(time_seconds-?),ordinal DESC LIMIT 1");
        self.conn
            .query_row(&sql, params![index, time], Self::read_frame)
            .optional()
            .map_err(err)?
            .ok_or_else(|| AppError::new("INDEX_PENDING", "No indexed frames are available."))
    }
    pub fn frame_ref(&self, source: &SourceSummary, f: &IndexedFrame) -> AppResult<FrameRefV1> {
        let s = source
            .metadata
            .as_ref()
            .and_then(|m| m.streams.iter().find(|s| s.index == m.selected_stream))
            .ok_or_else(|| AppError::new("INDEX_PENDING", "Metadata is not ready."))?;
        Ok(FrameRefV1 {
            source_revision_id: source.revision_id.clone(),
            stream_index: s.index,
            index_generation: source.index_generation.clone(),
            presentation_ordinal: f.ordinal.clone(),
            pts_ticks: f.pts_ticks.clone(),
            time_base_numerator: s.time_base_numerator,
            time_base_denominator: s.time_base_denominator,
            timestamp_origin: f.timestamp_origin,
        })
    }
    pub fn anchor(&self, index: &str, n: &str) -> AppResult<Option<SeekAnchor>> {
        let f=self.conn.query_row("SELECT ordinal,pts_ticks,time_seconds,duration_seconds,timestamp_origin,key_frame FROM frames f WHERE index_id=? AND ordinal<=? AND key_frame=1 AND pts_ticks IS NOT NULL AND (SELECT COUNT(*) FROM frames p WHERE p.index_id=f.index_id AND p.pts_ticks=f.pts_ticks)=1 ORDER BY ordinal DESC LIMIT 1",params![index,ordinal(n)?],Self::read_frame).optional().map_err(err)?;
        Ok(f.and_then(|f| {
            Some(SeekAnchor {
                ordinal: f.ordinal.parse().ok()?,
                pts: f.pts_ticks?.parse().ok()?,
                time_seconds: f.time_seconds,
            })
        }))
    }
    pub fn select(&self, source: &SourceSummary, n: &str, selected: bool) -> AppResult<()> {
        let n = ordinal(n)?;
        if selected {
            self.conn.execute("INSERT OR IGNORE INTO frame_selections(source_id,index_id,ordinal) VALUES(?,?,?)",params![source.id,source.index_generation,n]).map_err(err)?;
        } else {
            self.conn
                .execute(
                    "DELETE FROM frame_selections WHERE index_id=? AND ordinal=?",
                    params![source.index_generation, n],
                )
                .map_err(err)?;
        }
        Ok(())
    }
    pub fn selections(&self, project: &str) -> AppResult<Vec<Selection>> {
        let rows=self.conn.prepare("SELECT q.source_id,q.index_id,q.ordinal FROM frame_selections q JOIN sources s ON s.id=q.source_id WHERE s.project_id=? ORDER BY q.created_at,q.ordinal").map_err(err)?.query_map([project],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
        rows.into_iter()
            .map(|(id, index, n)| {
                let s = self.source(&id)?;
                let f = self.frame(&index, &n.to_string())?;
                Ok(Selection {
                    source_id: id,
                    frame_ref: self.frame_ref(&s, &f)?,
                    time_seconds: f.time_seconds,
                })
            })
            .collect()
    }
    pub fn relocate_source(
        &mut self,
        id: &str,
        path: &Path,
        verified_hash: &str,
    ) -> AppResult<SourceSummary> {
        let source = self.source(id)?;
        if source.status != "ready" {
            return Err(AppError::new(
                "SOURCE_BUSY",
                "Finish indexing before relocating this source.",
            ));
        }
        if self.revision_hash(&source)? != verified_hash {
            return Err(AppError::new("SOURCE_MISMATCH", "This file does not match the indexed source. Import it separately to create a new revision."));
        }
        let path = path.canonicalize().map_err(err)?;
        let (bytes, modified) = file_stamp(&path)?;
        let duplicate: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sources WHERE project_id=? AND path=? AND id<>?)",
                params![source.project_id, path.to_string_lossy(), id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if duplicate {
            return Err(AppError::new(
                "SOURCE_PATH_CONFLICT",
                "Another source in this project already uses this file.",
            ));
        }
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute(
            "UPDATE sources SET path=?,name=? WHERE id=?",
            params![
                path.to_string_lossy(),
                path.file_name().unwrap_or_default().to_string_lossy(),
                id
            ],
        )
        .map_err(err)?;
        tx.execute(
            "UPDATE source_revisions SET bytes=?,modified_ns=? WHERE id=?",
            params![bytes as i64, modified, source.revision_id],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        self.source(id)
    }
    pub fn save_export_plan(&mut self, plan: &ExportPlanV1) -> AppResult<()> {
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute(
            "INSERT INTO export_jobs(job_id,destination,options_json,project_id) VALUES(?,?,?,?)",
            params![
                plan.id,
                plan.destination,
                serde_json::to_string(&plan.options).map_err(err)?,
                plan.project_id
            ],
        )
        .map_err(err)?;
        for item in &plan.items {
            tx.execute("INSERT INTO export_items(job_id,index_id,ordinal,image_path,status,source_id,frame_json,source_hash) VALUES(?,?,?,?,'pending',?,?,?)", params![plan.id,item.frame.frame_ref.index_generation,ordinal(&item.frame.frame_ref.presentation_ordinal)?,item.image_path,item.source_id,serde_json::to_string(&item.frame).map_err(err)?,item.source_hash]).map_err(err)?;
        }
        tx.execute(
            "INSERT INTO export_attempts(job_id,plan_id) VALUES(?,?)",
            params![plan.id, plan.id],
        )
        .map_err(err)?;
        tx.execute("INSERT INTO export_attempt_items(job_id,index_id,ordinal) SELECT ?,index_id,ordinal FROM export_items WHERE job_id=?",params![plan.id,plan.id]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(())
    }
    pub fn export_plan(&self, id: &str) -> AppResult<ExportPlanV1> {
        let (project_id,destination,options): (String,String,String) = self.conn.query_row("SELECT project_id,destination,options_json FROM export_jobs WHERE job_id=? AND project_id IS NOT NULL",[id],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
        let rows = self.conn.prepare("SELECT source_id,frame_json,source_hash,image_path,checksum,sidecar_checksum,status,error_json FROM export_items WHERE job_id=? ORDER BY rowid").map_err(err)?.query_map([id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,String>(6)?,r.get::<_,Option<String>>(7)?))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
        let items = rows
            .into_iter()
            .map(
                |(
                    source_id,
                    frame,
                    source_hash,
                    image_path,
                    checksum,
                    sidecar_checksum,
                    status,
                    e,
                )| {
                    Ok(ExportItemV1 {
                        source_id,
                        frame: serde_json::from_str(&frame).map_err(err)?,
                        source_hash,
                        image_path,
                        checksum,
                        sidecar_checksum,
                        status,
                        error: e
                            .map(|s| serde_json::from_str(&s))
                            .transpose()
                            .map_err(err)?,
                    })
                },
            )
            .collect::<AppResult<Vec<_>>>()?;
        let active = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM export_attempts a JOIN jobs j ON j.id=a.job_id WHERE a.plan_id=? AND j.status IN('queued','running','cancelling'))",[id],|r|r.get(0)).map_err(err)?;
        Ok(ExportPlanV1 {
            id: id.into(),
            project_id,
            destination,
            options: serde_json::from_str(&options).map_err(err)?,
            items,
            active,
        })
    }
    pub fn export_plans(&self, project: &str) -> AppResult<Vec<ExportPlanV1>> {
        let ids = self.conn.prepare("SELECT job_id FROM export_jobs WHERE project_id=? AND dismissed=0 ORDER BY rowid DESC").map_err(err)?.query_map([project],|r|r.get::<_,String>(0)).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
        ids.into_iter().map(|id| self.export_plan(&id)).collect()
    }
    pub fn export_attempt(&self, job: &str, plan: &str) -> AppResult<()> {
        let tx = self.conn.unchecked_transaction().map_err(err)?;
        tx.execute(
            "INSERT INTO export_attempts(job_id,plan_id) VALUES(?,?)",
            params![job, plan],
        )
        .map_err(err)?;
        tx.execute("INSERT INTO export_attempt_items(job_id,index_id,ordinal) SELECT ?,index_id,ordinal FROM export_items WHERE job_id=?",params![job,plan]).map_err(err)?;
        tx.commit().map_err(err)
    }
    pub fn export_attempt_outcome(
        &self,
        job: &str,
        item: &ExportItemV1,
        outcome: &str,
    ) -> AppResult<()> {
        if !matches!(outcome, "completed" | "skipped" | "failed" | "cancelled") {
            return Err(AppError::new(
                "INVALID_EXPORT_OUTCOME",
                "Invalid export outcome.",
            ));
        }
        let changed = self.conn.execute("UPDATE export_attempt_items SET outcome=? WHERE job_id=? AND index_id=? AND ordinal=?",params![outcome,job,item.frame.frame_ref.index_generation,ordinal(&item.frame.frame_ref.presentation_ordinal)?]).map_err(err)?;
        if changed != 1 {
            return Err(AppError::new(
                "EXPORT_ATTEMPT_MISSING",
                "Export attempt item is missing.",
            ));
        }
        Ok(())
    }
    pub fn export_attempt_summary(&self, job: &str) -> AppResult<ExportAttemptSummaryV1> {
        let plan_id: String = self
            .conn
            .query_row(
                "SELECT plan_id FROM export_attempts WHERE job_id=?",
                [job],
                |r| r.get(0),
            )
            .map_err(err)?;
        // A pre-008 attempt cannot be reconstructed. Expose that explicitly.
        let (total, completed, skipped, failed, cancelled, pending): (u32,u32,u32,u32,u32,u32) = self.conn.query_row("SELECT COUNT(*),COALESCE(SUM(outcome='completed'),0),COALESCE(SUM(outcome='skipped'),0),COALESCE(SUM(outcome='failed'),0),COALESCE(SUM(outcome='cancelled'),0),COALESCE(SUM(outcome='pending'),0) FROM export_attempt_items WHERE job_id=?",[job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(err)?;
        Ok(ExportAttemptSummaryV1 {
            job_id: job.into(),
            plan_id,
            accounting_available: total > 0,
            completed,
            skipped,
            failed,
            cancelled,
            pending,
        })
    }
    pub fn update_export_item(&self, plan: &str, item: &ExportItemV1) -> AppResult<()> {
        self.conn.execute("UPDATE export_items SET checksum=?,sidecar_checksum=?,status=?,error_json=? WHERE job_id=? AND index_id=? AND ordinal=?",params![item.checksum,item.sidecar_checksum,item.status,item.error.as_ref().map(serde_json::to_string).transpose().map_err(err)?,plan,item.frame.frame_ref.index_generation,ordinal(&item.frame.frame_ref.presentation_ordinal)?]).map_err(err)?;
        Ok(())
    }
    pub fn dismiss_export(&self, id: &str) -> AppResult<()> {
        if self.export_plan(id)?.active {
            return Err(AppError::new(
                "EXPORT_BUSY",
                "Cancel this export before dismissing it.",
            ));
        }
        self.conn
            .execute("UPDATE export_jobs SET dismissed=1 WHERE job_id=?", [id])
            .map_err(err)?;
        Ok(())
    }
    pub fn begin_analysis(
        &self,
        id: &str,
        source: &SourceSummary,
        algorithm: &str,
    ) -> AppResult<()> {
        self.conn
            .execute(
                "INSERT INTO analysis_runs(id,source_id,index_id,algorithm) VALUES(?,?,?,?)",
                params![id, source.id, source.index_generation, algorithm],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn record_quality(
        &mut self,
        run: &str,
        index: &str,
        n: &str,
        algorithm: &str,
        metrics: &QualityMetricsV1,
    ) -> AppResult<()> {
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute("INSERT INTO frame_features(index_id,ordinal,algorithm,metrics_json) VALUES(?,?,?,?) ON CONFLICT(index_id,ordinal,algorithm) DO UPDATE SET metrics_json=excluded.metrics_json",params![index,ordinal(n)?,algorithm,serde_json::to_string(metrics).map_err(err)?]).map_err(err)?;
        tx.execute("INSERT OR IGNORE INTO analysis_samples(run_id,index_id,ordinal,algorithm) VALUES(?,?,?,?)",params![run,index,ordinal(n)?,algorithm]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(())
    }
    pub fn quality_frames(
        &self,
        source: &SourceSummary,
        algorithm: &str,
    ) -> AppResult<Vec<QualityFrameV1>> {
        let rows=self.conn.prepare("SELECT ordinal,metrics_json FROM frame_features WHERE index_id=? AND algorithm=? ORDER BY ordinal").map_err(err)?.query_map(params![source.index_generation,algorithm],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
        rows.into_iter()
            .map(|(n, json)| {
                let f = self.frame(&source.index_generation, &n.to_string())?;
                Ok(QualityFrameV1 {
                    frame: Selection {
                        source_id: source.id.clone(),
                        frame_ref: self.frame_ref(source, &f)?,
                        time_seconds: f.time_seconds,
                    },
                    metrics: serde_json::from_str(&json).map_err(err)?,
                })
            })
            .collect()
    }
    pub fn complete_analysis(&self, id: &str) -> AppResult<()> {
        self.conn
            .execute(
                "UPDATE analysis_runs SET coverage_complete=1 WHERE id=?",
                [id],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn latest_analysis(
        &self,
        source: &SourceSummary,
        algorithm: &str,
    ) -> AppResult<Option<(String, bool)>> {
        self.conn.query_row("SELECT id,coverage_complete FROM analysis_runs WHERE source_id=? AND index_id=? AND algorithm=? ORDER BY rowid DESC LIMIT 1",params![source.id,source.index_generation,algorithm],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)
    }
    pub fn save_recommendations(&self, run: &str, set: &RecommendationSetV1) -> AppResult<()> {
        self.conn.execute("INSERT INTO recommendation_sets(id,run_id,config_json,result_json) VALUES(?,?,?,?)",params![set.id,run,serde_json::to_string(&set.config).map_err(err)?,serde_json::to_string(set).map_err(err)?]).map_err(err)?;
        Ok(())
    }
    pub fn model_installations(&self) -> AppResult<Vec<ModelInstallationV1>> {
        let rows=self.conn.prepare("SELECT id,manifest_json,status,path,error_json FROM model_installations WHERE status<>'removed' ORDER BY rowid DESC").map_err(err)?.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
        rows.into_iter()
            .map(|(id, m, status, path, e)| {
                Ok(ModelInstallationV1 {
                    id,
                    manifest: serde_json::from_str(&m).map_err(err)?,
                    status,
                    path,
                    error: e
                        .map(|s| serde_json::from_str(&s))
                        .transpose()
                        .map_err(err)?,
                })
            })
            .collect()
    }
    pub fn save_model(&self, model: &ModelInstallationV1) -> AppResult<()> {
        self.conn.execute("INSERT INTO model_installations(id,manifest_json,status,path,error_json) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET manifest_json=excluded.manifest_json,status=excluded.status,path=excluded.path,error_json=excluded.error_json",params![model.id,serde_json::to_string(&model.manifest).map_err(err)?,model.status,model.path,model.error.as_ref().map(serde_json::to_string).transpose().map_err(err)?]).map_err(err)?;
        Ok(())
    }
    pub fn save_benchmark(&self, result: &ModelBenchmarkV1) -> AppResult<()> {
        self.conn
            .execute(
                "INSERT INTO model_benchmarks(id,model_id,result_json) VALUES(?,?,?)",
                params![
                    Uuid::new_v4().to_string(),
                    result.model_id,
                    serde_json::to_string(result).map_err(err)?
                ],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn benchmarks(&self) -> AppResult<Vec<ModelBenchmarkV1>> {
        let rows = self
            .conn
            .prepare("SELECT result_json FROM model_benchmarks ORDER BY rowid DESC LIMIT 100")
            .map_err(err)?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        rows.into_iter()
            .map(|s| serde_json::from_str(&s).map_err(err))
            .collect()
    }
    pub fn save_job(&self, event: &JobEvent, source: Option<&str>) -> AppResult<()> {
        self.conn.execute("INSERT INTO jobs(id,source_id,snapshot_json,status) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET snapshot_json=excluded.snapshot_json,status=excluded.status,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",params![event.job_id,source,serde_json::to_string(event).map_err(err)?,serde_json::to_value(&event.status).map_err(err)?.as_str().unwrap()]).map_err(err)?;
        Ok(())
    }
    pub fn list_jobs(&self) -> AppResult<Vec<JobEvent>> {
        let rows = self
            .conn
            .prepare("SELECT snapshot_json FROM jobs ORDER BY updated_at DESC LIMIT 100")
            .map_err(err)?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        rows.into_iter()
            .map(|s| serde_json::from_str(&s).map_err(err))
            .collect()
    }
    pub fn get_job(&self, id: &str) -> AppResult<JobEvent> {
        let s: String = self
            .conn
            .query_row("SELECT snapshot_json FROM jobs WHERE id=?", [id], |r| {
                r.get(0)
            })
            .map_err(err)?;
        serde_json::from_str(&s).map_err(err)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrations_and_foreign_keys() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("db.sqlite");
        let mut db = Database::open(&p).unwrap();
        let project = db.create_project("Look").unwrap();
        assert_eq!(db.projects().unwrap().len(), 1);
        let file = t.path().join("x.mkv");
        std::fs::write(&file, b"fixture").unwrap();
        let s = db.create_source(&project.id, &file).unwrap();
        db.append_frames(
            &s.index_generation,
            &[IndexedFrame {
                ordinal: "0".into(),
                pts_ticks: Some("9007199254740993".into()),
                time_seconds: 0.,
                duration_seconds: Some(0.04),
                timestamp_origin: TimestampOrigin::Source,
                key_frame: true,
            }],
        )
        .unwrap();
        assert_eq!(
            db.frame(&s.index_generation, "0")
                .unwrap()
                .pts_ticks
                .unwrap(),
            "9007199254740993"
        );
        assert!(db.select(&s, "1", true).is_err());
        drop(db);
        assert_eq!(Database::open(&p).unwrap().projects().unwrap().len(), 1);
    }
    #[test]
    fn export_attempt_upgrade_preserves_history_without_inventing_counts() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("db.sqlite");
        let db = Database::open(&path).unwrap();
        db.save_job(
            &JobEvent {
                schema_version: 1,
                job_id: "old".into(),
                sequence: 0,
                status: JobStatus::Succeeded,
                stage: "exporting".into(),
                completed_units: "0".into(),
                total_units: None,
                message_code: "EXPORT_COMPLETE".into(),
                result_ids: vec![],
                error: None,
            },
            None,
        )
        .unwrap();
        db.conn.execute_batch("INSERT INTO export_jobs(job_id,destination,options_json) VALUES('old','/old','{}'); INSERT INTO export_attempts(job_id,plan_id) VALUES('old','old'); DROP TABLE export_attempt_items; DELETE FROM schema_migrations WHERE version=8;").unwrap();
        drop(db);
        let db = Database::open(&path).unwrap();
        let summary = db.export_attempt_summary("old").unwrap();
        assert!(!summary.accounting_available);
        assert_eq!(
            (
                summary.completed,
                summary.skipped,
                summary.failed,
                summary.pending
            ),
            (0, 0, 0, 0)
        );
        assert!(path.with_extension("pre-8.backup").is_file());
        assert!(db.export_attempt_summary("absent").is_err());
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r
                    .get::<_, i64>(
                    0
                ))
                .unwrap(),
            0
        );
    }
    #[test]
    fn upgrade_backs_up_wal_data_and_rejects_modified_migration() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("db.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,checksum TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT '') STRICT;").unwrap();
        let sql = include_str!("../../../migrations/001_core.sql");
        conn.execute_batch(sql).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations(version,checksum) VALUES(1,?)",
            [format!("{:x}", Sha256::digest(sql.as_bytes()))],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO projects(id,name) VALUES('preserved','Original')",
            [],
        )
        .unwrap();
        let db = Database::open(&path).unwrap();
        assert_eq!(db.projects().unwrap()[0].id, "preserved");
        let backup = Connection::open(path.with_extension("pre-2.backup")).unwrap();
        assert_eq!(
            backup
                .query_row("SELECT name FROM projects", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "Original"
        );
        assert_eq!(
            backup
                .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        drop(db);
        conn.execute(
            "UPDATE schema_migrations SET checksum='tampered' WHERE version=1",
            [],
        )
        .unwrap();
        assert!(
            matches!(Database::open(&path),Err(AppError{code,..}) if code=="MIGRATION_MISMATCH")
        );
    }
    #[test]
    fn detection_cache_is_bound_to_frame_model_algorithm_and_color() {
        let d = tempfile::tempdir().unwrap();
        let mut db = Database::open(&d.path().join("db.sqlite")).unwrap();
        let p = db.create_project("Cache fixture").unwrap();
        let file = d.path().join("source.mkv");
        std::fs::write(&file, b"fixture").unwrap();
        let source = db.create_source(&p.id, &file).unwrap();
        let frame = IndexedFrame {
            ordinal: "0".into(),
            pts_ticks: Some("0".into()),
            time_seconds: 0.,
            duration_seconds: Some(0.04),
            timestamp_origin: TimestampOrigin::Source,
            key_frame: true,
        };
        db.append_frames(&source.index_generation, &[frame.clone()])
            .unwrap();
        let manifest: ModelManifestV1 =
            serde_json::from_str(include_str!("../../../models/manifests/yunet-2023mar.json"))
                .unwrap();
        let model = ModelInstallationV1 {
            id: "face-model".into(),
            manifest,
            path: "unused".into(),
            status: "ready".into(),
            error: None,
        };
        db.save_model(&model).unwrap();
        let feature = FaceFrameV1 {
            id: "feature".into(),
            model_id: model.id.clone(),
            frame: Selection {
                source_id: source.id.clone(),
                frame_ref: FrameRefV1 {
                    source_revision_id: source.revision_id.clone(),
                    stream_index: 0,
                    index_generation: source.index_generation.clone(),
                    presentation_ordinal: "0".into(),
                    pts_ticks: Some("0".into()),
                    time_base_numerator: 1,
                    time_base_denominator: 1000,
                    timestamp_origin: TimestampOrigin::Source,
                },
                time_seconds: 0.,
            },
            color_policy: "sdr-v1".into(),
            result: FaceResultV1 {
                width: 640,
                height: 480,
                algorithm: "face-v1".into(),
                model_hash: model.manifest.sha256.clone(),
                faces: vec![],
            },
        };
        db.save_face_feature(&feature).unwrap();
        // Emulate an existing migration-005 database with preserved detector output.
        db.conn.execute_batch("DROP TABLE preset_results; DROP TABLE track_observations; DROP TABLE subject_tracks; DROP TABLE export_attempt_items; DROP TABLE region_subjects; DROP TABLE subject_choices; DROP TABLE pose_features; DROP TABLE person_features; DELETE FROM schema_migrations WHERE version>=6;").unwrap();
        drop(db);
        let db = Database::open(&d.path().join("db.sqlite")).unwrap();
        assert!(d.path().join("db.pre-6.backup").exists());
        assert!(db
            .face_feature(&source, "0", &model, "face-v1", "sdr-v1")
            .unwrap()
            .is_some());
        assert!(db
            .face_feature(&source, "0", &model, "face-v2", "sdr-v1")
            .unwrap()
            .is_none());
        assert!(db
            .face_feature(&source, "0", &model, "face-v1", "sdr-v2")
            .unwrap()
            .is_none());
        let mut changed = model.clone();
        changed.manifest.sha256 = "a".repeat(64);
        assert!(db
            .face_feature(&source, "0", &changed, "face-v1", "sdr-v1")
            .unwrap()
            .is_none());
        let mut person_model = model.clone();
        person_model.id = "person-model".into();
        person_model.manifest = serde_json::from_str(include_str!(
            "../../../models/manifests/person_detection_mediapipe-2023mar.json"
        ))
        .unwrap();
        db.save_model(&person_model).unwrap();
        let parent = PersonFrameV1 {
            id: "person-feature".into(),
            model_id: person_model.id.clone(),
            frame: feature.frame.clone(),
            color_policy: feature.color_policy.clone(),
            result: PersonResultV1 {
                width: 640,
                height: 480,
                algorithm: "person-v1".into(),
                model_hash: person_model.manifest.sha256.clone(),
                persons: vec![PersonDetectionV1 {
                    detector_box: [0., 0., 640., 480.],
                    landmarks: [[320., 240.]; 4],
                    score: 0.9,
                }],
            },
        };
        db.save_person_feature(&parent).unwrap();
        let mut duplicate = parent.clone();
        duplicate.id = "racing-parent".into();
        assert_eq!(db.save_person_feature(&duplicate).unwrap().id, parent.id);
        assert!(db
            .person_feature(&source, "0", &person_model, "person-v2", "sdr-v1")
            .unwrap()
            .is_none());
        assert!(db
            .person_feature(&source, "0", &person_model, "person-v1", "sdr-v2")
            .unwrap()
            .is_none());
        let mut pose_model = model.clone();
        pose_model.id = "pose-model".into();
        pose_model.manifest = serde_json::from_str(include_str!(
            "../../../models/manifests/pose_estimation_mediapipe-2023mar.json"
        ))
        .unwrap();
        db.save_model(&pose_model).unwrap();
        let pose = PoseFrameV1 {
            id: "pose-feature".into(),
            model_id: pose_model.id.clone(),
            person_feature_id: parent.id.clone(),
            person_index: 0,
            frame: parent.frame.clone(),
            color_policy: parent.color_policy.clone(),
            result: PoseResultV1 {
                width: 640,
                height: 480,
                algorithm: "pose-v1".into(),
                model_hash: pose_model.manifest.sha256.clone(),
                pose: None,
                mask_path: None,
                mask_sha256: None,
            },
        };
        let mut invalid = pose.clone();
        invalid.person_index = 1;
        assert_eq!(
            db.save_pose_feature(&invalid).unwrap_err().code,
            "INVALID_SUBJECT"
        );
        invalid = pose.clone();
        invalid.result.width = 100;
        assert_eq!(
            db.save_pose_feature(&invalid).unwrap_err().code,
            "INVALID_SUBJECT"
        );
        invalid = pose.clone();
        invalid.frame.frame_ref.source_revision_id = "changed".into();
        assert_eq!(
            db.save_pose_feature(&invalid).unwrap_err().code,
            "SOURCE_CHANGED"
        );
        db.save_pose_feature(&pose).unwrap();
        db.conn.execute_batch("CREATE TEMP TRIGGER reject_removal BEFORE UPDATE OF status ON model_installations WHEN NEW.status='removed' BEGIN SELECT RAISE(ABORT,'injected removal failure'); END;").unwrap();
        assert_eq!(
            db.remove_model_transaction(&person_model.id)
                .unwrap_err()
                .code,
            "DATABASE_ERROR"
        );
        assert!(db.person_feature_by_id(&parent.id).is_ok());
        assert!(db
            .pose_feature(&parent.id, 0, &pose_model, "pose-v1", "sdr-v1")
            .unwrap()
            .is_some());
        assert!(db
            .model_installations()
            .unwrap()
            .iter()
            .any(|m| m.id == person_model.id && m.status == "ready"));
        db.conn
            .execute_batch("DROP TRIGGER reject_removal;")
            .unwrap();
        assert!(db
            .pose_feature(&parent.id, 0, &pose_model, "pose-v1", "sdr-v1")
            .unwrap()
            .is_some());
        assert!(db
            .pose_feature(&parent.id, 1, &pose_model, "pose-v1", "sdr-v1")
            .unwrap()
            .is_none());
        db.clear_model_features(&pose_model.id).unwrap();
        assert!(db
            .pose_feature(&parent.id, 0, &pose_model, "pose-v1", "sdr-v1")
            .unwrap()
            .is_none());
        assert!(db.person_feature_by_id(&parent.id).is_ok());
        db.save_pose_feature(&pose).unwrap();
        db.clear_model_features(&person_model.id).unwrap();
        assert!(db
            .pose_feature(&parent.id, 0, &pose_model, "pose-v1", "sdr-v1")
            .unwrap()
            .is_none());
        assert_eq!(
            db.person_feature_by_id(&parent.id).unwrap_err().code,
            "DETECTION_MISSING"
        );
        db.clear_model_features(&model.id).unwrap();
        assert!(db
            .face_feature(&source, "0", &model, "face-v1", "sdr-v1")
            .unwrap()
            .is_none());
        let mut removed = model.clone();
        removed.status = "removed".into();
        db.save_model(&removed).unwrap();
        assert_eq!(
            db.save_face_feature(&feature).unwrap_err().code,
            "STALE_DETECTION"
        );
    }
    #[test]
    fn source_changes_are_detected() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("v");
        std::fs::write(&f, b"first").unwrap();
        let mut db = Database::open(&t.path().join("db")).unwrap();
        let p = db.create_project("p").unwrap();
        let s = db.create_source(&p.id, &f).unwrap();
        std::fs::write(&f, b"changed").unwrap();
        assert_eq!(db.validate_source(&s).unwrap_err().code, "SOURCE_CHANGED");
    }
}

impl Database {
    pub fn subject_track(
        &mut self,
        source: &SourceSummary,
        key: &str,
    ) -> AppResult<Option<SubjectTrackV1>> {
        let value: Option<String> = self.conn.query_row("SELECT result_json FROM subject_tracks WHERE source_id=? AND index_id=? AND cache_key=?", params![source.id,source.index_generation,key], |r|r.get(0)).optional().map_err(err)?;
        let track: Option<SubjectTrackV1> = value
            .map(|v| serde_json::from_str(&v).map_err(err))
            .transpose()?;
        if let Some(t) = &track {
            let count: i64 = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM track_observations WHERE track_id=?",
                    [&t.id],
                    |r| r.get(0),
                )
                .map_err(err)?;
            if count != t.observations.len() as i64 {
                self.discard_subject_track(&t.id)?;
                return Ok(None);
            }
        }
        Ok(track)
    }
    pub fn detach_interrupted_track(&self, id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM subject_tracks WHERE id=?", [id])
            .map_err(err)?;
        Ok(())
    }
    pub fn discard_subject_track(&mut self, id: &str) -> AppResult<()> {
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute("DELETE FROM target_regions WHERE id IN (SELECT region_id FROM track_observations WHERE track_id=?)",[id]).map_err(err)?;
        tx.execute("DELETE FROM subject_tracks WHERE id=?", [id])
            .map_err(err)?;
        tx.commit().map_err(err)
    }
    pub fn save_subject_track(
        &mut self,
        track: &SubjectTrackV1,
        person: &str,
        face: Option<&str>,
    ) -> AppResult<()> {
        let tx = self.conn.transaction().map_err(err)?;
        for id in std::iter::once(person).chain(face) {
            let ready:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM model_installations WHERE id=? AND status='ready')",[id],|r|r.get(0)).map_err(err)?;
            if !ready {
                return Err(AppError::new(
                    "MODEL_MISSING",
                    "A tracking model was removed during execution.",
                ));
            }
        }
        tx.execute("INSERT INTO subject_tracks(id,source_id,index_id,person_model_id,face_model_id,cache_key,result_json) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET result_json=excluded.result_json",params![track.id,track.source_id,track.index_generation,person,face,track.cache_key,serde_json::to_string(track).map_err(err)?]).map_err(err)?;
        for o in &track.observations {
            tx.execute("INSERT OR IGNORE INTO track_observations(track_id,region_id,person_feature_id,person_index) VALUES(?,?,?,?)",params![track.id,o.region_id,o.subject.person_feature_id,o.subject.person_index]).map_err(err)?;
        }
        tx.commit().map_err(err)
    }
    pub fn save_preset_result(&self, result: &PresetResultV1) -> AppResult<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO preset_results(job_id,track_id,result_json) VALUES(?,?,?)",
                params![
                    result.job_id,
                    result.track.id,
                    serde_json::to_string(result).map_err(err)?
                ],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn preset_result(&self, job: &str) -> AppResult<Option<PresetResultV1>> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT result_json FROM preset_results WHERE job_id=?",
                [job],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        let result: Option<PresetResultV1> = value
            .map(|v| serde_json::from_str(&v).map_err(err))
            .transpose()?;
        if let Some(r) = &result {
            let count: i64 = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM track_observations WHERE track_id=?",
                    [&r.track.id],
                    |row| row.get(0),
                )
                .map_err(err)?;
            if count != r.track.observations.len() as i64 {
                return Ok(None);
            }
        }
        Ok(result)
    }
}
