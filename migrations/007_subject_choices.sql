CREATE TABLE subject_choices (
    index_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    person_feature_id TEXT NOT NULL REFERENCES person_features(id) ON DELETE CASCADE,
    person_index INTEGER NOT NULL CHECK(person_index >= 0),
    PRIMARY KEY(index_id,ordinal),
    FOREIGN KEY(index_id,ordinal) REFERENCES frames(index_id,ordinal) ON DELETE CASCADE
) STRICT;
CREATE INDEX subject_choices_person ON subject_choices(person_feature_id);
CREATE TABLE region_subjects (
    region_id TEXT PRIMARY KEY REFERENCES target_regions(id) ON DELETE CASCADE,
    person_feature_id TEXT NOT NULL REFERENCES person_features(id) ON DELETE CASCADE,
    person_index INTEGER NOT NULL CHECK(person_index >= 0)
) STRICT;
CREATE INDEX region_subjects_person ON region_subjects(person_feature_id);
