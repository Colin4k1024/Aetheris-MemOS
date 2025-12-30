# 快速修复指南

## 🚨 权限问题快速修复

如果遇到权限错误（EACCES: permission denied），运行以下命令：

```bash
cd /Users/jiafan/Desktop/work-code/adaptive-memory-system/frontend/ant-design-pro-template

# 一键修复所有权限问题（需要输入密码）
sudo chown -R $(whoami) node_modules/.cache src/.umi node_modules ~/.npm

# 然后启动项目
npm start
```

## 或者删除缓存目录

```bash
cd /Users/jiafan/Desktop/work-code/adaptive-memory-system/frontend/ant-design-pro-template

# 删除所有缓存目录（需要输入密码）
sudo rm -rf node_modules/.cache src/.umi

# 然后启动项目（会自动重新创建）
npm start
```

## 预防措施

**重要：永远不要使用 `sudo npm install` 或 `sudo npm start`！**

如果必须使用 sudo，安装完成后立即运行：
```bash
sudo chown -R $(whoami) node_modules
```

