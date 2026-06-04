# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

import math

from app.rag.embeddings import EMBED_DIM, embed, embed_one, local_embed_one


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
