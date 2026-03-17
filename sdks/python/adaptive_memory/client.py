"""
Adaptive Memory Client

Provides a simple client for interacting with the Adaptive Memory System.
"""

import json
from typing import Optional, List, Dict, Any
from urllib.parse import urljoin

import requests


class MemoryClient:
    """
    Synchronous client for Adaptive Memory System.

    Example:
        >>> client = MemoryClient("http://localhost:8008")
        >>> client.store_stm("user1", "agent1", "Hello, world!")
        >>> results = client.search_ltm("greeting")
    """

    def __init__(
        self,
        base_url: str = "http://localhost:8008",
        api_key: Optional[str] = None,
        timeout: int = 30,
    ):
        """
        Initialize the memory client.

        Args:
            base_url: Base URL of the Adaptive Memory API
            api_key: Optional API key for authentication
            timeout: Request timeout in seconds
        """
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.timeout = timeout
        self._session = requests.Session()

        if api_key:
            self._session.headers.update({"Authorization": f"Bearer {api_key}"})

    def _request(
        self,
        method: str,
        path: str,
        json: Optional[Dict[str, Any]] = None,
        params: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Make an HTTP request."""
        url = urljoin(self.base_url + "/", path.lstrip("/"))

        response = self._session.request(
            method=method,
            url=url,
            json=json,
            params=params,
            timeout=self.timeout,
        )
        response.raise_for_status()
        return response.json()

    # === MCP Tools ===

    def initialize_mcp(self) -> Dict[str, Any]:
        """Initialize MCP connection."""
        return self._request("POST", "api/mcp/initialize")

    def list_mcp_tools(self) -> Dict[str, Any]:
        """List available MCP tools."""
        return self._request("GET", "api/mcp/tools")

    def call_mcp_tool(
        self,
        tool_name: str,
        arguments: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Call an MCP tool."""
        return self._request(
            "POST",
            "api/mcp/tools/call",
            json={"name": tool_name, "arguments": arguments or {}},
        )

    # === Memory Write ===

    def store_stm(
        self,
        user_id: str,
        agent_id: str,
        content: str,
        session_type: str = "default",
        role: str = "user",
    ) -> Dict[str, Any]:
        """
        Store content in Short-Term Memory.

        Args:
            user_id: User identifier
            agent_id: Agent identifier
            content: Content to store
            session_type: Type of session
            role: Role of the message sender

        Returns:
            Dict with session_id and message_id
        """
        return self._request(
            "POST",
            "api/v1/memory/storage/stm",
            json={
                "userId": user_id,
                "agentId": agent_id,
                "sessionType": session_type,
                "role": role,
                "content": content,
            },
        )

    def store_ltm(
        self,
        source_id: str,
        source_type: str,
        content: str,
        title: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Store content in Long-Term Memory.

        Args:
            source_id: Source identifier
            source_type: Type of source (document, api, database, web, user_input)
            content: Content to store
            title: Optional title

        Returns:
            Dict with entry_id
        """
        return self._request(
            "POST",
            "api/v1/memory/storage/ltm",
            json={
                "sourceId": source_id,
                "sourceType": source_type,
                "content": content,
                "title": title,
            },
        )

    # === Memory Search ===

    def search_stm(
        self,
        query: str,
        user_id: Optional[str] = None,
        limit: int = 10,
    ) -> Dict[str, Any]:
        """Search in Short-Term Memory."""
        return self.call_mcp_tool(
            "memory_search",
            {"query": query, "layer": "stm", "user_id": user_id, "limit": limit},
        )

    def search_ltm(
        self,
        query: str,
        user_id: Optional[str] = None,
        limit: int = 10,
    ) -> Dict[str, Any]:
        """Search in Long-Term Memory."""
        return self.call_mcp_tool(
            "memory_search",
            {"query": query, "layer": "ltm", "user_id": user_id, "limit": limit},
        )

    def search_hybrid(
        self,
        query: str,
        user_id: Optional[str] = None,
        limit: int = 10,
    ) -> Dict[str, Any]:
        """Perform hybrid search (semantic + keyword)."""
        return self._request(
            "POST",
            "api/v1/memory/search/hybrid",
            json={"query": query, "userId": user_id, "limit": limit},
        )

    # === Memory List ===

    def list_sessions(
        self,
        user_id: Optional[str] = None,
        limit: int = 20,
    ) -> Dict[str, Any]:
        """List STM sessions."""
        return self._request(
            "GET",
            "api/v1/memory/storage/sessions",
            params={"userId": user_id, "limit": limit} if user_id else {"limit": limit},
        )

    def list_ltm_entries(
        self,
        limit: int = 20,
        offset: int = 0,
    ) -> Dict[str, Any]:
        """List LTM entries."""
        return self.call_mcp_tool(
            "memory_list", {"layer": "ltm", "limit": limit, "offset": offset}
        )

    # === Memory Recall ===

    def recall_session(
        self,
        session_id: str,
        limit: int = 10,
    ) -> Dict[str, Any]:
        """Recall memories from a specific session."""
        return self.call_mcp_tool(
            "memory_recall", {"session_id": session_id, "limit": limit}
        )

    # === Adaptive Memory ===

    def select_memory_config(
        self,
        task_description: str,
        complexity: str = "medium",
        modality: str = "text",
    ) -> Dict[str, Any]:
        """Select optimal memory configuration for a task."""
        return self._request(
            "POST",
            "api/v1/memory/adaptive/select",
            json={
                "taskDescription": task_description,
                "complexity": complexity,
                "modality": modality,
            },
        )

    def analyze_task(self, task_description: str) -> Dict[str, Any]:
        """Analyze task characteristics."""
        return self._request(
            "POST",
            "api/v1/memory/analyzer/task-characteristics",
            json={"taskDescription": task_description},
        )

    # === Health Check ===

    def health_check(self) -> Dict[str, Any]:
        """Check API health status."""
        return self._request("GET", "api/v1/memory/health")

    def close(self):
        """Close the client session."""
        self._session.close()


class AsyncMemoryClient:
    """
    Asynchronous client for Adaptive Memory System.

    Example:
        >>> import asyncio
        >>> async def main():
        ...     client = AsyncMemoryClient("http://localhost:8008")
        ...     await client.store_stm("user1", "agent1", "Hello!")
        ...     await client.close()
    """

    def __init__(
        self,
        base_url: str = "http://localhost:8008",
        api_key: Optional[str] = None,
        timeout: int = 30,
    ):
        import aiohttp

        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.timeout = aiohttp.ClientTimeout(total=timeout)
        self._client: Optional[aiohttp.ClientSession] = None

    async def _get_client(self) -> aiohttp.ClientSession:
        if self._client is None or self._client.closed:
            headers = {}
            if self.api_key:
                headers["Authorization"] = f"Bearer {self.api_key}"
            self._client = aiohttp.ClientSession(headers=headers, timeout=self.timeout)
        return self._client

    async def _request(
        self,
        method: str,
        path: str,
        json: Optional[Dict[str, Any]] = None,
        params: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        import aiohttp

        client = await self._get_client()
        url = f"{self.base_url}/{path.lstrip('/')}"

        async with client.request(
            method=method, url=url, json=json, params=params
        ) as response:
            response.raise_for_status()
            return await response.json()

    async def store_stm(
        self,
        user_id: str,
        agent_id: str,
        content: str,
        session_type: str = "default",
        role: str = "user",
    ) -> Dict[str, Any]:
        return await self._request(
            "POST",
            "api/v1/memory/storage/stm",
            json={
                "userId": user_id,
                "agentId": agent_id,
                "sessionType": session_type,
                "role": role,
                "content": content,
            },
        )

    async def store_ltm(
        self,
        source_id: str,
        source_type: str,
        content: str,
        title: Optional[str] = None,
    ) -> Dict[str, Any]:
        return await self._request(
            "POST",
            "api/v1/memory/storage/ltm",
            json={
                "sourceId": source_id,
                "sourceType": source_type,
                "content": content,
                "title": title,
            },
        )

    async def search_ltm(
        self,
        query: str,
        user_id: Optional[str] = None,
        limit: int = 10,
    ) -> Dict[str, Any]:
        return await self.call_mcp_tool(
            "memory_search",
            {"query": query, "layer": "ltm", "user_id": user_id, "limit": limit},
        )

    async def call_mcp_tool(
        self,
        tool_name: str,
        arguments: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return await self._request(
            "POST",
            "api/mcp/tools/call",
            json={"name": tool_name, "arguments": arguments or {}},
        )

    async def health_check(self) -> Dict[str, Any]:
        return await self._request("GET", "api/v1/memory/health")

    async def close(self):
        if self._client and not self._client.closed:
            await self._client.close()
