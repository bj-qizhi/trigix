-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- Affiliate payout (cashout) requests. An affiliate requests a payout of their
-- accrued balance to an address (e.g. a USDT wallet); an operator approves or
-- rejects it. Approval books the payout transaction in the double-entry ledger
-- (Dr affiliate_payable, Cr cash). The actual on-chain transfer is out-of-band.

CREATE TABLE IF NOT EXISTS af_payout_requests (
    id           TEXT PRIMARY KEY,
    tenant_id    TEXT NOT NULL,
    method       TEXT NOT NULL,
    address      TEXT NOT NULL,
    amount_cents BIGINT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'requested',
    note         TEXT,
    created_at   BIGINT NOT NULL,
    processed_at BIGINT
);

CREATE INDEX IF NOT EXISTS af_payout_requests_tenant_idx ON af_payout_requests (tenant_id);
CREATE INDEX IF NOT EXISTS af_payout_requests_status_idx ON af_payout_requests (status);
