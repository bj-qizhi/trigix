-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_desktop_evidence (
    id                      TEXT PRIMARY KEY,
    tenant_id               TEXT NOT NULL,
    project_id              TEXT NOT NULL,
    execution_id            TEXT NOT NULL REFERENCES af_executions(id) ON DELETE CASCADE,
    command_id              TEXT NOT NULL REFERENCES af_desktop_commands(id) ON DELETE CASCADE,
    device_id               TEXT NOT NULL REFERENCES af_desktop_devices(id) ON DELETE CASCADE,
    kind                    TEXT NOT NULL CHECK (kind IN ('adapter_audit', 'screenshot')),
    selector_strategy       TEXT NOT NULL CHECK (selector_strategy IN
                                ('automation_id', 'control_type_and_name', 'name_and_sibling',
                                 'window_automation_id', 'application_identity', 'not_applicable')),
    application_id          TEXT NOT NULL,
    started_at_unix_ms      BIGINT NOT NULL CHECK (started_at_unix_ms >= 0),
    completed_at_unix_ms    BIGINT NOT NULL CHECK (completed_at_unix_ms >= started_at_unix_ms),
    outcome                 TEXT NOT NULL CHECK (outcome IN
                                ('succeeded', 'failed', 'rejected', 'cancelled', 'timed_out')),
    policy_version          TEXT NOT NULL,
    redacted_regions        INTEGER NOT NULL DEFAULT 0 CHECK (redacted_regions >= 0),
    content_type            TEXT,
    content_sha256          TEXT,
    byte_size               BIGINT NOT NULL DEFAULT 0 CHECK (byte_size BETWEEN 0 AND 1048576),
    payload_ciphertext      TEXT,
    expires_at              TIMESTAMPTZ NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (kind = 'adapter_audit' AND content_type IS NULL AND content_sha256 IS NULL
         AND byte_size = 0 AND payload_ciphertext IS NULL)
        OR
        (kind = 'screenshot' AND content_type IN ('image/png', 'image/webp')
         AND length(content_sha256) = 64 AND byte_size > 0
         AND payload_ciphertext LIKE 'enc:v1:%')
    )
);

CREATE INDEX IF NOT EXISTS idx_af_desktop_evidence_execution
    ON af_desktop_evidence (tenant_id, execution_id, created_at);
CREATE INDEX IF NOT EXISTS idx_af_desktop_evidence_expiry
    ON af_desktop_evidence (expires_at);

ALTER TABLE af_desktop_evidence ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON af_desktop_evidence
    USING (tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''))
    WITH CHECK (tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''));
