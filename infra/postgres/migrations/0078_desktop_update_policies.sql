-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_desktop_update_policies (
    tenant_id                         TEXT PRIMARY KEY,
    revision                          BIGINT NOT NULL CHECK (revision > 0),
    mode                              TEXT NOT NULL CHECK (mode IN ('disabled', 'manual', 'automatic')),
    channel                           TEXT NOT NULL CHECK (channel IN ('internal', 'closed_beta', 'stable')),
    required_version                  TEXT NOT NULL CHECK (length(required_version) BETWEEN 1 AND 64),
    pinned_version                    TEXT CHECK (length(pinned_version) BETWEEN 1 AND 64),
    maintenance_start_minute_utc      SMALLINT,
    maintenance_duration_minutes      SMALLINT,
    allow_offline_import              BOOLEAN NOT NULL DEFAULT FALSE,
    allow_emergency_rollback          BOOLEAN NOT NULL DEFAULT FALSE,
    updated_by                        TEXT NOT NULL CHECK (length(updated_by) BETWEEN 1 AND 128),
    updated_at                        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (maintenance_start_minute_utc IS NULL AND maintenance_duration_minutes IS NULL)
        OR
        (maintenance_start_minute_utc BETWEEN 0 AND 1439
         AND maintenance_duration_minutes BETWEEN 1 AND 1440)
    ),
    CHECK (mode <> 'automatic' OR maintenance_start_minute_utc IS NOT NULL),
    CHECK (pinned_version IS NULL OR pinned_version = required_version)
);

ALTER TABLE af_desktop_update_policies ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON af_desktop_update_policies
    USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
    WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));
