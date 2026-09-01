# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

import json

import pytest

from app.agent import tools as tools_mod
from app.agent.tools import (
    browser_runtime_tools,
    build_tools,
    calculator_tool,
    custom_node_tool,
    http_request_tool,
    is_safe_url,
    safe_calc,
)


def test_safe_calc_arithmetic():
    assert safe_calc("2 + 3 * 4") == 14
    assert safe_calc("(1 + 2) ** 3") == 27
    assert safe_calc("-10 // 3") == -4


def test_safe_calc_rejects_non_arithmetic():
    with pytest.raises(ValueError):
        safe_calc("__import__('os').system('echo hi')")
    with pytest.raises(ValueError):
        safe_calc("a + 1")  # names not allowed


async def test_calculator_tool_run():
    tool = calculator_tool()
    assert await tool.run({"expression": "10 / 4"}) == "2.5"


def test_build_tools_skips_unknown_and_rag_without_store():
    tools = build_tools(["calculator", "rag_search", "bogus"])
    assert [t.name for t in tools] == ["calculator"]  # rag_search needs a store


def test_build_tools_includes_rag_with_store():
    tools = build_tools(["rag_search"], store=object(), tenant_id="t", default_kb="kb")
    assert [t.name for t in tools] == ["rag_search"]


# ── Egress guard ────────────────────────────────────────────────────────────


@pytest.mark.parametrize(
    "url",
    [
        "http://127.0.0.1/admin",
        "http://localhost:8080/",
        "http://10.0.0.5/internal",
        "http://169.254.169.254/latest/meta-data/",  # cloud metadata
        "http://[::1]/",
        "ftp://example.com/x",
        "file:///etc/passwd",
        "https:///nohost",
    ],
)
def test_open_egress_blocks_dangerous_targets(url):
    # allow_public reaches the IP validation; every target must still be blocked.
    assert is_safe_url(url, allow_hosts=None, allow_public=True)[0] is False


def test_open_egress_allows_public_ip():
    ok, reason = is_safe_url("https://8.8.8.8/", allow_hosts=None, allow_public=True)
    assert ok is True, reason


def test_default_denies_open_egress():
    ok, reason = is_safe_url("https://8.8.8.8/", allow_hosts=None)
    assert ok is False and "disabled" in reason


def test_allowlist_restricts_to_exact_hosts():
    assert is_safe_url("https://api.internal/x", ["api.internal"])[0] is True
    assert is_safe_url("https://evil.com/x", ["api.internal"])[0] is False
    # An allowlisted host is trusted even if it would otherwise be private.
    assert is_safe_url("http://10.0.0.5/x", ["10.0.0.5"])[0] is True


# ── Fake httpx so the tools never touch the network ─────────────────────────


class _FakeResp:
    def __init__(self, status=200, chunks=(b"",), payload=None, text=""):
        self.status_code = status
        self._chunks = list(chunks)
        self._payload = payload
        self.text = text

    async def aiter_bytes(self):
        for chunk in self._chunks:
            yield chunk

    async def aclose(self):
        pass

    def json(self):
        return self._payload

    def raise_for_status(self):
        if self.status_code >= 400:
            raise tools_mod.httpx.HTTPError("bad status")


class _FakeClient:
    def __init__(self, resp, capture):
        self._resp = resp
        self._capture = capture

    async def __aenter__(self):
        return self

    async def __aexit__(self, *exc):
        return False

    def build_request(self, method, url, **kw):
        self._capture.update(method=method, url=str(url), build=kw)
        return ("request", method, str(url), kw)

    async def send(self, request, stream=False):
        self._capture["stream"] = stream
        return self._resp

    async def post(self, url, **kw):
        self._capture.update(method="POST", url=str(url), **kw)
        return self._resp


def _patch_httpx(monkeypatch, resp, capture):
    monkeypatch.setattr(tools_mod.httpx, "AsyncClient", lambda **kw: _FakeClient(resp, capture))


def _patch_dns(monkeypatch, ip):
    monkeypatch.setattr(
        tools_mod.socket, "getaddrinfo", lambda host, port, **kw: [(2, 1, 6, "", (ip, port))]
    )


async def test_http_request_default_denies(monkeypatch):
    capture: dict = {}
    _patch_httpx(monkeypatch, _FakeResp(), capture)
    out = await http_request_tool().run({"url": "https://example.com/"})
    assert out.startswith("error:") and "disabled" in out
    assert capture == {}  # no request issued


async def test_http_request_allowlisted_is_unpinned(monkeypatch):
    capture: dict = {}
    _patch_httpx(monkeypatch, _FakeResp(status=201, chunks=[b"crea", b"ted"]), capture)
    out = await http_request_tool(allow_hosts=["api.internal"]).run(
        {"url": "https://api.internal/v1", "method": "POST", "body": {"x": 1}}
    )
    assert json.loads(out) == {"status": 201, "body": "created"}
    assert capture["method"] == "POST"
    assert capture["url"] == "https://api.internal/v1"  # not rewritten to an IP
    assert capture["build"]["json"] == {"x": 1}
    assert "extensions" not in capture["build"]
    assert capture["stream"] is True


async def test_http_request_pins_validated_ip(monkeypatch):
    capture: dict = {}
    _patch_httpx(monkeypatch, _FakeResp(status=200, chunks=[b"ok"]), capture)
    _patch_dns(monkeypatch, "93.184.216.34")
    out = await http_request_tool(allow_hosts=None, allow_public=True).run(
        {"url": "https://example.com/data"}
    )
    assert json.loads(out) == {"status": 200, "body": "ok"}
    # Connected to the validated IP, with the real Host and TLS SNI preserved.
    assert capture["url"] == "https://93.184.216.34/data"
    assert capture["build"]["headers"]["Host"] == "example.com"
    assert capture["build"]["extensions"] == {"sni_hostname": "example.com"}


async def test_http_request_blocks_rebind_to_private(monkeypatch):
    capture: dict = {}
    _patch_httpx(monkeypatch, _FakeResp(), capture)
    _patch_dns(monkeypatch, "10.0.0.7")  # hostname resolves to an internal IP
    out = await http_request_tool(allow_hosts=None, allow_public=True).run(
        {"url": "http://sneaky.example/"}
    )
    assert out.startswith("error:") and "non-public" in out
    assert capture == {}  # blocked before any connection


async def test_custom_node_tool_uses_executor_contract(monkeypatch):
    capture: dict = {}
    _patch_httpx(monkeypatch, _FakeResp(payload={"output_json": '{"label":"spam"}'}), capture)
    tool = custom_node_tool(
        {"name": "classify", "url": "http://nodes.local/nodes/classify", "description": "classify text"}
    )
    out = await tool.run({"text": "buy now"})
    assert out == '{"label":"spam"}'
    assert capture["json"]["node_id"] == "classify"
    assert json.loads(capture["json"]["input_json"]) == {"text": "buy now"}


def test_build_tools_wires_new_tools():
    built = build_tools(
        ["calculator", "http_request"],
        node_tools=[{"name": "n1", "url": "http://x/nodes/n1"}],
        http_allow_hosts=["x"],
    )
    assert [t.name for t in built] == ["calculator", "http_request", "n1"]


def test_browser_tools_require_authenticated_runtime_and_tenant():
    assert browser_runtime_tools("", "x" * 32, "tenant", "execution", [], [], 5, 30) == []
    assert browser_runtime_tools("http://browser", "short", "tenant", "execution", [], [], 5, 30) == []
    assert browser_runtime_tools("http://browser", "x" * 32, "", "execution", [], [], 5, 30) == []


async def test_browser_tools_enforce_host_step_and_runtime_contract(monkeypatch):
    calls: list[dict] = []
    responses = [
        _BrowserResp(201, {"id": "bs_1"}),
        _BrowserResp(202, {"task_id": "bt_1"}),
        _BrowserResp(200, {"status": "running"}),
        _BrowserResp(200, {"status": "completed", "result": {"actions": [{"data": {"url": "https://example.com"}}], "final_url": "https://example.com", "title": "Example"}}),
        _BrowserResp(200, {"status": "closed"}),
    ]

    class BrowserClient:
        async def __aenter__(self):
            return self

        async def __aexit__(self, *exc):
            return False

        async def request(self, method, url, **kwargs):
            calls.append({"method": method, "url": url, **kwargs})
            return responses.pop(0)

    monkeypatch.setattr(tools_mod.httpx, "AsyncClient", lambda **kwargs: BrowserClient())
    tools = browser_runtime_tools(
        "http://browser:38100", "x" * 32, "tenant-1", "execution-1",
        ["example.com"], ["navigate"], 1, 30,
    )
    by_name = {tool.name: tool for tool in tools}
    assert set(by_name) == {"browser_start", "browser_navigate", "browser_close"}
    assert json.loads(await by_name["browser_start"].run({}))["session_id"] == "bs_1"
    assert (await by_name["browser_navigate"].run({"url": "http://127.0.0.1"})).startswith("error:")
    result = json.loads(await by_name["browser_navigate"].run({"url": "https://example.com"}))
    assert result["steps_remaining"] == 0
    assert (await by_name["browser_navigate"].run({"url": "https://example.com"})).startswith("error:")
    await by_name["browser_close"].run({})
    assert calls[0]["headers"]["x-trigix-tenant-id"] == "tenant-1"
    assert calls[1]["json"]["execution_id"] == "execution-1"


class _BrowserResp:
    def __init__(self, status_code, payload):
        self.status_code = status_code
        self._payload = payload
        self.text = json.dumps(payload)

    def json(self):
        return self._payload
