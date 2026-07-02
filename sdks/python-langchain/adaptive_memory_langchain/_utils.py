"""Shared utilities for response parsing and type conversion."""

from __future__ import annotations

from typing import Any, Dict, List


def extract_search_results(response: Dict[str, Any]) -> List[Dict[str, Any]]:
    """Extract search result items from various API response shapes.

    The backend may return results under different keys depending on the
    endpoint (hybrid search vs MCP tool call vs direct LTM search).
    """
    if "results" in response:
        return response["results"]
    if "content" in response and isinstance(response["content"], list):
        # MCP tool response - content is a list of text items
        import json as _json

        items = []
        for item in response["content"]:
            if isinstance(item, dict) and "text" in item:
                try:
                    parsed = _json.loads(item["text"])
                    if isinstance(parsed, list):
                        items.extend(parsed)
                    elif isinstance(parsed, dict) and "results" in parsed:
                        items.extend(parsed["results"])
                    else:
                        items.append(parsed)
                except (ValueError, TypeError):
                    pass
            elif isinstance(item, str):
                try:
                    parsed = _json.loads(item)
                    if isinstance(parsed, list):
                        items.extend(parsed)
                    else:
                        items.append(parsed)
                except (ValueError, TypeError):
                    pass
        return items
    return []


def parse_message_role(role: str) -> str:
    """Normalize role string to LangChain convention."""
    role = role.lower().strip()
    if role in ("user", "human"):
        return "human"
    if role in ("assistant", "ai", "bot"):
        return "ai"
    if role in ("system",):
        return "system"
    return "human"
