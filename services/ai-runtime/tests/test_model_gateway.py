from app.model_gateway import select_provider


def test_explicit_provider_wins():
    assert select_provider({"provider": "anthropic", "model": "gpt-4o"}) == "anthropic"
    assert select_provider({"provider": "openai", "model": "claude-x"}) == "openai"


def test_provider_inference_preserves_compatibility():
    assert select_provider({"model": "claude-sonnet"}) == "anthropic"
    assert select_provider({"model": "qwen-max"}) == "openai"
    assert select_provider({"base_url": "http://model.local/v1"}) == "openai"
