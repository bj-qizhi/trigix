-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_email_verifications (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    email TEXT NOT NULL,
    token TEXT NOT NULL UNIQUE,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    used_at BIGINT
);
CREATE INDEX IF NOT EXISTS idx_af_email_verifications_token ON af_email_verifications(token);
