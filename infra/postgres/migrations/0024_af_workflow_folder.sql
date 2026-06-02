-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

ALTER TABLE af_workflows ADD COLUMN IF NOT EXISTS folder TEXT;

CREATE INDEX IF NOT EXISTS idx_af_workflows_tenant_folder ON af_workflows (tenant_id, folder);
