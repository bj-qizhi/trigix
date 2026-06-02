-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_api_keys (
    id          TEXT PRIMARY KEY,
    tenant_id   TEXT NOT NULL,
    name        TEXT NOT NULL,
    prefix      TEXT NOT NULL,
    key_hash    TEXT NOT NULL UNIQUE,
    created_at  BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS af_api_keys_tenant ON af_api_keys (tenant_id);
