-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_orgs (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    owner_id    TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS af_org_members (
    org_id      TEXT NOT NULL REFERENCES af_orgs(id) ON DELETE CASCADE,
    user_id     TEXT NOT NULL,
    role        TEXT NOT NULL DEFAULT 'editor',
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_af_org_members_user ON af_org_members (user_id);
