-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

ALTER TABLE af_desktop_devices
    DROP CONSTRAINT IF EXISTS af_desktop_devices_state_check;

ALTER TABLE af_desktop_devices
    ADD CONSTRAINT af_desktop_devices_state_check
    CHECK (state IN (
        'paired', 'online', 'offline', 'busy', 'awaiting_approval', 'degraded',
        'suspended', 'revoked'
    )),
    ADD COLUMN IF NOT EXISTS connection_session_id TEXT,
    ADD COLUMN IF NOT EXISTS connected_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS active_execution_id TEXT,
    ADD COLUMN IF NOT EXISTS health_detail TEXT;

CREATE INDEX IF NOT EXISTS idx_af_desktop_devices_connection_session
    ON af_desktop_devices (connection_session_id)
    WHERE connection_session_id IS NOT NULL;
