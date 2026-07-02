"""Tests for AdaptiveMemoryRetriever."""

import pytest
from langchain_core.documents import Document

from adaptive_memory import MemoryClient
from adaptive_memory_langchain import AdaptiveMemoryRetriever


class MockClient(MemoryClient):
    """Mock client returning search fixtures."""

    def __init__(self, results=None):
        super().__init__(base_url="http://test.local")
        self.calls = []
        self._results = results or []

    def _request(self, method, path, json=None, params=None):
        self.calls.append({"method": method, "path": path, "json": json, "params": params})
        return {"results": self._results}


def test_retriever_returns_documents():
    client = MockClient(results=[
        {
            "memoryId": "mem-1",
            "sourceLayer": "ltm",
            "score": 0.92,
            "content": "User prefers Python for data science",
            "title": "User Preference",
            "metadata": {"source": "conversation"},
        },
        {
            "memoryId": "mem-2",
            "sourceLayer": "ltm",
            "score": 0.85,
            "content": "User works at Acme Corp",
            "metadata": {},
        },
    ])

    retriever = AdaptiveMemoryRetriever(client=client, top_k=5)
    docs = retriever.invoke("user preferences")

    assert len(docs) == 2
    assert isinstance(docs[0], Document)
    assert docs[0].page_content == "User prefers Python for data science"
    assert docs[0].metadata["memory_id"] == "mem-1"
    assert docs[0].metadata["score"] == 0.92
    assert docs[0].metadata["source_layer"] == "ltm"
    assert docs[0].metadata["title"] == "User Preference"


def test_retriever_filters_by_min_score():
    client = MockClient(results=[
        {"memoryId": "m1", "score": 0.9, "content": "High score", "metadata": {}},
        {"memoryId": "m2", "score": 0.3, "content": "Low score", "metadata": {}},
    ])

    retriever = AdaptiveMemoryRetriever(client=client, min_score=0.5)
    docs = retriever.invoke("test")

    assert len(docs) == 1
    assert docs[0].metadata["memory_id"] == "m1"


def test_retriever_respects_search_type():
    client = MockClient(results=[])
    retriever = AdaptiveMemoryRetriever(
        client=client, search_type="triple", user_id="u1"
    )
    retriever.invoke("query")

    # "triple" is not a direct REST layer so it goes through MCP tools/call
    last_call = client.calls[-1]
    # The client.search() dispatches non-standard layers via MCP
    assert "tools/call" in last_call["path"]
    assert last_call["json"]["arguments"]["layer"] == "triple"
    assert last_call["json"]["arguments"]["user_id"] == "u1"


def test_retriever_handles_empty_results():
    client = MockClient(results=[])
    retriever = AdaptiveMemoryRetriever(client=client)
    docs = retriever.invoke("nothing matches")

    assert docs == []


def test_retriever_handles_missing_content_gracefully():
    client = MockClient(results=[
        {"memoryId": "m1", "score": 0.8, "title": "Fallback Title", "metadata": {}},
    ])

    retriever = AdaptiveMemoryRetriever(client=client)
    docs = retriever.invoke("test")

    assert len(docs) == 1
    assert docs[0].page_content == "Fallback Title"
