-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_notifications (
    id          TEXT PRIMARY KEY,
    tenant_id   TEXT NOT NULL,
    user_id     TEXT,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    level       TEXT NOT NULL DEFAULT 'info',
    read        BOOLEAN NOT NULL DEFAULT false,
    created_at  BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_af_notifications_tenant ON af_notifications(tenant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_af_notifications_unread ON af_notifications(tenant_id, read) WHERE read = false;
