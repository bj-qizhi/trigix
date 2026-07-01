# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

import math

import pytest

import app.rag.embeddings as embeddings
from app.rag.embeddings import (
    EMBED_DIM,
    backend_name,
    embed,
    embed_one,
    local_embed_one,
    using_remote,
)


def test_dimension_and_normalization():
    v = local_embed_one("the quick brown fox")
    assert len(v) == EMBED_DIM
    norm = math.sqrt(sum(x * x for x in v))
    assert abs(norm - 1.0) < 1e-9


def test_deterministic():
    assert local_embed_one("hello world") == local_embed_one("hello world")


def test_similar_texts_are_closer_than_unrelated():
    def cos(a, b):
        return sum(x * y for x, y in zip(a, b))

    base = embed_one("invoices and billing for the finance team")
    similar = embed_one("billing invoices finance department")
    unrelated = embed_one("photos of cats playing in the garden")
    assert cos(base, similar) > cos(base, unrelated)


def test_empty_input():
    assert embed([]) == []


def test_empty_string_is_zero_vector():
    v = local_embed_one("")
    assert len(v) == EMBED_DIM
    assert all(x == 0.0 for x in v)


def test_chinese_is_not_a_zero_vector():
    # Before the CJK tokenizer fix the local fallback dropped every Chinese
    # character, producing an all-zero (useless) vector for Chinese documents.
    v = local_embed_one("机器学习模型")
    assert any(x != 0.0 for x in v)


def test_chinese_similar_closer_than_unrelated():
    def cos(a, b):
        return sum(x * y for x, y in zip(a, b))

    base = local_embed_one("发票 财务 账单")
    similar = local_embed_one("账单 发票")
    unrelated = local_embed_one("猫 花园 玩耍")
    assert cos(base, similar) > cos(base, unrelated)


def _clear_embed_env(monkeypatch):
    for var in (
        "EMBED_BASE_URL",
        "EMBED_API_KEY",
        "OPENAI_BASE_URL",
        "OPENAI_API_KEY",
        "LLM_API_KEY",
    ):
        monkeypatch.delenv(var, raising=False)


def test_local_backend_when_nothing_configured(monkeypatch):
    _clear_embed_env(monkeypatch)
    assert using_remote() is False
    assert backend_name() == "local"


def test_api_key_selects_remote(monkeypatch):
    _clear_embed_env(monkeypatch)
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test")
    assert using_remote() is True
    assert backend_name() == "openai"


def test_self_hosted_base_url_needs_no_key(monkeypatch):
    _clear_embed_env(monkeypatch)
    # A keyless self-hosted endpoint (vLLM/TEI) is enabled by base_url alone.
    monkeypatch.setenv("EMBED_BASE_URL", "http://localhost:9000/v1")
    assert using_remote() is True
    assert backend_name().startswith("remote:")


def test_remote_embed_passes_base_url_and_placeholder_key(monkeypatch):
    """The remote path must construct the client with the configured base_url
    and tolerate a missing key (keyless self-hosted servers)."""
    openai = pytest.importorskip("openai")  # optional dep; skip if absent
    _clear_embed_env(monkeypatch)
    monkeypatch.setenv("EMBED_BASE_URL", "http://localhost:9000/v1")
    # EMBED_MODEL is bound at import (set before process start in production), so
    # patch the resolved value rather than the env var.
    monkeypatch.setattr(embeddings, "_OPENAI_MODEL", "bge-m3")

    captured = {}

    class FakeEmbeddings:
        def create(self, model, input):
            captured["model"] = model
            data = [type("D", (), {"embedding": [0.1, 0.2, 0.3]})() for _ in input]
            return type("R", (), {"data": data})()

    class FakeOpenAI:
        def __init__(self, **kwargs):
            captured["kwargs"] = kwargs
            self.embeddings = FakeEmbeddings()

    monkeypatch.setattr(openai, "OpenAI", FakeOpenAI)

    out = embeddings.embed(["hello", "world"])
    assert len(out) == 2 and out[0] == [0.1, 0.2, 0.3]
    assert captured["model"] == "bge-m3"
    assert captured["kwargs"]["base_url"] == "http://localhost:9000/v1"
    assert captured["kwargs"]["api_key"] == "no-key"
