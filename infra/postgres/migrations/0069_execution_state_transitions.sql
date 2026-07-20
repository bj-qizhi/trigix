CREATE TABLE IF NOT EXISTS af_execution_state_transitions (
    id            TEXT PRIMARY KEY,
    tenant_id     TEXT NOT NULL,
    execution_id  TEXT NOT NULL REFERENCES af_executions(id) ON DELETE CASCADE,
    from_status   TEXT,
    to_status     TEXT NOT NULL,
    reason        TEXT,
    created_at    BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS af_execution_state_transitions_lookup_idx
    ON af_execution_state_transitions (tenant_id, execution_id, created_at);

ALTER TABLE af_execution_state_transitions ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON af_execution_state_transitions
    USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
    WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));
