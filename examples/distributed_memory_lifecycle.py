"""
分布式记忆生命周期演示

本脚本演示完整的记忆生命周期：
  STM（短时记忆）存储 → 传输到 LTM（长时记忆）→ 混合搜索 → 遗忘 → 解释 → 反馈

运行方式：
    python examples/distributed_memory_lifecycle.py

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
BASE_URL = os.getenv("AETHERIS_BASE_URL", "http://localhost:8008")


def section(title: str) -> None:
    """打印章节分隔符。"""
    print(f"\n{'=' * 60}")
    print(f"  {title}")
    print(f"{'=' * 60}")


def main() -> None:
    client = MemoryClient(base_url=BASE_URL)

    # ------------------------------------------------------------------
    # 1. 健康检查 — 确认后端可达
    # ------------------------------------------------------------------
    section("1. 健康检查")
    try:
        health = client.health_check()
        print(f"[OK] 后端状态: {health}")
    except Exception as exc:
        print(f"[ERROR] 后端不可达: {exc}")
        print("请确保后端已启动：cd backend && cargo run")
        sys.exit(1)

    # ------------------------------------------------------------------
    # 2. 向 STM 写入 5 条对话消息（复用同一个 sessionId）
    # ------------------------------------------------------------------
    section("2. 存储 STM 消息（5 条对话消息）")

    user_id = "lifecycle-user-001"
    agent_id = "lifecycle-agent"

    # 对话内容：模拟一次关于 Rust 异步编程的技术讨论
    messages = [
        ("user",      "我想了解 Rust 的 async/await 机制是如何工作的。"),
        ("assistant", "Rust 的异步模型基于 Future trait，执行器（executor）负责轮询 Future 直到完成。"),
        ("user",      "Tokio 和 async-std 有什么区别？"),
        ("assistant", "Tokio 更适合生产环境，提供完整的异步运行时；async-std 接口更接近标准库。"),
        ("user",      "我应该在什么场景下选择 Tokio？"),
    ]

    session_id = None  # 第一次调用时自动创建 session

    for role, content in messages:
        resp = client.store_stm(
            user_id=user_id,
            agent_id=agent_id,
            content=content,
            session_type="conversation",
            role=role,
            session_id=session_id,
        )
        # 复用第一次返回的 sessionId，将后续消息追加到同一 session
        if session_id is None:
            session_id = resp.get("sessionId")
            print(f"[创建] 新 Session: {session_id}")
        print(f"  [{role:9s}] messageId={resp.get('messageId')}  内容: {content[:40]}...")

    print(f"\n共写入 {len(messages)} 条 STM 消息，sessionId = {session_id}")

    # ------------------------------------------------------------------
    # 3. 列出 STM sessions，确认写入成功
    # ------------------------------------------------------------------
    section("3. 列出 STM Sessions")
    try:
        sessions_resp = client.list_sessions(user_id=user_id, limit=5)
        sessions = sessions_resp if isinstance(sessions_resp, list) else sessions_resp.get("sessions", [])
        print(f"查询到 {len(sessions)} 个 session:")
        for s in sessions:
            print(f"  - sessionId={s.get('sessionId') or s.get('id')}  "
                  f"type={s.get('sessionType')}  "
                  f"messages={s.get('messageCount', '?')}")
    except Exception as exc:
        print(f"[WARN] list_sessions 失败: {exc}")

    # ------------------------------------------------------------------
    # 4. 将 STM 传输到 LTM（调用传输接口）
    # ------------------------------------------------------------------
    section("4. STM → LTM 传输")
    try:
        transfer_resp = requests.post(
            f"{BASE_URL}/api/v1/memory/storage/transfer",
            json={"sessionId": session_id},
            timeout=30,
        )
        transfer_resp.raise_for_status()
        print(f"[OK] 传输结果: {transfer_resp.json()}")
    except requests.HTTPError as exc:
        print(f"[WARN] 传输接口返回错误 (状态码 {exc.response.status_code}): {exc}")
    except Exception as exc:
        print(f"[WARN] 传输请求失败: {exc}")

    # ------------------------------------------------------------------
    # 5. 直接写入一条独立 LTM 条目（知识库文档片段）
    # ------------------------------------------------------------------
    section("5. 存储独立 LTM 条目")
    entry_id = None
    try:
        ltm_resp = client.store_ltm(
            source_id="rust-book-ch17",
            source_type="document",
            content=(
                "Rust 的 async/await 语法糖在编译期被展开为状态机。"
                "每个 .await 点对应一次 Future::poll 调用。"
                "Tokio 提供多线程工作窃取（work-stealing）调度器，"
                "适合 I/O 密集型高并发服务。"
            ),
            title="Rust 异步编程核心概念",
        )
        entry_id = ltm_resp.get("entryId")
        print(f"[OK] LTM 条目已创建，entryId = {entry_id}")
        print(f"     响应: {ltm_resp}")
    except Exception as exc:
        print(f"[WARN] LTM 存储失败（需要 Embedding 服务，请确认 Ollama 已运行）: {exc}")
        print("       跳过后续依赖 LTM 的操作...")

    # ------------------------------------------------------------------
    # 6. 混合搜索（语义 + 关键词）
    # ------------------------------------------------------------------
    section("6. 混合搜索")
    query = "Rust 异步运行时 Tokio 如何调度任务"
    try:
        search_resp = client.search_hybrid(query=query, user_id=user_id, limit=5)
        results = (
            search_resp if isinstance(search_resp, list)
            else search_resp.get("results", search_resp.get("data", []))
        )
        print(f"查询: '{query}'")
        print(f"返回 {len(results)} 条结果:")
        for i, r in enumerate(results, 1):
            # 字段名可能是 content / text / snippet
            snippet = (
                r.get("content") or r.get("text") or r.get("snippet") or str(r)
            )[:80]
            score = r.get("score", r.get("relevanceScore", "N/A"))
            print(f"  [{i}] score={score}  {snippet}...")
    except Exception as exc:
        print(f"[WARN] 混合搜索失败: {exc}")
        results = []

    # ------------------------------------------------------------------
    # 7. 遗忘一条 LTM 条目
    # ------------------------------------------------------------------
    section("7. 遗忘 LTM 条目")
    forget_id = entry_id  # 遗忘刚才写入的独立条目
    if forget_id:
        try:
            forget_resp = client.forget(memory_id=forget_id, layer="ltm")
            print(f"[OK] 已遗忘 entryId={forget_id}")
            print(f"     响应: {forget_resp}")
        except Exception as exc:
            print(f"[WARN] 遗忘操作失败: {exc}")
    else:
        print("[SKIP] 无可遗忘的条目（entryId 为空）")

    # ------------------------------------------------------------------
    # 8. 获取决策追踪（explain）
    # ------------------------------------------------------------------
    section("8. 获取决策追踪（Explain）")
    try:
        explain_resp = client.explain(limit=5)
        traces = (
            explain_resp if isinstance(explain_resp, list)
            else explain_resp.get("traces", explain_resp.get("data", []))
        )
        print(f"最近 {len(traces)} 条决策追踪:")
        for t in traces:
            trace_id = t.get("traceId") or t.get("id", "N/A")
            action   = t.get("action") or t.get("decision", "N/A")
            ts       = t.get("createdAt") or t.get("timestamp", "N/A")
            print(f"  - traceId={trace_id}  action={action}  time={ts}")
    except Exception as exc:
        print(f"[WARN] explain 接口失败: {exc}")
        traces = []

    # ------------------------------------------------------------------
    # 9. 反馈 — 标记搜索结果是否有用
    # ------------------------------------------------------------------
    section("9. 提交反馈（Feedback）")
    # 取第一条搜索结果的 ID 做反馈；若搜索无结果则用 entry_id 兜底
    feedback_target = None
    if results:
        r0 = results[0]
        feedback_target = (
            r0.get("id") or r0.get("memoryId") or r0.get("entryId")
        )
    if not feedback_target:
        feedback_target = entry_id  # 用 LTM 条目 ID 兜底

    if feedback_target:
        try:
            fb_resp = client.feedback(
                memory_id=feedback_target,
                useful=True,
                query=query,
                metadata={"demo": "distributed_memory_lifecycle", "step": 9},
            )
            print(f"[OK] 反馈已提交，memoryId={feedback_target}")
            print(f"     响应: {fb_resp}")
        except Exception as exc:
            print(f"[WARN] 反馈提交失败: {exc}")
    else:
        print("[SKIP] 无可反馈的记忆 ID")

    # ------------------------------------------------------------------
    # 汇总
    # ------------------------------------------------------------------
    section("演示完成 — 汇总")
    print(f"  STM sessionId : {session_id}")
    print(f"  LTM entryId   : {entry_id}")
    print(f"  搜索结果数量   : {len(results)}")
    print(f"  决策追踪数量   : {len(traces) if isinstance(traces, list) else '?'}")
    print(f"  反馈目标 ID    : {feedback_target}")
    print("\n分布式记忆生命周期演示结束。")

    client.close()


if __name__ == "__main__":
    main()
