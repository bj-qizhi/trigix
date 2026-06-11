-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

-- One node may only have a single execution record per execution. Inline
-- execution writes node results twice — once from the live progress callback
-- and once when the run completes — and those two paths could race and leave
-- duplicate rows. Enforce a single row per (tenant_id, execution_id, node_id)
-- so the writers can upsert idempotently and the race can no longer duplicate.

-- Collapse any pre-existing duplicates, keeping one row per node.
DELETE FROM af_node_executions a
USING af_node_executions b
WHERE a.ctid < b.ctid
  AND a.tenant_id = b.tenant_id
  AND a.execution_id = b.execution_id
  AND a.node_id = b.node_id;

CREATE UNIQUE INDEX IF NOT EXISTS af_node_executions_exec_node_uq
  ON af_node_executions (tenant_id, execution_id, node_id);
