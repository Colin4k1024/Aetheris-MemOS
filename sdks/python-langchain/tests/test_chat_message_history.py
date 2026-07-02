"""Tests for AdaptiveMemoryChatMessageHistory."""

import pytest
from adaptive_memory import MemoryClient
from langchain_core.messages import AIMessage, HumanMessage, SystemMessage

from adaptive_memory_langchain import AdaptiveMemoryChatMessageHistory


class MockClient(MemoryClient):
    """Mock client that records calls and returns fixtures."""

    def __init__(self):
        super().__init__(base_url="http://test.local")
        self.calls = []
        self._session_messages = []

    def _request(self, method, path, json=None, params=None):
        call = {"method": method, "path": path, "json": json, "params": params}
        self.calls.append(call)

        if path.endswith("/stm"):
            return {"sessionId": "session-001", "messageId": "msg-001"}
        if "/stm/" in path:
            return self._session_messages
        return {}

    def recall_session(self, session_id, limit=10):
        self.calls.append(
            {"method": "GET", "path": f"recall_session/{session_id}", "limit": limit}
        )
        return self._session_messages


def test_empty_messages_when_no_session():
    client = MockClient()
    history = AdaptiveMemoryChatMessageHistory(
        client=client, user_id="u1", agent_id="a1"
    )
    assert history.messages == []


def test_add_messages_creates_session():
    client = MockClient()
    history = AdaptiveMemoryChatMessageHistory(
        client=client, user_id="u1", agent_id="a1"
    )

    history.add_messages([HumanMessage(content="Hello")])

    assert history.session_id == "session-001"
    # Verify remember was called with correct params
    stm_call = next(c for c in client.calls if c["path"].endswith("/stm"))
    assert stm_call["json"]["content"] == "Hello"
    assert stm_call["json"]["role"] == "user"


def test_add_multiple_messages():
    client = MockClient()
    history = AdaptiveMemoryChatMessageHistory(
        client=client, user_id="u1", agent_id="a1", session_id="s1"
    )

    history.add_messages([
        HumanMessage(content="Hi"),
        AIMessage(content="Hello!"),
        SystemMessage(content="Be helpful"),
    ])

    stm_calls = [c for c in client.calls if c["path"].endswith("/stm")]
    assert len(stm_calls) == 3
    assert stm_calls[0]["json"]["role"] == "user"
    assert stm_calls[1]["json"]["role"] == "assistant"
    assert stm_calls[2]["json"]["role"] == "system"


def test_messages_property_returns_langchain_types():
    client = MockClient()
    client._session_messages = [
        {"role": "user", "content": "What is Python?"},
        {"role": "assistant", "content": "A programming language."},
        {"role": "system", "content": "Be concise."},
    ]

    history = AdaptiveMemoryChatMessageHistory(
        client=client, user_id="u1", agent_id="a1", session_id="s1"
    )

    msgs = history.messages
    assert len(msgs) == 3
    assert isinstance(msgs[0], HumanMessage)
    assert isinstance(msgs[1], AIMessage)
    assert isinstance(msgs[2], SystemMessage)
    assert msgs[0].content == "What is Python?"


def test_clear_resets_session():
    client = MockClient()
    history = AdaptiveMemoryChatMessageHistory(
        client=client, user_id="u1", agent_id="a1", session_id="s1"
    )

    history.clear()
    assert history.session_id is None
