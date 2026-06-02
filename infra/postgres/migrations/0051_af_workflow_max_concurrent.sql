-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

ALTER TABLE af_workflows ADD COLUMN IF NOT EXISTS max_concurrent_runs INTEGER;
