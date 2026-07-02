"""Tests for AdaptiveMemoryTool and create_memory_tools."""

import json

import pytest
from adaptive_memory import MemoryClient

from adaptive_memory_langchain import AdaptiveMemoryTool, create_memory_tools


class MockClient(MemoryClient):
    """Mock client for tool tests."""

    def __init__(self):
        super().__init__(base_url="http://test.local")
        self.calls = []

    def _request(self, method, path, json_body=None, params=None):
        call = {"method": method, "path": path, "json": json_body, "params": params}
        self.calls.append(call)

        if path.endswith("/stm"):
            return {"sessionId": "s1", "messageId": "m1"}
        if path.endswith("/ltm"):
            return {"entryId": "e1"}
        if "search" in path:
            return {
                "results": [
                    {
                        "memoryId": "mem-1",
                        "content": "Remembered fact",
                        "score": 0.95,
                        "sourceLayer": "ltm",
                        "metadata": {},
                    }
                ]
            }
        if path.endswith("/forget"):
            return {"forgotten": True, "memoryId": "m1"}
        if "tools/call" in path:
            return {"content": [], "is_error": False}
        return {}

    def remember(self, **kwargs):
        self.calls.append({"action": "remember", "kwargs": kwargs})
        return {"sessionId": "s1", "messageId": "m1"}

    def search(self, **kwargs):
        self.calls.append({"action": "search", "kwargs": kwargs})
        return {
            "results": [
                {
                    "memoryId": "mem-1",
                    "content": "Remembered fact",
                    "score": 0.95,
                    "metadata": {},
                }
            ]
        }

    def forget(self, **kwargs):
        self.calls.append({"action": "forget", "kwargs": kwargs})
        return {"forgotten": True}


def test_unified_tool_remember():
    client = MockClient()
    tool = AdaptiveMemoryTool(client=client, user_id="u1", agent_id="a1")

    result = json.loads(tool.invoke({
        "action": "remember",
        "content": "User prefers dark mode",
        "layer": "stm",
    }))

    assert result["success"] is True
    remember_call = next(c for c in client.calls if c.get("action") == "remember")
    assert remember_call["kwargs"]["content"] == "User prefers dark mode"


def test_unified_tool_search():
    client = MockClient()
    tool = AdaptiveMemoryTool(client=client, user_id="u1", agent_id="a1")

    result = json.loads(tool.invoke({
        "action": "search",
        "query": "user preferences",
        "limit": 3,
    }))

    assert "results" in result
    assert len(result["results"]) == 1
    assert result["results"][0]["memory_id"] == "mem-1"


def test_unified_tool_forget():
    client = MockClient()
    tool = AdaptiveMemoryTool(client=client, user_id="u1", agent_id="a1")

    result = json.loads(tool.invoke({
        "action": "forget",
        "memory_id": "mem-1",
        "layer": "ltm",
    }))

    assert result["success"] is True


def test_unified_tool_error_on_missing_content():
    client = MockClient()
    tool = AdaptiveMemoryTool(client=client, user_id="u1", agent_id="a1")

    result = json.loads(tool.invoke({"action": "remember"}))
    assert "error" in result


def test_unified_tool_error_on_unknown_action():
    client = MockClient()
    tool = AdaptiveMemoryTool(client=client, user_id="u1", agent_id="a1")

    result = json.loads(tool.invoke({"action": "unknown_action"}))
    assert "error" in result


def test_create_memory_tools_returns_three():
    client = MockClient()
    tools = create_memory_tools(client=client, user_id="u1", agent_id="a1")

    assert len(tools) == 3
    names = {t.name for t in tools}
    assert names == {"memory_store", "memory_search", "memory_forget"}


def test_split_store_tool():
    client = MockClient()
    tools = create_memory_tools(client=client, user_id="u1", agent_id="a1")
    store_tool = next(t for t in tools if t.name == "memory_store")

    result = json.loads(store_tool.invoke({"content": "Important fact", "layer": "ltm"}))
    assert result["success"] is True


def test_split_search_tool():
    client = MockClient()
    tools = create_memory_tools(client=client, user_id="u1", agent_id="a1")
    search_tool = next(t for t in tools if t.name == "memory_search")

    result = json.loads(search_tool.invoke({"query": "preferences", "limit": 3}))
    assert "results" in result
