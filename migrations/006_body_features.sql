CREATE TABLE person_features (
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
CREATE INDEX person_features_source ON person_features(source_id,index_id,ordinal);
CREATE TABLE pose_features (
    id TEXT PRIMARY KEY,
    person_feature_id TEXT NOT NULL REFERENCES person_features(id) ON DELETE CASCADE,
    person_index INTEGER NOT NULL CHECK(person_index >= 0),
    model_id TEXT NOT NULL REFERENCES model_installations(id) ON DELETE CASCADE,
    model_hash TEXT NOT NULL,
    algorithm TEXT NOT NULL,
    color_policy TEXT NOT NULL,
    result_json TEXT NOT NULL,
    UNIQUE(person_feature_id,person_index,model_id,model_hash,algorithm,color_policy)
) STRICT;
CREATE INDEX pose_features_model ON pose_features(model_id);
