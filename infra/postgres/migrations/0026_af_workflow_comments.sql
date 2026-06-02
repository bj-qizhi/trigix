-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_workflow_comments (
    id          TEXT    PRIMARY KEY,
    tenant_id   TEXT    NOT NULL,
    workflow_id TEXT    NOT NULL,
    author      TEXT    NOT NULL,
    body        TEXT    NOT NULL,
    created_at  BIGINT  NOT NULL DEFAULT 0,
    edited_at   BIGINT
);

CREATE INDEX IF NOT EXISTS idx_af_comments_tenant_workflow
    ON af_workflow_comments (tenant_id, workflow_id, created_at);
