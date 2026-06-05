# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

from examples.useful_nodes import html_to_text, redact_pii, sentiment


def test_html_to_text_strips_tags_and_scripts():
    html = "<html><body><h1>Title</h1><p>Hello <b>world</b></p><script>evil()</script></body></html>"
    out = html_to_text(html)
    assert "Title" in out
    assert "Hello world" in out
    assert "evil" not in out


def test_html_to_text_decodes_entities():
    assert "A & B" in html_to_text("<p>A &amp; B</p>")


def test_html_to_text_keep_links():
    out = html_to_text('<a href="https://x.com">site</a>', keep_links=True)
    assert "site" in out and "https://x.com" in out


def test_redact_pii_masks_each_category():
    text = "Email a@b.com, call +1 415 555 0100, card 4111 1111 1111 1111, ip 10.0.0.1"
    redacted, counts = redact_pii(text)
    assert "a@b.com" not in redacted
    assert "[EMAIL]" in redacted
    assert "[CREDIT_CARD]" in redacted
    assert "[IPV4]" in redacted
    assert "[PHONE]" in redacted
    assert counts.get("EMAIL") == 1


def test_redact_pii_category_filter():
    redacted, counts = redact_pii("a@b.com 10.0.0.1", ["EMAIL"])
    assert "[EMAIL]" in redacted
    assert "10.0.0.1" in redacted  # IP left untouched
    assert set(counts) == {"EMAIL"}


def test_sentiment_positive_and_negative():
    assert sentiment("This is great and amazing, I love it").get("label") == "positive"
    assert sentiment("Terrible, worst experience, broken and slow").get("label") == "negative"
    assert sentiment("It is a chair").get("label") == "neutral"
