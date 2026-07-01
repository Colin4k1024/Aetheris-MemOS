"""Minimal agent memory contract demo.

Run after starting the backend and logging in / providing a bearer token:

    python examples/agent_memory_demo.py

Environment:
    AETHERIS_BASE_URL  default: http://localhost:8008
    AETHERIS_TOKEN     optional bearer token
"""

import os

from adaptive_memory import MemoryClient


def main() -> None:
    client = MemoryClient(
        base_url=os.getenv("AETHERIS_BASE_URL", "http://localhost:8008"),
        api_key=os.getenv("AETHERIS_TOKEN"),
    )

    user_id = "demo-user"
    agent_id = "demo-agent"
    session_id = "demo-session"

    stored = client.remember(
        content="User prefers concise technical answers in Chinese.",
        user_id=user_id,
        agent_id=agent_id,
        session_id=session_id,
        layer="stm",
        metadata={"source": "agent_memory_demo"},
    )
    print("remember:", stored)

    recalled = client.recall(
        query="How should this agent answer the user?",
        user_id=user_id,
        agent_id=agent_id,
        session_id=stored.get("sessionId") or session_id,
        limit=5,
    )
    print("recall:", recalled)

    searched = client.search(
        query="concise technical answers Chinese",
        layer="hybrid",
        user_id=user_id,
        limit=5,
    )
    print("search:", searched)

    memory_id = stored.get("messageId") or stored.get("entryId") or "unknown"
    feedback = client.feedback(
        memory_id=memory_id,
        useful=True,
        query="concise technical answers Chinese",
        metadata={"demo": True},
    )
    print("feedback:", feedback)


if __name__ == "__main__":
    main()
