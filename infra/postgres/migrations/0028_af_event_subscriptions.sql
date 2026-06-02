-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_event_subscriptions (
    id          TEXT    PRIMARY KEY,
    tenant_id   TEXT    NOT NULL,
    url         TEXT    NOT NULL,
    events      TEXT[]  NOT NULL DEFAULT '{}',
    created_at  BIGINT  NOT NULL DEFAULT 0,
    description TEXT
);

CREATE INDEX IF NOT EXISTS idx_af_event_subs_tenant ON af_event_subscriptions (tenant_id);
