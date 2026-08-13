-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_desktop_commands (
    id                  TEXT PRIMARY KEY,
    tenant_id           TEXT NOT NULL,
    project_id          TEXT NOT NULL,
    workflow_id         TEXT NOT NULL,
    execution_id        TEXT NOT NULL REFERENCES af_executions(id),
    device_id           TEXT NOT NULL REFERENCES af_desktop_devices(id),
    requested_by        TEXT NOT NULL,
    protocol_version    TEXT NOT NULL DEFAULT 'desktop.v1',
    lease_id            TEXT NOT NULL UNIQUE,
    command_json        JSONB NOT NULL,
    status              TEXT NOT NULL DEFAULT 'queued'
                        CHECK (status IN ('queued', 'delivered', 'acknowledged',
                              'succeeded', 'failed', 'rejected', 'cancelled', 'timed_out')),
    result_json         JSONB,
    expires_at          TIMESTAMPTZ NOT NULL,
    delivered_at        TIMESTAMPTZ,
    acknowledged_at     TIMESTAMPTZ,
    completed_at        TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_af_desktop_commands_device_pending
    ON af_desktop_commands (device_id, created_at)
    WHERE status IN ('queued', 'delivered');
CREATE INDEX IF NOT EXISTS idx_af_desktop_commands_execution
    ON af_desktop_commands (tenant_id, execution_id, created_at DESC);

ALTER TABLE af_desktop_commands ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON af_desktop_commands
    USING (tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''))
    WITH CHECK (tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''));
