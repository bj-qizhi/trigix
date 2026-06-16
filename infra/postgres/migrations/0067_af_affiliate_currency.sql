-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- Make the affiliate ledger currency-aware, aligning it with per-currency
-- revenue. Commissions accrue in the paid invoice's currency; an affiliate's
-- owed balance and payouts are tracked per currency. Existing rows default to
-- 'usd'.

ALTER TABLE af_ledger_postings ADD COLUMN IF NOT EXISTS currency TEXT NOT NULL DEFAULT 'usd';
ALTER TABLE af_payout_requests ADD COLUMN IF NOT EXISTS currency TEXT NOT NULL DEFAULT 'usd';

CREATE INDEX IF NOT EXISTS af_ledger_postings_acct_tenant_cur_idx
    ON af_ledger_postings (account, tenant_id, currency);
