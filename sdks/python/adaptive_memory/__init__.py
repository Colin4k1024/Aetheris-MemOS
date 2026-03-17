"""
Adaptive Memory Python SDK

A Python SDK for interacting with the Adaptive Memory System.
"""

__version__ = "0.1.0"

from .client import MemoryClient, AsyncMemoryClient
from .models import (
    MemoryEntry,
    MemorySearchResult,
    MemoryConfig,
    Session,
)

__all__ = [
    "MemoryClient",
    "AsyncMemoryClient",
    "MemoryEntry",
    "MemorySearchResult",
    "MemoryConfig",
    "Session",
]
