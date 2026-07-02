# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""RAG end-to-end generation — retrieve then answer, with a fake store + LLM so
no pgvector DB or API key is required."""

from types import SimpleNamespace

import pytest
from fastapi.testclient import TestClient

import app.main as main


class _Hit:
    def __init__(self, doc_id, chunk_index, content, score):
        self.doc_id = doc_id
        self.chunk_index = chunk_index
        self.content = content
        self.score = score


class _FakeStore:
    async def query(self, tenant, kb, query, top_k, *, mode, min_score, rerank):
        return [_Hit("doc-1", 0, "Paris is the capital of France.", 0.91)]


class _FakeLLM:
    def __init__(self, text):
        self._text = text

    async def respond(self, system, messages, tools, on_text_delta=None):
        assert tools == []  # generation is a plain, tool-free completion
        if on_text_delta is not None:
            on_text_delta(self._text)
        return SimpleNamespace(
            text=self._text,
            tool_calls=[],
            assistant_content=[],
            usage={"input_tokens": 5, "output_tokens": 3},
        )


@pytest.fixture
def client(monkeypatch):
    async def fake_get_store():
        return _FakeStore()

    monkeypatch.setattr("app.rag.router.get_store", fake_get_store)
    monkeypatch.setattr(
        main, "_build_llm", lambda config, model, max_tokens: _FakeLLM("The capital is Paris.")
    )
    return TestClient(main.app)


def test_rag_generate_returns_answer_and_sources(client):
    r = client.post(
        "/v1/rag/generate",
        json={"tenant_id": "t1", "kb": "kb", "query": "capital of France?"},
    )
    assert r.status_code == 200
    body = r.json()
    assert body["answer"] == "The capital is Paris."
    assert len(body["sources"]) == 1
    assert body["sources"][0]["doc_id"] == "doc-1"
    assert body["usage"]["output_tokens"] == 3


def test_rag_prompt_includes_context_and_question():
    hits = [SimpleNamespace(content="alpha"), SimpleNamespace(content="beta")]
    p = main._rag_prompt("What?", hits)
    assert "alpha" in p and "beta" in p and "What?" in p


def test_rag_prompt_handles_no_hits():
    assert "no relevant context" in main._rag_prompt("Q?", []).lower()


def test_rag_generate_stream_emits_delta_then_done(client):
    with client.stream(
        "POST", "/v1/rag/generate/stream", json={"tenant_id": "t1", "kb": "kb", "query": "q"}
    ) as r:
        assert r.status_code == 200
        text = "".join(r.iter_text())
    assert '"delta"' in text
    assert '"done"' in text
    assert "Paris" in text
