ALTER TABLE af_desktop_evidence
    ADD COLUMN IF NOT EXISTS selector_fallback_depth SMALLINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS selector_fallback_used BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE af_desktop_evidence
    DROP CONSTRAINT IF EXISTS af_desktop_evidence_selector_strategy_check;

ALTER TABLE af_desktop_evidence
    ADD CONSTRAINT af_desktop_evidence_selector_strategy_check
    CHECK (selector_strategy IN (
        'automation_id', 'control_type_and_name', 'name_and_sibling',
        'window_automation_id', 'executable_and_title', 'executable', 'title',
        'control_type', 'application_identity', 'not_applicable'
    ));

ALTER TABLE af_desktop_evidence
    DROP CONSTRAINT IF EXISTS af_desktop_evidence_selector_fallback_check;

ALTER TABLE af_desktop_evidence
    ADD CONSTRAINT af_desktop_evidence_selector_fallback_check
    CHECK (
        selector_fallback_depth BETWEEN 0 AND 4
        AND selector_fallback_used = (selector_fallback_depth > 0)
        AND NOT (selector_strategy = 'not_applicable' AND selector_fallback_used)
    );
