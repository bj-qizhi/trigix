-- Registry of community / third-party custom nodes served over HTTP (node SDK).
CREATE TABLE IF NOT EXISTS af_custom_nodes (
    id            TEXT   NOT NULL PRIMARY KEY,
    tenant_id     TEXT   NOT NULL,
    slug          TEXT   NOT NULL,
    label         TEXT   NOT NULL,
    description   TEXT   NOT NULL DEFAULT '',
    endpoint      TEXT   NOT NULL,
    config_schema JSONB  NOT NULL DEFAULT '{}'::jsonb,
    created_at    BIGINT NOT NULL,
    UNIQUE (tenant_id, slug)
);

CREATE INDEX IF NOT EXISTS af_custom_nodes_tenant_idx ON af_custom_nodes (tenant_id);
