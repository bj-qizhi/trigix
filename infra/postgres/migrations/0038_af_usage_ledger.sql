-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_usage_ledger (
    tenant_id TEXT NOT NULL,
    year_month TEXT NOT NULL,
    executions_used INTEGER NOT NULL DEFAULT 0,
    tokens_used BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, year_month)
);
CREATE INDEX IF NOT EXISTS idx_af_usage_ledger_tenant ON af_usage_ledger(tenant_id);
