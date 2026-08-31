-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

ALTER TABLE af_desktop_commands
    DROP CONSTRAINT IF EXISTS af_desktop_commands_status_check;

ALTER TABLE af_desktop_commands
    ADD CONSTRAINT af_desktop_commands_status_check
    CHECK (status IN ('waiting_approval', 'queued', 'delivered', 'acknowledged',
                      'succeeded', 'failed', 'rejected', 'cancelled', 'timed_out'));

CREATE INDEX IF NOT EXISTS idx_af_desktop_commands_tenant_approvals
    ON af_desktop_commands (tenant_id, created_at)
    WHERE status = 'waiting_approval';
