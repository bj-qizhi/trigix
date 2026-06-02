-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

ALTER TABLE af_executions ADD COLUMN IF NOT EXISTS node_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE af_executions ADD COLUMN IF NOT EXISTS completed_node_count INTEGER NOT NULL DEFAULT 0;
