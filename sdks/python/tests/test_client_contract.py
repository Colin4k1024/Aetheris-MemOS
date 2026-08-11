import asyncio
import json

import pytest

from adaptive_memory import AsyncMemoryClient, MemoryApiError, MemoryClient
from adaptive_memory.client import _error_detail_from_text


class RecordingClient(MemoryClient):
    def __init__(self):
        super().__init__(base_url="http://example.test")
        self.calls = []

    def _request(self, method, path, json=None, params=None):
        call = {"method": method, "path": path, "json": json, "params": params}
        self.calls.append(call)
        if path.endswith("/stm"):
            return {"sessionId": "s1", "messageId": "m1"}
        if path.endswith("/ltm"):
            return {"entryId": "e1"}
        if "tools/call" in path:
            return {"content": [], "is_error": False}
        if path.endswith("/explain"):
            return {"traces": [{"trace_id": "t1", "task_id": "task1"}]}
        if path.endswith("/feedback"):
            return {"success": True, "feedback": json}
        if path.endswith("/forget"):
            return {"success": True, "deleted": True}
        return {}


def test_remember_stm_uses_storage_api():
    client = RecordingClient()

    result = client.remember(
        content="hello",
        user_id="u1",
        agent_id="a1",
        session_id="s1",
        layer="stm",
    )

    assert result["messageId"] == "m1"
    assert client.calls[-1]["path"] == "api/v1/memory/storage/stm"
    assert client.calls[-1]["json"]["userId"] == "u1"
    assert client.calls[-1]["json"]["agentId"] == "a1"


def test_remember_ltm_uses_storage_api():
    client = RecordingClient()

    result = client.remember(
        content="durable fact",
        user_id="u1",
        agent_id="a1",
        layer="ltm",
        metadata={"sourceId": "src1", "title": "Fact"},
    )

    assert result["entryId"] == "e1"
    assert client.calls[-1]["path"] == "api/v1/memory/storage/ltm"
    assert client.calls[-1]["json"]["sourceId"] == "src1"


def test_search_non_rest_layer_uses_mcp_tool():
    client = RecordingClient()

    client.search(query="entity", layer="kg", user_id="u1")

    assert client.calls[-1]["path"] == "api/mcp/tools/call"
    assert client.calls[-1]["json"]["name"] == "memory_search"
    assert client.calls[-1]["json"]["arguments"]["layer"] == "kg"


def test_explain_uses_rest_contract():
    client = RecordingClient()

    result = client.explain(trace_id="t1")

    assert result["traces"][0]["trace_id"] == "t1"
    assert client.calls[-1]["path"] == "api/v1/memory/explain"
    assert client.calls[-1]["params"]["traceId"] == "t1"


def test_feedback_uses_rest_contract():
    client = RecordingClient()

    result = client.feedback(memory_id="m1", useful=True)

    assert result["success"] is True
    assert client.calls[-1]["path"] == "api/v1/memory/feedback"
    assert client.calls[-1]["json"]["memoryId"] == "m1"


def test_forget_uses_rest_contract():
    client = RecordingClient()

    result = client.forget(memory_id="m1", layer="ltm")

    assert result["deleted"] is True
    assert client.calls[-1]["path"] == "api/v1/memory/forget"


# ---------------------------------------------------------------------------
# Enum handling (backlog D-j): the SDK must NOT rewrite an invalid enum value
# to a valid one, and must surface the server's 400 (which lists the accepted
# values) so the caller learns what went wrong. The server is the single
# source of truth for the valid set — the SDK keeps no copy of it.
# ---------------------------------------------------------------------------

# A realistic body for the backend's ErrorBody { code, message, error }.
_INVALID_ENUM_400_BODY = json.dumps(
    {
        "code": 400,
        "message": (
            "invalid sessionType: 'chat' is not a valid value; "
            "valid values are: conversation, task, query"
        ),
        "error": (
            "invalid sessionType: 'chat' is not a valid value; "
            "valid values are: conversation, task, query"
        ),
    }
)


class _FakeResponse:
    """Minimal stand-in for requests.Response (sync path)."""

    def __init__(self, status_code, text):
        self.status_code = status_code
        self.text = text

    def json(self):
        return json.loads(self.text)


class _FakeSession:
    """requests.Session stand-in that records the last request it was given.

    RecordingClient overrides ``_request`` entirely, so it cannot exercise the
    real error-surfacing path; this stubs the transport one level lower.
    """

    def __init__(self, response):
        self._response = response
        self.headers = {}
        self.last_request = None

    def request(self, **kwargs):
        self.last_request = kwargs
        return self._response


def test_store_stm_does_not_rewrite_invalid_session_type_and_surfaces_error():
    client = MemoryClient("http://example.test")
    fake = _FakeSession(_FakeResponse(400, _INVALID_ENUM_400_BODY))
    client._session = fake

    with pytest.raises(MemoryApiError) as excinfo:
        client.store_stm("u1", "a1", "hi", session_type="chat")

    # 1. The caller's value is sent UNCHANGED — no silent client-side remap.
    assert fake.last_request["json"]["sessionType"] == "chat"
    # 2. The raised error carries the server's valid-values list.
    message = str(excinfo.value)
    assert "conversation" in message
    assert "task" in message
    assert "query" in message
    assert excinfo.value.status_code == 400


def test_store_stm_default_session_type_is_a_valid_value():
    client = MemoryClient("http://example.test")
    fake = _FakeSession(_FakeResponse(200, json.dumps({"sessionId": "s", "messageId": "m"})))
    client._session = fake

    client.store_stm("u1", "a1", "hi")  # no session_type -> uses the default

    # The default must itself be a server-accepted value, otherwise the common
    # no-session_type call would 400.
    assert fake.last_request["json"]["sessionType"] == "conversation"


def test_remember_stm_forwards_session_id_not_as_session_type():
    client = RecordingClient()

    client.remember(content="hi", user_id="u1", agent_id="a1", session_id="s1", layer="stm")

    sent = client.calls[-1]["json"]
    # Regression: session_id must land in sessionId with a valid sessionType.
    # Previously session_id was passed as session_type (a CHECK-constrained
    # enum) and silently dropped.
    assert sent["sessionId"] == "s1"
    assert sent["sessionType"] == "conversation"


def test_error_detail_prefers_structured_message():
    body = json.dumps({"error": "bad enum", "message": "ignored"})
    assert _error_detail_from_text(400, body) == "bad enum"


def test_error_detail_falls_back_to_raw_text():
    assert _error_detail_from_text(500, "plain text error") == "plain text error"


def test_error_detail_handles_empty_body():
    assert _error_detail_from_text(503, "") == "HTTP 503"


# --- Async client mirrors the sync behavior ------------------------------


class _FakeAioResponse:
    def __init__(self, status, text):
        self.status = status
        self._text = text

    async def text(self):
        return self._text

    async def json(self):
        return json.loads(self._text)


class _FakeAioCM:
    def __init__(self, response):
        self._response = response

    async def __aenter__(self):
        return self._response

    async def __aexit__(self, *args):
        return False


class _FakeAioSession:
    def __init__(self, response):
        self._response = response
        self.closed = False
        self.last_request = None

    def request(self, **kwargs):
        self.last_request = kwargs
        return _FakeAioCM(self._response)


def test_async_store_stm_does_not_rewrite_invalid_session_type_and_surfaces_error():
    async def run():
        client = AsyncMemoryClient("http://example.test")
        fake = _FakeAioSession(_FakeAioResponse(400, _INVALID_ENUM_400_BODY))
        client._client = fake  # bypass real aiohttp session creation

        with pytest.raises(MemoryApiError) as excinfo:
            await client.store_stm("u1", "a1", "hi", session_type="chat")

        assert fake.last_request["json"]["sessionType"] == "chat"
        message = str(excinfo.value)
        assert "conversation" in message and "task" in message and "query" in message

    asyncio.run(run())
