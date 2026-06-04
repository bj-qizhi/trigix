# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""pgvector-backed knowledge store: ingest document chunks and retrieve the
nearest neighbours for a query embedding (cosine distance)."""

from __future__ import annotations

import time
import uuid
from dataclasses import dataclass

import asyncpg

from .embeddings import EMBED_DIM, embed, embed_one


def _vec_literal(vec: list[float]) -> str:
    """Format a float vector as pgvector's text representation: ``[a,b,c]``."""
    return "[" + ",".join(repr(float(x)) for x in vec) + "]"


@dataclass
class RetrievedChunk:
    doc_id: str
    chunk_index: int
    content: str
    score: float  # cosine similarity in [-1, 1]; higher is more similar


class RagStore:
    """Owns its schema (extension + table + index) so the runtime can be tested
    standalone against any pgvector database."""

    def __init__(self, pool: asyncpg.Pool) -> None:
        self._pool = pool

    @classmethod
    async def connect(cls, dsn: str) -> "RagStore":
        pool = await asyncpg.create_pool(dsn, min_size=1, max_size=5)
        store = cls(pool)
        await store.ensure_schema()
        return store

    async def close(self) -> None:
        await self._pool.close()

    async def ensure_schema(self) -> None:
        async with self._pool.acquire() as conn:
            await conn.execute("CREATE EXTENSION IF NOT EXISTS vector")
            await conn.execute(
                f"""
                CREATE TABLE IF NOT EXISTS af_kb_chunks (
                    id          TEXT PRIMARY KEY,
                    tenant_id   TEXT NOT NULL,
                    kb          TEXT NOT NULL,
                    doc_id      TEXT NOT NULL,
                    chunk_index INT  NOT NULL,
                    content     TEXT NOT NULL,
                    embedding   VECTOR({EMBED_DIM}) NOT NULL,
                    created_at  BIGINT NOT NULL
                )
                """
            )
            await conn.execute(
                "CREATE INDEX IF NOT EXISTS af_kb_chunks_kb_idx "
                "ON af_kb_chunks (tenant_id, kb)"
            )

    async def ingest(
        self, tenant_id: str, kb: str, doc_id: str, chunks: list[str]
    ) -> int:
        """Replace all chunks for `doc_id` with freshly embedded `chunks`."""
        if not chunks:
            await self.delete_document(tenant_id, kb, doc_id)
            return 0
        vectors = embed(chunks)
        now = int(time.time())
        rows = [
            (
                str(uuid.uuid4()),
                tenant_id,
                kb,
                doc_id,
                i,
                content,
                _vec_literal(vec),
                now,
            )
            for i, (content, vec) in enumerate(zip(chunks, vectors))
        ]
        async with self._pool.acquire() as conn:
            async with conn.transaction():
                await conn.execute(
                    "DELETE FROM af_kb_chunks WHERE tenant_id=$1 AND kb=$2 AND doc_id=$3",
                    tenant_id,
                    kb,
                    doc_id,
                )
                await conn.executemany(
                    "INSERT INTO af_kb_chunks "
                    "(id, tenant_id, kb, doc_id, chunk_index, content, embedding, created_at) "
                    "VALUES ($1,$2,$3,$4,$5,$6,$7::vector,$8)",
                    rows,
                )
        return len(rows)

    async def query(
        self, tenant_id: str, kb: str, query: str, top_k: int = 4
    ) -> list[RetrievedChunk]:
        qvec = _vec_literal(embed_one(query))
        async with self._pool.acquire() as conn:
            records = await conn.fetch(
                "SELECT doc_id, chunk_index, content, "
                "1 - (embedding <=> $4::vector) AS score "
                "FROM af_kb_chunks WHERE tenant_id=$1 AND kb=$2 "
                "ORDER BY embedding <=> $4::vector LIMIT $3",
                tenant_id,
                kb,
                top_k,
                qvec,
            )
        return [
            RetrievedChunk(
                doc_id=r["doc_id"],
                chunk_index=r["chunk_index"],
                content=r["content"],
                score=float(r["score"]),
            )
            for r in records
        ]

    async def delete_document(self, tenant_id: str, kb: str, doc_id: str) -> int:
        async with self._pool.acquire() as conn:
            result = await conn.execute(
                "DELETE FROM af_kb_chunks WHERE tenant_id=$1 AND kb=$2 AND doc_id=$3",
                tenant_id,
                kb,
                doc_id,
            )
        # result like "DELETE 3"
        try:
            return int(result.split()[-1])
        except (ValueError, IndexError):
            return 0
