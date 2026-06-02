-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

-- Add Stripe customer/subscription tracking to tenant quota table
ALTER TABLE af_tenant_quotas
  ADD COLUMN IF NOT EXISTS stripe_customer_id TEXT,
  ADD COLUMN IF NOT EXISTS stripe_subscription_id TEXT;

CREATE INDEX IF NOT EXISTS idx_af_tenant_quotas_stripe_customer
  ON af_tenant_quotas (stripe_customer_id)
  WHERE stripe_customer_id IS NOT NULL;
