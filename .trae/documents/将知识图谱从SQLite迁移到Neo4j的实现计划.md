## 1. 概述

当前系统的知识图谱使用SQLite存储，通过关系表模拟图结构。现在需要将其迁移到Neo4j，利用专业图数据库的优势实现更高效的图查询和关系管理。

## 2. 技术栈

- **Neo4j驱动**: neo4rs (Rust的Neo4j驱动)
- **配置管理**: 扩展现有配置系统
- **事务管理**: 使用Neo4j的事务机制

## 3. 实现步骤

### 3.1 添加依赖

在`Cargo.toml`中添加Neo4j驱动依赖：

```toml
neo4rs = "0.8"
```

### 3.2 扩展配置模块

1. 在`src/config/db_config.rs`中添加Neo4j配置项
2. 在`src/config/mod.rs`中添加Neo4j配置处理
3. 在`config.toml`中添加Neo4j配置示例

### 3.3 创建Neo4j连接管理

1. 在`src/db/`目录下创建`neo4j.rs`文件，实现：
   - Neo4j连接池管理
   - 连接健康检查
   - 事务管理

### 3.4 重写知识图谱仓库

1. 修改`src/db/kg.rs`，将SQLite实现替换为Neo4j实现：
   - 保持现有结构体(`Entity`, `Relation`)不变
   - 重写所有方法，使用Neo4j Cypher查询替代SQL
   - 实现高效的图查询算法

### 3.5 实现Neo4j初始化

1. 在`src/db/init.rs`中添加Neo4j初始化逻辑
2. 在主程序中初始化Neo4j连接

### 3.6 测试和验证

1. 运行现有测试，确保API兼容性
2. 添加Neo4j特定测试
3. 验证图查询性能提升

## 4. 关键实现细节

### 4.1 实体和关系映射

- **实体**: 映射为Neo4j节点，标签为`Entity`
- **关系**: 映射为Neo4j关系，关系类型为`relation_type`字段值
- **属性**: 所有结构体字段映射为节点/关系属性

### 4.2 Cypher查询设计

- **创建实体**: `CREATE (e:Entity {id: $id, name: $name, ...})`
- **创建关系**: `MATCH (s:Entity {id: $source_id}), (t:Entity {id: $target_id}) CREATE (s)-[r:REL_TYPE {weight: $weight, ...}]->(t)`
- **查询相关实体**: `MATCH (s:Entity {id: $entity_id})-[r]->(t:Entity) RETURN t, r ORDER BY r.weight DESC`

### 4.3 事务处理

使用Neo4j的事务机制确保数据一致性，特别是在创建实体和关系时。

## 5. 预期效果

- 提高知识图谱查询性能
- 支持更复杂的图算法
- 简化关系查询逻辑
- 提高数据扩展性

## 6. 风险和注意事项

- 确保Neo4j服务可用
- 处理Neo4j连接失败情况
- 保持现有API兼容性
- 考虑数据迁移策略（如需要）

## 7. 实现时间表

1. **依赖和配置**: 1小时
2. **Neo4j连接管理**: 2小时
3. **重写知识图谱仓库**: 3小时
4. **测试和验证**: 2小时

总时间: 8小时