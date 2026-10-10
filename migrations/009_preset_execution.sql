CREATE TABLE subject_tracks (
 id TEXT PRIMARY KEY,
 source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
 index_id TEXT NOT NULL REFERENCES frame_indexes(id) ON DELETE CASCADE,
 person_model_id TEXT NOT NULL REFERENCES model_installations(id) ON DELETE CASCADE,
 face_model_id TEXT REFERENCES model_installations(id) ON DELETE CASCADE,
 cache_key TEXT NOT NULL,
 result_json TEXT NOT NULL,
 UNIQUE(index_id,cache_key)
) STRICT;
CREATE INDEX subject_tracks_source ON subject_tracks(source_id,index_id);
CREATE TABLE track_observations (
 track_id TEXT NOT NULL REFERENCES subject_tracks(id) ON DELETE CASCADE,
 region_id TEXT NOT NULL REFERENCES target_regions(id) ON DELETE CASCADE,
 person_feature_id TEXT NOT NULL REFERENCES person_features(id) ON DELETE CASCADE,
 person_index INTEGER NOT NULL CHECK(person_index>=0),
 PRIMARY KEY(track_id,region_id)
) STRICT;
CREATE TABLE preset_results (
 job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
 track_id TEXT NOT NULL REFERENCES subject_tracks(id) ON DELETE CASCADE,
 result_json TEXT NOT NULL
) STRICT;
