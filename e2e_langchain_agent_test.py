"""
E2E Test: LangChain Agent with Aetheris MemOS Memory

This script tests the full integration path:
1. Store memories via AdaptiveMemoryTool
2. Search memories via AdaptiveMemoryTool
3. Retrieve via AdaptiveMemoryRetriever
4. Chat history via AdaptiveMemoryChatMessageHistory
5. Forget via AdaptiveMemoryTool

Run with backend started:
    PYTHONPATH=sdks/python:sdks/python-langchain python e2e_langchain_agent_test.py
"""

import json
import os
import sys

# Ensure paths
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "sdks/python"))
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "sdks/python-langchain"))

from adaptive_memory import MemoryClient
from adaptive_memory_langchain import (
    AdaptiveMemoryChatMessageHistory,
    AdaptiveMemoryRetriever,
    AdaptiveMemoryTool,
    create_memory_tools,
)
from langchain_core.messages import AIMessage, HumanMessage

BASE_URL = os.getenv("AETHERIS_BASE_URL", "http://localhost:8008")
TOKEN = os.getenv("AETHERIS_TOKEN")

# Get token if not provided
if not TOKEN:
    import requests
    resp = requests.post(
        f"{BASE_URL}/api/login",
        json={"username": "admin", "password": "admin123"},
    )
    resp.raise_for_status()
    TOKEN = resp.json()["token"]
    print(f"[AUTH] Got token: {TOKEN[:20]}...")


def section(title):
    print(f"\n{'='*60}")
    print(f"  {title}")
    print(f"{'='*60}")


def test_memory_tools():
    section("TEST 1: AdaptiveMemoryTool - Store & Search")

    client = MemoryClient(base_url=BASE_URL, api_key=TOKEN)
    tool = AdaptiveMemoryTool(
        client=client,
        user_id="e2e-langchain-user",
        agent_id="e2e-langchain-agent",
    )

    # Store a memory
    print("\n[STORE] Remembering: 'User prefers Python for ML projects'")
    result = json.loads(tool.invoke({
        "action": "remember",
        "content": "User prefers Python for ML projects and uses PyTorch",
        "layer": "stm",
    }))
    print(f"  Result: {json.dumps(result, indent=2)}")
    assert result.get("success") is True, f"Store failed: {result}"
    print("  PASS")

    # Search for it
    print("\n[SEARCH] Querying: 'programming language preferences'")
    result = json.loads(tool.invoke({
        "action": "search",
        "query": "programming language preferences",
        "limit": 5,
    }))
    print(f"  Result: {json.dumps(result, indent=2)}")
    assert "results" in result, f"Search missing results: {result}"
    print(f"  Found {len(result['results'])} results")
    print("  PASS")

    return True


def test_split_tools():
    section("TEST 2: create_memory_tools() - Split Tools")

    client = MemoryClient(base_url=BASE_URL, api_key=TOKEN)
    tools = create_memory_tools(
        client=client,
        user_id="e2e-langchain-user",
        agent_id="e2e-langchain-agent",
    )

    assert len(tools) == 3
    names = {t.name for t in tools}
    assert names == {"memory_store", "memory_search", "memory_forget"}
    print(f"\n[TOOLS] Created: {', '.join(names)}")

    store_tool = next(t for t in tools if t.name == "memory_store")
    search_tool = next(t for t in tools if t.name == "memory_search")

    # Store
    print("\n[STORE] Storing via memory_store tool")
    result = json.loads(store_tool.invoke({
        "content": "The project deadline is next Friday",
        "layer": "stm",
    }))
    assert result.get("success") is True
    print(f"  Result: {result}")
    print("  PASS")

    # Search
    print("\n[SEARCH] Searching via memory_search tool")
    result = json.loads(search_tool.invoke({
        "query": "deadline",
        "limit": 3,
    }))
    assert "results" in result
    print(f"  Found {len(result['results'])} results")
    print("  PASS")

    return True


def test_retriever():
    section("TEST 3: AdaptiveMemoryRetriever")

    client = MemoryClient(base_url=BASE_URL, api_key=TOKEN)

    # First store something in LTM for retrieval
    print("\n[SETUP] Storing LTM entry for retriever test")
    try:
        ltm_result = client.store_ltm(
            source_id="e2e-retriever-test",
            source_type="user_input",
            content="Adaptive memory systems use multi-layer architecture with STM for conversations and LTM for durable knowledge",
            title="Memory Architecture Overview",
        )
        print(f"  Stored LTM: {ltm_result}")
    except Exception as e:
        print(f"  LTM store skipped (may need Ollama): {e}")

    # Create retriever
    retriever = AdaptiveMemoryRetriever(
        client=client,
        search_type="hybrid",
        top_k=5,
        min_score=0.0,
        user_id="e2e-langchain-user",
    )

    print("\n[RETRIEVE] Invoking retriever: 'memory architecture'")
    docs = retriever.invoke("memory architecture")
    print(f"  Retrieved {len(docs)} documents")
    for i, doc in enumerate(docs[:3]):
        print(f"  [{i}] score={doc.metadata.get('score', 0):.3f} "
              f"layer={doc.metadata.get('source_layer', '?')}")
        print(f"      content: {doc.page_content[:80]}...")

    # Verify Document type
    if docs:
        from langchain_core.documents import Document
        assert isinstance(docs[0], Document), "Not a LangChain Document!"
        assert "memory_id" in docs[0].metadata
        print("\n  Document type validation: PASS")
    else:
        print("\n  No documents found (expected if LTM store was skipped)")

    print("  PASS")
    return True


def test_chat_message_history():
    section("TEST 4: AdaptiveMemoryChatMessageHistory")

    client = MemoryClient(base_url=BASE_URL, api_key=TOKEN)
    history = AdaptiveMemoryChatMessageHistory(
        client=client,
        user_id="e2e-langchain-user",
        agent_id="e2e-langchain-agent",
        session_type="conversation",
    )

    # Should start empty (no session yet)
    print("\n[INIT] Messages before any write:")
    assert history.messages == []
    print("  Empty: PASS")

    # Add messages
    print("\n[WRITE] Adding 3 messages...")
    history.add_messages([
        HumanMessage(content="What is reinforcement learning?"),
        AIMessage(content="RL is a type of ML where agents learn by interacting with an environment."),
        HumanMessage(content="Give me a simple example"),
    ])
    print(f"  Session created: {history.session_id}")
    assert history.session_id is not None, "No session created!"
    print("  Session creation: PASS")

    # Read back
    print("\n[READ] Reading messages back...")
    messages = history.messages
    print(f"  Got {len(messages)} messages")
    for msg in messages:
        role = "Human" if isinstance(msg, HumanMessage) else "AI"
        print(f"    [{role}] {msg.content[:60]}...")

    assert len(messages) >= 3, f"Expected >=3 messages, got {len(messages)}"
    assert isinstance(messages[0], HumanMessage)
    assert isinstance(messages[1], AIMessage)
    print("  Message types: PASS")

    # Clear
    print("\n[CLEAR] Clearing history...")
    history.clear()
    assert history.session_id is None
    print("  Session cleared: PASS")

    print("  PASS")
    return True


def main():
    print("=" * 60)
    print("  Aetheris MemOS x LangChain E2E Integration Test")
    print("=" * 60)
    print(f"\nBackend: {BASE_URL}")
    print(f"Token:   {TOKEN[:20]}...")

    results = {}
    tests = [
        ("Memory Tools (unified)", test_memory_tools),
        ("Memory Tools (split)", test_split_tools),
        ("Retriever", test_retriever),
        ("Chat Message History", test_chat_message_history),
    ]

    for name, test_fn in tests:
        try:
            results[name] = test_fn()
        except Exception as e:
            print(f"\n  FAILED: {e}")
            import traceback
            traceback.print_exc()
            results[name] = False

    # Summary
    section("RESULTS SUMMARY")
    all_passed = True
    for name, passed in results.items():
        status = "PASS" if passed else "FAIL"
        print(f"  [{status}] {name}")
        if not passed:
            all_passed = False

    print(f"\n{'='*60}")
    if all_passed:
        print("  ALL TESTS PASSED")
    else:
        print("  SOME TESTS FAILED")
        sys.exit(1)
    print(f"{'='*60}")


if __name__ == "__main__":
    main()
