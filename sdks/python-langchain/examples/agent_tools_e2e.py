"""
LangChain Tools End-to-End Demo — Aetheris MemOS

Full lifecycle demo of LangChain memory tools without requiring a live LLM.
Demonstrates every integration point in isolation so you can understand each
component before wiring it into a real agent.

Phases covered:
  1. Create memory tools (memory_store, memory_search, memory_forget)
  2. Store knowledge via tool.invoke()
  3. Search via tool.invoke()
  4. RAG-style retrieval with AdaptiveMemoryRetriever
  5. Persistent chat history with AdaptiveMemoryChatMessageHistory
  6. Forget a memory and verify deletion

Run:
    python sdks/python-langchain/examples/agent_tools_e2e.py
    AETHERIS_BASE_URL=http://myhost:8008 python sdks/python-langchain/examples/agent_tools_e2e.py
"""

from __future__ import annotations

import json
import os
import sys

# Add the core Python SDK
sys.path.insert(
    0,
    os.path.join(os.path.dirname(__file__), "..", "..", "..", "sdks", "python"),
)
# Add the LangChain SDK
sys.path.insert(
    0,
    os.path.join(os.path.dirname(__file__), "..", "..", "python-langchain"),
)

from adaptive_memory import MemoryClient  # noqa: E402
from adaptive_memory_langchain import (  # noqa: E402
    AdaptiveMemoryChatMessageHistory,
    AdaptiveMemoryRetriever,
    create_memory_tools,
)

# LangChain message types (used in Phase 5)
from langchain_core.messages import AIMessage, HumanMessage  # noqa: E402

BASE_URL = os.environ.get("AETHERIS_BASE_URL", "http://localhost:8008")

USER_ID = "demo-user"
AGENT_ID = "demo-agent"


def _pp(label: str, data: object) -> None:
    """Pretty-print a result value."""
    print(f"  {label}:")
    text = json.dumps(data, ensure_ascii=False, indent=4)
    for line in text.splitlines():
        print(f"    {line}")


def main() -> None:
    print("=" * 60)
    print("  Aetheris MemOS — LangChain Tools E2E Demo")
    print(f"  Backend : {BASE_URL}")
    print(f"  User    : {USER_ID}  Agent: {AGENT_ID}")
    print("=" * 60)

    client = MemoryClient(BASE_URL)

    # ------------------------------------------------------------------ #
    # Phase 1: Create Memory Tools
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  Phase 1: Create Memory Tools")
    print("=" * 60)

    tools = create_memory_tools(client, USER_ID, AGENT_ID)

    print(f"  Created {len(tools)} tool(s):")
    tool_map: dict[str, object] = {}
    for tool in tools:
        print(f"    [{tool.name}]")
        print(f"      {tool.description[:80]}...")
        tool_map[tool.name] = tool

    # ------------------------------------------------------------------ #
    # Phase 2: Store Knowledge via Tool
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  Phase 2: Store Knowledge via Tool")
    print("=" * 60)

    store_tool = tool_map.get("memory_store")
    if store_tool is None:
        print("  WARNING: memory_store tool not found — skipping phase")
    else:
        knowledge_items = [
            {
                "content": (
                    "Python was created by Guido van Rossum and first released in 1991. "
                    "It emphasises code readability and supports multiple programming paradigms."
                ),
                "layer": "ltm",
            },
            {
                "content": (
                    "LangChain is a framework for building LLM-powered applications. "
                    "It provides abstractions for chains, agents, tools, and memory."
                ),
                "layer": "ltm",
            },
            {
                "content": (
                    "The user prefers concise answers with code examples when available."
                ),
                "layer": "stm",
            },
        ]

        stored_results = []
        for item in knowledge_items:
            print(f"  Storing [{item['layer']}]: {item['content'][:60]}...")
            try:
                raw = store_tool.invoke(item)
                result = json.loads(raw) if isinstance(raw, str) else raw
                stored_results.append(result)
                success = result.get("success", "?")
                inner = result.get("result", {})
                entry_id = inner.get("entryId", inner.get("entry_id", inner.get("sessionId", "?")))
                print(f"    success={success}  id={entry_id}")
            except Exception as exc:
                print(f"    ERROR: {exc}")
                stored_results.append({})

    # ------------------------------------------------------------------ #
    # Phase 3: Search via Tool
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  Phase 3: Search via Tool")
    print("=" * 60)

    search_tool = tool_map.get("memory_search")
    if search_tool is None:
        print("  WARNING: memory_search tool not found — skipping phase")
        search_results_raw = []
    else:
        query = "Python programming language history"
        print(f"  Query: \"{query}\"")

        search_results_raw = []
        try:
            raw = search_tool.invoke({"query": query, "limit": 5})
            result = json.loads(raw) if isinstance(raw, str) else raw
            items = result.get("results", [])
            search_results_raw = items
            print(f"  Found {len(items)} result(s):")
            for i, item in enumerate(items, 1):
                content = item.get("content", "")[:70]
                score = item.get("score", 0.0)
                mem_id = item.get("memory_id", "?")
                print(f"    [{i}] score={score:.3f}  id={mem_id}  {content!r}")
        except Exception as exc:
            print(f"  ERROR: {exc}")

    # ------------------------------------------------------------------ #
    # Phase 4: Retriever (RAG-style)
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  Phase 4: Retriever (RAG-style)")
    print("=" * 60)

    retriever = AdaptiveMemoryRetriever(
        client=client,
        search_type="hybrid",
        user_id=USER_ID,
        top_k=5,
    )

    rag_query = "LangChain framework for LLM applications"
    print(f"  Query: \"{rag_query}\"")

    try:
        docs = retriever.invoke(rag_query)
        print(f"  Returned {len(docs)} Document(s):")
        for i, doc in enumerate(docs, 1):
            content = doc.page_content[:70]
            meta = doc.metadata
            score = meta.get("score", 0.0)
            layer = meta.get("source_layer", "?")
            mem_id = meta.get("memory_id", "?")
            print(f"    [{i}] score={score:.3f}  layer={layer}  id={mem_id}")
            print(f"         {content!r}")
    except Exception as exc:
        print(f"  ERROR: {exc}")

    # ------------------------------------------------------------------ #
    # Phase 5: Chat History
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  Phase 5: Chat History")
    print("=" * 60)

    history = AdaptiveMemoryChatMessageHistory(
        client=client,
        user_id=USER_ID,
        agent_id=AGENT_ID,
        # session_id left None so the first write auto-creates one
    )

    print("  Adding messages to history...")
    try:
        history.add_messages(
            [
                HumanMessage(content="What is the best Python web framework?"),
                AIMessage(
                    content=(
                        "It depends on the use case. FastAPI is great for async APIs, "
                        "Django for full-stack apps, and Flask for lightweight services."
                    )
                ),
                HumanMessage(content="I'll go with FastAPI for my project."),
            ]
        )
        print(f"  Session ID after writes: {history.session_id}")

        print("\n  Reading messages back:")
        messages = history.messages
        if messages:
            for i, msg in enumerate(messages, 1):
                role = "Human" if isinstance(msg, HumanMessage) else "AI"
                print(f"    [{i}] {role}: {msg.content[:70]!r}")
        else:
            print("  (no messages returned — session may not be readable yet)")
    except Exception as exc:
        print(f"  ERROR: {exc}")

    # ------------------------------------------------------------------ #
    # Phase 6: Forget via Tool
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  Phase 6: Forget via Tool")
    print("=" * 60)

    forget_tool = tool_map.get("memory_forget")
    if forget_tool is None:
        print("  WARNING: memory_forget tool not found — skipping phase")
    else:
        # Pick a memory_id from Phase 3 search results if available
        candidate_id: str | None = None
        for item in search_results_raw:
            mid = item.get("memory_id", "")
            if mid and mid != "?":
                candidate_id = mid
                break

        if candidate_id is None:
            print("  No candidate memory_id from search results.")
            print("  Using a placeholder id to show tool invocation pattern.")
            candidate_id = "placeholder-00000000"

        print(f"  Forgetting memory id={candidate_id!r} (layer=ltm)...")
        try:
            raw = forget_tool.invoke(
                {"memory_id": candidate_id, "layer": "ltm"}
            )
            result = json.loads(raw) if isinstance(raw, str) else raw
            print(f"  Forget result: success={result.get('success', '?')}")
        except Exception as exc:
            # 404 is expected for placeholder ids
            print(f"  Forget returned: {exc}")

        print("\n  Verifying with a follow-up search...")
        if search_tool is not None:
            try:
                raw = search_tool.invoke({"query": "Python programming language history", "limit": 3})
                result = json.loads(raw) if isinstance(raw, str) else raw
                items = result.get("results", [])
                remaining_ids = [r.get("memory_id", "") for r in items]
                if candidate_id in remaining_ids:
                    print(f"  Memory {candidate_id!r} still present (backend may need propagation time)")
                else:
                    print(f"  Memory {candidate_id!r} no longer in top results — forget succeeded")
                print(f"  Search returned {len(items)} result(s) after forget")
            except Exception as exc:
                print(f"  ERROR during verification search: {exc}")

    # ------------------------------------------------------------------ #
    # Summary
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  Demo complete.")
    print()
    print("  Integration points demonstrated:")
    print("    create_memory_tools()             — Phase 1")
    print("    memory_store.invoke()             — Phase 2")
    print("    memory_search.invoke()            — Phase 3")
    print("    AdaptiveMemoryRetriever.invoke()  — Phase 4")
    print("    AdaptiveMemoryChatMessageHistory  — Phase 5")
    print("    memory_forget.invoke()            — Phase 6")
    print("=" * 60)


if __name__ == "__main__":
    main()
