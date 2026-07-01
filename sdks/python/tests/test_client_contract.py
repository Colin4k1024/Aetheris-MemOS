from adaptive_memory import MemoryClient


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
