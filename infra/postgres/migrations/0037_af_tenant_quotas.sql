-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_tenant_quotas (
    tenant_id TEXT PRIMARY KEY,
    tier TEXT NOT NULL DEFAULT 'free',
    max_executions_per_month INTEGER NOT NULL DEFAULT 1000,
    max_concurrent_executions INTEGER NOT NULL DEFAULT 10,
    max_workflows INTEGER NOT NULL DEFAULT 50,
    updated_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
);
