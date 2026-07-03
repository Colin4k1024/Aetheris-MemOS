"""
Full-Stack Distributed Demo — Aetheris MemOS

The showcase example tying ALL memory layers together in one coherent flow:

  1. System health & adaptive configuration selection
  2. Short-Term Memory (STM) — live conversation turns
  3. Long-Term Memory (LTM) — domain knowledge documents
  4. Knowledge Graph (KG) — structured entity/relation facts
  5. MCP Protocol — tool discovery and invocation
  6. STM → LTM transfer — promote session content to persistent memory
  7. Hybrid Search — unified query across every layer
  8. Data-flow diagram — console summary

Run:
    python examples/full_stack_distributed_demo.py
    AETHERIS_BASE_URL=http://myhost:8008 python examples/full_stack_distributed_demo.py
"""

from __future__ import annotations

import json
import os
import sys

# Add the Python SDK to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "sdks", "python"))

import requests  # noqa: E402 — used for raw KG/transfer calls

from adaptive_memory import MemoryClient  # noqa: E402

BASE_URL = os.environ.get("AETHERIS_BASE_URL", "http://localhost:8008")


def _print_json(label: str, data: object, indent: int = 4) -> None:
    """Pretty-print a JSON blob with a label prefix."""
    print(f"  {label}:")
    text = json.dumps(data, ensure_ascii=False, indent=indent)
    for line in text.splitlines():
        print(f"    {line}")


def _post(path: str, payload: dict) -> dict:
    """Raw POST helper (used for endpoints not yet in the SDK)."""
    url = f"{BASE_URL.rstrip('/')}/{path.lstrip('/')}"
    resp = requests.post(url, json=payload, timeout=30)
    resp.raise_for_status()
    return resp.json()


def main() -> None:
    print("=" * 60)
    print("  Aetheris MemOS — Full-Stack Distributed Demo")
    print(f"  Backend: {BASE_URL}")
    print("=" * 60)

    client = MemoryClient(BASE_URL)

    # ------------------------------------------------------------------ #
    # 1. System Health & Adaptive Configuration
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  1. System Health & Adaptive Configuration")
    print("=" * 60)

    try:
        health = client.health_check()
        status = health.get("status", health)
        print(f"  Health status: {status}")
    except Exception as exc:
        print(f"  WARNING: health check failed — {exc}")

    task_description = "multi-turn Q&A with domain expertise about European history"
    print(f"\n  Selecting config for task: \"{task_description}\"")
    try:
        config = client.select_memory_config(task_description)
        selected = config.get("selectedConfig", config.get("config", config))
        config_name = (
            selected.get("name", "?")
            if isinstance(selected, dict)
            else str(selected)
        )
        print(f"  Selected config: {config_name}")
    except Exception as exc:
        print(f"  WARNING: adaptive config selection failed — {exc}")

    # ------------------------------------------------------------------ #
    # 2. Short-Term Memory: Conversation
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  2. Short-Term Memory: Conversation")
    print("=" * 60)

    conversation = [
        ("user",      "What were the major causes of the French Revolution?"),
        ("assistant", (
            "The major causes included financial crisis, social inequality "
            "between the estates, Enlightenment ideas, and weak royal leadership."
        )),
        ("user",      "Which Enlightenment thinkers most influenced it?"),
    ]

    session_id: str | None = None
    for role, content in conversation:
        print(f"  Storing [{role}]: {content[:60]}...")
        try:
            result = client.store_stm(
                user_id="demo-user",
                agent_id="demo-agent",
                content=content,
                session_type="conversation",
                role=role,
                session_id=session_id,
            )
            if session_id is None:
                session_id = result.get("sessionId")
                print(f"  Session created: {session_id}")
        except Exception as exc:
            print(f"  ERROR storing STM message: {exc}")

    print(f"\n  STM session ID: {session_id}")

    # ------------------------------------------------------------------ #
    # 3. Long-Term Memory: Domain Knowledge
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  3. Long-Term Memory: Domain Knowledge")
    print("=" * 60)

    ltm_docs = [
        {
            "source_id": "history-voltaire-001",
            "source_type": "document",
            "title": "Voltaire and the Enlightenment",
            "content": (
                "Voltaire (1694–1778) was a French Enlightenment writer and philosopher. "
                "He championed civil liberties, freedom of religion, and free trade. "
                "His criticism of the Church and French aristocracy profoundly influenced "
                "the ideological climate that led to the French Revolution."
            ),
        },
        {
            "source_id": "history-rousseau-001",
            "source_type": "document",
            "title": "Rousseau's Social Contract",
            "content": (
                "Jean-Jacques Rousseau (1712–1778) argued in 'The Social Contract' (1762) "
                "that legitimate political authority rests on a contract among citizens. "
                "His ideas about popular sovereignty and equality directly inspired "
                "revolutionary leaders including Robespierre."
            ),
        },
    ]

    ltm_ids = []
    for doc in ltm_docs:
        try:
            result = client.store_ltm(
                source_id=doc["source_id"],
                source_type=doc["source_type"],
                content=doc["content"],
                title=doc["title"],
            )
            entry_id = result.get("entryId", result.get("entry_id", "?"))
            ltm_ids.append(str(entry_id))
            print(f"  Stored LTM: '{doc['title']}'  id={entry_id}")
        except Exception as exc:
            print(f"  ERROR storing LTM doc: {exc}")

    # ------------------------------------------------------------------ #
    # 4. Knowledge Graph: Structured Facts
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  4. Knowledge Graph: Structured Facts")
    print("=" * 60)

    entities = [
        {
            "name": "Voltaire",
            "entityType": "Person",
            "properties": {"born": "1694", "died": "1778", "nationality": "French"},
        },
        {
            "name": "French Revolution",
            "entityType": "HistoricalEvent",
            "properties": {"start": "1789", "end": "1799", "location": "France"},
        },
    ]

    entity_ids = []
    for entity in entities:
        print(f"  Creating entity: {entity['name']} ({entity['entityType']})")
        try:
            result = _post("api/kg/entities", entity)
            eid = result.get("id", result.get("entityId", "?"))
            entity_ids.append(str(eid))
            print(f"    Created with id={eid}")
        except Exception as exc:
            print(f"    ERROR: {exc}")

    if len(entity_ids) >= 2:
        relation = {
            "fromEntityId": entity_ids[0],
            "toEntityId": entity_ids[1],
            "relationType": "INFLUENCED",
            "properties": {"confidence": 0.95},
        }
        print(
            f"  Creating relation: entity[{entity_ids[0]}] "
            f"--INFLUENCED--> entity[{entity_ids[1]}]"
        )
        try:
            result = _post("api/kg/relations", relation)
            rid = result.get("id", result.get("relationId", "?"))
            print(f"    Created relation id={rid}")
        except Exception as exc:
            print(f"    ERROR: {exc}")

    # ------------------------------------------------------------------ #
    # 5. MCP Protocol: Tool Discovery & Usage
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  5. MCP Protocol: Tool Discovery & Usage")
    print("=" * 60)

    print("  Initializing MCP connection...")
    try:
        init_result = client.initialize_mcp()
        print(f"  Init result: {init_result.get('status', init_result)}")
    except Exception as exc:
        print(f"  WARNING: MCP init failed — {exc}")

    print("  Listing available MCP tools...")
    try:
        tools_result = client.list_mcp_tools()
        tools = tools_result.get("tools", tools_result if isinstance(tools_result, list) else [])
        if tools:
            for t in tools:
                name = t.get("name", t) if isinstance(t, dict) else t
                desc = (t.get("description", "")[:50] + "...") if isinstance(t, dict) else ""
                print(f"    - {name}: {desc}")
        else:
            print("    (no tools listed — server may not expose MCP tool registry)")
    except Exception as exc:
        print(f"  WARNING: list_mcp_tools failed — {exc}")

    print("  Calling MCP tool: memory_search...")
    try:
        tool_result = client.call_mcp_tool(
            "memory_search",
            {
                "query": "Enlightenment philosophers French Revolution",
                "layer": "ltm",
                "limit": 3,
            },
        )
        results = tool_result.get("results", tool_result.get("data", []))
        print(f"  MCP search returned {len(results) if isinstance(results, list) else '?'} result(s)")
        if isinstance(results, list):
            for r in results[:3]:
                content = r.get("content", "")[:60] if isinstance(r, dict) else str(r)[:60]
                print(f"    - {content!r}")
    except Exception as exc:
        print(f"  WARNING: MCP tool call failed — {exc}")

    # ------------------------------------------------------------------ #
    # 6. STM → LTM Transfer
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  6. STM → LTM Transfer")
    print("=" * 60)

    if session_id:
        print(f"  Promoting session {session_id} to long-term memory...")
        try:
            transfer_result = _post(
                "api/v1/memory/storage/transfer",
                {"sessionId": session_id},
            )
            print(f"  Transfer result: {transfer_result}")
        except Exception as exc:
            print(f"  WARNING: transfer failed — {exc}")
    else:
        print("  Skipping transfer (no session_id available)")

    # ------------------------------------------------------------------ #
    # 7. Hybrid Search: Unified Query Across All Layers
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  7. Hybrid Search: Unified Query Across All Layers")
    print("=" * 60)

    query = "Enlightenment philosophers influence French Revolution"
    print(f"  Query: \"{query}\"")
    print()

    try:
        results = client.search_hybrid(query=query, user_id="demo-user", limit=8)
        items = results if isinstance(results, list) else results.get("results", [])
        print(f"  Total results: {len(items)}")

        layer_counts: dict[str, int] = {}
        for i, item in enumerate(items[:8], 1):
            content = item.get("content", item.get("summary", ""))[:70]
            score = item.get("score", 0.0)
            layer = item.get("sourceLayer", item.get("layer", "unknown"))
            layer_counts[layer] = layer_counts.get(layer, 0) + 1
            print(f"  [{i:02d}] layer={layer:<6} score={score:.3f}  {content!r}")

        print()
        print("  Layer breakdown:")
        for layer, count in sorted(layer_counts.items()):
            print(f"    {layer}: {count} result(s)")
    except Exception as exc:
        print(f"  ERROR during hybrid search: {exc}")

    # ------------------------------------------------------------------ #
    # 8. Summary — Data Flow Diagram
    # ------------------------------------------------------------------ #
    print("\n" + "=" * 60)
    print("  8. Summary — Data Flow")
    print("=" * 60)
    print("""
  User Input
       │
       ▼
  ┌─────────────────────────────────────────────────┐
  │             Aetheris MemOS Backend              │
  │                                                 │
  │  ┌─────────┐   transfer   ┌──────────────────┐  │
  │  │   STM   │ ──────────▶  │       LTM        │  │
  │  │ (turns) │              │  (documents +    │  │
  │  └─────────┘              │   embeddings)    │  │
  │                           └──────────────────┘  │
  │                                                 │
  │  ┌──────────────────┐   ┌──────────────────┐   │
  │  │  Knowledge Graph │   │   MCP Protocol   │   │
  │  │  (entities +     │   │  (tool calls +   │   │
  │  │   relations)     │   │   sandboxing)    │   │
  │  └──────────────────┘   └──────────────────┘   │
  │                                                 │
  │  Adaptive Scheduler → selects optimal config    │
  │                                                 │
  │  ┌────────────────────────────────────────────┐ │
  │  │   Hybrid Search (semantic + keyword BM25)  │ │
  │  │   returns ranked results from all layers   │ │
  │  └────────────────────────────────────────────┘ │
  └─────────────────────────────────────────────────┘
       │
       ▼
  Agent / LangChain / User
""")
    print("  Demo complete.")


if __name__ == "__main__":
    main()
