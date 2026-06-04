# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Text embeddings with two backends:

- **OpenAI** (`text-embedding-3-small`) when ``OPENAI_API_KEY`` is set — the
  production path.
- A deterministic **local hashing embedding** otherwise, so RAG works offline
  and is fully testable without any external API or key.

Both produce vectors of ``EMBED_DIM`` dimensions (default 1536, matching
text-embedding-3-small) so a single pgvector column type fits either backend.
"""

from __future__ import annotations

import hashlib
import math
import os
import re

EMBED_DIM = int(os.environ.get("EMBED_DIM", "1536"))
_OPENAI_MODEL = os.environ.get("EMBED_MODEL", "text-embedding-3-small")

_TOKEN_RE = re.compile(r"[a-z0-9]+")


def using_openai() -> bool:
    return bool(os.environ.get("OPENAI_API_KEY"))


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


def _openai_embed(texts: list[str]) -> list[list[float]]:
    from openai import OpenAI  # imported lazily so the dep is optional

    client = OpenAI()
    resp = client.embeddings.create(model=_OPENAI_MODEL, input=texts)
    return [d.embedding for d in resp.data]


def embed(texts: list[str]) -> list[list[float]]:
    """Embed a batch of texts using the active backend."""
    if not texts:
        return []
    if using_openai():
        return _openai_embed(texts)
    return [local_embed_one(t) for t in texts]


def embed_one(text: str) -> list[float]:
    return embed([text])[0]
