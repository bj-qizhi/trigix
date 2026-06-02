-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_invitations (
    id          TEXT PRIMARY KEY,
    email       TEXT NOT NULL,
    token       TEXT NOT NULL UNIQUE,
    role        TEXT NOT NULL DEFAULT 'editor',
    tenant_id   TEXT NOT NULL,
    created_at  BIGINT NOT NULL,
    expires_at  BIGINT NOT NULL,
    used_at     BIGINT
);
CREATE INDEX IF NOT EXISTS idx_af_invitations_tenant ON af_invitations (tenant_id);
CREATE INDEX IF NOT EXISTS idx_af_invitations_token  ON af_invitations (token);
