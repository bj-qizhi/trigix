-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_webhook_deliveries (
    id              TEXT        NOT NULL PRIMARY KEY,
    webhook_token   TEXT        NOT NULL,
    tenant_id       TEXT        NOT NULL,
    delivered_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status_code     INT,
    success         BOOLEAN     NOT NULL DEFAULT FALSE,
    error_message   TEXT,
    execution_id    TEXT
);

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_token ON af_webhook_deliveries (webhook_token, delivered_at DESC);
