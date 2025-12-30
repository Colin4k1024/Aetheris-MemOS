## 添加Neo4j配置到config.toml

### 目标
在config.toml文件中添加Neo4j数据库配置，使系统能够连接和使用Neo4j知识图谱存储。

### 实现步骤
1. **在config.toml末尾添加[neo4j]配置节**
   - 添加host、port、username、password和database配置项
   - 使用与代码中默认值一致的配置
   - 便于用户根据实际环境修改

### 配置内容
```toml
[neo4j]
# Neo4j图数据库配置
host = "localhost"
port = 7687
username = "neo4j"
password = "password"
database = "neo4j"
```

### 配置说明
- `host`: Neo4j服务器地址，默认localhost
- `port`: Neo4j Bolt协议端口，默认7687
- `username`: Neo4j用户名，默认neo4j
- `password`: Neo4j密码，默认password
- `database`: 要使用的数据库名称，默认neo4j

### 预期效果
- 系统启动时会读取这些配置
- 用于初始化Neo4j连接
- 为后续完整迁移到Neo4j做好准备
- 保持与现有代码的兼容性