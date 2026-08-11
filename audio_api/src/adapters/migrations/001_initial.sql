-- Mixlirous SQLite schema
-- Migration: 001_initial

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    config TEXT NOT NULL,
    blocks TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'Queued',
    worker_id TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_heartbeat TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_status_created ON jobs(status, created_at);
CREATE INDEX IF NOT EXISTS idx_jobs_tenant ON jobs(tenant_id);
CREATE INDEX IF NOT EXISTS idx_jobs_worker ON jobs(worker_id);

CREATE TABLE IF NOT EXISTS audit_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL,
    action TEXT NOT NULL,
    new_status TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    FOREIGN KEY (job_id) REFERENCES jobs(id)
);

CREATE INDEX IF NOT EXISTS idx_audit_job ON audit_records(job_id);

CREATE TABLE IF NOT EXISTS consent_records (
    tenant_id TEXT PRIMARY KEY,
    assisted_mode_accepted_at TEXT NOT NULL,
    provider_at_accept TEXT NOT NULL
);