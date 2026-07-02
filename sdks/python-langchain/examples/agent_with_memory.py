"""Example: LangChain Agent with Adaptive Memory Tools.

This demo shows how to create a LangChain agent that can store, search,
and forget memories using Aetheris MemOS.

Requirements:
    pip install adaptive-memory adaptive-memory-langchain langchain-openai

Usage:
    export AETHERIS_BASE_URL=http://localhost:8008
    export AETHERIS_TOKEN=your-token
    export OPENAI_API_KEY=your-openai-key
    python agent_with_memory.py
"""

import os

from adaptive_memory import MemoryClient
from adaptive_memory_langchain import create_memory_tools

# Initialize the memory client
client = MemoryClient(
    base_url=os.getenv("AETHERIS_BASE_URL", "http://localhost:8008"),
    api_key=os.getenv("AETHERIS_TOKEN"),
)

# Create memory tools for the agent
tools = create_memory_tools(
    client=client,
    user_id="demo-user",
    agent_id="demo-agent",
)

print("Available tools:")
for tool in tools:
    print(f"  - {tool.name}: {tool.description[:60]}...")

# --- LangChain Agent Setup ---
# Uncomment below when running with a real OpenAI key:
#
# from langchain_openai import ChatOpenAI
# from langchain.agents import AgentExecutor, create_tool_calling_agent
# from langchain_core.prompts import ChatPromptTemplate
#
# llm = ChatOpenAI(model="gpt-4o-mini", temperature=0)
#
# prompt = ChatPromptTemplate.from_messages([
#     ("system", "You are a helpful assistant with persistent memory. "
#                "Use memory_store to remember important facts. "
#                "Use memory_search to check your memory before answering. "
#                "Use memory_forget to remove outdated information."),
#     ("human", "{input}"),
#     ("placeholder", "{agent_scratchpad}"),
# ])
#
# agent = create_tool_calling_agent(llm, tools, prompt)
# executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
#
# # Store a memory
# executor.invoke({"input": "Remember that my favorite language is Python"})
#
# # Recall from memory
# executor.invoke({"input": "What is my favorite programming language?"})

# --- Demo without LLM (direct tool invocation) ---
print("\n--- Direct tool demo ---")

store_tool = next(t for t in tools if t.name == "memory_store")
search_tool = next(t for t in tools if t.name == "memory_search")

# Store a memory
print("\nStoring memory...")
result = store_tool.invoke({"content": "User's favorite color is blue", "layer": "stm"})
print(f"  Result: {result}")

# Search memory
print("\nSearching memory...")
result = search_tool.invoke({"query": "favorite color", "limit": 3})
print(f"  Result: {result}")
