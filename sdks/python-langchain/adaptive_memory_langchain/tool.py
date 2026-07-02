"""LangChain BaseTool implementations for Aetheris MemOS memory operations."""

from __future__ import annotations

import json
from typing import Any, Dict, List, Optional, Type

from langchain_core.tools import BaseTool
from pydantic import BaseModel, Field

from adaptive_memory import MemoryClient

from ._utils import extract_search_results


# --- Input schemas for tool calling ---


class MemoryStoreInput(BaseModel):
    """Input for storing a memory."""

    content: str = Field(description="The text content to remember")
    layer: str = Field(
        default="stm",
        description="Memory layer: stm (short-term) or ltm (long-term)",
    )


class MemorySearchInput(BaseModel):
    """Input for searching memories."""

    query: str = Field(description="Search query to find relevant memories")
    limit: int = Field(default=5, description="Maximum number of results")


class MemoryForgetInput(BaseModel):
    """Input for forgetting a memory."""

    memory_id: str = Field(description="ID of the memory to forget")
    layer: str = Field(default="ltm", description="Layer the memory belongs to")


class AdaptiveMemoryInput(BaseModel):
    """Input for the unified adaptive memory tool."""

    action: str = Field(
        description="Action to perform: 'remember', 'search', or 'forget'"
    )
    content: Optional[str] = Field(
        default=None, description="Content to remember (required for 'remember')"
    )
    query: Optional[str] = Field(
        default=None, description="Search query (required for 'search')"
    )
    memory_id: Optional[str] = Field(
        default=None, description="Memory ID (required for 'forget')"
    )
    layer: str = Field(
        default="stm",
        description="Memory layer: stm, ltm, hybrid",
    )
    limit: int = Field(default=5, description="Max results for search")


# --- Tool implementations ---


class AdaptiveMemoryTool(BaseTool):
    """Unified tool for storing, searching, and forgetting agent memories.

    This single tool exposes multiple memory actions that LangChain agents
    can invoke based on the task at hand.

    Example:
        >>> from adaptive_memory import MemoryClient
        >>> from adaptive_memory_langchain import AdaptiveMemoryTool
        >>>
        >>> client = MemoryClient("http://localhost:8008", api_key="...")
        >>> tool = AdaptiveMemoryTool(
        ...     client=client, user_id="user-1", agent_id="agent-1"
        ... )
        >>> result = tool.invoke({"action": "remember", "content": "User likes Python"})
    """

    name: str = "adaptive_memory"
    description: str = (
        "Manage agent memories. Actions: "
        "'remember' (store content to memory), "
        "'search' (find relevant memories by query), "
        "'forget' (delete a memory by ID). "
        "Use 'remember' to store important facts. "
        "Use 'search' to retrieve relevant context before answering. "
        "Use 'forget' to remove outdated information."
    )
    args_schema: Type[BaseModel] = AdaptiveMemoryInput

    client: Any = Field(description="MemoryClient instance")
    user_id: str = Field(description="User identifier")
    agent_id: str = Field(description="Agent identifier")
    session_id: Optional[str] = Field(default=None, description="Session identifier")

    class Config:
        arbitrary_types_allowed = True

    def _run(
        self,
        action: str,
        content: Optional[str] = None,
        query: Optional[str] = None,
        memory_id: Optional[str] = None,
        layer: str = "stm",
        limit: int = 5,
        **kwargs: Any,
    ) -> str:
        """Execute a memory action."""
        action = action.lower().strip()

        if action == "remember":
            if not content:
                return json.dumps({"error": "content is required for 'remember'"})
            result = self.client.remember(
                content=content,
                user_id=self.user_id,
                agent_id=self.agent_id,
                session_id=self.session_id,
                layer=layer,
            )
            return json.dumps({"success": True, "result": result})

        if action == "search":
            if not query:
                return json.dumps({"error": "query is required for 'search'"})
            response = self.client.search(
                query=query,
                layer=layer if layer != "stm" else "hybrid",
                user_id=self.user_id,
                limit=limit,
            )
            results = extract_search_results(response)
            summaries = [
                {
                    "memory_id": r.get("memoryId", r.get("entry_id", "")),
                    "content": r.get("content", "")[:200],
                    "score": r.get("score", 0.0),
                }
                for r in results
            ]
            return json.dumps({"results": summaries})

        if action == "forget":
            if not memory_id:
                return json.dumps({"error": "memory_id is required for 'forget'"})
            result = self.client.forget(memory_id=memory_id, layer=layer)
            return json.dumps({"success": True, "result": result})

        return json.dumps({"error": f"Unknown action: {action}"})


# --- Split tools for agents that prefer focused tools ---


class MemoryStoreTool(BaseTool):
    """Tool for storing memories."""

    name: str = "memory_store"
    description: str = (
        "Store important information to memory for later retrieval. "
        "Use this when you learn something important about the user, task, "
        "or environment that should be remembered."
    )
    args_schema: Type[BaseModel] = MemoryStoreInput

    client: Any = Field(description="MemoryClient instance")
    user_id: str = Field(description="User identifier")
    agent_id: str = Field(description="Agent identifier")
    session_id: Optional[str] = Field(default=None)

    class Config:
        arbitrary_types_allowed = True

    def _run(self, content: str, layer: str = "stm", **kwargs: Any) -> str:
        result = self.client.remember(
            content=content,
            user_id=self.user_id,
            agent_id=self.agent_id,
            session_id=self.session_id,
            layer=layer,
        )
        return json.dumps({"success": True, "result": result})


class MemorySearchTool(BaseTool):
    """Tool for searching memories."""

    name: str = "memory_search"
    description: str = (
        "Search through stored memories to find relevant context. "
        "Use this before answering questions to check if you have "
        "relevant information stored."
    )
    args_schema: Type[BaseModel] = MemorySearchInput

    client: Any = Field(description="MemoryClient instance")
    user_id: Optional[str] = Field(default=None)

    class Config:
        arbitrary_types_allowed = True

    def _run(self, query: str, limit: int = 5, **kwargs: Any) -> str:
        response = self.client.search(
            query=query, layer="hybrid", user_id=self.user_id, limit=limit
        )
        results = extract_search_results(response)
        summaries = [
            {
                "memory_id": r.get("memoryId", r.get("entry_id", "")),
                "content": r.get("content", "")[:200],
                "score": r.get("score", 0.0),
            }
            for r in results
        ]
        return json.dumps({"results": summaries})


class MemoryForgetTool(BaseTool):
    """Tool for forgetting/deleting memories."""

    name: str = "memory_forget"
    description: str = (
        "Remove outdated or incorrect information from memory. "
        "Use this when you discover a stored memory is no longer accurate."
    )
    args_schema: Type[BaseModel] = MemoryForgetInput

    client: Any = Field(description="MemoryClient instance")

    class Config:
        arbitrary_types_allowed = True

    def _run(self, memory_id: str, layer: str = "ltm", **kwargs: Any) -> str:
        result = self.client.forget(memory_id=memory_id, layer=layer)
        return json.dumps({"success": True, "result": result})


def create_memory_tools(
    client: MemoryClient,
    user_id: str,
    agent_id: str,
    session_id: Optional[str] = None,
) -> List[BaseTool]:
    """Create a set of focused memory tools for a LangChain agent.

    Returns three tools: memory_store, memory_search, memory_forget.

    Args:
        client: Adaptive Memory client instance.
        user_id: User identifier.
        agent_id: Agent identifier.
        session_id: Optional session identifier.

    Returns:
        List of BaseTool instances ready for agent use.
    """
    return [
        MemoryStoreTool(
            client=client,
            user_id=user_id,
            agent_id=agent_id,
            session_id=session_id,
        ),
        MemorySearchTool(client=client, user_id=user_id),
        MemoryForgetTool(client=client),
    ]
