"""LangChain BaseChatMessageHistory implementation backed by Aetheris MemOS STM."""

from __future__ import annotations

from typing import List, Optional

from langchain_core.chat_history import BaseChatMessageHistory
from langchain_core.messages import (
    AIMessage,
    BaseMessage,
    HumanMessage,
    SystemMessage,
)

from adaptive_memory import MemoryClient

from ._utils import parse_message_role


class AdaptiveMemoryChatMessageHistory(BaseChatMessageHistory):
    """Chat message history persisted in Aetheris MemOS short-term memory.

    Each instance is bound to a specific user/agent/session triple.  Messages
    are stored in STM and retrieved as a chronological list.

    Example:
        >>> from adaptive_memory import MemoryClient
        >>> from adaptive_memory_langchain import AdaptiveMemoryChatMessageHistory
        >>>
        >>> client = MemoryClient("http://localhost:8008", api_key="...")
        >>> history = AdaptiveMemoryChatMessageHistory(
        ...     client=client,
        ...     user_id="user-1",
        ...     agent_id="agent-1",
        ...     session_id="session-abc",
        ... )
        >>> history.add_user_message("Hello!")
        >>> history.add_ai_message("Hi there!")
        >>> print(history.messages)
    """

    def __init__(
        self,
        client: MemoryClient,
        user_id: str,
        agent_id: str,
        session_id: Optional[str] = None,
        session_type: str = "conversation",
    ) -> None:
        """Initialize chat message history.

        Args:
            client: Adaptive Memory client instance.
            user_id: User identifier for session scoping.
            agent_id: Agent identifier for session scoping.
            session_id: Explicit session ID.  If None, one will be created on
                first write.
            session_type: Session type for STM (conversation, task, query).
        """
        self.client = client
        self.user_id = user_id
        self.agent_id = agent_id
        self.session_id = session_id
        self.session_type = session_type

    @property
    def messages(self) -> List[BaseMessage]:
        """Retrieve all messages from the session."""
        if not self.session_id:
            return []

        try:
            response = self.client.recall_session(
                session_id=self.session_id, limit=1000
            )
        except Exception:
            return []

        messages: List[BaseMessage] = []
        items = response if isinstance(response, list) else response.get("messages", [])

        for item in items:
            content = item.get("content", "")
            role = parse_message_role(item.get("role", "user"))

            if role == "ai":
                messages.append(AIMessage(content=content))
            elif role == "system":
                messages.append(SystemMessage(content=content))
            else:
                messages.append(HumanMessage(content=content))

        return messages

    def add_messages(self, messages: List[BaseMessage]) -> None:
        """Add messages to the session.

        If no session_id exists yet, the first write will create one and store
        the session_id for subsequent operations.
        """
        for message in messages:
            role = self._message_to_role(message)
            result = self.client.store_stm(
                user_id=self.user_id,
                agent_id=self.agent_id,
                content=message.content,
                session_type=self.session_id or self.session_type,
                role=role,
            )
            # Capture session_id from first successful write
            if not self.session_id and "sessionId" in result:
                self.session_id = result["sessionId"]

    def clear(self) -> None:
        """Clear all messages in the current session.

        Note: This attempts to forget the session.  If the server does not
        support session-level forget, messages may remain but a new session_id
        will be used for subsequent writes.
        """
        if self.session_id:
            try:
                self.client.forget(memory_id=self.session_id, layer="stm")
            except Exception:
                pass
            self.session_id = None

    @staticmethod
    def _message_to_role(message: BaseMessage) -> str:
        """Map a LangChain message to a MemOS role string."""
        if isinstance(message, HumanMessage):
            return "user"
        if isinstance(message, AIMessage):
            return "assistant"
        if isinstance(message, SystemMessage):
            return "system"
        return "user"
