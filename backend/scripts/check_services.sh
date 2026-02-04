#!/bin/bash

# 检查记忆系统依赖服务是否运行
# 使用方法: ./scripts/check_services.sh

echo "=== 记忆系统依赖服务检查 ==="
echo ""

# 检查 Ollama 服务
echo "1. 检查 Ollama 服务 (http://localhost:11434)..."
if curl -s -f http://localhost:11434 > /dev/null 2>&1; then
    echo "   ✓ Ollama 服务运行中"
    
    # 检查已安装的模型
    echo "   检查已安装的模型..."
    MODELS=$(curl -s http://localhost:11434/api/tags 2>/dev/null | grep -o '"name":"[^"]*"' | cut -d'"' -f4 || echo "")
    if [ -n "$MODELS" ]; then
        echo "   已安装的模型:"
        echo "$MODELS" | while read -r model; do
            echo "     - $model"
        done
        
        # 检查必需的模型
        if echo "$MODELS" | grep -q "llama2"; then
            echo "   ✓ 找到模型: llama2"
        else
            echo "   ⚠ 未找到模型: llama2"
            echo "     安装命令: ollama pull llama2"
        fi
        
        if echo "$MODELS" | grep -q "nomic-embed-text"; then
            echo "   ✓ 找到模型: nomic-embed-text"
        else
            echo "   ⚠ 未找到模型: nomic-embed-text"
            echo "     安装命令: ollama pull nomic-embed-text"
        fi
    else
        echo "   ⚠ 无法获取模型列表"
    fi
else
    echo "   ✗ Ollama 服务未运行"
    echo "     启动命令: ollama serve"
    echo "     或: brew services start ollama (macOS)"
fi

echo ""

# 检查 Qdrant 服务
echo "2. 检查 Qdrant 服务 (HTTP API: http://localhost:6333, gRPC: localhost:6334)..."
if curl -s -f http://localhost:6333 > /dev/null 2>&1; then
    echo "   ✓ Qdrant HTTP API 可用（gRPC 端口 6334 应该也可用）"
else
    echo "   ✗ Qdrant 服务未运行"
    echo "     启动命令: docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant"
    echo "     注意：qdrant-client 使用 gRPC 端口 6334，需要同时暴露两个端口"
fi

echo ""

# 检查后端服务
echo "3. 检查后端服务 (http://127.0.0.1:8008)..."
if curl -s -f http://127.0.0.1:8008/api/v1/memory/health > /dev/null 2>&1; then
    echo "   ✓ 后端服务运行中"
else
    echo "   ✗ 后端服务未运行"
    echo "     启动命令: cd backend && cargo run"
fi

echo ""
echo "=== 检查完成 ==="

