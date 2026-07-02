"""
Adaptive Memory LangChain Integration

Provides LangChain-native interfaces for Aetheris MemOS:
- AdaptiveMemoryChatMessageHistory (BaseChatMessageHistory)
- AdaptiveMemoryRetriever (BaseRetriever)
- AdaptiveMemoryTool (BaseTool)
- create_memory_tools() factory
"""

__version__ = "0.1.0"

from .chat_message_history import AdaptiveMemoryChatMessageHistory
from .retriever import AdaptiveMemoryRetriever
from .tool import AdaptiveMemoryTool, create_memory_tools

__all__ = [
    "AdaptiveMemoryChatMessageHistory",
    "AdaptiveMemoryRetriever",
    "AdaptiveMemoryTool",
    "create_memory_tools",
]
