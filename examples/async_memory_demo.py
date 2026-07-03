"""
Async Memory Demo — Aetheris MemOS

Demonstrates the AsyncMemoryClient with concurrent operations.

Key patterns shown:
- Context-manager usage (async with AsyncMemoryClient(...) as client)
- asyncio.gather for concurrent STM writes
- Async LTM storage
- Async hybrid search
- Remember/forget lifecycle

Run:
    python examples/async_memory_demo.py
    AETHERIS_BASE_URL=http://myhost:8008 python examples/async_memory_demo.py
"""

from __future__ import annotations

import asyncio
import os
import sys
import time

# Add the Python SDK to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "sdks", "python"))

from adaptive_memory import AsyncMemoryClient  # noqa: E402

BASE_URL = os.environ.get("AETHERIS_BASE_URL", "http://localhost:8008")


async def main() -> None:
    total_start = time.perf_counter()

    print("=" * 60)
    print("  Aetheris MemOS — Async Memory Demo")
    print(f"  Backend: {BASE_URL}")
    print("=" * 60)

    async with AsyncMemoryClient(BASE_URL) as client:

        # ------------------------------------------------------------------ #
        # Health check
        # ------------------------------------------------------------------ #
        print("\n[Health Check]")
        try:
            health = await client.health_check()
            status = health.get("status", health)
            print(f"  Status: {status}")
        except Exception as exc:
            print(f"  WARNING: health check failed — {exc}")
            print("  Continuing demo (some sections may fail if backend is down)")

        # ------------------------------------------------------------------ #
        # Section: Concurrent STM Writes
        # ------------------------------------------------------------------ #
        print("\n" + "=" * 60)
        print("  Section: Concurrent STM Writes")
        print("=" * 60)

        messages = [
            ("async-user", "async-agent", "What is the capital of France?"),
            ("async-user", "async-agent", "Paris is the capital of France."),
            ("async-user", "async-agent", "What is the population of Paris?"),
            ("async-user", "async-agent", "Paris has about 2.1 million residents."),
            ("async-user", "async-agent", "Thank you for the information!"),
        ]

        section_start = time.perf_counter()
        try:
            results = await asyncio.gather(
                *[
                    client.store_stm(
                        user_id=uid,
                        agent_id=aid,
                        content=content,
                        session_type="conversation",
                    )
                    for uid, aid, content in messages
                ],
                return_exceptions=True,
            )
            elapsed = time.perf_counter() - section_start

            session_ids = []
            for i, result in enumerate(results):
                if isinstance(result, Exception):
                    print(f"  Message {i+1}: ERROR — {result}")
                else:
                    sid = result.get("sessionId", "?")
                    session_ids.append(sid)
                    print(f"  Message {i+1}: stored  session={sid}")

            print(f"\n  All 5 messages written concurrently in {elapsed:.3f}s")
        except Exception as exc:
            print(f"  ERROR during concurrent writes: {exc}")
            session_ids = []

        # ------------------------------------------------------------------ #
        # Section: Async LTM Storage
        # ------------------------------------------------------------------ #
        print("\n" + "=" * 60)
        print("  Section: Async LTM Storage")
        print("=" * 60)

        ltm_docs = [
            {
                "source_id": "wiki-paris-001",
                "source_type": "document",
                "title": "Paris Overview",
                "content": (
                    "Paris is the capital and most populous city of France. "
                    "It is situated in northern France on the Seine River. "
                    "Paris is known for the Eiffel Tower, the Louvre museum, "
                    "and its historic boulevards."
                ),
            },
            {
                "source_id": "wiki-france-001",
                "source_type": "document",
                "title": "France Overview",
                "content": (
                    "France is a country in Western Europe. "
                    "Its capital is Paris. France is known for its culture, "
                    "cuisine, and history. It is a founding member of the EU."
                ),
            },
        ]

        ltm_ids = []
        for doc in ltm_docs:
            try:
                result = await client.store_ltm(
                    source_id=doc["source_id"],
                    source_type=doc["source_type"],
                    content=doc["content"],
                    title=doc["title"],
                )
                entry_id = result.get("entryId", result.get("entry_id", "?"))
                ltm_ids.append(entry_id)
                print(f"  Stored LTM: '{doc['title']}'  id={entry_id}")
            except Exception as exc:
                print(f"  ERROR storing LTM doc '{doc['title']}': {exc}")

        # ------------------------------------------------------------------ #
        # Section: Async Hybrid Search
        # ------------------------------------------------------------------ #
        print("\n" + "=" * 60)
        print("  Section: Async Hybrid Search")
        print("=" * 60)

        query = "capital city France population"
        print(f"  Query: \"{query}\"")

        try:
            results = await client.search_hybrid(
                query=query,
                user_id="async-user",
                limit=5,
            )

            # Handle both list and dict response shapes
            items = results if isinstance(results, list) else results.get("results", [])
            print(f"  Found {len(items)} result(s):")
            for i, item in enumerate(items[:5], 1):
                content = item.get("content", item.get("summary", ""))[:80]
                score = item.get("score", 0.0)
                layer = item.get("sourceLayer", item.get("layer", "?"))
                print(f"    [{i}] score={score:.3f} layer={layer}  {content!r}")
        except Exception as exc:
            print(f"  ERROR during hybrid search: {exc}")

        # ------------------------------------------------------------------ #
        # Section: Async Remember/Forget Lifecycle
        # ------------------------------------------------------------------ #
        print("\n" + "=" * 60)
        print("  Section: Async Remember/Forget Lifecycle")
        print("=" * 60)

        try:
            print("  Storing a temporary LTM entry via remember()...")
            remember_result = await client.remember(
                content="Temporary note: async demo running at " + str(time.time()),
                user_id="async-user",
                agent_id="async-agent",
                layer="ltm",
                metadata={
                    "sourceId": "async-demo-temp",
                    "sourceType": "user_input",
                    "title": "Async Demo Temp Note",
                },
            )
            mem_id = remember_result.get(
                "entryId", remember_result.get("entry_id", None)
            )
            print(f"  Remembered: id={mem_id}")

            if mem_id:
                print(f"  Forgetting memory id={mem_id}...")
                try:
                    forget_result = await client.forget(
                        memory_id=str(mem_id), layer="ltm"
                    )
                    print(f"  Forget result: {forget_result}")
                except Exception as exc:
                    # 404 is acceptable — memory may not be searchable yet
                    print(f"  Forget returned: {exc} (may be expected)")
            else:
                print("  No entry_id returned — skipping forget step")

        except Exception as exc:
            print(f"  ERROR in remember/forget lifecycle: {exc}")

    # ------------------------------------------------------------------ #
    # Summary
    # ------------------------------------------------------------------ #
    total_elapsed = time.perf_counter() - total_start
    print("\n" + "=" * 60)
    print(f"  Demo complete.  Total wall-clock time: {total_elapsed:.3f}s")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())
