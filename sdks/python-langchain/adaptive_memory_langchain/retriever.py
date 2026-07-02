"""LangChain BaseRetriever implementation backed by Aetheris MemOS search."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from langchain_core.callbacks import CallbackManagerForRetrieverRun
from langchain_core.documents import Document
from langchain_core.retrievers import BaseRetriever
from pydantic import Field

from adaptive_memory import MemoryClient

from ._utils import extract_search_results


class AdaptiveMemoryRetriever(BaseRetriever):
    """Retriever that searches Aetheris MemOS for relevant memory documents.

    Supports multiple search strategies: hybrid, ltm, triple, scored.
    Returns LangChain Document objects with memory metadata.

    Example:
        >>> from adaptive_memory import MemoryClient
        >>> from adaptive_memory_langchain import AdaptiveMemoryRetriever
        >>>
        >>> client = MemoryClient("http://localhost:8008", api_key="...")
        >>> retriever = AdaptiveMemoryRetriever(
        ...     client=client,
        ...     search_type="hybrid",
        ...     top_k=5,
        ... )
        >>> docs = retriever.invoke("What did the user say about preferences?")
    """

    client: Any = Field(description="MemoryClient instance")
    search_type: str = Field(
        default="hybrid",
        description="Search strategy: hybrid, ltm, triple, scored",
    )
    top_k: int = Field(default=5, description="Maximum documents to return")
    min_score: float = Field(
        default=0.0, description="Minimum score threshold (0.0 to 1.0)"
    )
    user_id: Optional[str] = Field(
        default=None, description="User ID for scoped search"
    )

    class Config:
        arbitrary_types_allowed = True

    def _get_relevant_documents(
        self,
        query: str,
        *,
        run_manager: CallbackManagerForRetrieverRun,
    ) -> List[Document]:
        """Search MemOS and return matching documents.

        Args:
            query: Search query string.
            run_manager: Callback manager (unused but required by interface).

        Returns:
            List of Document objects with page_content and metadata.
        """
        response = self.client.search(
            query=query,
            layer=self.search_type,
            user_id=self.user_id,
            limit=self.top_k,
        )

        results = extract_search_results(response)
        documents: List[Document] = []

        for item in results:
            score = item.get("score", 0.0)
            if score < self.min_score:
                continue

            content = (
                item.get("content")
                or item.get("summary")
                or item.get("title")
                or ""
            )

            metadata: Dict[str, Any] = {
                "memory_id": item.get("memoryId") or item.get("entry_id", ""),
                "source_layer": item.get("sourceLayer", self.search_type),
                "score": score,
            }

            if item.get("traceId"):
                metadata["trace_id"] = item["traceId"]
            if item.get("explanation"):
                metadata["explanation"] = item["explanation"]
            if item.get("title"):
                metadata["title"] = item["title"]
            if item.get("metadata") and isinstance(item["metadata"], dict):
                metadata.update(item["metadata"])

            documents.append(Document(page_content=content, metadata=metadata))

        return documents
