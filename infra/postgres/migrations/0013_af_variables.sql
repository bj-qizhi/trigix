-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_workflow_variables (
    tenant_id   TEXT NOT NULL,
    workflow_id TEXT NOT NULL,
    key         TEXT NOT NULL,
    value_json  JSONB NOT NULL,
    updated_at  BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, workflow_id, key)
);
