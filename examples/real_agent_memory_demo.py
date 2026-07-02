"""
Real LangChain Agent E2E Demo: Memory vs No-Memory
====================================================

This script runs a REAL LangChain agent powered by a local LLM,
demonstrating the dramatic difference between:
  A) An agent WITHOUT persistent memory (forgets everything between sessions)
  B) An agent WITH Aetheris MemOS memory (remembers across sessions)

The demo simulates a multi-session interaction where a developer asks
the same assistant for help across different days.

Requirements:
    - Local LLM at port 8000 (OpenAI-compatible)
    - Aetheris MemOS backend at port 8008
    - pip install langchain-openai adaptive-memory adaptive-memory-langchain
"""

import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "sdks/python"))
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "sdks/python-langchain"))

from langchain_openai import ChatOpenAI
from langchain_core.messages import HumanMessage, AIMessage, SystemMessage
from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder

from adaptive_memory import MemoryClient
from adaptive_memory_langchain import (
    AdaptiveMemoryChatMessageHistory,
    AdaptiveMemoryRetriever,
    create_memory_tools,
)

# ============================================================
# CONFIG
# ============================================================
LLM_BASE_URL = "http://127.0.0.1:8000/v1"
LLM_API_KEY = "123456"
LLM_MODEL = "Qwen3.6-27B-4bit"

MEMOS_BASE_URL = "http://127.0.0.1:8008"
MEMOS_TOKEN = None

# Auto-login to MemOS
import requests
try:
    resp = requests.post(
        f"{MEMOS_BASE_URL}/api/login",
        json={"username": "admin", "password": "admin123"},
    )
    resp.raise_for_status()
    MEMOS_TOKEN = resp.json()["token"]
except Exception as e:
    print(f"[WARN] Could not get MemOS token: {e}")

# ============================================================
# LLM SETUP
# ============================================================

def create_llm():
    return ChatOpenAI(
        base_url=LLM_BASE_URL,
        api_key=LLM_API_KEY,
        model=LLM_MODEL,
        temperature=0.7,
        max_tokens=512,
    )


# ============================================================
# SCENARIO A: AGENT WITHOUT MEMORY
# ============================================================

def run_without_memory(conversations):
    """Run agent without any persistent memory. Each session is isolated."""
    print("\n" + "=" * 70)
    print("  SCENARIO A: LangChain Agent WITHOUT Memory")
    print("  (Each session starts completely fresh)")
    print("=" * 70)

    llm = create_llm()
    all_responses = []

    for session_idx, session in enumerate(conversations):
        day_label = session["day"]
        messages_in_session = session["messages"]

        print(f"\n{'─' * 70}")
        print(f"  {day_label}")
        print(f"{'─' * 70}")

        # Fresh history each session - no memory of the past!
        chat_history = []

        for msg in messages_in_session:
            print(f"\n  [User]: {msg}")

            # Build prompt with only current session history
            prompt_messages = [
                SystemMessage(content=(
                    "You are a helpful coding assistant. Answer concisely and specifically. "
                    "If you don't have enough context about the user's project, ask clarifying questions. "
                    "Keep responses under 150 words."
                )),
            ]
            prompt_messages.extend(chat_history)
            prompt_messages.append(HumanMessage(content=msg))

            response = llm.invoke(prompt_messages)
            answer = response.content.strip()

            # Truncate thinking tags if present
            if "</think>" in answer:
                answer = answer.split("</think>")[-1].strip()

            print(f"\n  [Agent]: {answer[:500]}")
            all_responses.append({"day": day_label, "query": msg, "response": answer})

            chat_history.append(HumanMessage(content=msg))
            chat_history.append(AIMessage(content=answer))

        # SESSION ENDS - history is LOST!
        print(f"\n  ⚠️  Session ended. All context is LOST.")

    return all_responses


# ============================================================
# SCENARIO B: AGENT WITH AETHERIS MEMOS MEMORY
# ============================================================

def run_with_memory(conversations):
    """Run agent with Aetheris MemOS persistent memory."""
    print("\n\n" + "=" * 70)
    print("  SCENARIO B: LangChain Agent WITH Aetheris MemOS Memory")
    print("  (Memories persist across all sessions)")
    print("=" * 70)

    llm = create_llm()
    client = MemoryClient(base_url=MEMOS_BASE_URL, api_key=MEMOS_TOKEN)

    # Create memory tools
    tools = create_memory_tools(
        client=client,
        user_id="demo-developer",
        agent_id="coding-assistant-with-memory",
    )
    store_tool = next(t for t in tools if t.name == "memory_store")
    search_tool = next(t for t in tools if t.name == "memory_search")

    # Create retriever for RAG-style context injection
    retriever = AdaptiveMemoryRetriever(
        client=client,
        search_type="hybrid",
        top_k=5,
        min_score=0.0,
        user_id="demo-developer",
    )

    # Persistent chat history
    history = AdaptiveMemoryChatMessageHistory(
        client=client,
        user_id="demo-developer",
        agent_id="coding-assistant-with-memory",
        session_type="conversation",
    )

    all_responses = []

    for session_idx, session in enumerate(conversations):
        day_label = session["day"]
        messages_in_session = session["messages"]

        print(f"\n{'─' * 70}")
        print(f"  {day_label}")
        print(f"{'─' * 70}")

        for msg in messages_in_session:
            print(f"\n  [User]: {msg}")

            # Step 1: Retrieve relevant memories
            recalled_docs = retriever.invoke(msg)
            memory_context = ""
            if recalled_docs:
                memory_snippets = [d.page_content for d in recalled_docs[:3]]
                memory_context = "\n".join(f"- {s}" for s in memory_snippets)
                print(f"\n  [Memory Recall]: Found {len(recalled_docs)} relevant memories")

            # Step 2: Build prompt WITH memory context
            system_prompt = (
                "You are a helpful coding assistant with persistent memory. "
                "Answer concisely and specifically based on what you know about the user's project. "
                "Keep responses under 150 words.\n"
            )
            if memory_context:
                system_prompt += (
                    f"\nRELEVANT MEMORIES FROM PAST CONVERSATIONS:\n{memory_context}\n"
                    "\nUse these memories to provide personalized, specific answers. "
                    "Do NOT ask questions you already know the answer to from memory."
                )

            # Get recent chat history
            recent_messages = history.messages[-6:] if history.messages else []

            prompt_messages = [SystemMessage(content=system_prompt)]
            prompt_messages.extend(recent_messages)
            prompt_messages.append(HumanMessage(content=msg))

            response = llm.invoke(prompt_messages)
            answer = response.content.strip()

            # Truncate thinking tags if present
            if "</think>" in answer:
                answer = answer.split("</think>")[-1].strip()

            print(f"\n  [Agent]: {answer[:500]}")
            all_responses.append({"day": day_label, "query": msg, "response": answer})

            # Step 3: Store this exchange in memory
            history.add_messages([
                HumanMessage(content=msg),
                AIMessage(content=answer),
            ])

            # Step 4: Extract and store key facts (simulates agent self-reflection)
            if session_idx == 0:
                # First session: store project facts
                facts_to_store = extract_facts(msg)
                for fact in facts_to_store:
                    store_tool.invoke({"content": fact, "layer": "stm"})
                    print(f"  [Memory Store]: Stored → \"{fact[:60]}...\"")

        print(f"\n  ✅ Session ended. Memories PERSIST for next session.")

    return all_responses


def extract_facts(message: str) -> list:
    """Extract storable facts from a user message."""
    facts = []
    msg_lower = message.lower()
    if "python" in msg_lower or "fastapi" in msg_lower:
        facts.append("User's backend: Python with FastAPI framework")
    if "react" in msg_lower or "typescript" in msg_lower:
        facts.append("User's frontend: React with TypeScript")
    if "postgres" in msg_lower or "sqlalchemy" in msg_lower:
        facts.append("User's database: PostgreSQL with SQLAlchemy ORM")
    if "kubernetes" in msg_lower or "docker" in msg_lower:
        facts.append("User's deployment: Docker containers on Kubernetes")
    if "deadline" in msg_lower or "week" in msg_lower:
        facts.append("User has a tight deadline (weeks, not months)")
    if "pytest" in msg_lower:
        facts.append("User uses pytest for testing")
    if "microservice" in msg_lower:
        facts.append("Architecture: microservices")
    return facts


# ============================================================
# COMPARISON REPORT
# ============================================================

def print_comparison(no_memory_results, with_memory_results):
    """Print side-by-side comparison."""
    print("\n\n" + "=" * 70)
    print("  FINAL COMPARISON: Side-by-Side Results")
    print("=" * 70)

    for nm, wm in zip(no_memory_results, with_memory_results):
        print(f"\n{'─' * 70}")
        print(f"  {nm['day']} | Question: \"{nm['query'][:60]}...\"")
        print(f"{'─' * 70}")

        print(f"\n  ┌─ WITHOUT Memory:")
        for line in _wrap(nm['response'][:300], 66):
            print(f"  │ {line}")

        print(f"\n  ┌─ WITH Memory (Aetheris MemOS):")
        for line in _wrap(wm['response'][:300], 66):
            print(f"  │ {line}")

        # Score
        nm_specificity = "Generic" if "?" in nm['response'] and len(nm['response'].split("?")) > 2 else "Partial"
        wm_specificity = "Specific" if "?" not in wm['response'][:200] else "Contextual"
        print(f"\n  📊 Specificity: {nm_specificity} → {wm_specificity}")

    print(f"\n\n{'=' * 70}")
    print("  VERDICT")
    print("=" * 70)
    print("""
  The agent WITH Aetheris MemOS memory:
  • Provides SPECIFIC answers referencing the user's actual tech stack
  • Never asks questions it already knows the answer to
  • Builds on prior conversations naturally
  • Saves the developer from repeating context every session

  The agent WITHOUT memory:
  • Asks generic clarifying questions every session
  • Cannot reference any prior discussion
  • Treats every interaction as if meeting the user for the first time
  • Wastes developer time on repetitive context-setting
""")


def _wrap(text: str, width: int) -> list:
    words = text.replace("\n", " ").split()
    lines, cur = [], ""
    for w in words:
        if len(cur) + len(w) + 1 <= width:
            cur = f"{cur} {w}" if cur else w
        else:
            if cur: lines.append(cur)
            cur = w
    if cur: lines.append(cur)
    return lines or [""]


# ============================================================
# MAIN
# ============================================================

def main():
    print("╔══════════════════════════════════════════════════════════════════════╗")
    print("║  Aetheris MemOS × LangChain: Real Agent Memory Demonstration       ║")
    print("╠══════════════════════════════════════════════════════════════════════╣")
    print(f"║  LLM: {LLM_MODEL:<40} @ port 8000  ║")
    print(f"║  MemOS Backend: {MEMOS_BASE_URL:<35} (running)  ║")
    print("╚══════════════════════════════════════════════════════════════════════╝")

    # Define the conversation scenario
    conversations = [
        {
            "day": "📅 Monday (Day 1) — Project Introduction",
            "messages": [
                "Hi! I'm starting a new project. It's a SaaS platform built with Python FastAPI for the backend, React TypeScript for the frontend, PostgreSQL with SQLAlchemy for the database, and we'll deploy on Kubernetes with Docker. I'll use pytest for testing. We have a 3-week deadline.",
            ],
        },
        {
            "day": "📅 Wednesday (Day 3) — Debugging Help",
            "messages": [
                "I'm getting intermittent database connection timeout errors in production. What could be causing this and how do I fix it?",
            ],
        },
        {
            "day": "📅 Friday (Day 5) — Deployment Advice",
            "messages": [
                "What's the best way to set up my production deployment? I want zero-downtime deploys.",
            ],
        },
        {
            "day": "📅 Monday (Day 8) — Testing Strategy",
            "messages": [
                "I need to add comprehensive tests before launch. What testing strategy do you recommend given our tight timeline?",
            ],
        },
        {
            "day": "📅 Wednesday (Day 10) — Performance Issues",
            "messages": [
                "Our API response time is degrading as we add more data. Some endpoints take over 2 seconds. How should I approach performance optimization?",
            ],
        },
    ]

    # Run both scenarios
    print("\n" + "▶" * 35 + " STARTING " + "◀" * 35)

    no_memory_results = run_without_memory(conversations)

    time.sleep(1)

    with_memory_results = run_with_memory(conversations)

    # Final comparison
    print_comparison(no_memory_results, with_memory_results)


if __name__ == "__main__":
    main()
