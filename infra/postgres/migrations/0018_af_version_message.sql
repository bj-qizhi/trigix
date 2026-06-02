-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- Contact: managecode@gmail.com

-- Add optional save message to workflow versions
ALTER TABLE af_workflow_versions ADD COLUMN IF NOT EXISTS message TEXT;
