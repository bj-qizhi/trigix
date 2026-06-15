-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- Affiliate / referral program.
--   af_affiliate_codes : each tenant's shareable referral code.
--   af_referrals       : first-touch referrer for a referred tenant (one per referee).
--   af_affiliate_ledger: signed commission/clawback/payout entries; a referrer's
--                        balance is SUM(amount_cents). Commission accrues from a
--                        referred tenant's paid invoices; refunds claw it back;
--                        operator payouts debit the balance.

CREATE TABLE IF NOT EXISTS af_affiliate_codes (
    tenant_id  TEXT PRIMARY KEY,
    code       TEXT UNIQUE NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS af_referrals (
    referee_tenant  TEXT PRIMARY KEY,
    referrer_tenant TEXT NOT NULL,
    code            TEXT,
    created_at      BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS af_referrals_referrer_idx ON af_referrals (referrer_tenant);

CREATE TABLE IF NOT EXISTS af_affiliate_ledger (
    id              TEXT PRIMARY KEY,
    referrer_tenant TEXT NOT NULL,
    referee_tenant  TEXT,
    amount_cents    BIGINT NOT NULL,
    kind            TEXT NOT NULL,
    source_ref      TEXT,
    created_at      BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS af_affiliate_ledger_referrer_idx ON af_affiliate_ledger (referrer_tenant);
