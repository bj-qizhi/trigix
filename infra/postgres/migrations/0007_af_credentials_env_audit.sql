-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- Slice 67: Postgres persistence for credentials, env vars, and audit log

CREATE TABLE IF NOT EXISTS af_credentials (
    id          TEXT PRIMARY KEY,
    tenant_id   TEXT NOT NULL,
    name        TEXT NOT NULL,
    secret      TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name)
);

CREATE TABLE IF NOT EXISTS af_env_vars (
    tenant_id   TEXT NOT NULL,
    env_set     TEXT NOT NULL DEFAULT 'default',
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, env_set, key)
);

CREATE TABLE IF NOT EXISTS af_audit_log (
    id            TEXT PRIMARY KEY,
    tenant_id     TEXT NOT NULL,
    action        TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id   TEXT NOT NULL,
    detail_json   JSONB,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS af_credentials_tenant_idx ON af_credentials (tenant_id);
CREATE INDEX IF NOT EXISTS af_env_vars_tenant_set_idx ON af_env_vars (tenant_id, env_set);
CREATE INDEX IF NOT EXISTS af_audit_log_tenant_idx ON af_audit_log (tenant_id, created_at DESC);
