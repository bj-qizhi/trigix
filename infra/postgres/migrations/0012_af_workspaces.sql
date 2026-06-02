-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_workspaces (
    id          TEXT PRIMARY KEY,
    tenant_id   TEXT NOT NULL,
    name        TEXT NOT NULL,
    description TEXT,
    created_at  BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS af_workspaces_tenant ON af_workspaces (tenant_id);

CREATE TABLE IF NOT EXISTS af_projects (
    id           TEXT PRIMARY KEY,
    tenant_id    TEXT NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES af_workspaces(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    description  TEXT,
    created_at   BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS af_projects_workspace ON af_projects (tenant_id, workspace_id);
