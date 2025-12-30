# 故障排查指南

## ⚠️ 重要提示

**永远不要使用 `sudo npm install` 或 `sudo npm start`！**

如果之前使用了 `sudo` 安装，会导致所有创建的文件和目录被 root 拥有，从而引发权限问题。

## 权限问题快速修复

### 一键修复所有权限问题

在终端运行以下命令（需要输入密码）：

```bash
cd /Users/jiafan/Desktop/work-code/adaptive-memory-system/frontend/ant-design-pro-template

# 修复所有受影响的目录权限
sudo chown -R $(whoami) node_modules/.cache src/.umi node_modules

# 或者使用修复脚本（需要输入密码）
./fix-cache-permissions.sh
```

### 如果无法使用 sudo

删除缓存目录，让系统重新创建：

```bash
cd /Users/jiafan/Desktop/work-code/adaptive-memory-system/frontend/ant-design-pro-template

# 删除缓存目录（需要 sudo）
sudo rm -rf node_modules/.cache src/.umi

# 然后正常启动
npm start
```

## npm 安装权限问题

### 问题描述
```
npm error code EEXIST
npm error EACCES: permission denied
```

### 解决方案

#### 方案 1: 修复 npm 缓存权限（推荐）

```bash
sudo chown -R $(whoami) ~/.npm
npm install
```

#### 方案 2: 使用项目本地缓存

已配置 `.npmrc` 文件，npm 会使用项目目录下的 `.npm-cache` 目录。

#### 方案 3: 清理缓存后重试

```bash
npm cache clean --force
rm -rf node_modules package-lock.json
npm install
```

## node_modules/.cache 权限问题

### 问题描述
```
Error: EACCES: permission denied, open 'node_modules/.cache/logger/umi.log'
```

### 解决方案

```bash
sudo chown -R $(whoami) node_modules/.cache
# 或
sudo rm -rf node_modules/.cache
```

## src/.umi 权限问题

### 问题描述
```
Error: EACCES: permission denied, unlink 'src/.umi/appData.json'
```

### 解决方案

```bash
sudo chown -R $(whoami) src/.umi
# 或
sudo rm -rf src/.umi
```

## 常见问题

### 1. 端口被占用

如果 8000 端口被占用，可以修改端口：

```bash
PORT=3000 npm start
```

### 2. 依赖安装失败

尝试使用不同的包管理器：

```bash
# 使用 yarn
yarn install

# 或使用 pnpm
pnpm install
```

### 3. 构建错误

清理构建缓存：

```bash
rm -rf .umi
rm -rf dist
npm start
```

## 联系支持

如果以上方案都无法解决问题，请检查：
1. Node.js 版本是否符合要求（>= 20.0.0）
2. npm 版本是否最新
3. 磁盘空间是否充足
4. 网络连接是否正常

