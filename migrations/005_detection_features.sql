CREATE TABLE detection_features (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    index_id TEXT NOT NULL REFERENCES frame_indexes(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    model_id TEXT NOT NULL REFERENCES model_installations(id) ON DELETE CASCADE,
    model_hash TEXT NOT NULL,
    algorithm TEXT NOT NULL,
    color_policy TEXT NOT NULL,
    result_json TEXT NOT NULL,
    FOREIGN KEY(index_id,ordinal) REFERENCES frames(index_id,ordinal) ON DELETE CASCADE,
    UNIQUE(index_id,ordinal,model_id,model_hash,algorithm,color_policy)
) STRICT;
CREATE INDEX detection_features_source ON detection_features(source_id,index_id,ordinal);
