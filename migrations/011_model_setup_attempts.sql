CREATE TABLE model_setup_attempts(
    job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    downloaded INTEGER NOT NULL DEFAULT 0 CHECK(downloaded IN(0,1))
) STRICT;
CREATE INDEX model_setup_attempts_fingerprint ON model_setup_attempts(fingerprint);
