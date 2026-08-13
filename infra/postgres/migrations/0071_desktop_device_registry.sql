-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

ALTER TABLE af_desktop_devices
    ADD COLUMN IF NOT EXISTS last_seen_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS pending_rotation_id TEXT,
    ADD COLUMN IF NOT EXISTS pending_credential_ciphertext TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_af_desktop_devices_pending_rotation
    ON af_desktop_devices (pending_rotation_id)
    WHERE pending_rotation_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_af_desktop_devices_tenant_state
    ON af_desktop_devices (tenant_id, state, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_af_desktop_devices_last_seen
    ON af_desktop_devices (tenant_id, last_seen_at)
    WHERE state IN ('online', 'offline');
