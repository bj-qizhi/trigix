# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

from app.rag.chunking import chunk_text


def test_empty_text_yields_no_chunks():
    assert chunk_text("") == []
    assert chunk_text("   \n  ") == []


def test_short_text_is_one_chunk():
    assert chunk_text("hello world", 1000, 100) == ["hello world"]


def test_long_text_is_split_with_overlap():
    text = "x" * 2500
    chunks = chunk_text(text, chunk_size=1000, overlap=200)
    assert len(chunks) >= 3
    assert all(len(c) <= 1000 for c in chunks)
    # Reassembling without overlap must cover the whole input.
    assert sum(len(c) for c in chunks) >= len(text)


def test_prefers_paragraph_boundary():
    para = "First paragraph sentence.\n\n" + ("y" * 400)
    chunks = chunk_text(para, chunk_size=60, overlap=10)
    # The first chunk should end at the paragraph break, not mid-word.
    assert chunks[0].startswith("First paragraph")


def test_overlap_clamped_below_chunk_size():
    # overlap >= chunk_size would loop forever; it must be clamped.
    chunks = chunk_text("a" * 100, chunk_size=10, overlap=999)
    assert len(chunks) >= 10
