"""
Adaptive Memory Data Models

Pydantic models for type-safe interactions with the Adaptive Memory System.
"""

from typing import Optional, List, Dict, Any, Literal
from datetime import datetime
from pydantic import BaseModel, Field


class MemoryMetadata(BaseModel):
    """Memory metadata."""

    user_id: Optional[str] = None
    session_id: Optional[str] = None
    agent_id: Optional[str] = None
    tags: List[str] = Field(default_factory=list)
    importance: float = 0.5
    access_count: int = 0
    last_accessed: Optional[datetime] = None
    expires_at: Optional[datetime] = None
    source: Optional[str] = None


class MemoryEntry(BaseModel):
    """A memory entry."""

    id: str
    layer: Literal["stm", "ltm", "kg", "mm"]
    content: str
    metadata: MemoryMetadata = Field(default_factory=MemoryMetadata)
    created_at: datetime
    updated_at: datetime


class MemorySearchResult(BaseModel):
    """Memory search result."""

    entry: MemoryEntry
    score: float
    highlights: List[str] = Field(default_factory=list)


class Session(BaseModel):
    """A session in short-term memory."""

    session_id: str
    user_id: str
    agent_id: str
    session_type: str
    status: str
    created_at: datetime
    updated_at: datetime
    context_length: int = 0
    max_context_length: int = 4000


class SessionMessage(BaseModel):
    """A message within a session."""

    message_id: str
    session_id: str
    role: str
    content: str
    created_at: datetime
    token_count: Optional[int] = None
    importance_score: Optional[float] = None


class MemoryConfig(BaseModel):
    """Memory configuration."""

    primary_memory: str
    secondary_memory: List[str] = Field(default_factory=list)
    stm_weight: float = 0.25
    ltm_weight: float = 0.25
    kg_weight: float = 0.25
    mm_weight: float = 0.25
    reasoning_depth: str = "medium"
    enable_multimodal: bool = False


class TaskCharacteristic(BaseModel):
    """Task characteristic analysis."""

    complexity: str
    modality: str
    reasoning_depth: str
    estimated_tokens: int
    recommended_memory_layers: List[str]


class DecisionTrace(BaseModel):
    """Decision trace for memory selection."""

    task_description: str
    selected_config: MemoryConfig
    reasoning: List[str]
    timestamp: datetime
