-- Migration: 002_tracks
-- Adds tracks table and extends jobs with mode/user_prompt/track_id

CREATE TABLE IF NOT EXISTS tracks (
    id             TEXT PRIMARY KEY,
    tenant_id      TEXT NOT NULL,
    project_id     TEXT,
    object_key     TEXT NOT NULL,
    display_name   TEXT NOT NULL,
    status         TEXT NOT NULL DEFAULT 'Uploaded',
    duration_sec   REAL,
    sample_rate    INTEGER,
    channels       INTEGER,
    sha256         TEXT,
    analysis       TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tracks_tenant   ON tracks(tenant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tracks_status   ON tracks(status);

-- Extend jobs with mode, user_prompt, track_id (nullable for backwards compat)
ALTER TABLE jobs ADD COLUMN mode TEXT;
ALTER TABLE jobs ADD COLUMN user_prompt TEXT;
ALTER TABLE jobs ADD COLUMN track_id TEXT;
