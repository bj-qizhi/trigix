-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_desktop_pairing_sessions (
    id                              TEXT PRIMARY KEY,
    pairing_code                    TEXT NOT NULL UNIQUE,
    claim_secret_hash               TEXT NOT NULL,
    pending_credential_ciphertext   TEXT,
    credential_id                   TEXT NOT NULL UNIQUE,
    device_json                     JSONB NOT NULL,
    device_public_key               TEXT NOT NULL,
    tenant_id                       TEXT,
    approved_by                     TEXT,
    status                          TEXT NOT NULL DEFAULT 'pending'
                                    CHECK (status IN ('pending', 'approved', 'claimed', 'expired', 'failed')),
    attempt_count                   INTEGER NOT NULL DEFAULT 0
                                    CHECK (attempt_count >= 0 AND attempt_count <= 5),
    expires_at                      TIMESTAMPTZ NOT NULL,
    created_at                      TIMESTAMPTZ NOT NULL DEFAULT now(),
    approved_at                     TIMESTAMPTZ,
    claimed_at                      TIMESTAMPTZ,
    CHECK (char_length(pairing_code) = 8),
    CHECK (char_length(device_public_key) BETWEEN 32 AND 4096)
);

CREATE INDEX IF NOT EXISTS idx_af_desktop_pairing_sessions_expiry
    ON af_desktop_pairing_sessions (status, expires_at);
CREATE INDEX IF NOT EXISTS idx_af_desktop_pairing_sessions_tenant
    ON af_desktop_pairing_sessions (tenant_id, created_at DESC);

CREATE TABLE IF NOT EXISTS af_desktop_devices (
    id                  TEXT PRIMARY KEY,
    tenant_id           TEXT NOT NULL,
    display_name        TEXT NOT NULL,
    operating_system    TEXT NOT NULL,
    agent_version       TEXT NOT NULL,
    capabilities_json   JSONB NOT NULL DEFAULT '[]'::jsonb,
    public_key          TEXT NOT NULL UNIQUE,
    credential_id       TEXT NOT NULL UNIQUE,
    credential_hash     TEXT NOT NULL,
    state               TEXT NOT NULL DEFAULT 'paired'
                        CHECK (state IN ('paired', 'online', 'offline', 'suspended', 'revoked')),
    paired_by           TEXT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_af_desktop_devices_tenant
    ON af_desktop_devices (tenant_id, created_at DESC);

ALTER TABLE af_desktop_pairing_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE af_desktop_devices ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON af_desktop_pairing_sessions
    USING (tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''))
    WITH CHECK (tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''));

CREATE POLICY tenant_isolation ON af_desktop_devices
    USING (tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''))
    WITH CHECK (tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''));
