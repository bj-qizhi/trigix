# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

from app.main import _resolve_template


def test_resolve_template_reads_input_and_node_paths():
    rendered = _resolve_template(
        "Hello {{ input.profile.name }} from {{lookup.city}}",
        '{"profile":{"name":"Ada"}}',
        {"lookup": '{"city":"Beijing"}'},
    )
    assert rendered == "Hello Ada from Beijing"


def test_resolve_template_preserves_malformed_and_empty_placeholders():
    template = "before {{}} middle {{input.name} after"
    assert _resolve_template(template, '{"name":"Ada"}', {}) == template


def test_resolve_template_handles_many_unclosed_delimiters_linearly():
    template = "{{" * 20_000
    assert _resolve_template(template, "{}", {}) == template
