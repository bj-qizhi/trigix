-- Copyright © 2026 北京祺智科技有限公司. All rights reserved.
-- https://www.qzso.com/ · managecode@gmail.com

CREATE TABLE IF NOT EXISTS af_voice_privacy_policies (
    tenant_id                       TEXT PRIMARY KEY,
    policy_version                  TEXT NOT NULL,
    retain_redacted_transcripts     BOOLEAN NOT NULL DEFAULT FALSE,
    transcript_retention_days       SMALLINT NOT NULL DEFAULT 0,
    metadata_retention_days         SMALLINT NOT NULL DEFAULT 7,
    updated_at                      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (length(policy_version) BETWEEN 1 AND 128),
    CHECK (metadata_retention_days BETWEEN 1 AND 30),
    CHECK (
        (retain_redacted_transcripts AND transcript_retention_days BETWEEN 1 AND 30)
        OR
        (NOT retain_redacted_transcripts AND transcript_retention_days = 0)
    )
);

CREATE TABLE IF NOT EXISTS af_voice_conversations (
    id                      TEXT PRIMARY KEY,
    tenant_id               TEXT NOT NULL,
    session_id              TEXT NOT NULL,
    sequence                BIGINT NOT NULL CHECK (sequence BETWEEN 1 AND 4294967295),
    occurred_at_unix_ms     BIGINT NOT NULL CHECK (occurred_at_unix_ms > 0),
    accepted_at_unix_ms     BIGINT NOT NULL CHECK (accepted_at_unix_ms > 0),
    policy_version          TEXT NOT NULL,
    transcript_retained     BOOLEAN NOT NULL DEFAULT FALSE,
    redacted_transcript     TEXT,
    request_fingerprint     TEXT NOT NULL,
    expires_at              TIMESTAMPTZ NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, session_id, sequence),
    CHECK (length(session_id) BETWEEN 1 AND 128),
    CHECK (length(policy_version) BETWEEN 1 AND 128),
    CHECK (length(request_fingerprint) BETWEEN 64 AND 512),
    CHECK (
        (transcript_retained AND redacted_transcript IS NOT NULL
         AND octet_length(redacted_transcript) BETWEEN 1 AND 4096)
        OR
        (NOT transcript_retained AND redacted_transcript IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_af_voice_conversations_expiry
    ON af_voice_conversations (expires_at);
CREATE INDEX IF NOT EXISTS idx_af_voice_conversations_tenant_session
    ON af_voice_conversations (tenant_id, session_id, sequence);

ALTER TABLE af_voice_privacy_policies ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON af_voice_privacy_policies
    USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
    WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));

ALTER TABLE af_voice_conversations ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON af_voice_conversations
    USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
    WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));

-- The runtime application role is intentionally subject to tenant RLS and
-- cannot issue a global DELETE. This narrowly scoped definer function lets the
-- retention worker remove only records whose policy deadline has passed.
CREATE OR REPLACE FUNCTION af_sweep_expired_voice_conversations(cutoff TIMESTAMPTZ)
RETURNS BIGINT
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
    removed BIGINT;
BEGIN
    DELETE FROM public.af_voice_conversations WHERE expires_at <= cutoff;
    GET DIAGNOSTICS removed = ROW_COUNT;
    RETURN removed;
END;
$$;

REVOKE ALL ON FUNCTION af_sweep_expired_voice_conversations(TIMESTAMPTZ) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION af_sweep_expired_voice_conversations(TIMESTAMPTZ) TO PUBLIC;
