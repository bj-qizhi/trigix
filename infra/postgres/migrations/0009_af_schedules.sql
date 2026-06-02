-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_schedules (
    workflow_version_id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    interval_secs BIGINT NOT NULL DEFAULT 0,
    cron_expression TEXT,
    next_run_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_af_schedules_tenant ON af_schedules(tenant_id);
