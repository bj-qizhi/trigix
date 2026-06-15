-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- Double-entry general ledger for the affiliate program. Replaces the signed
-- single-entry af_affiliate_ledger (introduced one migration earlier, no
-- production data yet) with balanced GL postings.
--
-- Convention: debit-positive. Every business transaction (txn_id) posts two or
-- more rows whose amount_cents sum to zero, so the books always balance.
-- Accounts:
--   commission_expense (expense, debit-natural) — platform's referral cost
--   affiliate_payable  (liability, credit-natural, tenant_id = the affiliate)
--   cash               (asset, debit-natural)   — reduced on payout
-- An affiliate's payable balance is SUM(amount_cents) over its affiliate_payable
-- rows; the amount owed to them is the negation of that (credit balance).

DROP TABLE IF EXISTS af_affiliate_ledger;

CREATE TABLE IF NOT EXISTS af_ledger_postings (
    id             TEXT PRIMARY KEY,
    txn_id         TEXT NOT NULL,
    account        TEXT NOT NULL,
    tenant_id      TEXT,
    referee_tenant TEXT,
    amount_cents   BIGINT NOT NULL,
    kind           TEXT NOT NULL,
    source_ref     TEXT,
    created_at     BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS af_ledger_postings_account_tenant_idx
    ON af_ledger_postings (account, tenant_id);
CREATE INDEX IF NOT EXISTS af_ledger_postings_txn_idx ON af_ledger_postings (txn_id);
