-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_form_tokens (
    token        TEXT        PRIMARY KEY,
    tenant_id    TEXT        NOT NULL,
    workflow_id  TEXT        NOT NULL,
    title        TEXT        NOT NULL,
    description  TEXT,
    input_schema JSONB       NOT NULL DEFAULT '[]'::jsonb,
    created_at   BIGINT      NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_af_form_tokens_tenant_workflow
    ON af_form_tokens (tenant_id, workflow_id);
