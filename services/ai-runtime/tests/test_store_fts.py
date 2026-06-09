# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Unit tests for the hybrid full-text config selection (no database)."""

from app.rag.store import _pick_fts_config


def test_override_wins_when_available():
    assert _pick_fts_config({"jiebacfg", "simple", "myzh"}, "myzh") == "myzh"


def test_prefers_cjk_config_without_override():
    assert _pick_fts_config({"jiebacfg", "english", "simple"}, None) == "jiebacfg"
    assert _pick_fts_config({"zhparsercfg", "simple"}, None) == "zhparsercfg"


def test_defaults_to_simple_when_no_cjk():
    assert _pick_fts_config({"english", "simple"}, None) == "simple"


def test_missing_override_falls_through():
    # Operator asked for jiebacfg but it isn't installed → fall back to simple.
    assert _pick_fts_config({"simple"}, "jiebacfg") == "simple"


def test_rejects_non_identifier_override():
    assert _pick_fts_config({"simple"}, "x'; DROP TABLE af_kb_chunks; --") == "simple"
