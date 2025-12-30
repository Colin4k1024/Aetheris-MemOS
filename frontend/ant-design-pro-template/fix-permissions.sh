#!/bin/bash

# 修复 node_modules/.cache 权限问题的脚本

echo "正在修复 node_modules/.cache 目录权限..."

# 方法1: 尝试修复权限（需要 sudo）
if command -v sudo &> /dev/null; then
    echo "尝试使用 sudo 修复权限..."
    sudo chown -R $(whoami) node_modules/.cache 2>/dev/null && echo "✅ 权限修复成功" || echo "❌ 需要输入密码"
fi

# 方法2: 如果无法修复，则删除缓存目录
if [ -d "node_modules/.cache" ] && [ ! -w "node_modules/.cache" ]; then
    echo "无法修复权限，尝试删除缓存目录..."
    sudo rm -rf node_modules/.cache 2>/dev/null && echo "✅ 缓存目录已删除" || echo "❌ 需要手动删除: sudo rm -rf node_modules/.cache"
fi

# 方法3: 创建新的缓存目录
if [ ! -d "node_modules/.cache" ]; then
    mkdir -p node_modules/.cache/logger
    chmod -R 755 node_modules/.cache
    echo "✅ 新的缓存目录已创建"
fi

echo ""
echo "修复完成！现在可以运行: npm start"

