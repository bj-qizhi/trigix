# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Text embeddings with two backends:

- **Remote, OpenAI-compatible** — any embeddings endpoint speaking the OpenAI
  dialect: OpenAI itself, or, via ``EMBED_BASE_URL``, a China-reachable / private
  one (DashScope ``text-embedding-v3``, a self-hosted bge-m3 behind vLLM / TEI /
  Xinference, SiliconFlow, Zhipu…). This is the production path and, like the
  agent and reranker, is what keeps RAG working in a private deployment where
  the Anthropic/OpenAI public APIs are unreachable.
- A deterministic **local hashing embedding** otherwise, so RAG works offline
  and is fully testable without any external API or key.

Both produce vectors of ``EMBED_DIM`` dimensions (default 1536, matching
text-embedding-3-small) so a single pgvector column type fits either backend.
Point ``EMBED_DIM`` / ``EMBED_MODEL`` at the self-hosted model's dimension.
"""

from __future__ import annotations

import hashlib
import math
import os
import re

# `… or default` (not get's default) so an env var set to "" — as a compose/Helm
# passthrough does for an unset value — falls back instead of crashing int("").
EMBED_DIM = int(os.environ.get("EMBED_DIM") or "1536")
_OPENAI_MODEL = os.environ.get("EMBED_MODEL") or "text-embedding-3-small"

# Latin/digit words OR individual CJK characters. Without the CJK class the
# local fallback embedding dropped every Chinese character, making offline
# Chinese RAG produce near-zero, indistinguishable vectors. Char-level CJK is a
# reasonable bag-of-words fallback (the remote path is still preferred).
_TOKEN_RE = re.compile(r"[a-z0-9]+|[一-鿿㐀-䶿]")


def _embed_base_url() -> str | None:
    """Self-hosted / China-reachable OpenAI-compatible embeddings endpoint."""
    return os.environ.get("EMBED_BASE_URL") or os.environ.get("OPENAI_BASE_URL")


def _embed_api_key() -> str | None:
    return (
        os.environ.get("EMBED_API_KEY")
        or os.environ.get("OPENAI_API_KEY")
        or os.environ.get("LLM_API_KEY")
    )


def using_remote() -> bool:
    """Whether a real (remote or self-hosted) embedding backend is configured.

    True when an API key is set, or when ``EMBED_BASE_URL`` points at a
    self-hosted endpoint (TEI / vLLM) that needs no key. Otherwise embeddings
    fall back to the deterministic local hashing model.
    """
    return bool(_embed_api_key() or _embed_base_url())


# Back-compat alias: this predicate used to be OpenAI-specific.
using_openai = using_remote


def backend_name() -> str:
    """A human-readable label for the active embedding backend."""
    if not using_remote():
        return "local"
    base = _embed_base_url()
    return f"remote:{_OPENAI_MODEL}" if base else "openai"


def _tokenize(text: str) -> list[str]:
    return _TOKEN_RE.findall(text.lower())


def local_embed_one(text: str, dim: int = EMBED_DIM) -> list[float]:
    """Deterministic bag-of-words hashing embedding, L2-normalized.

    Tokens are hashed into `dim` buckets with a signed contribution, capturing
    lexical overlap well enough for retrieval in tests and offline use.
    """
    vec = [0.0] * dim
    for tok in _tokenize(text):
        h = int(hashlib.md5(tok.encode()).hexdigest(), 16)
        vec[h % dim] += 1.0 if (h >> 8) & 1 else -1.0
    norm = math.sqrt(sum(v * v for v in vec))
    if norm == 0.0:
        return vec
    return [v / norm for v in vec]


def _remote_embed(texts: list[str]) -> list[list[float]]:
    from openai import OpenAI  # imported lazily so the dep is optional

    base_url = _embed_base_url()
    # The OpenAI SDK requires a key string even for keyless self-hosted
    # endpoints (TEI/vLLM ignore it), so supply a harmless placeholder.
    api_key = _embed_api_key() or "no-key"
    client = OpenAI(api_key=api_key, base_url=base_url) if base_url else OpenAI(api_key=api_key)
    resp = client.embeddings.create(model=_OPENAI_MODEL, input=texts)
    return [d.embedding for d in resp.data]


def embed(texts: list[str]) -> list[list[float]]:
    """Embed a batch of texts using the active backend."""
    if not texts:
        return []
    if using_remote():
        return _remote_embed(texts)
    return [local_embed_one(t) for t in texts]


def embed_one(text: str) -> list[float]:
    return embed([text])[0]
