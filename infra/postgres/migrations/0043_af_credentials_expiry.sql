-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

ALTER TABLE af_credentials
    ADD COLUMN IF NOT EXISTS description TEXT,
    ADD COLUMN IF NOT EXISTS expires_at  BIGINT,
    ADD COLUMN IF NOT EXISTS created_at  BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS updated_at  BIGINT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_af_credentials_expiry ON af_credentials(tenant_id, expires_at) WHERE expires_at IS NOT NULL;
