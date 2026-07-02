"""Example: Conversational Chain with Adaptive Memory Chat History.

This demo shows how to use AdaptiveMemoryChatMessageHistory as the
persistent message store for a LangChain conversational chain.

Requirements:
    pip install adaptive-memory adaptive-memory-langchain langchain-openai

Usage:
    export AETHERIS_BASE_URL=http://localhost:8008
    export AETHERIS_TOKEN=your-token
    export OPENAI_API_KEY=your-openai-key
    python chat_with_history.py
"""

import os

from adaptive_memory import MemoryClient
from adaptive_memory_langchain import AdaptiveMemoryChatMessageHistory
from langchain_core.messages import AIMessage, HumanMessage

# Initialize memory client
client = MemoryClient(
    base_url=os.getenv("AETHERIS_BASE_URL", "http://localhost:8008"),
    api_key=os.getenv("AETHERIS_TOKEN"),
)

# Create chat history backed by MemOS STM
history = AdaptiveMemoryChatMessageHistory(
    client=client,
    user_id="demo-user",
    agent_id="demo-agent",
    session_type="conversation",
)

print("AdaptiveMemoryChatMessageHistory initialized")
print(f"  user_id: {history.user_id}")
print(f"  agent_id: {history.agent_id}")
print(f"  session_id: {history.session_id or '(will be created on first write)'}")

# --- Add messages ---
print("\n--- Adding messages ---")
history.add_messages([
    HumanMessage(content="Hi, I'm working on a Python ML project"),
    AIMessage(content="Great! What kind of ML are you working on?"),
    HumanMessage(content="Computer vision with PyTorch"),
])
print(f"  Session created: {history.session_id}")

# --- Read messages back ---
print("\n--- Reading messages ---")
messages = history.messages
for msg in messages:
    role = "Human" if isinstance(msg, HumanMessage) else "AI"
    print(f"  [{role}] {msg.content}")

# --- Use with RunnableWithMessageHistory (uncomment with OpenAI key) ---
#
# from langchain_openai import ChatOpenAI
# from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder
# from langchain_core.runnables.history import RunnableWithMessageHistory
#
# llm = ChatOpenAI(model="gpt-4o-mini", temperature=0)
#
# prompt = ChatPromptTemplate.from_messages([
#     ("system", "You are a helpful coding assistant."),
#     MessagesPlaceholder(variable_name="history"),
#     ("human", "{input}"),
# ])
#
# chain = prompt | llm
#
# def get_session_history(session_id: str):
#     return AdaptiveMemoryChatMessageHistory(
#         client=client,
#         user_id="demo-user",
#         agent_id="demo-agent",
#         session_id=session_id,
#     )
#
# with_history = RunnableWithMessageHistory(
#     chain,
#     get_session_history,
#     input_messages_key="input",
#     history_messages_key="history",
# )
#
# response = with_history.invoke(
#     {"input": "What ML framework am I using?"},
#     config={"configurable": {"session_id": history.session_id}},
# )
# print(f"\nAssistant: {response.content}")
