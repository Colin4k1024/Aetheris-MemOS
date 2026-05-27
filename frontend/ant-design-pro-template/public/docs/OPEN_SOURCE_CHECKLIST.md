# 开源审查清单

本文档记录对外开源前的审查结果与发布时待办，便于维护者核对。

## 已完成的审查项

| 类别 | 项目 | 状态 |
|------|------|------|
| 法律与合规 | 根目录 [LICENSE](../LICENSE)（MIT） | 已就绪 |
| 法律与合规 | [SECURITY.md](../SECURITY.md)（支持版本、漏洞报告方式） | 已就绪 |
| 法律与合规 | [CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md) | 已就绪 |
| 文档 | [CHANGELOG.md](../CHANGELOG.md)、[README.md](../README.md) 含徽章与开源说明 | 已就绪 |
| 文档 | [CONTRIBUTING.md](../CONTRIBUTING.md)、[ARCHITECTURE.md](ARCHITECTURE.md) 等 | 已就绪 |
| 敏感信息 | [backend/config.toml](../backend/config.toml) 使用占位符，无真实密钥 | 已清理 |
| 敏感信息 | 用户迁移无预设账号 | 已清理 |
| 敏感信息 | 文档与前端登录/Mock 使用占位符或 demo | 已清理 |
| 项目清理 | [.gitignore](../.gitignore) 含 `.trae/`、`.env`、数据库文件等 | 已配置 |
| CI | 根目录 [.github/workflows/backend-ci.yml](../.github/workflows/backend-ci.yml) | 已就绪 |

## 发布前待办（维护者）

- 在 GitHub 创建仓库后，将 [SECURITY.md](../SECURITY.md) 中的 `OWNER/REPO` 替换为实际组织/用户名与仓库名。
- 若曾将 `.trae/` 提交过，执行 `git rm -r --cached .trae` 并从版本历史中移除后再公开仓库（可选）。

## 结论

当前仓库已满足常规对外开源要求，可据此清单在发布前做最后核对。
