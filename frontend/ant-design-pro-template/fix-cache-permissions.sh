#!/bin/bash
# 修复项目目录权限问题（解决 sudo npm install 导致的权限问题）

echo "🔧 正在修复项目目录权限问题..."

# 需要修复的目录列表
DIRS=(
    "node_modules/.cache"
    "src/.umi"
    "node_modules"
)

FIXED=0
FAILED=0

for dir in "${DIRS[@]}"; do
    if [ -d "$dir" ] || [ -f "$dir" ]; then
        echo "  检查 $dir..."
        # 尝试修复权限
        if sudo chown -R $(whoami) "$dir" 2>/dev/null; then
            echo "  ✅ $dir 权限修复成功"
            ((FIXED++))
        else
            echo "  ⚠️  $dir 权限修复失败，尝试删除..."
            if sudo rm -rf "$dir" 2>/dev/null; then
                echo "  ✅ $dir 已删除，将在下次启动时自动创建"
                ((FIXED++))
            else
                echo "  ❌ $dir 无法修复，需要手动处理"
                ((FAILED++))
            fi
        fi
    else
        echo "  ℹ️  $dir 不存在，跳过"
    fi
done

echo ""
if [ $FAILED -eq 0 ]; then
    echo "🎉 所有权限问题已修复！现在可以运行: npm start"
    exit 0
else
    echo "⚠️  部分目录需要手动修复权限："
    echo "   sudo chown -R \$(whoami) node_modules/.cache src/.umi node_modules"
    echo "   或: sudo rm -rf node_modules/.cache src/.umi"
    exit 1
fi
