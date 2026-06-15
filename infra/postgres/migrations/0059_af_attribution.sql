-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- First-touch acquisition attribution per tenant: which channel / campaign
-- brought the signup, captured at registration and read back at paid conversion
-- so revenue can be attributed to its acquisition source (and forwarded to
-- PostHog server-side). One row per tenant — first touch wins.

CREATE TABLE IF NOT EXISTS af_attribution (
    tenant_id    TEXT PRIMARY KEY,
    user_id      TEXT,
    utm_source   TEXT,
    utm_medium   TEXT,
    utm_campaign TEXT,
    utm_term     TEXT,
    utm_content  TEXT,
    referrer     TEXT,
    landing_page TEXT,
    distinct_id  TEXT,
    created_at   BIGINT NOT NULL
);
