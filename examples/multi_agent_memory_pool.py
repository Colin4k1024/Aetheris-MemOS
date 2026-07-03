"""
多 Agent 共享记忆池演示

本脚本演示多个 AI Agent 之间如何共享和协作记忆：
  注册 Agent → 各自存储记忆 → 共享记忆 → 查询可见记忆

注意：/api/v1/memory/memory-pool/* 接口为实验性功能，
      若后端未实现则会优雅降级并打印提示。

运行方式：
    python examples/multi_agent_memory_pool.py

环境变量：
    AETHERIS_BASE_URL  默认: http://localhost:8008
"""

import os
import sys

# 将 SDK 路径加入 Python 模块搜索路径
_REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(_REPO_ROOT, "sdks", "python"))

import requests
from adaptive_memory import MemoryClient

# 从环境变量读取后端地址，默认指向本地开发服务器
BASE_URL = os.getenv("AETHERIS_BASE_URL", "http://localhost:8008").rstrip("/")

# Agent 定义
RESEARCH_AGENT = "research-agent"
CODING_AGENT   = "coding-agent"
USER_ID        = "pool-demo-user"


def pool_api(method: str, path: str, **kwargs) -> requests.Response:
    """
    封装记忆池 HTTP 请求：自动拼接 BASE_URL、设置超时。

    Args:
        method: HTTP 方法（GET / POST / PUT / DELETE）
        path:   API 路径，以 / 开头（通常为 /api/v1/memory/memory-pool/...）
        **kwargs: 透传给 requests.request（json、params 等）
    Returns:
        requests.Response 对象（未自动 raise_for_status）
    """
    url = f"{BASE_URL}{path}"
    return requests.request(method, url, timeout=30, **kwargs)


def section(title: str) -> None:
    """打印章节分隔符。"""
    print(f"\n{'=' * 60}")
    print(f"  {title}")
    print(f"{'=' * 60}")


def register_agent(agent_id: str, capabilities: list[str]) -> str | None:
    """
    注册 Agent 到记忆池。

    返回 agentPoolId（若端点可用），否则返回 None。
    """
    resp = pool_api(
        "POST",
        "/api/v1/memory/memory-pool/register",
        json={"agentId": agent_id, "capabilities": capabilities},
    )
    if resp.status_code in (200, 201):
        data = resp.json()
        pool_id = (
            data.get("agentPoolId")
            or data.get("poolId")
            or data.get("id")
            or agent_id  # 后端可能只返回 agentId 本身
        )
        print(f"[OK] 注册成功: agentId={agent_id}  poolId={pool_id}")
        return pool_id
    elif resp.status_code == 404:
        print(f"[SKIP] 记忆池注册接口不可用 (404)，跳过 Agent '{agent_id}' 注册")
        return None
    else:
        print(f"[WARN] 注册 Agent '{agent_id}' 失败: "
              f"HTTP {resp.status_code}  {resp.text[:120]}")
        return None


def main() -> None:
    client = MemoryClient(base_url=BASE_URL)

    # ------------------------------------------------------------------
    # 1. 注册两个 Agent 到记忆池
    # ------------------------------------------------------------------
    section("1. 注册 Agent 到记忆池")

    try:
        research_pool_id = register_agent(
            RESEARCH_AGENT,
            capabilities=["literature_search", "paper_summary", "hypothesis_generation"],
        )
        coding_pool_id = register_agent(
            CODING_AGENT,
            capabilities=["code_generation", "code_review", "debugging", "refactoring"],
        )
    except Exception as exc:
        print(f"[WARN] 注册请求异常: {exc}")
        research_pool_id = None
        coding_pool_id   = None

    # ------------------------------------------------------------------
    # 2. 研究 Agent 存储调研记忆
    # ------------------------------------------------------------------
    section("2. 研究 Agent 存储调研记忆")

    research_memories = [
        "最新论文显示，混合精度训练可将 LLM 微调速度提升 2.3 倍，同时保持 99.1% 的精度。",
        "调研结论：RAG（检索增强生成）与微调结合使用时，幻觉率降低 47%。",
        "数据集分析：用于代码生成任务的 instruction tuning 数据集最优长度约为 512 tokens。",
    ]

    research_message_ids: list[str] = []
    research_session_id = None

    for content in research_memories:
        resp = client.store_stm(
            user_id=USER_ID,
            agent_id=RESEARCH_AGENT,
            content=content,
            session_type="task",
            role="assistant",
            session_id=research_session_id,
        )
        if research_session_id is None:
            research_session_id = resp.get("sessionId")
        mid = resp.get("messageId")
        research_message_ids.append(mid)
        print(f"  [research-agent] 存储: {content[:50]}...  messageId={mid}")

    print(f"\n研究 Agent sessionId: {research_session_id}")
    print(f"共存储 {len(research_message_ids)} 条调研记忆")

    # ------------------------------------------------------------------
    # 3. 编程 Agent 存储代码实现记忆
    # ------------------------------------------------------------------
    section("3. 编程 Agent 存储代码实现记忆")

    coding_memories = [
        "已实现基于 LoRA 的微调脚本，支持 4-bit 量化，显存占用减少 60%。",
        "代码审查发现：批处理大小设置为 16 时，训练吞吐量最优（A100 GPU）。",
    ]

    coding_session_id = None

    for content in coding_memories:
        resp = client.store_stm(
            user_id=USER_ID,
            agent_id=CODING_AGENT,
            content=content,
            session_type="task",
            role="assistant",
            session_id=coding_session_id,
        )
        if coding_session_id is None:
            coding_session_id = resp.get("sessionId")
        mid = resp.get("messageId")
        print(f"  [coding-agent]    存储: {content[:50]}...  messageId={mid}")

    print(f"\n编程 Agent sessionId: {coding_session_id}")

    # ------------------------------------------------------------------
    # 4. 将研究 Agent 的记忆共享给编程 Agent
    # ------------------------------------------------------------------
    section("4. 跨 Agent 记忆共享")

    # 使用第一条研究记忆的 messageId 进行共享
    share_memory_id = research_message_ids[0] if research_message_ids else None

    if share_memory_id:
        try:
            share_resp = pool_api(
                "POST",
                "/api/v1/memory/memory-pool/share",
                json={
                    "ownerId":       RESEARCH_AGENT,
                    "targetAgentId": CODING_AGENT,
                    "memoryId":      share_memory_id,
                    "permissions":   "read",
                },
            )
            if share_resp.status_code in (200, 201):
                print(f"[OK] 共享成功: research-agent → coding-agent")
                print(f"     共享记忆 ID: {share_memory_id}")
                print(f"     权限级别:   read")
                print(f"     响应: {share_resp.json()}")
            elif share_resp.status_code == 404:
                print(f"[SKIP] 记忆共享接口不可用 (404)，该功能尚未实现")
            else:
                print(f"[WARN] 共享失败: HTTP {share_resp.status_code}  {share_resp.text[:120]}")
        except Exception as exc:
            print(f"[WARN] 共享请求异常: {exc}")
    else:
        print("[SKIP] 无可共享的记忆 ID")

    # ------------------------------------------------------------------
    # 5. 查询编程 Agent 可见的记忆
    # ------------------------------------------------------------------
    section("5. 查询编程 Agent 可见记忆")

    try:
        visible_resp = pool_api(
            "GET",
            f"/api/v1/memory/memory-pool/visible/{CODING_AGENT}",
        )
        if visible_resp.status_code == 200:
            data = visible_resp.json()
            visible_memories = (
                data if isinstance(data, list)
                else data.get("memories", data.get("data", []))
            )
            print(f"编程 Agent 可见记忆（共 {len(visible_memories)} 条）:")
            for m in visible_memories:
                content   = m.get("content") or m.get("text") or str(m)
                owner     = m.get("ownerId") or m.get("agentId", "unknown")
                memory_id = m.get("memoryId") or m.get("id", "N/A")
                print(f"  - [来自 {owner}] {content[:60]}...  id={memory_id}")
        elif visible_resp.status_code == 404:
            print(f"[SKIP] 可见记忆查询接口不可用 (404)，该功能尚未实现")
            print(f"       编程 Agent 将通过 STM 搜索访问自己的记忆")
        else:
            print(f"[WARN] 查询可见记忆失败: HTTP {visible_resp.status_code}  {visible_resp.text[:120]}")
    except Exception as exc:
        print(f"[WARN] 可见记忆查询异常: {exc}")

    # ------------------------------------------------------------------
    # 6. 兜底验证：通过混合搜索验证跨 Agent 记忆可达性
    # ------------------------------------------------------------------
    section("6. 兜底验证：通过混合搜索检索 Agent 记忆")

    fallback_query = "LLM 微调训练优化方法"
    try:
        search_resp = client.search_hybrid(
            query=fallback_query,
            user_id=USER_ID,
            limit=5,
        )
        results = (
            search_resp if isinstance(search_resp, list)
            else search_resp.get("results", search_resp.get("data", []))
        )
        print(f"搜索 '{fallback_query}' 返回 {len(results)} 条结果:")
        for i, r in enumerate(results, 1):
            snippet = (r.get("content") or r.get("text") or str(r))[:70]
            score   = r.get("score", r.get("relevanceScore", "N/A"))
            print(f"  [{i}] score={score}  {snippet}...")
    except Exception as exc:
        print(f"[WARN] 混合搜索失败: {exc}")

    # ------------------------------------------------------------------
    # 汇总
    # ------------------------------------------------------------------
    section("多 Agent 记忆池演示完成 — 协作摘要")
    print(f"  研究 Agent:    {RESEARCH_AGENT}")
    print(f"    sessionId:   {research_session_id}")
    print(f"    存储记忆数:  {len(research_memories)} 条")
    print(f"    共享记忆 ID: {share_memory_id}")
    print()
    print(f"  编程 Agent:    {CODING_AGENT}")
    print(f"    sessionId:   {coding_session_id}")
    print(f"    存储记忆数:  {len(coding_memories)} 条")
    print()
    print("  说明：/api/v1/memory/memory-pool/* 为实验性接口。")
    print("        若返回 404，表示该功能尚在开发中，脚本已优雅降级。")
    print("\n多 Agent 记忆共享演示结束。")

    client.close()


if __name__ == "__main__":
    main()
