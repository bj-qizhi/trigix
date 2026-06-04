# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Split documents into overlapping character windows for embedding."""

from __future__ import annotations


def chunk_text(text: str, chunk_size: int = 1000, overlap: int = 150) -> list[str]:
    """Split `text` into chunks of at most `chunk_size` characters with `overlap`
    characters shared between consecutive chunks.

    Splitting prefers paragraph / sentence boundaries near the window edge so
    chunks stay coherent, falling back to a hard cut when no boundary is near.
    """
    text = text.strip()
    if not text:
        return []
    if chunk_size <= 0:
        raise ValueError("chunk_size must be positive")
    overlap = max(0, min(overlap, chunk_size - 1))

    chunks: list[str] = []
    start = 0
    n = len(text)
    while start < n:
        end = min(start + chunk_size, n)
        if end < n:
            # Try to break on a paragraph or sentence boundary in the last 30%.
            window = text[start:end]
            floor = int(chunk_size * 0.7)
            cut = -1
            for sep in ("\n\n", "\n", ". ", "。", "! ", "? "):
                idx = window.rfind(sep)
                if idx >= floor:
                    cut = idx + len(sep)
                    break
            if cut != -1:
                end = start + cut
        chunk = text[start:end].strip()
        if chunk:
            chunks.append(chunk)
        if end >= n:
            break
        start = max(end - overlap, start + 1)
    return chunks
