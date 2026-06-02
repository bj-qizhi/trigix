-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_test_cases (
    id              TEXT    PRIMARY KEY,
    tenant_id       TEXT    NOT NULL,
    workflow_id     TEXT    NOT NULL,
    name            TEXT    NOT NULL,
    input_json      TEXT    NOT NULL DEFAULT '{}',
    expected_output TEXT,
    created_at      BIGINT  NOT NULL DEFAULT 0,
    updated_at      BIGINT  NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_af_test_cases_tenant_workflow
    ON af_test_cases (tenant_id, workflow_id);
