-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- Stripe webhook idempotency. Stripe retries delivery on any non-2xx or slow
-- response, so the same event can arrive multiple times. Each processed event id
-- is recorded here; the webhook handler claims an id with INSERT ... ON CONFLICT
-- DO NOTHING RETURNING and skips the event when no row is returned, so an upgrade
-- or clawback is never applied twice.

CREATE TABLE IF NOT EXISTS af_stripe_events (
    event_id    TEXT PRIMARY KEY,
    received_at BIGINT NOT NULL
);
