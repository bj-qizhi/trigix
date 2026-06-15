-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- Converted revenue per tenant, accumulated from Stripe checkout.session.completed
-- (amount_total, in the currency's minor unit). Joined against af_attribution by
-- tenant to attribute revenue to its first-touch acquisition channel, closing the
-- acquisition → revenue ROI loop. Recurring invoices are not summed here.

ALTER TABLE af_tenant_quotas
    ADD COLUMN IF NOT EXISTS revenue_cents BIGINT NOT NULL DEFAULT 0;
