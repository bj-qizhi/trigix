-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

-- Slice 446: node execution waterfall — persist per-node start offset and duration
ALTER TABLE af_node_executions
    ADD COLUMN IF NOT EXISTS duration_ms   BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS started_at_ms BIGINT NOT NULL DEFAULT 0;
