-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_notification_prefs (
    user_id TEXT PRIMARY KEY,
    email_on_failure BOOLEAN NOT NULL DEFAULT FALSE,
    email_on_success BOOLEAN NOT NULL DEFAULT FALSE
);
