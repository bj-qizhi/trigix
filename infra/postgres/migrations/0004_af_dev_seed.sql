-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

-- Dev seed for af_* tables — mirrors the in-memory dev seed so the
-- frontend's hardcoded TENANT_ID / WORKSPACE_ID / PROJECT_ID constants work.

INSERT INTO af_workflows (id, tenant_id, workspace_id, project_id, name, status, latest_version_id)
VALUES (
  'workflow-1',
  'tenant-1',
  'workspace-1',
  'project-1',
  'Dev Lead Workflow',
  'published',
  'version-1'
)
ON CONFLICT (id) DO UPDATE
SET name             = EXCLUDED.name,
    status           = EXCLUDED.status,
    latest_version_id = EXCLUDED.latest_version_id,
    updated_at       = now();

INSERT INTO af_workflow_versions (id, tenant_id, workflow_id, version, graph_json, status, published_at)
VALUES (
  'version-1',
  'tenant-1',
  'workflow-1',
  1,
  '{
    "workflow_version_id": "version-1",
    "nodes": [
      {"id": "trigger", "type": "trigger"},
      {"id": "agent",   "type": "agent"}
    ],
    "edges": [
      {"source": "trigger", "target": "agent"}
    ]
  }'::jsonb,
  'published',
  now()
)
ON CONFLICT (workflow_id, version) DO UPDATE
SET graph_json  = EXCLUDED.graph_json,
    status      = EXCLUDED.status,
    published_at = EXCLUDED.published_at;
