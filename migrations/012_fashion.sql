CREATE TABLE photo_assets(revision_id TEXT PRIMARY KEY REFERENCES source_revisions(id),source_id TEXT NOT NULL REFERENCES sources(id),width INTEGER NOT NULL CHECK(width>0),height INTEGER NOT NULL CHECK(height>0),image_path TEXT NOT NULL,image_sha256 TEXT NOT NULL) STRICT;
CREATE TABLE fashion_items(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),media_key TEXT NOT NULL,revision INTEGER NOT NULL,category TEXT NOT NULL,corrected INTEGER NOT NULL,deleted INTEGER NOT NULL,payload TEXT NOT NULL) STRICT;
CREATE INDEX fashion_items_media ON fashion_items(project_id,media_key,deleted);
CREATE TABLE fashion_item_history(item_id TEXT NOT NULL REFERENCES fashion_items(id),revision INTEGER NOT NULL,payload TEXT NOT NULL,undoable INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(item_id,revision)) STRICT;
CREATE TABLE garment_groups(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),revision INTEGER NOT NULL,name TEXT NOT NULL,payload TEXT NOT NULL) STRICT;
CREATE TABLE garment_group_items(group_id TEXT NOT NULL REFERENCES garment_groups(id) ON DELETE CASCADE,item_id TEXT NOT NULL UNIQUE REFERENCES fashion_items(id),PRIMARY KEY(group_id,item_id)) STRICT;
CREATE TABLE fashion_products(id TEXT PRIMARY KEY,catalog INTEGER NOT NULL,name TEXT NOT NULL,brand TEXT,category TEXT NOT NULL,payload TEXT NOT NULL) STRICT;
CREATE VIRTUAL TABLE fashion_product_search USING fts5(id UNINDEXED,name,brand,category);
CREATE TABLE retailer_listings(id TEXT PRIMARY KEY,product_id TEXT NOT NULL REFERENCES fashion_products(id),provider TEXT NOT NULL,url TEXT NOT NULL,payload TEXT NOT NULL) STRICT;
CREATE TABLE product_evidence(id TEXT PRIMARY KEY,url TEXT NOT NULL,independence_group TEXT NOT NULL,payload TEXT NOT NULL) STRICT;
CREATE TABLE product_candidates(id TEXT PRIMARY KEY,group_id TEXT NOT NULL REFERENCES garment_groups(id),group_revision INTEGER NOT NULL,product_id TEXT NOT NULL REFERENCES fashion_products(id),stale INTEGER NOT NULL DEFAULT 0,payload TEXT NOT NULL) STRICT;
CREATE INDEX product_candidates_group ON product_candidates(group_id,group_revision,stale);
CREATE TABLE candidate_evidence(candidate_id TEXT NOT NULL REFERENCES product_candidates(id) ON DELETE CASCADE,evidence_id TEXT NOT NULL REFERENCES product_evidence(id),PRIMARY KEY(candidate_id,evidence_id)) STRICT;
CREATE TABLE fashion_research_runs(id TEXT PRIMARY KEY REFERENCES jobs(id),project_id TEXT NOT NULL REFERENCES projects(id),group_id TEXT REFERENCES garment_groups(id),group_revision INTEGER,payload TEXT NOT NULL) STRICT;
CREATE TABLE saved_looks(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),revision INTEGER NOT NULL,name TEXT NOT NULL,payload TEXT NOT NULL) STRICT;
CREATE TABLE look_entries(look_id TEXT NOT NULL REFERENCES saved_looks(id) ON DELETE CASCADE,group_id TEXT NOT NULL REFERENCES garment_groups(id),candidate_id TEXT REFERENCES product_candidates(id),PRIMARY KEY(look_id,group_id)) STRICT;
CREATE TABLE fashion_exports(id TEXT PRIMARY KEY REFERENCES jobs(id),project_id TEXT NOT NULL REFERENCES projects(id),look_id TEXT NOT NULL,payload TEXT NOT NULL) STRICT;
CREATE TABLE task_packs(id TEXT PRIMARY KEY,manifest TEXT NOT NULL,directory TEXT NOT NULL,status TEXT NOT NULL) STRICT;

CREATE TABLE preset_target_features(region_id TEXT PRIMARY KEY REFERENCES target_regions(id) ON DELETE CASCADE,payload TEXT NOT NULL) STRICT;
CREATE TABLE task_track_dependencies(track_id TEXT NOT NULL REFERENCES subject_tracks(id) ON DELETE CASCADE,pack_id TEXT NOT NULL REFERENCES task_packs(id),PRIMARY KEY(track_id,pack_id)) STRICT;

CREATE TABLE photo_heads(source_id TEXT PRIMARY KEY REFERENCES sources(id),revision_id TEXT NOT NULL REFERENCES photo_assets(revision_id)) STRICT;
CREATE TABLE fashion_provider_usage(provider TEXT NOT NULL,scope TEXT NOT NULL,day INTEGER NOT NULL,requests INTEGER NOT NULL,PRIMARY KEY(provider,scope,day)) STRICT;

CREATE TABLE pose_track_dependencies(track_id TEXT NOT NULL REFERENCES subject_tracks(id) ON DELETE CASCADE,model_id TEXT NOT NULL REFERENCES model_installations(id),PRIMARY KEY(track_id,model_id)) STRICT;

CREATE TABLE fashion_task_results(cache_key TEXT PRIMARY KEY,pack_id TEXT NOT NULL REFERENCES task_packs(id),payload TEXT NOT NULL) STRICT;
