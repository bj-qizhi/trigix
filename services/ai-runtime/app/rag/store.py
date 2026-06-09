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
            # ANN index for vector search at scale (cosine). HNSW gives
            # sub-linear nearest-neighbour lookups instead of a full scan.
            await conn.execute(
                "CREATE INDEX IF NOT EXISTS af_kb_chunks_hnsw "
                "ON af_kb_chunks USING hnsw (embedding vector_cosine_ops)"
            )
            # Full-text index backing the lexical half of hybrid retrieval.
            await conn.execute(
                "CREATE INDEX IF NOT EXISTS af_kb_chunks_fts "
                "ON af_kb_chunks USING gin (to_tsvector('simple', content))"
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
        self,
        tenant_id: str,
        kb: str,
        query: str,
        top_k: int = 4,
        mode: str = "vector",
        min_score: float | None = None,
    ) -> list[RetrievedChunk]:
        """Retrieve the most relevant chunks.

        - ``vector`` (default): cosine similarity; ``min_score`` drops weak hits
          (``score`` is cosine similarity in [-1, 1]).
        - ``hybrid``: Reciprocal Rank Fusion of the vector ranking and a
          full-text ranking — helps when the query hinges on exact tokens
          (codes, identifiers, English terms inside CJK text) that embeddings
          blur. ``score`` is then the fused RRF score, not cosine.
        """
        if mode == "hybrid":
            return await self._query_hybrid(tenant_id, kb, query, top_k)
        return await self._query_vector(tenant_id, kb, query, top_k, min_score)

    async def _query_vector(
        self, tenant_id: str, kb: str, query: str, top_k: int, min_score: float | None
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
        hits = [
            RetrievedChunk(
                doc_id=r["doc_id"],
                chunk_index=r["chunk_index"],
                content=r["content"],
                score=float(r["score"]),
            )
            for r in records
        ]
        if min_score is not None:
            hits = [h for h in hits if h.score >= min_score]
        return hits

    async def _query_hybrid(
        self, tenant_id: str, kb: str, query: str, top_k: int
    ) -> list[RetrievedChunk]:
        qvec = _vec_literal(embed_one(query))
        pool = max(top_k * 5, 50)
        rrf_k = 60
        async with self._pool.acquire() as conn:
            records = await conn.fetch(
                """
                WITH vec AS (
                    SELECT id, ROW_NUMBER() OVER (ORDER BY embedding <=> $1::vector) AS rnk
                    FROM af_kb_chunks WHERE tenant_id=$2 AND kb=$3
                    ORDER BY embedding <=> $1::vector LIMIT $4
                ),
                kw AS (
                    SELECT id, ROW_NUMBER() OVER (
                        ORDER BY ts_rank_cd(to_tsvector('simple', content),
                                            websearch_to_tsquery('simple', $5)) DESC) AS rnk
                    FROM af_kb_chunks
                    WHERE tenant_id=$2 AND kb=$3
                      AND to_tsvector('simple', content) @@ websearch_to_tsquery('simple', $5)
                    LIMIT $4
                ),
                fused AS (
                    SELECT COALESCE(vec.id, kw.id) AS id,
                           COALESCE(1.0 / ($6 + vec.rnk), 0.0)
                         + COALESCE(1.0 / ($6 + kw.rnk), 0.0) AS score
                    FROM vec FULL OUTER JOIN kw ON vec.id = kw.id
                )
                SELECT c.doc_id, c.chunk_index, c.content, f.score
                FROM fused f JOIN af_kb_chunks c ON c.id = f.id
                ORDER BY f.score DESC LIMIT $7
                """,
                qvec,
                tenant_id,
                kb,
                pool,
                query,
                rrf_k,
                top_k,
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

    async def list_kbs(self, tenant_id: str) -> list[dict]:
        async with self._pool.acquire() as conn:
            rows = await conn.fetch(
                "SELECT kb, count(DISTINCT doc_id) AS docs, count(*) AS chunks "
                "FROM af_kb_chunks WHERE tenant_id=$1 GROUP BY kb ORDER BY kb",
                tenant_id,
            )
        return [
            {"kb": r["kb"], "docs": r["docs"], "chunks": r["chunks"]} for r in rows
        ]

    async def list_documents(self, tenant_id: str, kb: str) -> list[dict]:
        async with self._pool.acquire() as conn:
            rows = await conn.fetch(
                "SELECT doc_id, count(*) AS chunks, max(created_at) AS created_at "
                "FROM af_kb_chunks WHERE tenant_id=$1 AND kb=$2 "
                "GROUP BY doc_id ORDER BY max(created_at) DESC",
                tenant_id,
                kb,
            )
        return [
            {
                "doc_id": r["doc_id"],
                "chunks": r["chunks"],
                "created_at": int(r["created_at"]),
            }
            for r in rows
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
