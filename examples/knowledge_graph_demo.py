"""
知识图谱（KG）CRUD 与图遍历演示

本脚本通过原始 HTTP 调用演示知识图谱的完整操作：
  创建实体 → 列出实体 → 创建关系 → 获取关联实体 → 搜索图谱

SDK 目前未封装 KG 方法，因此直接使用 requests 调用 /api/kg/* 接口。

运行方式：
    python examples/knowledge_graph_demo.py

环境变量：
    AETHERIS_BASE_URL  默认: http://localhost:8008
"""

import os
import sys

# 将 SDK 路径加入 Python 模块搜索路径
_REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(_REPO_ROOT, "sdks", "python"))

import requests

# 从环境变量读取后端地址，默认指向本地开发服务器
BASE_URL = os.getenv("AETHERIS_BASE_URL", "http://localhost:8008").rstrip("/")


def api(method: str, path: str, **kwargs) -> requests.Response:
    """
    封装 HTTP 请求：自动拼接 BASE_URL、设置超时。

    Args:
        method: HTTP 方法（GET / POST / PUT / DELETE）
        path:   API 路径，以 / 开头
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


def main() -> None:

    # ------------------------------------------------------------------
    # 1. 创建 3 个实体：Python / FastAPI / PostgreSQL
    # ------------------------------------------------------------------
    section("1. 创建知识图谱实体")

    entities_to_create = [
        {
            "entityName": "Python",
            "entityType": "language",
            "description": "一种高层次的通用编程语言，以简洁易读著称。",
        },
        {
            "entityName": "FastAPI",
            "entityType": "framework",
            "description": "基于 Python 的现代高性能 Web 框架，支持异步处理与自动文档生成。",
        },
        {
            "entityName": "PostgreSQL",
            "entityType": "database",
            "description": "功能强大的开源关系型数据库，支持 JSONB、全文搜索等高级特性。",
        },
    ]

    entity_ids: dict[str, str] = {}  # 名称 → 实体 ID

    for entity in entities_to_create:
        resp = api("POST", "/api/kg/entities", json=entity)
        if resp.status_code in (200, 201):
            data = resp.json()
            # 兼容不同后端返回字段名：entityId / id
            eid = data.get("entityId") or data.get("id") or data.get("entity_id")
            entity_ids[entity["entityName"]] = eid
            print(f"[OK] 创建实体: {entity['entityName']}  "
                  f"(type={entity['entityType']})  id={eid}")
        else:
            print(f"[WARN] 创建实体 '{entity['entityName']}' 失败: "
                  f"HTTP {resp.status_code}  {resp.text[:120]}")

    print(f"\n实体 ID 映射: {entity_ids}")

    # ------------------------------------------------------------------
    # 2. 列出所有实体，确认写入成功
    # ------------------------------------------------------------------
    section("2. 列出所有实体")
    resp = api("GET", "/api/kg/entities")
    if resp.status_code == 200:
        data = resp.json()
        # 兼容返回格式：列表 / {"entities": [...]} / {"data": [...]}
        all_entities = (
            data if isinstance(data, list)
            else data.get("entities", data.get("data", []))
        )
        print(f"图谱中共有 {len(all_entities)} 个实体:")
        for e in all_entities[:10]:  # 最多显示 10 条
            name  = e.get("entityName") or e.get("name", "N/A")
            etype = e.get("entityType") or e.get("type", "N/A")
            eid   = e.get("entityId")   or e.get("id",   "N/A")
            print(f"  - [{etype:12s}] {name:15s}  id={eid}")
        if len(all_entities) > 10:
            print(f"  ... 还有 {len(all_entities) - 10} 个（已省略）")
    else:
        print(f"[WARN] 获取实体列表失败: HTTP {resp.status_code}  {resp.text[:120]}")

    # ------------------------------------------------------------------
    # 3. 创建 2 条关系
    #    FastAPI --depends_on--> Python
    #    FastAPI --connects_to--> PostgreSQL
    # ------------------------------------------------------------------
    section("3. 创建实体间关系")

    fastapi_id    = entity_ids.get("FastAPI")
    python_id     = entity_ids.get("Python")
    postgres_id   = entity_ids.get("PostgreSQL")

    relations_to_create = []
    if fastapi_id and python_id:
        relations_to_create.append({
            "sourceEntityId": fastapi_id,
            "targetEntityId": python_id,
            "relationType":   "depends_on",
            "weight":         1.0,
            "label":          "FastAPI 依赖 Python 运行时",
        })
    if fastapi_id and postgres_id:
        relations_to_create.append({
            "sourceEntityId": fastapi_id,
            "targetEntityId": postgres_id,
            "relationType":   "connects_to",
            "weight":         0.8,
            "label":          "FastAPI 通过 asyncpg/SQLAlchemy 连接 PostgreSQL",
        })

    relation_ids: list[str] = []

    for rel in relations_to_create:
        resp = api("POST", "/api/kg/relations", json=rel)
        if resp.status_code in (200, 201):
            data = resp.json()
            rid = data.get("relationId") or data.get("id") or data.get("relation_id")
            relation_ids.append(rid)
            print(f"[OK] 创建关系: {rel['relationType']}  "
                  f"({rel['sourceEntityId']} → {rel['targetEntityId']})  id={rid}")
        else:
            print(f"[WARN] 创建关系 '{rel['relationType']}' 失败: "
                  f"HTTP {resp.status_code}  {resp.text[:120]}")

    # ------------------------------------------------------------------
    # 4. 获取 FastAPI 的关联实体（图遍历）
    # ------------------------------------------------------------------
    section("4. 获取 FastAPI 的关联实体（图遍历）")
    if fastapi_id:
        resp = api("GET", f"/api/kg/entities/{fastapi_id}/related")
        if resp.status_code == 200:
            data = resp.json()
            related = (
                data if isinstance(data, list)
                else data.get("entities", data.get("related", data.get("data", [])))
            )
            print(f"FastAPI (id={fastapi_id}) 的关联实体（共 {len(related)} 个）:")
            for e in related:
                name     = e.get("entityName") or e.get("name", "N/A")
                etype    = e.get("entityType") or e.get("type", "N/A")
                rel_type = e.get("relationType") or e.get("relation", "N/A")
                print(f"  - {name} ({etype})  通过关系: {rel_type}")
        else:
            print(f"[WARN] 获取关联实体失败: HTTP {resp.status_code}  {resp.text[:120]}")
    else:
        print("[SKIP] FastAPI 实体 ID 未知，跳过图遍历")

    # ------------------------------------------------------------------
    # 5. 在知识图谱中搜索 "web framework"
    # ------------------------------------------------------------------
    section("5. 知识图谱搜索：'web framework'")
    resp = api("POST", "/api/kg/search", json={"query": "web framework"})
    if resp.status_code == 200:
        data = resp.json()
        results = (
            data if isinstance(data, list)
            else data.get("results", data.get("entities", data.get("data", [])))
        )
        print(f"搜索 'web framework' 返回 {len(results)} 个结果:")
        for r in results:
            name  = r.get("entityName") or r.get("name", "N/A")
            etype = r.get("entityType") or r.get("type", "N/A")
            score = r.get("score", r.get("relevanceScore", "N/A"))
            print(f"  - {name} ({etype})  score={score}")
    else:
        print(f"[WARN] KG 搜索失败: HTTP {resp.status_code}  {resp.text[:120]}")

    # ------------------------------------------------------------------
    # 汇总 — 图结构摘要
    # ------------------------------------------------------------------
    section("知识图谱演示完成 — 图结构摘要")
    print("  已创建实体:")
    for name, eid in entity_ids.items():
        print(f"    {name:15s}  id={eid}")
    print("\n  已创建关系:")
    rel_descriptions = [
        f"FastAPI --depends_on--> Python",
        f"FastAPI --connects_to--> PostgreSQL",
    ]
    for desc, rid in zip(rel_descriptions, relation_ids):
        print(f"    {desc}  id={rid}")
    print(
        "\n  图谱结构:\n"
        "    Python ←──(depends_on)── FastAPI ──(connects_to)──→ PostgreSQL\n"
        "\n知识图谱 CRUD 与遍历演示结束。"
    )


if __name__ == "__main__":
    main()
