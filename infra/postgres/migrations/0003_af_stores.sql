-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

-- Platform stores: TEXT PKs for compatibility with the current string-ID model.
-- The UUID-keyed tables (workflows, workflow_versions, etc.) from migration 0001
-- are retained for future auth/multi-tenant infrastructure.

CREATE TABLE IF NOT EXISTS af_workflows (
  id          TEXT PRIMARY KEY,
  tenant_id   TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  project_id  TEXT NOT NULL,
  name        TEXT NOT NULL,
  status      TEXT NOT NULL DEFAULT 'draft',
  latest_version_id TEXT,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS af_workflow_versions (
  id          TEXT PRIMARY KEY,
  tenant_id   TEXT NOT NULL,
  workflow_id TEXT NOT NULL REFERENCES af_workflows(id),
  version     INT  NOT NULL,
  graph_json  JSONB NOT NULL,
  status      TEXT NOT NULL DEFAULT 'draft',
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  published_at TIMESTAMPTZ,
  UNIQUE(workflow_id, version)
);

CREATE TABLE IF NOT EXISTS af_executions (
  id                   TEXT PRIMARY KEY,
  tenant_id            TEXT NOT NULL,
  workflow_id          TEXT NOT NULL,
  workflow_version_id  TEXT NOT NULL,
  status               TEXT NOT NULL,
  input_json           JSONB NOT NULL DEFAULT '{}',
  graph_json           JSONB NOT NULL DEFAULT '{}',
  started_at           BIGINT NOT NULL,
  finished_at          BIGINT
);

CREATE TABLE IF NOT EXISTS af_node_executions (
  id           TEXT PRIMARY KEY,
  tenant_id    TEXT NOT NULL,
  execution_id TEXT NOT NULL REFERENCES af_executions(id),
  node_id      TEXT NOT NULL,
  node_type    TEXT NOT NULL,
  status       TEXT NOT NULL,
  output_json  JSONB,
  error        TEXT
);

CREATE INDEX IF NOT EXISTS af_workflows_tenant_idx          ON af_workflows(tenant_id);
CREATE INDEX IF NOT EXISTS af_workflow_versions_workflow_idx ON af_workflow_versions(workflow_id);
CREATE INDEX IF NOT EXISTS af_executions_tenant_idx         ON af_executions(tenant_id, started_at DESC);
CREATE INDEX IF NOT EXISTS af_node_executions_exec_idx      ON af_node_executions(execution_id);
