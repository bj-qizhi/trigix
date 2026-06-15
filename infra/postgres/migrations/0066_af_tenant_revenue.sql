-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- Per-currency converted revenue. The single af_tenant_quotas.revenue_cents
-- column summed amounts across currencies, which is wrong once a tenant pays in
-- more than one currency. Track revenue per (tenant, currency) instead, sourced
-- from invoice.paid (amount_paid + currency). Existing single-column data is
-- migrated as 'usd', then the old column is dropped.

CREATE TABLE IF NOT EXISTS af_tenant_revenue (
    tenant_id TEXT NOT NULL,
    currency  TEXT NOT NULL,
    cents     BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, currency)
);

INSERT INTO af_tenant_revenue (tenant_id, currency, cents)
    SELECT tenant_id, 'usd', revenue_cents
    FROM af_tenant_quotas
    WHERE revenue_cents <> 0
    ON CONFLICT (tenant_id, currency) DO NOTHING;

ALTER TABLE af_tenant_quotas DROP COLUMN IF EXISTS revenue_cents;
