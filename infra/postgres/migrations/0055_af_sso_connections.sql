-- Enterprise SSO via OpenID Connect. One row per tenant IdP connection.
CREATE TABLE IF NOT EXISTS af_sso_connections (
    id            TEXT    NOT NULL PRIMARY KEY,
    tenant_id     TEXT    NOT NULL,
    slug          TEXT    NOT NULL UNIQUE,
    provider      TEXT    NOT NULL,
    issuer        TEXT    NOT NULL,
    client_id     TEXT    NOT NULL,
    client_secret TEXT    NOT NULL,
    scopes        TEXT    NOT NULL DEFAULT 'openid email profile',
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    BIGINT  NOT NULL
);

CREATE INDEX IF NOT EXISTS af_sso_connections_tenant_idx ON af_sso_connections (tenant_id);
