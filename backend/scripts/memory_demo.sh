#!/usr/bin/env bash
#
# Adaptive Memory System - Shell Demo Script
#
# This script demonstrates all core memory functions via curl:
# 1. Short-Term Memory (STM) - session and message storage
# 2. Long-Term Memory (LTM) - knowledge storage and search
# 3. Adaptive Scheduling - task analysis and performance prediction
# 4. System Status - resource monitoring and health check
#
# Usage:
#   # Default (localhost:8008, services must already be running)
#   bash backend/scripts/memory_demo.sh
#
#   # Auto start via Docker Compose (pull, up -d, wait for healthy, then run demo)
#   bash backend/scripts/memory_demo.sh --docker
#   # or: USE_DOCKER=1 bash backend/scripts/memory_demo.sh
#
#   # From host: use default or --docker. From another container in same compose:
#   BASE_URL=http://backend:8008 bash backend/scripts/memory_demo.sh
#
#   # CI mode (skip wait prompt)
#   CI=1 bash backend/scripts/memory_demo.sh
#

set -e

# Parse --docker before main
USE_DOCKER="${USE_DOCKER:-}"
if [ "${1:-}" = "--docker" ]; then
    USE_DOCKER=1
    shift
fi

# Configuration
BASE_URL="${BASE_URL:-http://localhost:8008}"
JQ_AVAILABLE=$(command -v jq &>/dev/null && echo "yes" || echo "no")

# Docker wait settings
DOCKER_BACKEND_HEALTH_URL="${DOCKER_BACKEND_HEALTH_URL:-http://localhost:8008/api/v1/memory/health}"
DOCKER_QDRANT_URL="${DOCKER_QDRANT_URL:-http://localhost:6333}"
DOCKER_WAIT_MAX="${DOCKER_WAIT_MAX:-120}"
DOCKER_WAIT_INTERVAL="${DOCKER_WAIT_INTERVAL:-3}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
print_header() {
    echo ""
    echo "============================================================"
    echo "$1"
    echo "============================================================"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}!${NC} $1"
}

print_section() {
    echo ""
    echo "--- $1 ---"
}

# Wait until URL returns HTTP 200 (for Docker service readiness)
wait_for_url() {
    local url="$1"
    local max_sec="${2:-$DOCKER_WAIT_MAX}"
    local interval="${3:-$DOCKER_WAIT_INTERVAL}"
    local elapsed=0
    while [ "$elapsed" -lt "$max_sec" ]; do
        if curl -s -f -o /dev/null "$url" 2>/dev/null; then
            return 0
        fi
        echo "   Waiting for $url ... (${elapsed}s/${max_sec}s)"
        sleep "$interval"
        elapsed=$((elapsed + interval))
    done
    return 1
}

# Ensure Docker Compose services are up and backend is healthy (when USE_DOCKER=1)
ensure_docker_services() {
    print_header "Docker Compose: pull and start services"

    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
    if [ ! -f "$PROJECT_ROOT/docker-compose.yml" ]; then
        echo "   docker-compose.yml not found at $PROJECT_ROOT"
        return 1
    fi

    cd "$PROJECT_ROOT"

    if command -v docker >/dev/null 2>&1; then
        :
    else
        echo "   docker not found; cannot run in Docker mode."
        return 1
    fi

    COMPOSE_CMD=""
    if docker compose version >/dev/null 2>&1; then
        COMPOSE_CMD="docker compose"
    elif command -v docker-compose >/dev/null 2>&1; then
        COMPOSE_CMD="docker-compose"
    else
        echo "   docker compose / docker-compose not found."
        return 1
    fi

    print_section "Build and start services (postgres, backend, qdrant)"
    echo "   (Postgres is started automatically via backend dependency.)"
    echo "   (Ollama uses your local instance; ensure 'ollama serve' is running on the host.)"
    if ! $COMPOSE_CMD up -d --build backend qdrant; then
        print_error "Failed to build or start backend/qdrant. Fix the errors above and retry."
        return 1
    fi

    print_section "Wait for backend to be healthy"
    if ! wait_for_url "$DOCKER_BACKEND_HEALTH_URL" "$DOCKER_WAIT_MAX" "$DOCKER_WAIT_INTERVAL"; then
        print_error "Backend did not become healthy at $DOCKER_BACKEND_HEALTH_URL"
        echo "   Check: docker compose logs backend"
        return 1
    fi
    print_success "Backend is healthy"

    print_section "Wait for Qdrant (optional, for LTM search)"
    if wait_for_url "$DOCKER_QDRANT_URL" "30" "$DOCKER_WAIT_INTERVAL" 2>/dev/null; then
        print_success "Qdrant is reachable"
    else
        print_warning "Qdrant not reachable; LTM vector search may fail"
    fi

    echo ""
}

# Make curl request and handle response
# Args: $1=description $2=method $3=path $4=body_file (optional)
request() {
    local desc="$1"
    local method="$2"
    local path="$3"
    local body_file="$4"
    local url="${BASE_URL}${path}"
    local response
    local http_code

    print_section "$desc"

    if [ -n "$body_file" ]; then
        response=$(curl -s -w "\n%{http_code}" -X "$method" "$url" \
            -H "Content-Type: application/json" \
            -d @"$body_file" 2>&1) || true
    else
        response=$(curl -s -w "\n%{http_code}" -X "$method" "$url" \
            -H "Content-Type: application/json" 2>&1) || true
    fi

    # Extract body and status code
    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')

    if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
        print_success "Success (HTTP $http_code)"
        if [ "$JQ_AVAILABLE" = "yes" ]; then
            echo "$body" | jq '.' 2>/dev/null || echo "$body"
        else
            echo "$body"
        fi
        return 0
    else
        print_error "Failed (HTTP $http_code)"
        echo "$body" | head -c 500
        echo ""
        return 1
    fi
}

# Demo 1: Short-Term Memory
demo_stm() {
    print_header "Demo 1: Short-Term Memory (STM)"

    # 1.1 Create session and add user message
    print_section "1.1 Create session and add user message"

    cat > /tmp/stm_user.json << 'EOF'
{
  "userId": "demo_user",
  "agentId": "demo_agent",
  "sessionType": "conversation",
  "role": "user",
  "content": "请给我讲讲人工智能的发展历史",
  "maxContextLength": 4096,
  "retentionHours": 24
}
EOF

    request "Store STM (user message)" "POST" "/api/v1/memory/storage/stm" "/tmp/stm_user.json" || true

    # 1.2 Add assistant response
    print_section "1.2 Add assistant response"

    cat > /tmp/stm_assistant.json << 'EOF'
{
  "userId": "demo_user",
  "agentId": "demo_agent",
  "sessionType": "conversation",
  "role": "assistant",
  "content": "人工智能的发展可以追溯到1956年的达特茅斯会议，经历了符号主义、连接主义、深度学习等多个阶段。",
  "maxContextLength": 4096,
  "retentionHours": 24
}
EOF

    request "Store STM (assistant response)" "POST" "/api/v1/memory/storage/stm" "/tmp/stm_assistant.json" || true

    rm -f /tmp/stm_user.json /tmp/stm_assistant.json
}

# Demo 2: Long-Term Memory
demo_ltm() {
    print_header "Demo 2: Long-Term Memory (LTM)"

    # 2.1 Store knowledge entries
    print_section "2.1 Store knowledge entries"

    # Entry 1: Python
    cat > /tmp/ltm_python.json << 'EOF'
{
  "sourceId": "doc_python",
  "sourceType": "document",
  "title": "Python编程",
  "content": "Python是一种高级编程语言,具有简洁易读的语法和丰富的库支持。"
}
EOF

    request "Store LTM (Python)" "POST" "/api/v1/memory/storage/ltm" "/tmp/ltm_python.json" || true

    # Entry 2: Rust
    cat > /tmp/ltm_rust.json << 'EOF'
{
  "sourceId": "doc_rust",
  "sourceType": "document",
  "title": "Rust系统编程",
  "content": "Rust是一种系统编程语言,强调内存安全和并发性能。"
}
EOF

    request "Store LTM (Rust)" "POST" "/api/v1/memory/storage/ltm" "/tmp/ltm_rust.json" || true

    # Entry 3: Machine Learning
    cat > /tmp/ltm_ml.json << 'EOF'
{
  "sourceId": "doc_ml",
  "sourceType": "document",
  "title": "机器学习基础",
  "content": "机器学习是人工智能的分支,让计算机从数据中学习并改进。"
}
EOF

    request "Store LTM (Machine Learning)" "POST" "/api/v1/memory/storage/ltm" "/tmp/ltm_ml.json" || true

    # 2.2 Search knowledge (requires Qdrant)
    print_section "2.2 Search knowledge entries (requires Qdrant)"

    cat > /tmp/ltm_search.json << 'EOF'
{
  "query": "神经网络 深度学习",
  "topK": 3
}
EOF

    if request "Search LTM" "POST" "/api/v1/memory/search/ltm" "/tmp/ltm_search.json"; then
        :
    else
        print_warning "Search failed - Qdrant service may not be running"
        print_warning "Start Qdrant with: docker compose up -d qdrant"
    fi

    rm -f /tmp/ltm_python.json /tmp/ltm_rust.json /tmp/ltm_ml.json /tmp/ltm_search.json
}

# Demo 3: Adaptive Memory Scheduling
demo_analysis() {
    print_header "Demo 3: Adaptive Memory Scheduling"

    # 3.1 Analyze task characteristics
    print_section "3.1 Analyze task characteristics"

    cat > /tmp/analyze.json << 'EOF'
{
  "task_context": {
    "content": "用户询问如何学习编程,需要给出详细的学习路线和建议",
    "modality": ["text"]
  }
}
EOF

    request "Analyze task characteristics" "POST" "/api/v1/memory/analyzer/task-characteristics" "/tmp/analyze.json" || true

    # 3.2 Predict performance
    print_section "3.2 Predict performance"

    cat > /tmp/predict.json << 'EOF'
{
  "task_profile": {
    "complexity": 0.6,
    "modality_count": 1,
    "temporal_scope": "medium",
    "reasoning_depth": 0.7,
    "context_dependency": 0.5
  },
  "memory_config": {
    "primary_memory": "stm",
    "secondary_memory": ["ltm"],
    "memory_weights": {
      "stm": 0.7,
      "ltm": 0.3,
      "kg": 0.0,
      "mm": 0.0
    },
    "reasoning_depth": "medium",
    "enable_multimodal": false
  }
}
EOF

    request "Predict performance" "POST" "/api/v1/memory/predictor/performance" "/tmp/predict.json" || true

    rm -f /tmp/analyze.json /tmp/predict.json
}

# Demo 4: System Status
demo_status() {
    print_header "Demo 4: System Status"

    # 4.1 Get resource status
    print_section "4.1 Get resource status"

    request "Get resource status" "GET" "/api/v1/memory/monitor/resources" || true

    # 4.2 Get health status
    print_section "4.2 Get health status"

    request "Get health status" "GET" "/api/v1/memory/health" || true
}

# Main
main() {
    echo ""
    echo "############################################################"
    echo "# Adaptive Memory System - Shell Demo"
    echo "############################################################"
    echo ""

    # Optional: start and wait for Docker Compose services before running demo
    if [ -n "$USE_DOCKER" ]; then
        ensure_docker_services || exit 1
        BASE_URL="http://localhost:8008"
    fi

    echo "Base URL: $BASE_URL"
    echo "jq available: $JQ_AVAILABLE"
    echo ""

    # Wait for user input unless in CI mode
    if [ -z "$CI" ] && [ -z "$SKIP_WAIT" ]; then
        echo "Prerequisites:"
        echo "1. Backend service running at $BASE_URL"
        echo "2. (Optional) Qdrant for vector search at localhost:6334"
        echo ""
        echo "Press Enter to start demo..."
        read -r
    fi

    # Run all demos
    demo_stm
    demo_ltm
    demo_analysis
    demo_status

    echo ""
    echo "############################################################"
    echo "# Demo Complete!"
    echo "############################################################"
    echo ""
    echo "For more API examples, see: docs/MEMORY_API_EXAMPLES.md"
}

main "$@"
