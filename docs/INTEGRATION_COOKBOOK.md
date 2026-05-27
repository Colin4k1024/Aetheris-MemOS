# Adaptive Memory System Integration Cookbook

This cookbook provides practical, runnable integration patterns for the Adaptive Memory System. All examples are designed for immediate use with minimal configuration.

---

## Table of Contents

1. [Quick Start Patterns](#1-quick-start-patterns)
2. [Python SDK Integration](#2-python-sdk-integration)
3. [JavaScript/TypeScript Integration](#3-javascripttypescript-integration)
4. [LLM Agent Integration Patterns](#4-llm-agent-integration-patterns)
5. [WebSocket Real-time Updates](#5-websocket-real-time-updates)
6. [Webhook/Callback Patterns](#6-webhookcallback-patterns)
7. [Testing Your Integration](#7-testing-your-integration)
8. [Troubleshooting](#8-troubleshooting)

---

## 1. Quick Start Patterns

### 1.1 Minimal Docker Compose Setup

For rapid deployment, use this single-file `docker-compose.yml`:

```yaml
# docker-compose.yml - Minimal setup for integration testing
version: '3.8'

services:
  backend:
    image: ghcr.io/adaptive-memory/backend:latest
    ports:
      - "8008:8008"
    environment:
      - DATABASE_URL=postgresql://memory:memory@postgres:5432/memory
      - QDRANT_URL=http://qdrant:6334
      - NEO4J_URI=bolt://neo4j:7687
      - NEO4J_USER=neo4j
      - NEO4J_PASSWORD=password
      - JWT_SECRET=your-secret-key-change-in-production
    depends_on:
      postgres:
        condition: service_healthy
      qdrant:
        condition: service_started
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8008/api/memory/health"]
      interval: 10s
      timeout: 5s
      retries: 5

  postgres:
    image: pgvector/pgvector:pg16
    ports:
      - "5432:5432"
    environment:
      - POSTGRES_USER=memory
      - POSTGRES_PASSWORD=memory
      - POSTGRES_DB=memory
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U memory -d memory"]
      interval: 5s
      timeout: 5s
      retries: 5

  qdrant:
    image: qdrant/qdrant:latest
    ports:
      - "6333:6333"
      - "6334:6334"
    volumes:
      - qdrant_data:/qdrant/storage

volumes:
  postgres_data:
  qdrant_data:
```

### 1.2 Health Check Verification

After starting services, verify system health:

```bash
# Basic health check
curl http://localhost:8008/api/memory/health

# Expected response:
# {
#   "status": "healthy",
#   "services": {
#     "database": "connected",
#     "qdrant": "connected",
#     "neo4j": "not_configured"
#   }
# }

# Full system status with resources
curl http://localhost:8008/api/v1/memory/monitor/resources

# Adaptive memory status
curl http://localhost:8008/api/v1/memory/adaptive/status

# Check authentication endpoint
curl -X POST http://localhost:8008/api/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}'

# Expected token response:
# {"token": "eyJhbGci..."}
```

### 1.3 Quick Test Script

```bash
#!/bin/bash
# quick_test.sh - Verify basic API connectivity

BASE_URL="${BASE_URL:-http://localhost:8008}"

echo "=== Adaptive Memory System Quick Test ==="
echo "Base URL: $BASE_URL"
echo ""

# 1. Health check
echo "1. Health Check..."
curl -s "$BASE_URL/api/memory/health" | jq '.status' && echo "OK" || echo "FAILED"

# 2. Get auth token
echo ""
echo "2. Authentication..."
TOKEN=$(curl -s -X POST "$BASE_URL/api/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' | jq -r '.token')

if [ "$TOKEN" = "null" ] || [ -z "$TOKEN" ]; then
  echo "AUTH FAILED - check credentials"
  exit 1
fi
echo "Token obtained: ${TOKEN:0:20}..."

# 3. Store STM
echo ""
echo "3. Store Short-Term Memory..."
curl -s -X POST "$BASE_URL/api/v1/memory/storage/stm" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "userId": "test_user",
    "agentId": "test_agent",
    "sessionType": "conversation",
    "role": "user",
    "content": "Hello, this is a test message",
    "maxContextLength": 4096,
    "retentionHours": 24
  }' | jq '.sessionId'

# 4. Search LTM
echo ""
echo "4. Search Long-Term Memory..."
curl -s -X POST "$BASE_URL/api/v1/memory/search/ltm" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"query": "test", "topK": 3}' | jq '.results'

echo ""
echo "=== Quick Test Complete ==="
```

---

## 2. Python SDK Integration

### 2.1 Complete Python Client Class

```python
"""
adaptive_memory_client.py
Complete Python SDK for Adaptive Memory System
"""

import requests
import json
import time
from typing import Optional, Dict, List, Any
from contextlib import contextmanager
from dataclasses import dataclass, field
from enum import Enum


class SessionType(Enum):
    CONVERSATION = "conversation"
    TASK = "task"
    QUERY = "query"
    BATCH = "batch"


@dataclass
class MemoryWeights:
    """Memory component weights for adaptive selection."""
    stm: float = 0.3
    ltm: float = 0.4
    kg: float = 0.3
    mm: float = 0.0


@dataclass
class SearchResult:
    """Represents a single search result."""
    entry_id: str
    score: float
    title: Optional[str] = None
    content: Optional[str] = None
    metadata: Dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, data: Dict) -> "SearchResult":
        return cls(
            entry_id=data.get("entryId", data.get("id", "")),
            score=data.get("score", 0.0),
            title=data.get("title"),
            content=data.get("content"),
            metadata=data.get("metadata", {})
        )


class AdaptiveMemoryClient:
    """
    Python client for Adaptive Memory System API.

    Handles authentication, session management, and all memory operations.
    """

    def __init__(
        self,
        base_url: str = "http://localhost:8008",
        username: str = "admin",
        password: str = "admin123",
        tenant_id: Optional[str] = None,
        timeout: int = 30
    ):
        self.base_url = base_url.rstrip("/")
        self.tenant_id = tenant_id
        self.timeout = timeout
        self._token: Optional[str] = None
        self._session = requests.Session()
        self._session.headers.update({"Content-Type": "application/json"})

        # Authenticate on initialization
        self.authenticate(username, password)

    def authenticate(self, username: str, password: str) -> str:
        """Authenticate and store JWT token."""
        response = self._session.post(
            f"{self.base_url}/api/login",
            json={"username": username, "password": password},
            timeout=self.timeout
        )
        response.raise_for_status()
        self._token = response.json().get("token")
        self._session.headers.update({
            "Authorization": f"Bearer {self._token}"
        })
        return self._token

    def _make_request(
        self,
        method: str,
        endpoint: str,
        data: Optional[Dict] = None,
        params: Optional[Dict] = None
    ) -> Dict:
        """Internal method for making API requests."""
        url = f"{self.base_url}{endpoint}"

        # Add tenant header if specified
        headers = {}
        if self.tenant_id:
            headers["X-Tenant-ID"] = self.tenant_id

        response = self._session.request(
            method=method,
            url=url,
            json=data,
            params=params,
            headers=headers,
            timeout=self.timeout
        )

        # Handle common errors
        if response.status_code == 401:
            raise AuthenticationError("Token expired or invalid")
        elif response.status_code == 429:
            raise RateLimitError("Rate limit exceeded")
        elif response.status_code >= 400:
            raise APIError(f"API error {response.status_code}: {response.text}")

        return response.json()

    # ==================== Health & Status ====================

    def health_check(self) -> Dict:
        """Check system health status."""
        return self._make_request("GET", "/api/memory/health")

    def get_status(self) -> Dict:
        """Get memory system status."""
        return self._make_request("GET", "/api/v1/memory/adaptive/status")

    def get_resources(self) -> Dict:
        """Get resource utilization."""
        return self._make_request("GET", "/api/v1/memory/monitor/resources")

    # ==================== Adaptive Memory Selection ====================

    def select_memory_config(
        self,
        task_context: str,
        expected_complexity: str = "medium",
        reasoning_depth: str = "medium",
        **kwargs
    ) -> Dict:
        """
        Select optimal memory configuration for a task.

        Args:
            task_context: Natural language description of the task
            expected_complexity: "low", "medium", or "high"
            reasoning_depth: "shallow", "medium", or "deep"
            **kwargs: Additional parameters for the selection algorithm

        Returns:
            Dict with selection weights and predictions
        """
        return self._make_request(
            "POST",
            "/api/v1/memory/adaptive/select",
            data={
                "task_context": task_context,
                "expected_complexity": expected_complexity,
                "reasoning_depth": reasoning_depth,
                **kwargs
            }
        )

    def analyze_task(self, task_context: str, **kwargs) -> Dict:
        """Analyze task characteristics."""
        return self._make_request(
            "POST",
            "/api/v1/memory/analyzer/task-characteristics",
            data={"task_context": task_context, **kwargs}
        )

    def predict_performance(self, memory_weights: MemoryWeights) -> Dict:
        """Predict performance for given memory weights."""
        return self._make_request(
            "POST",
            "/api/v1/memory/predictor/performance",
            data={
                "memory_config": {
                    "stmWeight": memory_weights.stm,
                    "ltmWeight": memory_weights.ltm,
                    "kgWeight": memory_weights.kg,
                    "mmWeight": memory_weights.mm
                }
            }
        )

    # ==================== Short-Term Memory (STM) ====================

    def store_stm(
        self,
        user_id: str,
        agent_id: str,
        content: str,
        role: str = "user",
        session_type: SessionType = SessionType.CONVERSATION,
        max_context_length: int = 4096,
        retention_hours: int = 24,
        metadata: Optional[Dict] = None
    ) -> Dict:
        """
        Store a message in short-term memory.

        Args:
            user_id: User identifier
            agent_id: Agent identifier
            content: Message content
            role: "user" or "assistant"
            session_type: Type of session
            max_context_length: Maximum context window size
            retention_hours: How long to retain (1-168)
            metadata: Optional additional metadata

        Returns:
            Dict with session_id and message_id
        """
        return self._make_request(
            "POST",
            "/api/v1/memory/storage/stm",
            data={
                "userId": user_id,
                "agentId": agent_id,
                "sessionType": session_type.value,
                "role": role,
                "content": content,
                "maxContextLength": max_context_length,
                "retentionHours": retention_hours,
                "metadata": metadata or {}
            }
        )

    def get_session_messages(
        self,
        session_id: str,
        limit: int = 100
    ) -> List[Dict]:
        """Get all messages in a session."""
        response = self._make_request(
            "GET",
            f"/api/v1/memory/storage/stm/{session_id}",
            params={"limit": limit}
        )
        return response.get("messages", [])

    def list_sessions(
        self,
        user_id: Optional[str] = None,
        status: Optional[str] = None,
        limit: int = 50
    ) -> List[Dict]:
        """List all sessions."""
        params = {"limit": limit}
        if user_id:
            params["user_id"] = user_id
        if status:
            params["status"] = status

        response = self._make_request(
            "GET",
            "/api/v1/memory/storage/sessions",
            params=params
        )
        return response.get("sessions", [])

    # ==================== Long-Term Memory (LTM) ====================

    def store_ltm(
        self,
        source_id: str,
        content: str,
        title: Optional[str] = None,
        source_type: str = "document",
        category: Optional[str] = None,
        domain: Optional[str] = None,
        metadata: Optional[Dict] = None
    ) -> str:
        """
        Store an entry in long-term memory.

        Args:
            source_id: Unique identifier for the source
            content: Main content to store
            title: Optional title
            source_type: Type of source (document, conversation, etc.)
            category: Optional category
            domain: Optional domain
            metadata: Optional additional metadata

        Returns:
            entry_id of the stored entry
        """
        response = self._make_request(
            "POST",
            "/api/v1/memory/storage/ltm",
            data={
                "sourceId": source_id,
                "sourceType": source_type,
                "title": title,
                "content": content,
                "category": category,
                "domain": domain,
                "metadata": metadata or {}
            }
        )
        return response.get("entryId", response.get("id", ""))

    def batch_store_ltm(self, entries: List[Dict]) -> List[str]:
        """Batch store multiple LTM entries."""
        response = self._make_request(
            "POST",
            "/api/v1/memory/storage/batch-ltm",
            data={"entries": entries}
        )
        return response.get("entryIds", [])

    def search_ltm(
        self,
        query: str,
        top_k: int = 5,
        min_score: float = 0.5,
        enable_rerank: bool = True,
        category: Optional[str] = None
    ) -> List[SearchResult]:
        """
        Search long-term memory with semantic similarity.

        Args:
            query: Search query text
            top_k: Number of results to return
            min_score: Minimum similarity score (0-1)
            enable_rerank: Whether to enable reranking
            category: Optional category filter

        Returns:
            List of SearchResult objects
        """
        data = {
            "query": query,
            "topK": top_k,
            "minScore": min_score,
            "enableRerank": enable_rerank
        }
        if category:
            data["category"] = category

        response = self._make_request(
            "POST",
            "/api/v1/memory/search/ltm",
            data=data
        )

        results = []
        for item in response.get("results", []):
            results.append(SearchResult.from_dict(item))

        return results

    def search_hybrid(
        self,
        query: str,
        top_k: int = 5,
        keyword_weight: float = 0.3,
        vector_weight: float = 0.7
    ) -> List[SearchResult]:
        """Hybrid search combining keyword and vector similarity."""
        response = self._make_request(
            "POST",
            "/api/v1/memory/search/hybrid",
            data={
                "query": query,
                "topK": top_k,
                "keywordWeight": keyword_weight,
                "vectorWeight": vector_weight
            }
        )

        results = []
        for item in response.get("results", []):
            results.append(SearchResult.from_dict(item))

        return results

    # ==================== Knowledge Graph ====================

    def create_entity(
        self,
        entity_name: str,
        entity_type: str,
        description: Optional[str] = None,
        aliases: Optional[List[str]] = None
    ) -> str:
        """Create a knowledge graph entity."""
        response = self._make_request(
            "POST",
            "/api/kg/entities",
            data={
                "entityName": entity_name,
                "entityType": entity_type,
                "description": description,
                "aliases": aliases or []
            }
        )
        return response.get("entityId", "")

    def create_relation(
        self,
        source_entity_id: str,
        target_entity_id: str,
        relation_type: str,
        relation_name: str,
        weight: float = 1.0
    ) -> str:
        """Create a relationship between entities."""
        response = self._make_request(
            "POST",
            "/api/kg/relations",
            data={
                "sourceEntityId": source_entity_id,
                "targetEntityId": target_entity_id,
                "relationType": relation_type,
                "relationName": relation_name,
                "weight": weight
            }
        )
        return response.get("relationId", "")

    def search_entities(
        self,
        entity_name: str,
        top_k: int = 10
    ) -> List[Dict]:
        """Search for entities by name."""
        response = self._make_request(
            "POST",
            "/api/kg/search",
            data={"entityName": entity_name, "topK": top_k}
        )
        return response.get("entities", [])

    # ==================== Memory Transfer ====================

    def transfer_stm_to_ltm(self, session_id: str) -> Dict:
        """Transfer session memories from STM to LTM."""
        return self._make_request(
            "POST",
            "/api/v1/memory/storage/transfer",
            data={"sessionId": session_id}
        )

    # ==================== Weight Management ====================

    def adjust_weights(
        self,
        component: str,
        new_weight: float,
        reason: Optional[str] = None
    ) -> Dict:
        """Adjust memory component weights."""
        return self._make_request(
            "POST",
            "/api/v1/memory/weights/adjust",
            data={
                "component": component,
                "weight": new_weight,
                "reason": reason
            }
        )

    def get_weight_history(
        self,
        start_time: Optional[str] = None,
        end_time: Optional[str] = None,
        limit: int = 100
    ) -> List[Dict]:
        """Get weight adjustment history."""
        params = {"limit": limit}
        if start_time:
            params["start_time"] = start_time
        if end_time:
            params["end_time"] = end_time

        response = self._make_request(
            "GET",
            "/api/v1/memory/weights/history",
            params=params
        )
        return response.get("history", [])


class AuthenticationError(Exception):
    """Raised when authentication fails."""
    pass


class RateLimitError(Exception):
    """Raised when rate limit is exceeded."""
    pass


class APIError(Exception):
    """Raised for general API errors."""
    pass
```

### 2.2 Session Management Helper

```python
"""
session_manager.py
Session management helper for multi-turn conversations
"""

from typing import Optional, List, Dict, Callable
from adaptive_memory_client import AdaptiveMemoryClient, SessionType, SearchResult


class ConversationSession:
    """Manages a single conversation session with the memory system."""

    def __init__(
        self,
        client: AdaptiveMemoryClient,
        user_id: str,
        agent_id: str,
        session_type: SessionType = SessionType.CONVERSATION,
        max_context_length: int = 4096,
        retention_hours: int = 24
    ):
        self.client = client
        self.user_id = user_id
        self.agent_id = agent_id
        self.session_type = session_type
        self.max_context_length = max_context_length
        self.retention_hours = retention_hours
        self.session_id: Optional[str] = None
        self.message_count = 0

    def start(self) -> str:
        """Start a new session with an initial message."""
        result = self.client.store_stm(
            user_id=self.user_id,
            agent_id=self.agent_id,
            content="[Session started]",
            role="system",
            session_type=self.session_type,
            max_context_length=self.max_context_length,
            retention_hours=self.retention_hours
        )
        self.session_id = result.get("sessionId")
        return self.session_id

    def add_message(
        self,
        content: str,
        role: str = "user"
    ) -> Dict:
        """Add a message to the session."""
        if not self.session_id:
            self.start()

        result = self.client.store_stm(
            user_id=self.user_id,
            agent_id=self.agent_id,
            content=content,
            role=role,
            session_type=self.session_type,
            max_context_length=self.max_context_length,
            retention_hours=self.retention_hours
        )
        self.message_count += 1
        return result

    def add_user_message(self, content: str) -> Dict:
        """Add a user message."""
        return self.add_message(content, role="user")

    def add_assistant_message(self, content: str) -> Dict:
        """Add an assistant message."""
        return self.add_message(content, role="assistant")

    def get_context(self, limit: int = 50) -> List[Dict]:
        """Get the conversation context."""
        if not self.session_id:
            return []
        return self.client.get_session_messages(self.session_id, limit=limit)

    def close(self) -> Dict:
        """Close session and optionally transfer to LTM."""
        if not self.session_id:
            return {}

        result = self.client.transfer_stm_to_ltm(self.session_id)
        self.session_id = None
        return result


class SessionManager:
    """Manages multiple conversation sessions for an agent."""

    def __init__(self, client: AdaptiveMemoryClient):
        self.client = client
        self.sessions: Dict[str, ConversationSession] = {}

    def create_session(
        self,
        session_id: str,
        user_id: str,
        agent_id: str,
        **kwargs
    ) -> ConversationSession:
        """Create a new session."""
        session = ConversationSession(
            client=self.client,
            user_id=user_id,
            agent_id=agent_id,
            **kwargs
        )
        self.sessions[session_id] = session
        return session

    def get_session(self, session_id: str) -> Optional[ConversationSession]:
        """Get an existing session."""
        return self.sessions.get(session_id)

    def close_session(self, session_id: str) -> Dict:
        """Close and cleanup a session."""
        session = self.sessions.pop(session_id, None)
        if session:
            return session.close()
        return {}

    def list_active_sessions(self) -> List[str]:
        """List all active session IDs."""
        return list(self.sessions.keys())
```

### 2.3 Context Manager for Conversation Sessions

```python
"""
context_manager.py
Context manager for conversation sessions with automatic cleanup
"""

from typing import Optional, List, Dict
from contextlib import contextmanager
from adaptive_memory_client import AdaptiveMemoryClient, SessionType


@contextmanager
def conversation_context(
    client: AdaptiveMemoryClient,
    user_id: str,
    agent_id: str,
    session_type: SessionType = SessionType.CONVERSATION,
    auto_transfer: bool = True
):
    """
    Context manager for conversation sessions.

    Automatically handles session creation, cleanup, and optional LTM transfer.

    Usage:
        client = AdaptiveMemoryClient()
        with conversation_context(client, "user1", "agent1") as session:
            session.add_user_message("Hello!")
            session.add_assistant_message("Hi there!")
            context = session.get_context()
        # Session automatically closed and transferred to LTM

    Args:
        client: AdaptiveMemoryClient instance
        user_id: User identifier
        agent_id: Agent identifier
        session_type: Type of session
        auto_transfer: Whether to transfer to LTM on exit
    """
    from .session_manager import ConversationSession

    session = ConversationSession(
        client=client,
        user_id=user_id,
        agent_id=agent_id,
        session_type=session_type
    )

    try:
        session.start()
        yield session
    finally:
        if auto_transfer:
            session.close()
        else:
            # Just close without transfer
            if session.session_id:
                pass  # Could implement non-transfer close here


class RAGContextBuilder:
    """Builds context for RAG (Retrieval Augmented Generation) applications."""

    def __init__(self, client: AdaptiveMemoryClient):
        self.client = client

    def build_context(
        self,
        query: str,
        stm_session_id: Optional[str] = None,
        max_stm_messages: int = 10,
        max_ltm_results: int = 5,
        include_weights: bool = False
    ) -> Dict:
        """
        Build a complete RAG context for a query.

        Args:
            query: The user query
            stm_session_id: Optional session ID for STM context
            max_stm_messages: Max STM messages to include
            max_ltm_results: Max LTM search results
            include_weights: Whether to include adaptive weights

        Returns:
            Dict with context components
        """
        # Get adaptive memory configuration
        selection = self.client.select_memory_config(query)

        context = {
            "query": query,
            "selection": selection.get("selection", {}),
            "stm_context": [],
            "ltm_context": [],
            "kg_context": []
        }

        weights = selection.get("selection", {}).get("weights", {})

        # Add STM context if weight is significant
        if weights.get("stm", 0) > 0.1 and stm_session_id:
            messages = self.client.get_session_messages(
                stm_session_id,
                limit=max_stm_messages
            )
            context["stm_context"] = messages

        # Add LTM context if weight is significant
        if weights.get("ltm", 0) > 0.1:
            results = self.client.search_ltm(query, top_k=max_ltm_results)
            context["ltm_context"] = [
                {"title": r.title, "content": r.content, "score": r.score}
                for r in results
            ]

        # Add KG context if weight is significant
        if weights.get("kg", 0) > 0.1:
            entities = self.client.search_entities(query, top_k=max_ltm_results)
            context["kg_context"] = entities

        return context

    def format_for_prompt(self, context: Dict) -> str:
        """Format context dictionary as a prompt string."""
        lines = ["[Context]"]

        # Format STM
        if context.get("stm_context"):
            lines.append("\n## Recent Conversation:")
            for msg in context["stm_context"][-5:]:  # Last 5 messages
                role = msg.get("role", "unknown")
                content = msg.get("content", "")
                lines.append(f"- {role}: {content}")

        # Format LTM
        if context.get("ltm_context"):
            lines.append("\n## Relevant Knowledge:")
            for item in context["ltm_context"]:
                lines.append(f"- [{item['title']}] {item['content']}")

        # Format KG
        if context.get("kg_context"):
            lines.append("\n## Related Entities:")
            for entity in context["kg_context"]:
                name = entity.get("entityName", "Unknown")
                desc = entity.get("description", "")
                lines.append(f"- {name}: {desc}")

        lines.append("\n[/Context]")
        return "\n".join(lines)
```

### 2.4 Complete Usage Example

```python
"""
example_usage.py
Complete example demonstrating all major features
"""

from adaptive_memory_client import AdaptiveMemoryClient, SessionType
from session_manager import SessionManager
from context_manager import conversation_context, RAGContextBuilder


def main():
    # Initialize client
    client = AdaptiveMemoryClient(
        base_url="http://localhost:8008",
        username="admin",
        password="admin123"
    )

    print("=== Adaptive Memory System Demo ===\n")

    # 1. Health check
    print("1. Health Check:")
    health = client.health_check()
    print(f"   Status: {health.get('status')}")
    print(f"   Services: {health.get('services')}")

    # 2. Get resources
    print("\n2. Resource Status:")
    resources = client.get_resources()
    print(f"   CPU: {resources.get('cpu', {}).get('usage_percent')}%")
    print(f"   Memory: {resources.get('memory', {}).get('usage_percent')}%")

    # 3. Adaptive memory selection
    print("\n3. Adaptive Memory Selection:")
    task = "用户询问如何学习Python编程，需要详细的学习路线"
    selection = client.select_memory_config(
        task_context=task,
        expected_complexity="medium",
        reasoning_depth="deep"
    )
    weights = selection.get("selection", {}).get("weights", {})
    print(f"   Weights: STM={weights.get('stm')}, LTM={weights.get('ltm')}, "
          f"KG={weights.get('kg')}, MM={weights.get('mm')}")

    # 4. Session management
    print("\n4. Session Management:")
    with conversation_context(
        client=client,
        user_id="demo_user",
        agent_id="demo_agent"
    ) as session:
        # Add messages
        session.add_user_message("我想学习Python编程")
        session.add_assistant_message("Python是一种高级编程语言...")
        session.add_user_message("从哪里开始学习比较好？")
        session.add_assistant_message("建议从基础语法开始...")

        # Get context
        context = session.get_context()
        print(f"   Session has {len(context)} messages")

    # 5. Store and search LTM
    print("\n5. Long-Term Memory Operations:")
    entry_id = client.store_ltm(
        source_id="doc_python_001",
        title="Python语言简介",
        content="Python是一种高级编程语言，由Guido van Rossum于1991年创建。"
    )
    print(f"   Stored entry: {entry_id}")

    # Batch store
    entries = [
        {
            "sourceId": "doc_python_002",
            "title": "Python基础语法",
            "content": "Python的基本语法包括变量、数据类型、条件语句等"
        },
        {
            "sourceId": "doc_python_003",
            "title": "Python函数",
            "content": "Python函数使用def关键字定义"
        }
    ]
    entry_ids = client.batch_store_ltm(entries)
    print(f"   Batch stored {len(entry_ids)} entries")

    # Search
    results = client.search_ltm("Python 编程 学习", top_k=3)
    print(f"   Search found {len(results)} results")
    for r in results:
        print(f"   - {r.title} (score: {r.score:.2f})")

    # 6. Knowledge Graph
    print("\n6. Knowledge Graph:")
    entity_id = client.create_entity(
        entity_name="Python",
        entity_type="编程语言",
        description="一种高级编程语言"
    )
    print(f"   Created entity: {entity_id}")

    entities = client.search_entities("Python", top_k=5)
    print(f"   Found {len(entities)} related entities")

    # 7. RAG context building
    print("\n7. RAG Context:")
    builder = RAGContextBuilder(client)
    rag_context = builder.build_context(
        query="Python学习路线",
        max_ltm_results=3
    )
    prompt_context = builder.format_for_prompt(rag_context)
    print(f"   Built context with {len(rag_context['ltm_context'])} LTM entries")

    print("\n=== Demo Complete ===")


if __name__ == "__main__":
    main()
```

---

## 3. JavaScript/TypeScript Integration

### 3.1 Node.js Client with Fetch

```typescript
/**
 * adaptive-memory-client.ts
 * TypeScript client for Adaptive Memory System
 */

interface MemoryWeights {
  stm: number;
  ltm: number;
  kg: number;
  mm: number;
}

interface SearchResult {
  entryId: string;
  score: number;
  title?: string;
  content?: string;
  metadata?: Record<string, unknown>;
}

interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

class AdaptiveMemoryClient {
  private baseUrl: string;
  private token: string | null = null;
  private tenantId?: string;

  constructor(
    baseUrl: string = "http://localhost:8008",
    tenantId?: string
  ) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.tenantId = tenantId;
  }

  async authenticate(
    username: string,
    password: string
  ): Promise<string> {
    const response = await fetch(`${this.baseUrl}/api/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ username, password }),
    });

    if (!response.ok) {
      throw new Error(`Auth failed: ${response.status}`);
    }

    const data = await response.json();
    this.token = data.token;
    return this.token;
  }

  private async request<T>(
    method: string,
    endpoint: string,
    body?: unknown,
    params?: Record<string, string>
  ): Promise<T> {
    if (!this.token) {
      throw new Error("Not authenticated. Call authenticate() first.");
    }

    let url = `${this.baseUrl}${endpoint}`;
    if (params) {
      const searchParams = new URLSearchParams(params);
      url += `?${searchParams.toString()}`;
    }

    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      Authorization: `Bearer ${this.token}`,
    };

    if (this.tenantId) {
      headers["X-Tenant-ID"] = this.tenantId;
    }

    const response = await fetch(url, {
      method,
      headers,
      body: body ? JSON.stringify(body) : undefined,
    });

    if (response.status === 401) {
      throw new Error("Token expired or invalid");
    }

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`API error ${response.status}: ${errorText}`);
    }

    return response.json();
  }

  // Health & Status
  async healthCheck(): Promise<Record<string, unknown>> {
    return this.request("GET", "/api/memory/health");
  }

  async getStatus(): Promise<Record<string, unknown>> {
    return this.request("GET", "/api/v1/memory/adaptive/status");
  }

  async getResources(): Promise<Record<string, unknown>> {
    return this.request("GET", "/api/v1/memory/monitor/resources");
  }

  // Adaptive Selection
  async selectMemoryConfig(
    taskContext: string,
    options?: {
      expectedComplexity?: string;
      reasoningDepth?: string;
    }
  ): Promise<Record<string, unknown>> {
    return this.request("POST", "/api/v1/memory/adaptive/select", {
      task_context: taskContext,
      ...options,
    });
  }

  // Short-Term Memory
  async storeStm(params: {
    userId: string;
    agentId: string;
    sessionType: string;
    role: string;
    content: string;
    maxContextLength?: number;
    retentionHours?: number;
  }): Promise<{ sessionId: string; messageId: string }> {
    return this.request("POST", "/api/v1/memory/storage/stm", {
      ...params,
      maxContextLength: params.maxContextLength ?? 4096,
      retentionHours: params.retentionHours ?? 24,
    });
  }

  async getSessionMessages(
    sessionId: string,
    limit: number = 100
  ): Promise<{ messages: unknown[] }> {
    return this.request(
      "GET",
      `/api/v1/memory/storage/stm/${sessionId}`,
      undefined,
      { limit: limit.toString() }
    );
  }

  // Long-Term Memory
  async storeLtm(params: {
    sourceId: string;
    sourceType?: string;
    title?: string;
    content: string;
    category?: string;
    domain?: string;
  }): Promise<{ entryId: string }> {
    return this.request("POST", "/api/v1/memory/storage/ltm", {
      sourceType: "document",
      ...params,
    });
  }

  async searchLtm(params: {
    query: string;
    topK?: number;
    minScore?: number;
    enableRerank?: boolean;
  }): Promise<{ results: SearchResult[] }> {
    return this.request("POST", "/api/v1/memory/search/ltm", {
      topK: 5,
      minScore: 0.5,
      enableRerank: true,
      ...params,
    });
  }

  async batchStoreLtm(
    entries: Array<{
      sourceId: string;
      title?: string;
      content: string;
      sourceType?: string;
    }>
  ): Promise<{ entryIds: string[] }> {
    return this.request("POST", "/api/v1/memory/storage/batch-ltm", {
      entries,
    });
  }

  // Knowledge Graph
  async createEntity(params: {
    entityName: string;
    entityType: string;
    description?: string;
    aliases?: string[];
  }): Promise<{ entityId: string }> {
    return this.request("POST", "/api/kg/entities", params);
  }

  async createRelation(params: {
    sourceEntityId: string;
    targetEntityId: string;
    relationType: string;
    relationName: string;
    weight?: number;
  }): Promise<{ relationId: string }> {
    return this.request("POST", "/api/kg/relations", params);
  }

  async searchEntities(params: {
    entityName: string;
    topK?: number;
  }): Promise<{ entities: unknown[] }> {
    return this.request("POST", "/api/kg/search", {
      topK: 10,
      ...params,
    });
  }

  // Transfer
  async transferStmToLtm(sessionId: string): Promise<{
    transferredCount: number;
    entryIds: string[];
  }> {
    return this.request("POST", "/api/v1/memory/storage/transfer", {
      sessionId,
    });
  }

  // Weight Management
  async adjustWeights(
    component: string,
    weight: number,
    reason?: string
  ): Promise<Record<string, unknown>> {
    return this.request("POST", "/api/v1/memory/weights/adjust", {
      component,
      weight,
      reason,
    });
  }

  async getWeightHistory(params?: {
    startTime?: string;
    endTime?: string;
    limit?: number;
  }): Promise<{ history: unknown[] }> {
    const queryParams: Record<string, string> = {};
    if (params?.startTime) queryParams.start_time = params.startTime;
    if (params?.endTime) queryParams.end_time = params.endTime;
    if (params?.limit) queryParams.limit = params.limit.toString();

    return this.request(
      "GET",
      "/api/v1/memory/weights/history",
      undefined,
      queryParams
    );
  }
}

export { AdaptiveMemoryClient, SearchResult, MemoryWeights };
```

### 3.2 Express Middleware Example

```typescript
/**
 * memoryMiddleware.ts
 * Express middleware for Adaptive Memory integration
 */

import { Request, Response, NextFunction } from "express";
import { AdaptiveMemoryClient } from "./adaptive-memory-client";

// Extend Express Request to include memory client
declare global {
  namespace Express {
    interface Request {
      memoryClient?: AdaptiveMemoryClient;
      sessionId?: string;
      userId?: string;
    }
  }
}

interface MemoryMiddlewareOptions {
  baseUrl: string;
  defaultTenantId?: string;
  skipRoutes?: string[];
}

export function createMemoryMiddleware(options: MemoryMiddlewareOptions) {
  const { baseUrl, defaultTenantId, skipRoutes = ["/health", "/login"] } = options;

  // Create shared client instance
  const client = new AdaptiveMemoryClient(baseUrl, defaultTenantId);

  return async (req: Request, res: Response, next: NextFunction) => {
    // Skip certain routes
    if (skipRoutes.some((route) => req.path.startsWith(route))) {
      return next();
    }

    // Attach client to request
    req.memoryClient = client;

    // Extract user/session info from headers or auth
    req.userId = req.headers["x-user-id"] as string || "anonymous";
    req.sessionId = req.headers["x-session-id"] as string;

    next();
  };
}

// Middleware for session-aware routes
export function requireSession(
  req: Request,
  res: Response,
  next: NextFunction
) {
  if (!req.memoryClient) {
    return res.status(500).json({ error: "Memory client not initialized" });
  }

  if (!req.sessionId) {
    return res.status(400).json({ error: "X-Session-ID header required" });
  }

  next();
}

// Route handler for memory search
export async function handleMemorySearch(
  req: Request,
  res: Response
) {
  if (!req.memoryClient) {
    return res.status(500).json({ error: "Memory client not initialized" });
  }

  try {
    const { query, topK, type } = req.body;

    let results;
    if (type === "hybrid") {
      results = await req.memoryClient.searchLtm({ query, topK });
    } else if (type === "entity") {
      const response = await req.memoryClient.searchEntities({ entityName: query, topK });
      results = { results: response.entities };
    } else {
      results = await req.memoryClient.searchLtm({ query, topK });
    }

    res.json({ success: true, data: results });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Search failed";
    res.status(500).json({ success: false, error: message });
  }
}

// Route handler for storing memory
export async function handleStoreMemory(
  req: Request,
  res: Response
) {
  if (!req.memoryClient) {
    return res.status(500).json({ error: "Memory client not initialized" });
  }

  try {
    const { content, role, metadata } = req.body;
    const userId = req.userId || "anonymous";
    const agentId = req.headers["x-agent-id"] as string || "default-agent";

    const result = await req.memoryClient.storeStm({
      userId,
      agentId,
      sessionType: "conversation",
      role: role || "user",
      content,
      metadata,
    });

    res.json({ success: true, data: result });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Store failed";
    res.status(500).json({ success: false, error: message });
  }
}

// Example Express app setup
/*
import express from "express";
import { createMemoryMiddleware, handleMemorySearch, handleStoreMemory } from "./memoryMiddleware";

const app = express();

app.use(express.json());

// Apply memory middleware to all routes
app.use(createMemoryMiddleware({
  baseUrl: process.env.MEMORY_API_URL || "http://localhost:8008",
  defaultTenantId: process.env.TENANT_ID,
  skipRoutes: ["/health", "/login", "/api-docs"]
}));

// Memory endpoints
app.post("/memory/search", handleMemorySearch);
app.post("/memory/store", handleStoreMemory);

// Example: Get session context
app.get("/memory/session/:sessionId/context", async (req, res) => {
  const client = req.memoryClient!;
  const { sessionId } = req.params;

  try {
    const messages = await client.getSessionMessages(sessionId);
    res.json({ success: true, data: messages });
  } catch (error) {
    res.status(500).json({ success: false, error: String(error) });
  }
});

app.listen(3000, () => {
  console.log("Server running on port 3000");
});
*/
```

### 3.3 Next.js API Route Example

```typescript
/**
 * app/api/memory/route.ts
 * Next.js App Router API route handler
 */

import { NextRequest, NextResponse } from "next/server";
import { AdaptiveMemoryClient } from "@/lib/adaptive-memory-client";

// Singleton client for API routes
let memoryClient: AdaptiveMemoryClient | null = null;

function getMemoryClient(): AdaptiveMemoryClient {
  if (!memoryClient) {
    memoryClient = new AdaptiveMemoryClient(
      process.env.MEMORY_API_URL || "http://localhost:8008",
      process.env.TENANT_ID
    );

    // Authenticate on first use
    memoryClient.authenticate(
      process.env.MEMORY_USERNAME || "admin",
      process.env.MEMORY_PASSWORD || "admin123"
    ).catch(console.error);
  }
  return memoryClient;
}

// GET /api/memory - List sessions or search
export async function GET(request: NextRequest) {
  const client = getMemoryClient();
  const { searchParams } = new URL(request.url);

  try {
    const query = searchParams.get("query");
    const sessionId = searchParams.get("sessionId");

    if (query) {
      // Search LTM
      const results = await client.searchLtm({
        query,
        topK: parseInt(searchParams.get("topK") || "5"),
      });
      return NextResponse.json({ success: true, results });
    }

    if (sessionId) {
      // Get session messages
      const messages = await client.getSessionMessages(sessionId);
      return NextResponse.json({ success: true, messages });
    }

    // List sessions
    const sessions = await client.getResources();
    return NextResponse.json({ success: true, data: sessions });

  } catch (error) {
    console.error("Memory API error:", error);
    return NextResponse.json(
      { success: false, error: String(error) },
      { status: 500 }
    );
  }
}

// POST /api/memory - Store memory
export async function POST(request: NextRequest) {
  const client = getMemoryClient();

  try {
    const body = await request.json();
    const { action } = body;

    if (action === "store_stm") {
      const result = await client.storeStm({
        userId: body.userId,
        agentId: body.agentId,
        sessionType: body.sessionType || "conversation",
        role: body.role,
        content: body.content,
        maxContextLength: body.maxContextLength,
        retentionHours: body.retentionHours,
      });
      return NextResponse.json({ success: true, data: result });
    }

    if (action === "store_ltm") {
      const result = await client.storeLtm({
        sourceId: body.sourceId,
        title: body.title,
        content: body.content,
        category: body.category,
        domain: body.domain,
      });
      return NextResponse.json({ success: true, data: result });
    }

    if (action === "select_config") {
      const result = await client.selectMemoryConfig(
        body.taskContext,
        {
          expectedComplexity: body.expectedComplexity,
          reasoningDepth: body.reasoningDepth,
        }
      );
      return NextResponse.json({ success: true, data: result });
    }

    if (action === "transfer") {
      const result = await client.transferStmToLtm(body.sessionId);
      return NextResponse.json({ success: true, data: result });
    }

    return NextResponse.json(
      { success: false, error: "Unknown action" },
      { status: 400 }
    );

  } catch (error) {
    console.error("Memory API error:", error);
    return NextResponse.json(
      { success: false, error: String(error) },
      { status: 500 }
    );
  }
}
```

### 3.4 Next.js RAG API Route Example

```typescript
/**
 * app/api/rag/route.ts
 * Next.js RAG (Retrieval Augmented Generation) endpoint
 */

import { NextRequest, NextResponse } from "next/server";
import { AdaptiveMemoryClient } from "@/lib/adaptive-memory-client";

function getMemoryClient(): AdaptiveMemoryClient {
  const client = new AdaptiveMemoryClient(
    process.env.MEMORY_API_URL || "http://localhost:8008"
  );

  // Sync authenticate
  client.authenticate(
    process.env.MEMORY_USERNAME || "admin",
    process.env.MEMORY_PASSWORD || "admin123"
  );

  return client;
}

interface RAGRequest {
  query: string;
  sessionId?: string;
  maxLtmResults?: number;
  maxStmMessages?: number;
  includeKnowledgeGraph?: boolean;
}

export async function POST(request: NextRequest) {
  try {
    const body: RAGRequest = await request.json();
    const { query, sessionId, maxLtmResults = 5, maxStmMessages = 10 } = body;

    const client = getMemoryClient();

    // Get adaptive memory configuration
    const selection = await client.selectMemoryConfig(query);
    const weights = selection.selection?.weights || { stm: 0, ltm: 0, kg: 0, mm: 0 };

    const context: {
      stm_context?: unknown[];
      ltm_context?: unknown[];
      kg_context?: unknown[];
    } = {};

    // Fetch STM context if significant
    if (weights.stm > 0.1 && sessionId) {
      const stmResponse = await client.getSessionMessages(sessionId, maxStmMessages);
      context.stm_context = stmResponse.messages?.slice(-maxStmMessages);
    }

    // Fetch LTM context if significant
    if (weights.ltm > 0.1) {
      const ltmResponse = await client.searchLtm({ query, topK: maxLtmResults });
      context.ltm_context = ltmResponse.results;
    }

    // Fetch KG context if significant
    if (weights.kg > 0.1) {
      const kgResponse = await client.searchEntities({ entityName: query, topK: maxLtmResults });
      context.kg_context = kgResponse.entities;
    }

    // Format for LLM consumption
    const formattedContext = formatContextForLLM(context);

    // Return context for client-side LLM调用
    return NextResponse.json({
      success: true,
      data: {
        query,
        weights: selection.selection?.weights,
        context: formattedContext,
        prompt: buildPrompt(query, formattedContext)
      }
    });

  } catch (error) {
    console.error("RAG API error:", error);
    return NextResponse.json(
      { success: false, error: String(error) },
      { status: 500 }
    );
  }
}

function formatContextForLLM(context: {
  stm_context?: unknown[];
  ltm_context?: unknown[];
  kg_context?: unknown[];
}): string {
  const parts: string[] = [];

  if (context.stm_context?.length) {
    parts.push("## Recent Conversation");
    context.stm_context.forEach((msg: any) => {
      parts.push(`- ${msg.role}: ${msg.content}`);
    });
  }

  if (context.ltm_context?.length) {
    parts.push("\n## Relevant Knowledge");
    context.ltm_context.forEach((item: any) => {
      parts.push(`- [${item.title || 'Document'}] ${item.content || item.description}`);
    });
  }

  if (context.kg_context?.length) {
    parts.push("\n## Related Entities");
    context.kg_context.forEach((entity: any) => {
      parts.push(`- ${entity.entityName}: ${entity.description || ''}`);
    });
  }

  return parts.join("\n");
}

function buildPrompt(query: string, context: string): string {
  return `You are a helpful AI assistant. Use the following context to answer the user's question.

${context}

## User Question
${query}

## Answer

`;
}
```

---

## 4. LLM Agent Integration Patterns

### 4.1 Python: OpenAI Assistants Integration

```python
"""
openai_assistants_integration.py
Using memory with OpenAI Assistants API
"""

import os
from openai import OpenAI
from adaptive_memory_client import AdaptiveMemoryClient, SessionType, RAGContextBuilder


class MemoryAssistant:
    """OpenAI Assistant with adaptive memory integration."""

    def __init__(
        self,
        memory_client: AdaptiveMemoryClient,
        openai_api_key: str = None,
        assistant_id: str = None
    ):
        self.memory = memory_client
        self.context_builder = RAGContextBuilder(memory_client)

        # Initialize OpenAI client
        self.client = OpenAI(api_key=openai_api_key or os.getenv("OPENAI_API_KEY"))
        self.assistant_id = assistant_id or os.getenv("OPENAI_ASSISTANT_ID")

    def get_or_create_thread(self, user_id: str) -> str:
        """Get or create a conversation thread."""
        thread_id = f"thread_{user_id}"
        # Store thread mapping in memory if needed
        return thread_id

    def process_message(
        self,
        user_id: str,
        message: str,
        session_id: str = None
    ) -> dict:
        """
        Process a user message with memory context.

        1. Store user message in STM
        2. Build RAG context from memory
        3. Call OpenAI with context
        4. Store assistant response in STM
        5. Return response
        """
        # 1. Store user message
        self.memory.store_stm(
            user_id=user_id,
            agent_id="assistant",
            content=message,
            role="user",
            session_type=SessionType.CONVERSATION
        )

        # 2. Build RAG context
        context = self.context_builder.build_context(
            query=message,
            stm_session_id=session_id,
            max_ltm_results=5,
            max_stm_messages=10
        )

        # 3. Call OpenAI with context
        response = self._call_openai_with_context(message, context)

        # 4. Store assistant response
        self.memory.store_stm(
            user_id=user_id,
            agent_id="assistant",
            content=response["content"],
            role="assistant",
            session_type=SessionType.CONVERSATION
        )

        return {
            "content": response["content"],
            "context_used": {
                "stm_messages": len(context.get("stm_context", [])),
                "ltm_entries": len(context.get("ltm_context", [])),
                "kg_entities": len(context.get("kg_context", []))
            }
        }

    def _call_openai_with_context(self, user_message: str, context: dict) -> dict:
        """Call OpenAI API with memory context."""
        # Build the system prompt with context
        system_prompt = self._build_system_prompt(context)

        # Create a run with the assistant
        if self.assistant_id:
            # Using Assistants API with a pre-created assistant
            thread = self.client.beta.threads.create()

            self.client.beta.threads.messages.create(
                thread_id=thread.id,
                role="user",
                content=user_message
            )

            # Add context as user message prefix
            context_prefix = self.context_builder.format_for_prompt(context)

            self.client.beta.threads.messages.create(
                thread_id=thread.id,
                role="user",
                content=f"[Context for reference]\n{context_prefix}\n\n[Current question]\n{user_message}"
            )

            run = self.client.beta.threads.runs.create_and_poll(
                thread_id=thread.id,
                assistant_id=self.assistant_id,
                instructions=system_prompt
            )

            messages = self.client.beta.threads.messages.list(thread_id=thread.id)
            latest_message = messages.data[0]

            return {"content": latest_message.content[0].text.value}

        else:
            # Using Chat Completions API directly
            messages = [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_message}
            ]

            chat_response = self.client.chat.completions.create(
                model="gpt-4",
                messages=messages,
                temperature=0.7
            )

            return {"content": chat_response.choices[0].message.content}

    def _build_system_prompt(self, context: dict) -> str:
        """Build system prompt with memory context."""
        prompt_parts = [
            "You are a helpful AI assistant with access to a user's memory system.",
            "Use the provided context to give personalized, informed responses.",
            ""
        ]

        weights = context.get("selection", {}).get("weights", {})

        if context.get("stm_context"):
            prompt_parts.append("## Recent Conversation")
            for msg in context["stm_context"][-5:]:
                prompt_parts.append(f"- {msg.get('role', 'unknown')}: {msg.get('content', '')}")

        if context.get("ltm_context"):
            prompt_parts.append("\n## User's Knowledge Base")
            for item in context["ltm_context"]:
                prompt_parts.append(f"- {item.get('title', 'Untitled')}: {item.get('content', '')}")

        if context.get("kg_context"):
            prompt_parts.append("\n## Related Entities")
            for entity in context["kg_context"]:
                prompt_parts.append(f"- {entity.get('entityName', 'Unknown')}: {entity.get('description', '')}")

        prompt_parts.append("\nProvide helpful, context-aware responses.")
        return "\n".join(prompt_parts)


# Usage example
def main():
    # Initialize memory client
    memory = AdaptiveMemoryClient(
        base_url="http://localhost:8008",
        username="admin",
        password="admin123"
    )

    # Initialize assistant
    assistant = MemoryAssistant(
        memory_client=memory,
        assistant_id=os.getenv("OPENAI_ASSISTANT_ID")
    )

    # Process conversation
    session_id = "user_123_session"

    # First message
    response1 = assistant.process_message(
        user_id="user_123",
        message="I'm learning Python programming",
        session_id=session_id
    )
    print(f"Assistant: {response1['content']}")
    print(f"Context used: {response1['context_used']}")

    # Follow-up message
    response2 = assistant.process_message(
        user_id="user_123",
        message="What should I learn next?",
        session_id=session_id
    )
    print(f"Assistant: {response2['content']}")
    print(f"Context used: {response2['context_used']}")


if __name__ == "__main__":
    main()
```

### 4.2 Python: LangChain Agent Integration

```python
"""
langchain_agent_integration.py
Using memory with LangChain agents
"""

from typing import List, Dict, Optional, Any
from langchain.agents import AgentExecutor, Tool
from langchain.memory import ConversationBufferMemory
from langchain.prompts import ChatPromptTemplate, MessagesPlaceholder
from langchain.schema import SystemMessage, HumanMessage, AIMessage
from langchain.chat_models import ChatOpenAI

from adaptive_memory_client import AdaptiveMemoryClient, SessionType, RAGContextBuilder


class AdaptiveMemoryTool:
    """Tool for LangChain that interfaces with Adaptive Memory System."""

    name = "adaptive_memory"
    description = """
    Query the adaptive memory system for context about the user's history,
    stored knowledge, and entities. Use when you need information about
    the user or want to search the user's knowledge base.

    Input should be a JSON object with keys:
    - action: "search_ltm", "get_session_context", "search_entities", or "select_config"
    - query: search query for LTM or entities
    - session_id: optional session ID for conversation context
    """

    def __init__(self, memory_client: AdaptiveMemoryClient):
        self.memory = memory_client
        self.context_builder = RAGContextBuilder(memory_client)

    def run(self, tool_input: str) -> str:
        """Execute the memory tool."""
        import json

        try:
            # Parse input
            if isinstance(tool_input, str):
                input_data = json.loads(tool_input)
            else:
                input_data = tool_input

            action = input_data.get("action", "search_ltm")
            query = input_data.get("query", "")
            session_id = input_data.get("session_id")

            if action == "search_ltm":
                results = self.memory.search_ltm(query, top_k=5)
                return self._format_ltm_results(results)

            elif action == "get_session_context":
                if not session_id:
                    return "Error: session_id required"
                messages = self.memory.get_session_messages(session_id, limit=10)
                return self._format_messages(messages)

            elif action == "search_entities":
                entities = self.memory.search_entities(query, top_k=5)
                return self._format_entities(entities)

            elif action == "select_config":
                selection = self.memory.select_memory_config(query)
                return str(selection)

            else:
                return f"Unknown action: {action}"

        except Exception as e:
            return f"Error: {str(e)}"

    def _format_ltm_results(self, results: List) -> str:
        if not results:
            return "No relevant knowledge found."
        parts = ["Relevant knowledge:"]
        for r in results:
            parts.append(f"- [{r.title}] {r.content[:200]}...")
        return "\n".join(parts)

    def _format_messages(self, messages: List[Dict]) -> str:
        if not messages:
            return "No conversation history."
        parts = ["Recent conversation:"]
        for msg in messages[-5:]:
            role = msg.get("role", "unknown")
            content = msg.get("content", "")[:100]
            parts.append(f"- {role}: {content}")
        return "\n".join(parts)

    def _format_entities(self, entities: List[Dict]) -> str:
        if not entities:
            return "No entities found."
        parts = ["Related entities:"]
        for e in entities:
            name = e.get("entityName", "Unknown")
            desc = e.get("description", "")[:100]
            parts.append(f"- {name}: {desc}")
        return "\n".join(parts)


class LangChainMemoryAgent:
    """LangChain agent with Adaptive Memory integration."""

    def __init__(
        self,
        memory_client: AdaptiveMemoryClient,
        llm: Optional[Any] = None,
        system_message: Optional[str] = None
    ):
        self.memory = memory_client
        self.memory_tool = AdaptiveMemoryTool(memory_client)
        self.llm = llm or ChatOpenAI(temperature=0.7, model="gpt-4")

        # Define tools
        self.tools = [
            Tool(
                name=self.memory_tool.name,
                func=self.memory_tool.run,
                description=self.memory_tool.description
            )
        ]

        # System prompt
        self.system_message = system_message or (
            "You are a helpful AI assistant with access to a user's memory system. "
            "Use the adaptive_memory tool to query relevant context when needed. "
            "Be helpful, concise, and use the available context to personalize responses."
        )

        # Create prompt template
        self.prompt = ChatPromptTemplate.from_messages([
            SystemMessage(content=self.system_message),
            MessagesPlaceholder(variable_name="chat_history", optional=True),
            HumanMessage(content="{input}"),
            MessagesPlaceholder(variable_name="agent_scratchpad")
        ])

        # Create agent (simplified - would use create_openai_functions_agent in production)
        from langchain.agents import AgentExecutor, create_structured_chat_agent

        self.agent = create_structured_chat_agent(
            llm=self.llm,
            tools=self.tools,
            prompt=self.prompt
        )

        # Memory for conversation history
        self.chat_memory = ConversationBufferMemory(
            memory_key="chat_history",
            return_messages=True
        )

        self.agent_executor = AgentExecutor(
            agent=self.agent,
            tools=self.tools,
            memory=self.chat_memory,
            verbose=True
        )

    def run(self, user_input: str, session_id: Optional[str] = None) -> str:
        """Run the agent with memory context."""
        # Inject session_id into input if provided
        if session_id:
            # Add context query to find relevant memory
            context_result = self.memory_tool.run(
                f'{{"action": "get_session_context", "session_id": "{session_id}"}}'
            )
            if context_result and "No conversation" not in context_result:
                user_input = f"[Context]\n{context_result}\n\n[User Message]\n{user_input}"

        # Run agent
        result = self.agent_executor.invoke({"input": user_input})
        return result["output"]

    def get_session_id(self, user_id: str) -> str:
        """Get or create session for user."""
        return f"session_{user_id}"


# Usage example
def main():
    from langchain_openai import ChatOpenAI

    # Initialize memory client
    memory = AdaptiveMemoryClient(
        base_url="http://localhost:8008"
    )

    # Initialize LLM
    llm = ChatOpenAI(model="gpt-4", temperature=0.7)

    # Create agent
    agent = LangChainMemoryAgent(
        memory_client=memory,
        llm=llm
    )

    session_id = agent.get_session_id("user_456")

    # Run conversation
    print("=== LangChain Memory Agent Demo ===\n")

    response1 = agent.run(
        "I'm interested in machine learning",
        session_id=session_id
    )
    print(f"Agent: {response1}\n")

    response2 = agent.run(
        "What are the key algorithms I should learn?",
        session_id=session_id
    )
    print(f"Agent: {response2}")


if __name__ == "__main__":
    main()
```

### 4.3 Python: RAG Pattern

```python
"""
rag_pattern.py
Retrieval Augmented Generation pattern with Adaptive Memory
"""

from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass
from adaptive_memory_client import AdaptiveMemoryClient, SearchResult


@dataclass
class RetrievedContext:
    """Container for retrieved context."""
    stm_messages: List[Dict]
    ltm_results: List[SearchResult]
    kg_entities: List[Dict]
    weights: Dict[str, float]


class RAGEngine:
    """
    Retrieval Augmented Generation engine using Adaptive Memory.

    Provides semantic search over user memories and knowledge base,
    then formats results for LLM consumption.
    """

    def __init__(
        self,
        memory_client: AdaptiveMemoryClient,
        default_top_k: int = 5,
        default_min_score: float = 0.5
    ):
        self.memory = memory_client
        self.default_top_k = default_top_k
        self.default_min_score = default_min_score

    def retrieve(
        self,
        query: str,
        session_id: Optional[str] = None,
        max_stm_messages: int = 10,
        max_ltm_results: int = 5,
        max_kg_entities: int = 5
    ) -> RetrievedContext:
        """
        Retrieve relevant context for a query.

        Uses adaptive memory selection to determine which memory
        components to query and with what weights.
        """
        # Get adaptive selection
        selection = self.memory.select_memory_config(query)
        weights = selection.get("selection", {}).get("weights", {
            "stm": 0.3, "ltm": 0.4, "kg": 0.3, "mm": 0.0
        })

        stm_messages = []
        ltm_results = []
        kg_entities = []

        # Retrieve STM context if significant
        if weights.get("stm", 0) > 0.1 and session_id:
            try:
                messages = self.memory.get_session_messages(
                    session_id,
                    limit=max_stm_messages
                )
                stm_messages = messages[-max_stm_messages:]
            except Exception:
                pass  # Ignore STM retrieval failures

        # Retrieve LTM context if significant
        if weights.get("ltm", 0) > 0.1:
            try:
                ltm_results = self.memory.search_ltm(
                    query,
                    top_k=max_ltm_results,
                    min_score=self.default_min_score
                )
            except Exception:
                pass  # Ignore LTM retrieval failures

        # Retrieve KG context if significant
        if weights.get("kg", 0) > 0.1:
            try:
                response = self.memory.search_entities(
                    entity_name=query,
                    top_k=max_kg_entities
                )
                kg_entities = response
            except Exception:
                pass  # Ignore KG retrieval failures

        return RetrievedContext(
            stm_messages=stm_messages,
            ltm_results=ltm_results,
            kg_entities=kg_entities,
            weights=weights
        )

    def format_context(self, context: RetrievedContext) -> str:
        """Format retrieved context as a string for LLM."""
        parts = []

        if context.stm_messages:
            parts.append("## Recent Conversation")
            for msg in context.stm_messages[-5:]:
                role = msg.get("role", "unknown")
                content = msg.get("content", "")
                parts.append(f"- {role}: {content}")

        if context.ltm_results:
            parts.append("\n## Relevant Knowledge")
            for result in context.ltm_results:
                title = result.title or "Untitled"
                content = result.content or ""
                parts.append(f"- [{title}] (score: {result.score:.2f})")
                parts.append(f"  {content[:300]}...")

        if context.kg_entities:
            parts.append("\n## Related Entities")
            for entity in context.kg_entities:
                name = entity.get("entityName", "Unknown")
                desc = entity.get("description", "No description")
                parts.append(f"- {name}: {desc}")

        return "\n".join(parts)

    def build_prompt(
        self,
        query: str,
        context: RetrievedContext,
        system_prompt: Optional[str] = None
    ) -> Tuple[str, str]:
        """
        Build a RAG prompt from query and context.

        Returns (system_prompt, user_prompt) tuple.
        """
        default_system = (
            "You are a helpful AI assistant. Use the provided context to answer "
            "the user's question accurately. If the context doesn't contain "
            "relevant information, say so."
        )

        formatted_context = self.format_context(context)

        system = system_prompt or default_system
        user = f"""Context:
{formatted_context}

Question: {query}

Answer:"""

        return system, user


class ConversationalRAG:
    """RAG engine with conversation history tracking."""

    def __init__(
        self,
        memory_client: AdaptiveMemoryClient,
        user_id: str,
        agent_id: str = "rag_agent",
        max_history: int = 10
    ):
        self.memory = memory_client
        self.user_id = user_id
        self.agent_id = agent_id
        self.rag = RAGEngine(memory_client)
        self.max_history = max_history

        # Create or get session
        self.session_id: Optional[str] = None
        self._init_session()

    def _init_session(self):
        """Initialize a new session."""
        result = self.memory.store_stm(
            user_id=self.user_id,
            agent_id=self.agent_id,
            content="[Session started]",
            role="system",
            session_type="conversation"
        )
        self.session_id = result.get("sessionId")

    def query(
        self,
        question: str,
        llm_callable: callable,
        system_prompt: Optional[str] = None
    ) -> str:
        """
        Query with RAG, storing conversation history.

        Args:
            question: User's question
            llm_callable: Function that takes (system_prompt, user_prompt) and returns response
            system_prompt: Optional custom system prompt

        Returns:
            LLM response string
        """
        # Retrieve context
        context = self.rag.retrieve(
            query=question,
            session_id=self.session_id,
            max_stm_messages=self.max_history
        )

        # Build prompt
        system, user = self.rag.build_prompt(question, context, system_prompt)

        # Store user message
        self.memory.store_stm(
            user_id=self.user_id,
            agent_id=self.agent_id,
            content=question,
            role="user",
            session_type="conversation"
        )

        # Call LLM
        response = llm_callable(system, user)

        # Store assistant response
        self.memory.store_stm(
            user_id=self.user_id,
            agent_id=self.agent_id,
            content=response,
            role="assistant",
            session_type="conversation"
        )

        return response

    def close(self, transfer_to_ltm: bool = True):
        """Close session and optionally transfer to LTM."""
        if self.session_id and transfer_to_ltm:
            self.memory.transfer_stm_to_ltm(self.session_id)


# Usage example
def main():
    import os

    # Initialize
    memory = AdaptiveMemoryClient(
        base_url="http://localhost:8008"
    )

    # Create conversational RAG
    rag = ConversationalRAG(
        memory_client=memory,
        user_id="demo_user",
        agent_id="demo_rag"
    )

    # Example LLM callable
    def call_openai(system: str, user: str) -> str:
        from openai import OpenAI
        client = OpenAI(api_key=os.getenv("OPENAI_API_KEY"))
        response = client.chat.completions.create(
            model="gpt-4",
            messages=[
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ]
        )
        return response.choices[0].message.content

    # Conversational query
    print("=== RAG Demo ===\n")

    response1 = rag.query(
        "I'm learning about neural networks",
        llm_callable=call_openai
    )
    print(f"Q: I'm learning about neural networks")
    print(f"A: {response1}\n")

    response2 = rag.query(
        "What types should I start with?",
        llm_callable=call_openai
    )
    print(f"Q: What types should I start with?")
    print(f"A: {response2}\n")

    # Close session
    rag.close(transfer_to_ltm=True)


if __name__ == "__main__":
    main()
```

---

## 5. WebSocket Real-time Updates

### 5.1 Client Example for WebSocket Subscriptions

```python
"""
websocket_client_example.py
WebSocket client for subscribing to memory changes
"""

import asyncio
import json
import websockets
from typing import Optional, Callable, Dict, List


class MemoryWebSocketClient:
    """
    WebSocket client for real-time memory updates.

    Subscribe to memory changes, weight adjustments, and system events.
    """

    def __init__(
        self,
        base_url: str = "ws://localhost:8008",
        token: Optional[str] = None
    ):
        self.base_url = base_url.replace("http://", "ws://").replace("https://", "wss://")
        self.token = token
        self.websocket = None
        self.subscriptions: Dict[str, Callable] = {}
        self.running = False

    async def connect(self):
        """Connect to WebSocket server."""
        headers = []
        if self.token:
            headers.append(f"Authorization: Bearer {self.token}")

        url = f"{self.base_url}/ws/memory"
        self.websocket = await websockets.connect(url, extra_headers=headers)
        self.running = True
        print(f"Connected to {url}")

    async def disconnect(self):
        """Disconnect from server."""
        self.running = False
        if self.websocket:
            await self.websocket.close()
            self.websocket = None
        print("Disconnected")

    async def subscribe(
        self,
        event_type: str,
        callback: Callable[[Dict], None]
    ):
        """
        Subscribe to an event type.

        Event types:
        - "memory.stm.created" - New STM message created
        - "memory.stm.updated" - STM message updated
        - "memory.ltm.created" - New LTM entry created
        - "memory.ltm.transferred" - STM transferred to LTM
        - "weights.adjusted" - Weight adjustment occurred
        - "session.created" - New session created
        - "session.closed" - Session closed
        - "system.health" - Health status update
        """
        self.subscriptions[event_type] = callback

        # Send subscription message
        await self.send({
            "action": "subscribe",
            "event_type": event_type
        })

        print(f"Subscribed to: {event_type}")

    async def unsubscribe(self, event_type: str):
        """Unsubscribe from an event type."""
        if event_type in self.subscriptions:
            del self.subscriptions[event_type]

            await self.send({
                "action": "unsubscribe",
                "event_type": event_type
            })

    async def send(self, message: Dict):
        """Send message to WebSocket server."""
        if self.websocket:
            await self.websocket.send(json.dumps(message))

    async def listen(self):
        """Listen for messages and dispatch to subscribers."""
        if not self.websocket:
            raise RuntimeError("Not connected. Call connect() first.")

        try:
            async for message in self.websocket:
                data = json.loads(message)
                event_type = data.get("event_type", "")
                payload = data.get("payload", {})

                # Dispatch to callback if subscribed
                if event_type in self.subscriptions:
                    callback = self.subscriptions[event_type]
                    await callback(payload)

        except websockets.exceptions.ConnectionClosed:
            print("Connection closed")
            self.running = False


async def example_usage():
    """Example WebSocket client usage."""

    client = MemoryWebSocketClient()

    # Define event handlers
    async def on_stm_created(payload: Dict):
        print(f"New STM message: {payload.get('session_id')}")
        print(f"  Content: {payload.get('content', '')[:50]}...")

    async def on_ltm_transferred(payload: Dict):
        print(f"Memory transferred: {payload.get('count')} items")
        print(f"  From session: {payload.get('session_id')}")

    async def on_weights_adjusted(payload: Dict):
        print(f"Weights adjusted: {payload}")

    async def on_health(payload: Dict):
        print(f"Health update: {payload.get('status')}")

    try:
        # Connect
        await client.connect()

        # Subscribe to events
        await client.subscribe("memory.stm.created", on_stm_created)
        await client.subscribe("memory.ltm.transferred", on_ltm_transferred)
        await client.subscribe("weights.adjusted", on_weights_adjusted)
        await client.subscribe("system.health", on_health)

        # Listen for events (runs until disconnected)
        print("Listening for events...")
        await client.listen()

    except KeyboardInterrupt:
        print("\nInterrupted")
    finally:
        await client.disconnect()


# Run example
if __name__ == "__main__":
    asyncio.run(example_usage())
```

### 5.2 Server-Side WebSocket Handler (Python)

```python
"""
websocket_server_example.py
Server-side WebSocket event handler example
"""

import asyncio
import json
from typing import Dict, Set, Callable
from dataclasses import dataclass, field
from datetime import datetime


@dataclass
class Subscription:
    """Represents a client subscription."""
    client_id: str
    event_type: str
    callback: Callable


class WebSocketEventBus:
    """
    In-memory event bus for WebSocket subscriptions.
    In production, use Redis pub/sub for distributed deployment.
    """

    def __init__(self):
        # event_type -> set of client connections
        self._subscriptions: Dict[str, Set[str]] = {}
        # client_id -> connection
        self._connections: Dict[str, 'WebSocketConnection'] = {}

    def subscribe(self, client_id: str, event_type: str):
        """Subscribe a client to an event type."""
        if event_type not in self._subscriptions:
            self._subscriptions[event_type] = set()
        self._subscriptions[event_type].add(client_id)

    def unsubscribe(self, client_id: str, event_type: str):
        """Unsubscribe a client from an event type."""
        if event_type in self._subscriptions:
            self._subscriptions[event_type].discard(client_id)

    def register_connection(self, client_id: str, connection: 'WebSocketConnection'):
        """Register a client connection."""
        self._connections[client_id] = connection

    def unregister_connection(self, client_id: str):
        """Unregister a client connection."""
        if client_id in self._connections:
            del self._connections[client_id]

        # Remove all subscriptions for this client
        for event_type in self._subscriptions:
            self._subscriptions[event_type].discard(client_id)

    async def publish(self, event_type: str, payload: Dict):
        """Publish an event to all subscribers."""
        message = json.dumps({
            "event_type": event_type,
            "payload": payload,
            "timestamp": datetime.utcnow().isoformat()
        })

        if event_type in self._subscriptions:
            disconnected = []

            for client_id in self._subscriptions[event_type]:
                connection = self._connections.get(client_id)
                if connection:
                    try:
                        await connection.send(message)
                    except Exception:
                        disconnected.append(client_id)

            # Cleanup disconnected clients
            for client_id in disconnected:
                self.unregister_connection(client_id)


# Global event bus instance
event_bus = WebSocketEventBus()


# Example: Integrate with memory events
async def on_memory_stored(session_id: str, content: str, role: str):
    """Called when a message is stored in STM."""
    await event_bus.publish("memory.stm.created", {
        "session_id": session_id,
        "content": content,
        "role": role
    })


async def on_transfer_completed(session_id: str, count: int):
    """Called when STM is transferred to LTM."""
    await event_bus.publish("memory.ltm.transferred", {
        "session_id": session_id,
        "count": count
    })


async def on_weights_changed(component: str, old_weight: float, new_weight: float):
    """Called when weights are adjusted."""
    await event_bus.publish("weights.adjusted", {
        "component": component,
        "old_weight": old_weight,
        "new_weight": new_weight
    })
```

---

## 6. Webhook/Callback Patterns

### 6.1 Memory Transfer Callbacks

```python
"""
webhook_callbacks.py
Webhook handler for memory transfer and weight adjustment callbacks
"""

from typing import Optional, Dict, Callable
from dataclasses import dataclass
from enum import Enum
import hashlib
import hmac
import json


class CallbackEventType(Enum):
    """Types of callback events."""
    TRANSFER_STARTED = "transfer.started"
    TRANSFER_COMPLETED = "transfer.completed"
    TRANSFER_FAILED = "transfer.failed"
    WEIGHTS_ADJUSTED = "weights.adjusted"
    SESSION_CREATED = "session.created"
    SESSION_EXPIRED = "session.expired"


@dataclass
class WebhookPayload:
    """Webhook payload structure."""
    event_type: str
    timestamp: str
    data: Dict
    signature: Optional[str] = None


class WebhookHandler:
    """
    Handles webhook callbacks for memory events.

    Can be used to trigger external actions on memory events.
    """

    def __init__(self, secret_key: Optional[str] = None):
        self.secret_key = secret_key
        self.handlers: Dict[CallbackEventType, Callable] = {}

    def register_handler(
        self,
        event_type: CallbackEventType,
        handler: Callable[[Dict], None]
    ):
        """Register a handler for an event type."""
        self.handlers[event_type] = handler

    def verify_signature(self, payload: str, signature: str) -> bool:
        """Verify webhook signature."""
        if not self.secret_key:
            return True  # Skip verification if no secret

        expected = hmac.new(
            self.secret_key.encode(),
            payload.encode(),
            hashlib.sha256
        ).hexdigest()

        return hmac.compare_digest(expected, signature)

    async def handle_webhook(
        self,
        payload: WebhookPayload
    ) -> Dict:
        """Process an incoming webhook."""
        # Verify signature if present
        if payload.signature:
            payload_json = json.dumps(payload.data, sort_keys=True)
            if not self.verify_signature(payload_json, payload.signature):
                return {"status": "error", "message": "Invalid signature"}

        # Find and execute handler
        event_type = CallbackEventType(payload.event_type)
        handler = self.handlers.get(event_type)

        if handler:
            try:
                await handler(payload.data)
                return {"status": "success"}
            except Exception as e:
                return {"status": "error", "message": str(e)}
        else:
            return {"status": "ignored", "message": "No handler registered"}


# Example: Transfer completion handler
async def on_transfer_completed(data: Dict):
    """Handle transfer completion - e.g., notify external system."""
    session_id = data.get("session_id")
    entry_ids = data.get("entry_ids", [])
    count = data.get("count", 0)

    print(f"Transfer completed for session {session_id}")
    print(f"  Transferred {count} entries: {entry_ids}")

    # Example: Send notification, update external DB, etc.
    # await external_service.notify_transfer_complete(session_id, entry_ids)


# Example: Weights adjustment handler
async def on_weights_adjusted(data: Dict):
    """Handle weight adjustment - e.g., log for monitoring."""
    component = data.get("component")
    old_weight = data.get("old_weight")
    new_weight = data.get("new_weight")
    reason = data.get("reason", "Unknown")

    print(f"Weight adjustment: {component}")
    print(f"  {old_weight} -> {new_weight}")
    print(f"  Reason: {reason}")

    # Example: Send metrics to monitoring system
    # await metrics.record_weight_change(component, old_weight, new_weight)


# Setup example
def setup_webhook_handlers() -> WebhookHandler:
    """Set up webhook handler with all handlers."""
    handler = WebhookHandler(secret_key="your-webhook-secret")

    handler.register_handler(CallbackEventType.TRANSFER_COMPLETED, on_transfer_completed)
    handler.register_handler(CallbackEventType.WEIGHTS_ADJUSTED, on_weights_adjusted)

    return handler
```

### 6.2 Webhook Receiver Server (Flask Example)

```python
"""
webhook_receiver.py
Flask server for receiving webhook callbacks
"""

from flask import Flask, request, jsonify
from webhook_callbacks import WebhookHandler, WebhookPayload, CallbackEventType

app = Flask(__name__)

# Initialize webhook handler
webhook_handler = setup_webhook_handlers()


@app.route("/webhook", methods=["POST"])
def handle_webhook():
    """
    Receive webhook callbacks from the memory system.

    Headers:
    - X-Webhook-Signature: HMAC signature of payload

    Body:
    {
        "event_type": "transfer.completed",
        "timestamp": "2026-03-29T10:00:00Z",
        "data": { ... }
    }
    """
    # Get signature from header
    signature = request.headers.get("X-Webhook-Signature")

    # Parse payload
    json_data = request.get_json()
    if not json_data:
        return jsonify({"error": "Invalid JSON"}), 400

    # Create payload object
    payload = WebhookPayload(
        event_type=json_data.get("event_type", ""),
        timestamp=json_data.get("timestamp", ""),
        data=json_data.get("data", {}),
        signature=signature
    )

    # Handle webhook
    result = webhook_handler.handle_webhook(payload)

    if result["status"] == "success":
        return jsonify(result), 200
    elif result["status"] == "ignored":
        return jsonify(result), 200
    else:
        return jsonify(result), 400


@app.route("/webhook/test", methods=["POST"])
def test_webhook():
    """Test endpoint to verify webhook connectivity."""
    return jsonify({
        "status": "ok",
        "message": "Webhook endpoint is reachable"
    })


# Health check
@app.route("/health", methods=["GET"])
def health():
    return jsonify({"status": "healthy"})


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000, debug=True)
```

---

## 7. Testing Your Integration

### 7.1 Mock Server for Testing

```python
"""
mock_memory_server.py
Mock server for testing memory integrations without a real backend
"""

import json
import uuid
from typing import Dict, List, Optional
from dataclasses import dataclass, field, asdict
from http.server import HTTPServer, BaseHTTPRequestHandler
import threading


@dataclass
class MockSession:
    """Mock session data."""
    session_id: str
    user_id: str
    agent_id: str
    session_type: str
    messages: List[Dict] = field(default_factory=list)
    created_at: str = ""


@dataclass
class MockLTMEntry:
    """Mock LTM entry."""
    entry_id: str
    source_id: str
    title: Optional[str]
    content: str
    category: Optional[str] = None
    score: float = 1.0


class MockMemoryServer:
    """
    Mock Adaptive Memory server for testing.

    Implements the same API as the real server but stores data in memory.
    """

    def __init__(self):
        self.sessions: Dict[str, MockSession] = {}
        self.ltm_entries: Dict[str, MockLTMEntry] = []
        self.entities: List[Dict] = []
        self.weights_history: List[Dict] = []
        self.next_entity_id = 1

    def reset(self):
        """Reset all data."""
        self.sessions.clear()
        self.ltm_entries.clear()
        self.entities.clear()
        self.weights_history.clear()

    # Health & Status
    def get_health(self) -> Dict:
        return {
            "status": "healthy",
            "services": {
                "database": "connected",
                "qdrant": "connected",
                "neo4j": "connected"
            }
        }

    def get_status(self) -> Dict:
        return {
            "stm_sessions": len(self.sessions),
            "ltm_entries": len(self.ltm_entries),
            "kg_entities": len(self.entities)
        }

    # STM Operations
    def store_stm(
        self,
        user_id: str,
        agent_id: str,
        session_type: str,
        role: str,
        content: str,
        **kwargs
    ) -> Dict:
        session_id = None

        # Find or create session
        for session in self.sessions.values():
            if session.user_id == user_id and session.agent_id == agent_id:
                session_id = session.session_id
                break

        if not session_id:
            session_id = f"session_{uuid.uuid4().hex[:12]}"
            self.sessions[session_id] = MockSession(
                session_id=session_id,
                user_id=user_id,
                agent_id=agent_id,
                session_type=session_type
            )

        # Add message
        message_id = f"msg_{uuid.uuid4().hex[:12]}"
        self.sessions[session_id].messages.append({
            "messageId": message_id,
            "role": role,
            "content": content,
            "timestamp": "2026-03-29T10:00:00Z"
        })

        return {"sessionId": session_id, "messageId": message_id}

    def get_session_messages(self, session_id: str, limit: int = 100) -> Dict:
        session = self.sessions.get(session_id)
        if not session:
            return {"messages": []}

        messages = session.messages[-limit:]
        return {"messages": messages}

    # LTM Operations
    def store_ltm(
        self,
        source_id: str,
        content: str,
        title: Optional[str] = None,
        category: Optional[str] = None,
        **kwargs
    ) -> Dict:
        entry_id = f"ltm_{uuid.uuid4().hex[:12]}"
        entry = MockLTMEntry(
            entry_id=entry_id,
            source_id=source_id,
            title=title,
            content=content,
            category=category
        )
        self.ltm_entries.append(entry)
        return {"entryId": entry_id}

    def search_ltm(self, query: str, top_k: int = 5, **kwargs) -> Dict:
        # Simple mock search - just return all entries with mock scores
        results = []
        query_lower = query.lower()

        for entry in self.ltm_entries:
            # Mock scoring based on keyword matching
            score = 0.5
            if query_lower in entry.content.lower():
                score = 0.9
            elif entry.title and query_lower in entry.title.lower():
                score = 0.8

            if score > 0.5:
                results.append({
                    "entryId": entry.entry_id,
                    "title": entry.title,
                    "content": entry.content,
                    "score": score
                })

        results.sort(key=lambda x: x["score"], reverse=True)
        return {"results": results[:top_k]}

    # Knowledge Graph
    def create_entity(
        self,
        entity_name: str,
        entity_type: str,
        description: Optional[str] = None,
        aliases: Optional[List[str]] = None,
        **kwargs
    ) -> Dict:
        entity_id = f"entity_{self.next_entity_id}"
        self.next_entity_id += 1

        entity = {
            "entityId": entity_id,
            "entityName": entity_name,
            "entityType": entity_type,
            "description": description,
            "aliases": aliases or []
        }
        self.entities.append(entity)
        return {"entityId": entity_id}

    def search_entities(self, entity_name: str, top_k: int = 10, **kwargs) -> Dict:
        name_lower = entity_name.lower()
        results = []

        for entity in self.entities:
            if name_lower in entity.get("entityName", "").lower():
                results.append(entity)

        return {"entities": results[:top_k]}

    # Transfer
    def transfer_stm_to_ltm(self, session_id: str) -> Dict:
        session = self.sessions.get(session_id)
        if not session:
            return {"transferredCount": 0, "entryIds": []}

        entry_ids = []
        for msg in session.messages:
            result = self.store_ltm(
                source_id=msg.get("messageId", ""),
                content=msg.get("content", ""),
                title="Transferred message"
            )
            entry_ids.append(result["entryId"])

        return {"transferredCount": len(entry_ids), "entryIds": entry_ids}

    # Adaptive Selection (simplified mock)
    def select_memory_config(self, task_context: str, **kwargs) -> Dict:
        # Return a balanced configuration
        return {
            "selection": {
                "config_id": "config_default",
                "weights": {"stm": 0.3, "ltm": 0.4, "kg": 0.3, "mm": 0.0},
                "use_stm": True,
                "use_ltm": True,
                "use_kg": True,
                "use_mm": False
            },
            "prediction": {
                "efficiency_gain": 0.75,
                "confidence": 0.85
            }
        }


# HTTP Handler for mock server
class MockServerHandler(BaseHTTPRequestHandler):
    server: MockMemoryServer

    def _send_json(self, data: Dict, status: int = 200):
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def do_GET(self):
        if self.path == "/api/memory/health":
            self._send_json(self.server.get_health())
        elif self.path == "/api/v1/memory/adaptive/status":
            self._send_json(self.server.get_status())
        elif self.path.startswith("/api/v1/memory/storage/stm/"):
            session_id = self.path.split("/")[-1]
            self._send_json(self.server.get_session_messages(session_id))
        else:
            self._send_json({"error": "Not found"}, 404)

    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length).decode()
        data = json.loads(body) if body else {}

        if self.path == "/api/login":
            self._send_json({"token": "mock_token_123"})
        elif self.path == "/api/v1/memory/storage/stm":
            result = self.server.store_stm(**data)
            self._send_json(result)
        elif self.path == "/api/v1/memory/storage/ltm":
            result = self.server.store_ltm(**data)
            self._send_json(result)
        elif self.path == "/api/v1/memory/search/ltm":
            result = self.server.search_ltm(**data)
            self._send_json(result)
        elif self.path == "/api/v1/memory/adaptive/select":
            result = self.server.select_memory_config(**data)
            self._send_json(result)
        elif self.path == "/api/v1/memory/storage/transfer":
            result = self.server.transfer_stm_to_ltm(**data)
            self._send_json(result)
        elif self.path == "/api/kg/entities":
            result = self.server.create_entity(**data)
            self._send_json(result)
        elif self.path == "/api/kg/search":
            result = self.server.search_entities(**data)
            self._send_json(result)
        else:
            self._send_json({"error": "Not found"}, 404)

    def log_message(self, format, *args):
        print(f"[MockServer] {format % args}")


def run_mock_server(port: int = 8008) -> HTTPServer:
    """Run mock server on specified port."""
    server = MockMemoryServer()
    handler = lambda *args: MockServerHandler(*args, server=server)
    httpd = HTTPServer(("localhost", port), handler)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    print(f"Mock server running on port {port}")
    return httpd


# Usage for testing
if __name__ == "__main__":
    # Run mock server
    httpd = run_mock_server(8008)

    # Now you can test against the mock server
    import requests

    # Test health
    resp = requests.get("http://localhost:8008/api/memory/health")
    print(f"Health: {resp.json()}")

    # Test store
    resp = requests.post(
        "http://localhost:8008/api/v1/memory/storage/stm",
        json={
            "userId": "user1",
            "agentId": "agent1",
            "sessionType": "conversation",
            "role": "user",
            "content": "Hello, world!"
        }
    )
    print(f"Store STM: {resp.json()}")

    # Test search
    resp = requests.post(
        "http://localhost:8008/api/v1/memory/storage/ltm",
        json={
            "sourceId": "doc1",
            "content": "Python is a programming language",
            "title": "Python Introduction"
        }
    )
    print(f"Store LTM: {resp.json()}")

    resp = requests.post(
        "http://localhost:8008/api/v1/memory/search/ltm",
        json={"query": "Python", "topK": 5}
    )
    print(f"Search LTM: {resp.json()}")

    # Keep server running
    print("\nMock server is running. Press Ctrl+C to stop.")
    input()
```

### 7.2 Test Fixtures

```python
"""
test_fixtures.py
Pytest fixtures for memory system integration tests
"""

import pytest
import requests
from typing import Generator
from adaptive_memory_client import AdaptiveMemoryClient, SessionType


@pytest.fixture(scope="session")
def mock_server():
    """Start mock server for tests."""
    from mock_memory_server import run_mock_server

    httpd = run_mock_server(8009)  # Use different port
    yield httpd
    httpd.shutdown()


@pytest.fixture(scope="session")
def base_url(mock_server) -> str:
    """Get base URL for mock server."""
    return "http://localhost:8009"


@pytest.fixture
def auth_token(base_url: str) -> str:
    """Get auth token from mock server."""
    response = requests.post(
        f"{base_url}/api/login",
        json={"username": "admin", "password": "admin"}
    )
    return response.json()["token"]


@pytest.fixture
def memory_client(base_url: str, auth_token: str) -> Generator[AdaptiveMemoryClient, None, None]:
    """Create authenticated memory client."""
    client = AdaptiveMemoryClient(
        base_url=base_url,
        username="admin",
        password="admin"
    )
    yield client


@pytest.fixture
def sample_stm_data() -> dict:
    """Sample STM data for testing."""
    return {
        "user_id": "test_user",
        "agent_id": "test_agent",
        "session_type": SessionType.CONVERSATION,
        "role": "user",
        "content": "This is a test message",
        "max_context_length": 4096,
        "retention_hours": 24
    }


@pytest.fixture
def sample_ltm_data() -> dict:
    """Sample LTM data for testing."""
    return {
        "source_id": "test_doc_001",
        "source_type": "document",
        "title": "Test Document",
        "content": "This is test content for the knowledge base.",
        "category": "testing",
        "domain": "test"
    }


@pytest.fixture
def sample_entities() -> list:
    """Sample KG entities for testing."""
    return [
        {
            "entity_name": "Python",
            "entity_type": "Programming Language",
            "description": "A high-level programming language"
        },
        {
            "entity_name": "Machine Learning",
            "entity_type": "Field",
            "description": "A branch of AI"
        }
    ]


# Example test using fixtures
class TestMemoryOperations:
    """Test suite for memory operations."""

    def test_health_check(self, memory_client: AdaptiveMemoryClient):
        """Test health check endpoint."""
        health = memory_client.health_check()
        assert health.get("status") == "healthy"

    def test_store_stm(self, memory_client: AdaptiveMemoryClient, sample_stm_data: dict):
        """Test storing STM."""
        result = memory_client.store_stm(
            user_id=sample_stm_data["user_id"],
            agent_id=sample_stm_data["agent_id"],
            content=sample_stm_data["content"],
            role=sample_stm_data["role"]
        )
        assert "sessionId" in result
        assert "messageId" in result

    def test_store_ltm(self, memory_client: AdaptiveMemoryClient, sample_ltm_data: dict):
        """Test storing LTM."""
        entry_id = memory_client.store_ltm(**sample_ltm_data)
        assert entry_id.startswith("ltm_")

    def test_search_ltm(self, memory_client: AdaptiveMemoryClient, sample_ltm_data: dict):
        """Test searching LTM."""
        # First store some data
        memory_client.store_ltm(**sample_ltm_data)

        # Search for it
        results = memory_client.search_ltm("Python", top_k=5)
        assert isinstance(results, list)

    def test_select_memory_config(self, memory_client: AdaptiveMemoryClient):
        """Test adaptive memory selection."""
        selection = memory_client.select_memory_config(
            task_context="User is learning Python programming",
            expected_complexity="medium",
            reasoning_depth="deep"
        )

        assert "selection" in selection
        assert "weights" in selection["selection"]

    def test_create_entity(self, memory_client: AdaptiveMemoryClient):
        """Test creating KG entity."""
        entity_id = memory_client.create_entity(
            entity_name="Test Entity",
            entity_type="Test Type",
            description="A test entity"
        )
        assert entity_id.startswith("entity_")

    def test_transfer_stm_to_ltm(self, memory_client: AdaptiveMemoryClient, sample_stm_data: dict):
        """Test STM to LTM transfer."""
        # Store some STM
        result = memory_client.store_stm(**sample_stm_data)
        session_id = result["sessionId"]

        # Transfer
        transfer_result = memory_client.transfer_stm_to_ltm(session_id)
        assert "transferredCount" in transfer_result
```

---

## 8. Troubleshooting

### 8.1 Common Errors and Solutions

| Error | Cause | Solution |
|-------|-------|----------|
| `401 Unauthorized` | Token expired or invalid | Re-authenticate and get a new token |
| `Connection refused` | Server not running | Start backend server with `cargo run` |
| `503 Service Unavailable` | Qdrant/Neo4j not running | Check Docker containers: `docker compose ps` |
| `400 Bad Request` | Invalid request parameters | Check API documentation for required fields |
| `429 Too Many Requests` | Rate limit exceeded | Implement exponential backoff, reduce request frequency |
| `Missing tenant_id` | Multi-tenant endpoint without tenant | Add `X-Tenant-ID` header |
| `Embedding service unavailable` | Ollama not running | Start Ollama: `ollama serve` |

### 8.2 Debug Tips

```python
"""
debugging.py
Debug utilities for memory system integration
"""

import requests
import logging
from functools import wraps

# Enable request logging
logging.basicConfig(level=logging.DEBUG)
requests_log = logging.getLogger("urllib3")
requests_log.setLevel(logging.DEBUG)
requests_log.propagate = True


def debug_requests(func):
    """Decorator to debug API calls."""
    @wraps(func)
    def wrapper(*args, **kwargs):
        print(f"\n=== DEBUG: {func.__name__} ===")
        print(f"Args: {args}")
        print(f"Kwargs: {kwargs}")
        try:
            result = func(*args, **kwargs)
            print(f"Result: {result}")
            return result
        except Exception as e:
            print(f"Error: {e}")
            raise
        finally:
            print(f"=== END DEBUG ===\n")
    return wrapper


class DebugClient:
    """Client wrapper with debug output."""

    def __init__(self, client):
        self.client = client

    def __getattr__(self, name):
        """Proxy all calls through debug wrapper."""
        attr = getattr(self.client, name)
        if callable(attr):
            return debug_requests(attr)
        return attr


# Usage in tests
def test_with_debug():
    client = AdaptiveMemoryClient()
    debug_client = DebugClient(client)

    # All calls will be logged
    debug_client.health_check()
    debug_client.store_stm(
        user_id="test",
        agent_id="test",
        content="test"
    )
```

### 8.3 Network/Firewall Checklist

```yaml
# Required ports and firewall rules

# Backend API
- Port: 8008 (TCP) - Main API server
  - Required for: All API clients

# Databases
- Port: 5432 (TCP) - PostgreSQL
  - Required for: Backend connecting to database
  - Only if: Using external PostgreSQL

- Port: 6333/6334 (TCP) - Qdrant
  - Required for: Vector search operations
  - Only if: Using external Qdrant

- Port: 7474/7687 (TCP) - Neo4j
  - Required for: Knowledge graph operations
  - Optional: Can use PostgreSQL fallback

# Ollama (if using local embedding models)
- Port: 11434 (TCP) - Ollama API
  - Required for: Embedding generation
  - Optional: If using external embedding service

# Firewall Configuration Example (ufw)
# sudo ufw allow 8008/tcp   # API
# sudo ufw allow 5432/tcp   # PostgreSQL
# sudo ufw allow 6333:6334/tcp  # Qdrant
# sudo ufw allow 11434/tcp  # Ollama
```

### 8.4 Diagnostic Script

```bash
#!/bin/bash
# diagnose.sh - Diagnose common integration issues

BASE_URL="${BASE_URL:-http://localhost:8008}"
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "=== Adaptive Memory System Diagnostic ==="
echo "Base URL: $BASE_URL"
echo ""

# Check 1: API server reachable
echo -n "1. API Server: "
if curl -s --connect-timeout 5 "$BASE_URL/api/memory/health" > /dev/null 2>&1; then
    echo -e "${GREEN}Reachable${NC}"
else
    echo -e "${RED}Not reachable${NC} - Is the backend running?"
    exit 1
fi

# Check 2: Authentication
echo -n "2. Authentication: "
TOKEN=$(curl -s -X POST "$BASE_URL/api/login" \
    -H "Content-Type: application/json" \
    -d '{"username":"admin","password":"admin123"}' | jq -r '.token' 2>/dev/null)

if [ "$TOKEN" = "null" ] || [ -z "$TOKEN" ]; then
    echo -e "${RED}Failed${NC} - Check credentials"
else
    echo -e "${GREEN}OK${NC} (token: ${TOKEN:0:15}...)"
fi

# Check 3: STM operations
echo -n "3. STM Storage: "
STM_RESULT=$(curl -s -X POST "$BASE_URL/api/v1/memory/storage/stm" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $TOKEN" \
    -d '{"userId":"diag","agentId":"diag","sessionType":"conversation","role":"user","content":"test"}')

if echo "$STM_RESULT" | jq -e '.sessionId' > /dev/null 2>&1; then
    SESSION_ID=$(echo "$STM_RESULT" | jq -r '.sessionId')
    echo -e "${GREEN}OK${NC} (session: ${SESSION_ID:0:15}...)"
else
    echo -e "${RED}Failed${NC}"
fi

# Check 4: LTM operations
echo -n "4. LTM Storage: "
LTM_RESULT=$(curl -s -X POST "$BASE_URL/api/v1/memory/storage/ltm" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $TOKEN" \
    -d '{"sourceId":"diag","content":"test content","title":"Diagnostic"}')

if echo "$LTM_RESULT" | jq -e '.entryId' > /dev/null 2>&1; then
    echo -e "${GREEN}OK${NC}"
else
    echo -e "${YELLOW}Warning${NC} - May need Qdrant"
fi

# Check 5: Search operations
echo -n "5. LTM Search: "
SEARCH_RESULT=$(curl -s -X POST "$BASE_URL/api/v1/memory/search/ltm" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $TOKEN" \
    -d '{"query":"test","topK":3}')

if echo "$SEARCH_RESULT" | jq -e '.results' > /dev/null 2>&1; then
    echo -e "${GREEN}OK${NC}"
else
    echo -e "${YELLOW}Warning${NC} - Check Qdrant"
fi

# Check 6: KG operations
echo -n "6. KG Entity: "
KG_RESULT=$(curl -s -X POST "$BASE_URL/api/kg/entities" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $TOKEN" \
    -d '{"entityName":"Test","entityType":"Diagnostic"}')

if echo "$KG_RESULT" | jq -e '.entityId' > /dev/null 2>&1; then
    echo -e "${GREEN}OK${NC}"
else
    echo -e "${YELLOW}Warning${NC} - Check Neo4j"
fi

# Summary
echo ""
echo "=== Diagnostic Complete ==="
echo "If all checks passed, your integration should work."
echo "Yellow warnings indicate optional dependencies."
echo "Red failures indicate required services are down."
```

### 8.5 Environment Variable Reference

```bash
# .env.example - Environment variables for production deployment

# Backend Configuration
BACKEND_HOST=0.0.0.0
BACKEND_PORT=8008
RUST_LOG=info

# Database
DATABASE_URL=postgresql://memory:memory@postgres:5432/memory
DATABASE_POOL_SIZE=10

# Qdrant Vector Database
QDRANT_URL=http://qdrant:6334
QDRANT_COLLECTION=memory_ltm

# Neo4j (Optional)
NEO4J_URI=bolt://neo4j:7687
NEO4J_USER=neo4j
NEO4J_PASSWORD=password

# Ollama Embedding (Optional)
OLLAMA_BASE_URL=http://localhost:11434
EMBEDDING_MODEL=nomic-embed-text

# Authentication
JWT_SECRET=your-super-secret-jwt-key-change-in-production
TOKEN_EXPIRY_HOURS=24

# Multi-tenancy
DEFAULT_TENANT_ID=default

# Rate Limiting
RATE_LIMIT_REQUESTS=100
RATE_LIMIT_WINDOW=60

# External Services (for webhooks/callbacks)
EXTERNAL_CALLBACK_URL=https://your-service.com/webhook
WEBHOOK_SECRET=your-webhook-secret
```

---

## Quick Reference

### Base URLs

| Environment | URL |
|-------------|-----|
| Local Development | `http://localhost:8008` |
| Docker Compose | `http://backend:8008` |
| Production | `https://api.your-domain.com` |

### Authentication

```python
# Get token
response = requests.post(f"{BASE_URL}/api/login", json={
    "username": "admin",
    "password": "admin123"
})
token = response.json()["token"]

# Use token
headers = {"Authorization": f"Bearer {token}"}
```

### Essential API Calls

```python
# 1. Health check
client.health_check()

# 2. Select memory config
client.select_memory_config("user wants to learn Python")

# 3. Store conversation
client.store_stm(user_id="user1", agent_id="agent1", content="Hello", role="user")

# 4. Search knowledge
results = client.search_ltm("Python programming", top_k=5)

# 5. Transfer to LTM
client.transfer_stm_to_ltm(session_id)
```

---

*Document Version: 1.0*
*Last Updated: 2026-03-29*
