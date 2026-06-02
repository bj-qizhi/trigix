-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

ALTER TABLE af_webhooks ADD COLUMN IF NOT EXISTS max_calls_per_minute INTEGER;
