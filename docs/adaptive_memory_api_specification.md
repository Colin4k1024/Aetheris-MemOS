# 自适应记忆管理算法 API 接口规范

## 概述

基于自适应记忆管理算法核心设计，提供完整的 RESTful API 接口规范，支持智能体记忆系统的动态调度、性能监控和资源优化。

## 基础信息

- **基础URL**: `https://api.agent-memory.com/api/v1`
- **认证方式**: Bearer Token
- **数据格式**: JSON
- **字符编码**: UTF-8

## 1. 自适应记忆调度器 API

### 1.1 自适应记忆选择

**接口**: `POST /memory/adaptive/select`

**描述**: 根据任务上下文和资源约束，选择最优的记忆配置

**请求参数**:

```json
{
  "task_context": {
    "task_id": "string",
    "task_type": "conversation|task|query",
    "complexity": 0.0-1.0,
    "modality_requirements": ["text", "image", "audio", "video"],
    "temporal_scope": "short|medium|long",
    "reasoning_depth": "shallow|medium|deep",
    "context_dependency": 0.0-1.0,
    "user_id": "string",
    "agent_id": "string"
  },
  "resource_constraints": {
    "max_memory_usage_mb": 1024,
    "max_cpu_usage_percent": 80,
    "max_response_time_ms": 2000,
    "storage_quota_percent": 90
  },
  "preferences": {
    "prioritize_efficiency": true,
    "prioritize_coherence": false,
    "enable_multimodal": false,
    "enable_reasoning": true
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "memory_config": {
      "primary_memory": "stm",
      "secondary_memory": ["ltm", "kg"],
      "memory_weights": {
        "stm": 1.0,
        "ltm": 0.8,
        "kg": 0.7,
        "mm": 0.0
      },
      "reasoning_depth": "deep",
      "enable_multimodal": false
    },
    "performance_prediction": {
      "efficiency_gain": 0.4273,
      "coherence_gain": 1.597,
      "resource_cost": 0.65,
      "cost_benefit_ratio": 1.85
    },
    "resource_requirements": {
      "estimated_memory_mb": 512,
      "estimated_cpu_percent": 45,
      "estimated_response_time_ms": 1200
    }
  },
  "message": "Memory configuration selected successfully"
}
```

### 1.2 获取当前记忆状态

**接口**: `GET /memory/adaptive/status`

**描述**: 获取当前记忆系统的运行状态

**请求参数**:

```json
{
  "session_id": "string",
  "include_metrics": true
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "current_config": {
      "primary_memory": "stm",
      "secondary_memory": ["ltm"],
      "memory_weights": {
        "stm": 1.0,
        "ltm": 0.6,
        "kg": 0.0,
        "mm": 0.0
      }
    },
    "performance_metrics": {
      "efficiency_score": 0.85,
      "coherence_score": 0.92,
      "response_time_ms": 850,
      "memory_usage_mb": 256,
      "cpu_usage_percent": 35
    },
    "resource_status": {
      "memory_usage_percent": 25,
      "cpu_usage_percent": 35,
      "storage_usage_percent": 45,
      "available_resources": true
    }
  }
}
```

## 2. 任务特征分析器 API

### 2.1 分析任务特征

**接口**: `POST /memory/analyzer/task-characteristics`

**描述**: 分析任务特征，确定记忆需求

**请求参数**:

```json
{
  "task_context": {
    "content": "string",
    "modality": ["text", "image"],
    "context_history": [
      {
        "role": "user|assistant|system",
        "content": "string",
        "timestamp": "2024-01-01T00:00:00Z"
      }
    ],
    "task_metadata": {
      "domain": "string",
      "complexity_hint": "low|medium|high",
      "expected_duration": "short|medium|long"
    }
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "characteristics": {
      "complexity": 0.75,
      "modality_count": 2,
      "temporal_scope": "medium",
      "reasoning_depth": 0.8,
      "context_dependency": 0.6
    },
    "memory_strategy": {
      "primary_memory": "stm",
      "secondary_memory": ["ltm", "kg"],
      "enable_multimodal": true,
      "reasoning_depth": "deep"
    },
    "confidence_score": 0.85
  }
}
```

### 2.2 批量分析任务特征

**接口**: `POST /memory/analyzer/batch-characteristics`

**描述**: 批量分析多个任务的特征

**请求参数**:

```json
{
  "tasks": [
    {
      "task_id": "string",
      "task_context": {
        "content": "string",
        "modality": ["text"]
      }
    }
  ]
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "task_id": "string",
        "characteristics": {
          "complexity": 0.6,
          "modality_count": 1,
          "temporal_scope": "short",
          "reasoning_depth": 0.4,
          "context_dependency": 0.3
        },
        "memory_strategy": {
          "primary_memory": "stm",
          "secondary_memory": [],
          "enable_multimodal": false,
          "reasoning_depth": "shallow"
        }
      }
    ],
    "batch_metrics": {
      "total_tasks": 1,
      "processed_tasks": 1,
      "average_complexity": 0.6,
      "processing_time_ms": 150
    }
  }
}
```

## 3. 性能预测模型 API

### 3.1 预测记忆性能

**接口**: `POST /memory/predictor/performance`

**描述**: 预测特定记忆配置的性能表现

**请求参数**:

```json
{
  "task_profile": {
    "complexity": 0.75,
    "modality_count": 2,
    "reasoning_depth": 0.8,
    "context_dependency": 0.6
  },
  "memory_config": {
    "primary_memory": "stm",
    "secondary_memory": ["ltm", "kg"],
    "memory_weights": {
      "stm": 1.0,
      "ltm": 0.8,
      "kg": 0.7,
      "mm": 0.0
    }
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "predicted_performance": {
      "efficiency_gain": 0.4273,
      "coherence_gain": 1.597,
      "resource_cost": 0.65,
      "confidence_score": 0.88
    },
    "synergy_factor": 1.15,
    "decay_factor": 0.92,
    "performance_breakdown": {
      "stm_contribution": 0.2473,
      "ltm_contribution": 0.1185,
      "kg_contribution": 0.0615,
      "mm_contribution": 0.0
    }
  }
}
```

### 3.2 获取性能基准数据

**接口**: `GET /memory/predictor/baselines`

**描述**: 获取各记忆层的性能基准数据

**响应参数**:

```json
{
  "success": true,
  "data": {
    "performance_baselines": {
      "stm": {
        "efficiency_gain": 0.2473,
        "coherence_gain": 0.5447,
        "resource_cost": 0.2
      },
      "ltm": {
        "efficiency_gain": 0.3698,
        "coherence_gain": 1.3751,
        "resource_cost": 0.4
      },
      "kg": {
        "efficiency_gain": 0.4273,
        "coherence_gain": 1.597,
        "resource_cost": 0.6
      },
      "mm": {
        "efficiency_gain": 0.4314,
        "coherence_gain": 1.9312,
        "resource_cost": 0.8
      }
    },
    "marginal_decay_factors": {
      "stm_to_ltm": 0.495,
      "ltm_to_kg": 0.47,
      "kg_to_mm": 0.071
    }
  }
}
```

## 4. 资源监控与优化器 API

### 4.1 获取资源状态

**接口**: `GET /memory/monitor/resources`

**描述**: 获取当前系统资源使用状态

**请求参数**:

```json
{
  "include_history": false,
  "time_range": "1h|24h|7d|30d"
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "current_status": {
      "memory_usage_mb": 512,
      "memory_usage_percent": 50,
      "cpu_usage_percent": 45,
      "response_time_ms": 850,
      "storage_usage_percent": 60
    },
    "resource_limits": {
      "memory_limit_mb": 1024,
      "cpu_limit_percent": 80,
      "response_time_limit_ms": 2000,
      "storage_limit_percent": 90
    },
    "status": "healthy|warning|critical",
    "alerts": []
  }
}
```

### 4.2 计算成本效益比

**接口**: `POST /memory/monitor/cost-benefit`

**描述**: 计算特定配置的成本效益比

**请求参数**:

```json
{
  "performance_prediction": {
    "efficiency": 0.4273,
    "coherence": 1.597
  },
  "resource_status": {
    "memory_usage_percent": 50,
    "cpu_usage_percent": 45,
    "response_time_ms": 850
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "cost_benefit_ratio": 1.85,
    "performance_score": 0.89,
    "resource_cost": 0.48,
    "recommendation": "optimal|suboptimal|poor",
    "optimization_suggestions": [
      "Consider reducing LTM weight to improve cost-benefit ratio",
      "KG memory may provide better value for this task type"
    ]
  }
}
```

### 4.3 资源优化建议

**接口**: `POST /memory/monitor/optimize`

**描述**: 获取资源优化建议

**请求参数**:

```json
{
  "current_config": {
    "memory_weights": {
      "stm": 1.0,
      "ltm": 0.8,
      "kg": 0.7,
      "mm": 0.0
    }
  },
  "performance_goals": {
    "target_efficiency": 0.4,
    "target_coherence": 1.5,
    "max_resource_cost": 0.7
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "optimization_suggestions": [
      {
        "type": "weight_adjustment",
        "description": "Reduce LTM weight from 0.8 to 0.6",
        "expected_improvement": 0.15,
        "risk_level": "low"
      },
      {
        "type": "memory_disable",
        "description": "Disable KG memory for simple tasks",
        "expected_improvement": 0.25,
        "risk_level": "medium"
      }
    ],
    "optimized_config": {
      "memory_weights": {
        "stm": 1.0,
        "ltm": 0.6,
        "kg": 0.0,
        "mm": 0.0
      }
    },
    "predicted_improvement": {
      "efficiency_gain": 0.05,
      "coherence_gain": 0.02,
      "resource_cost_reduction": 0.2
    }
  }
}
```

## 5. 动态权重调整器 API

### 5.1 调整记忆权重

**接口**: `POST /memory/weights/adjust`

**描述**: 动态调整记忆层权重

**请求参数**:

```json
{
  "task_profile": {
    "complexity": 0.75,
    "modality_count": 2,
    "reasoning_depth": 0.8,
    "context_dependency": 0.6
  },
  "cost_benefit_ratio": 1.85,
  "current_weights": {
    "stm": 1.0,
    "ltm": 0.8,
    "kg": 0.7,
    "mm": 0.0
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "adjusted_weights": {
      "stm": 1.0,
      "ltm": 0.8,
      "kg": 0.7,
      "mm": 0.6
    },
    "adjustment_reasons": {
      "stm": "Primary memory, always enabled",
      "ltm": "High complexity task requires long-term memory",
      "kg": "Deep reasoning requires knowledge graph",
      "mm": "Multi-modal task detected, enabling multimodal memory"
    },
    "confidence_score": 0.88
  }
}
```

### 5.2 获取权重调整历史

**接口**: `GET /memory/weights/history`

**描述**: 获取权重调整历史记录

**请求参数**:

```json
{
  "session_id": "string",
  "time_range": "1h|24h|7d|30d",
  "limit": 100
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "adjustment_history": [
      {
        "timestamp": "2024-01-01T12:00:00Z",
        "task_id": "string",
        "old_weights": {
          "stm": 1.0,
          "ltm": 0.6,
          "kg": 0.0,
          "mm": 0.0
        },
        "new_weights": {
          "stm": 1.0,
          "ltm": 0.8,
          "kg": 0.7,
          "mm": 0.0
        },
        "reason": "Task complexity increased",
        "performance_impact": 0.15
      }
    ],
    "summary": {
      "total_adjustments": 25,
      "average_performance_impact": 0.12,
      "most_common_adjustment": "ltm_weight_increase"
    }
  }
}
```

## 6. 系统管理 API

### 6.1 健康检查

**接口**: `GET /memory/health`

**描述**: 检查记忆系统健康状态

**响应参数**:

```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "timestamp": "2024-01-01T12:00:00Z",
    "components": {
      "scheduler": "healthy",
      "analyzer": "healthy",
      "predictor": "healthy",
      "monitor": "healthy",
      "weight_adjuster": "healthy"
    },
    "performance": {
      "avg_response_time_ms": 850,
      "success_rate": 0.98,
      "error_rate": 0.02
    }
  }
}
```

### 6.2 系统配置

**接口**: `GET /memory/config`

**描述**: 获取系统配置信息

**响应参数**:

```json
{
  "success": true,
  "data": {
    "resource_limits": {
      "memory_usage": 0.8,
      "cpu_usage": 0.8,
      "response_time": 2.0,
      "storage_quota": 0.9
    },
    "performance_baselines": {
      "stm": { "efficiency_gain": 0.2473, "coherence_gain": 0.5447 },
      "ltm": { "efficiency_gain": 0.3698, "coherence_gain": 1.3751 },
      "kg": { "efficiency_gain": 0.4273, "coherence_gain": 1.597 },
      "mm": { "efficiency_gain": 0.4314, "coherence_gain": 1.9312 }
    },
    "marginal_decay_factors": {
      "stm_to_ltm": 0.495,
      "ltm_to_kg": 0.47,
      "kg_to_mm": 0.071
    }
  }
}
```

## 错误码说明

| 错误码 | HTTP状态码 | 描述           |
| ------ | ---------- | -------------- |
| 1000   | 200        | 成功           |
| 1001   | 400        | 请求参数错误   |
| 1002   | 401        | 认证失败       |
| 1003   | 403        | 权限不足       |
| 1004   | 404        | 资源不存在     |
| 1005   | 429        | 请求频率过高   |
| 1006   | 500        | 内部服务器错误 |
| 1007   | 503        | 服务不可用     |

## 使用示例

### 完整工作流程示例

```bash
# 1. 分析任务特征
curl -X POST "https://api.agent-memory.com/api/v1/memory/analyzer/task-characteristics" \
  -H "Authorization: Bearer your-token" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "content": "请帮我分析这个复杂的多模态数据",
      "modality": ["text", "image"],
      "context_history": []
    }
  }'

# 2. 选择最优记忆配置
curl -X POST "https://api.agent-memory.com/api/v1/memory/adaptive/select" \
  -H "Authorization: Bearer your-token" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "task_id": "task_123",
      "complexity": 0.8,
      "modality_requirements": ["text", "image"],
      "reasoning_depth": "deep"
    },
    "resource_constraints": {
      "max_memory_usage_mb": 1024,
      "max_response_time_ms": 2000
    }
  }'

# 3. 监控系统状态
curl -X GET "https://api.agent-memory.com/api/v1/memory/monitor/resources" \
  -H "Authorization: Bearer your-token"
```

这个 API 规范提供了完整的自适应记忆管理算法接口，支持智能体记忆系统的动态调度、性能监控和资源优化。
