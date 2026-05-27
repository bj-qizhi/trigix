INSERT INTO tenants (id, name)
VALUES ('00000000-0000-4000-8000-000000000001', 'Dev Tenant')
ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;

INSERT INTO users (id, email, display_name)
VALUES ('00000000-0000-4000-8000-000000000002', 'dev@example.local', 'Dev User')
ON CONFLICT (id) DO UPDATE
SET email = EXCLUDED.email,
    display_name = EXCLUDED.display_name;

INSERT INTO workspaces (id, tenant_id, name)
VALUES (
  '00000000-0000-4000-8000-000000000003',
  '00000000-0000-4000-8000-000000000001',
  'Dev Workspace'
)
ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;

INSERT INTO projects (id, tenant_id, workspace_id, name, created_by)
VALUES (
  '00000000-0000-4000-8000-000000000004',
  '00000000-0000-4000-8000-000000000001',
  '00000000-0000-4000-8000-000000000003',
  'Dev Project',
  '00000000-0000-4000-8000-000000000002'
)
ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;

INSERT INTO workflows (
  id,
  tenant_id,
  workspace_id,
  project_id,
  name,
  status,
  created_by
)
VALUES (
  '00000000-0000-4000-8000-000000000005',
  '00000000-0000-4000-8000-000000000001',
  '00000000-0000-4000-8000-000000000003',
  '00000000-0000-4000-8000-000000000004',
  'Dev Lead Workflow',
  'published',
  '00000000-0000-4000-8000-000000000002'
)
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    status = EXCLUDED.status;

INSERT INTO workflow_versions (
  id,
  tenant_id,
  workflow_id,
  version,
  graph_json,
  status,
  created_by,
  published_at
)
VALUES (
  '00000000-0000-4000-8000-000000000006',
  '00000000-0000-4000-8000-000000000001',
  '00000000-0000-4000-8000-000000000005',
  1,
  '{
    "workflow_version_id": "00000000-0000-4000-8000-000000000006",
    "nodes": [
      {"id": "trigger", "type": "trigger"},
      {"id": "agent", "type": "agent"}
    ],
    "edges": [
      {"source": "trigger", "target": "agent"}
    ]
  }'::jsonb,
  'published',
  '00000000-0000-4000-8000-000000000002',
  now()
)
ON CONFLICT (workflow_id, version) DO UPDATE
SET graph_json = EXCLUDED.graph_json,
    status = EXCLUDED.status,
    published_at = EXCLUDED.published_at;

UPDATE workflows
SET latest_version_id = '00000000-0000-4000-8000-000000000006'
WHERE id = '00000000-0000-4000-8000-000000000005';
