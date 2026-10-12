CREATE TABLE project_activity(project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,last_opened_at TEXT NOT NULL) STRICT;
CREATE INDEX project_activity_last_opened ON project_activity(last_opened_at DESC);
