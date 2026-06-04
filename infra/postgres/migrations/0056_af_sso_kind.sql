-- Support non-OIDC SSO providers (Feishu / DingTalk / WeChat Work) alongside
-- standard OIDC. `kind` selects the flow; `agent_id` is needed by WeChat Work.
ALTER TABLE af_sso_connections
    ADD COLUMN IF NOT EXISTS kind     TEXT NOT NULL DEFAULT 'oidc',
    ADD COLUMN IF NOT EXISTS agent_id TEXT;
