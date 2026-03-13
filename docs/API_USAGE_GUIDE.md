# 自适应内存系统API使用指南

## 1. 基础信息

### 1.1 基本URL

- **开发环境**: `http://127.0.0.1:8008`
- **生产环境**: `https://api.example.com`
- **API版本**: `v1`

### 1.2 认证方式

系统使用Bearer Token认证，在请求头中添加：

```
Authorization: Bearer <your-access-token>
```

### 1.3 数据格式

- **请求格式**: JSON
- **响应格式**: JSON
- **字符编码**: UTF-8

### 1.4 响应结构

```json
{
  "success": true, // 请求是否成功
  "data": {}, // 响应数据
  "message": "" // 响应消息
}
```

## 2. 认证API

### 2.1 用户登录

**接口**: `POST /api/login`

**描述**: 用户登录，获取认证Token

**请求参数**:

```json
{
  "username": "<your-username>",
  "password": "<your-password>"
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "access_token": "<token-returned-by-login>",
    "token_type": "bearer",
    "expires_in": 3600,
    "user": {
      "id": "1",
      "name": "管理员",
      "username": "admin",
      "email": "admin@example.com",
      "avatar": "",
      "role": "admin"
    }
  },
  "message": "登录成功"
}
```

### 2.2 获取当前用户信息

**接口**: `GET /api/currentUser`

**描述**: 获取当前登录用户的信息

**请求头**:

```
Authorization: Bearer <your-access-token>
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "id": "1",
    "name": "管理员",
    "username": "admin",
    "email": "admin@example.com",
    "avatar": "",
    "role": "admin"
  },
  "message": "获取成功"
}
```

## 3. 自适应记忆调度器API

### 3.1 选择最优记忆配置

**接口**: `POST /api/v1/memory/adaptive/select`

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

### 3.2 获取当前记忆状态

**接口**: `GET /api/v1/memory/adaptive/status`

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

## 4. 任务特征分析器API

### 4.1 分析任务特征

**接口**: `POST /api/v1/memory/analyzer/task-characteristics`

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

### 4.2 批量分析任务特征

**接口**: `POST /api/v1/memory/analyzer/batch-characteristics`

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

## 5. 性能预测模型API

### 5.1 预测记忆性能

**接口**: `POST /api/v1/memory/predictor/performance`

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

### 5.2 获取性能基准数据

**接口**: `GET /api/v1/memory/predictor/baselines`

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

## 6. 资源监控与优化器API

### 6.1 获取资源状态

**接口**: `GET /api/v1/memory/monitor/resources`

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

### 6.2 计算成本效益比

**接口**: `POST /api/v1/memory/monitor/cost-benefit`

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

### 6.3 资源优化建议

**接口**: `POST /api/v1/memory/monitor/optimize`

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

## 7. 动态权重调整器API

### 7.1 调整记忆权重

**接口**: `POST /api/v1/memory/weights/adjust`

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

### 7.2 获取权重调整历史

**接口**: `GET /api/v1/memory/weights/history`

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

## 8. 记忆存储API

### 8.1 存储短期记忆

**接口**: `POST /api/v1/memory/storage/stm`

**描述**: 存储短期记忆（会话历史）

**请求参数**:

```json
{
  "session_id": "string",
  "user_id": "string",
  "messages": [
    {
      "role": "user|assistant|system",
      "content": "string",
      "timestamp": "2024-01-01T00:00:00Z",
      "metadata": {}
    }
  ]
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "session_id": "string",
    "message_count": 1
  },
  "message": "短期记忆存储成功"
}
```

### 8.2 获取会话消息

**接口**: `GET /api/v1/memory/storage/stm/{session_id}`

**描述**: 获取指定会话的消息历史

**响应参数**:

```json
{
  "success": true,
  "data": {
    "session_id": "string",
    "user_id": "string",
    "messages": [
      {
        "message_id": "string",
        "role": "user|assistant|system",
        "content": "string",
        "timestamp": "2024-01-01T00:00:00Z",
        "metadata": {}
      }
    ]
  }
}
```

### 8.3 存储长期记忆

**接口**: `POST /api/v1/memory/storage/ltm`

**描述**: 存储长期记忆

**请求参数**:

```json
{
  "content": "string",
  "user_id": "string",
  "source": "stm|manual",
  "metadata": {
    "domain": "string",
    "tags": ["string"],
    "related_entities": ["string"]
  },
  "embedding": [0.1, 0.2, 0.3]
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "entry_id": "string",
    "timestamp": "2024-01-01T00:00:00Z"
  },
  "message": "长期记忆存储成功"
}
```

### 8.4 批量存储长期记忆

**接口**: `POST /api/v1/memory/storage/batch-ltm`

**描述**: 批量存储长期记忆

**请求参数**:

```json
{
  "entries": [
    {
      "content": "string",
      "user_id": "string",
      "source": "stm|manual",
      "metadata": {},
      "embedding": [0.1, 0.2, 0.3]
    }
  ]
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "processed_count": 1,
    "success_count": 1,
    "failed_count": 0,
    "entry_ids": ["string"]
  },
  "message": "批量存储完成"
}
```

### 8.5 转移短期记忆到长期记忆

**接口**: `POST /api/v1/memory/storage/transfer`

**描述**: 将短期记忆转移到长期记忆

**请求参数**:

```json
{
  "session_id": "string",
  "user_id": "string",
  "filter_criteria": {
    "message_count_threshold": 10,
    "time_threshold_minutes": 30,
    "keywords": ["string"]
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "session_id": "string",
    "transferred_messages": 5,
    "ltm_entries_created": 3
  },
  "message": "记忆转移成功"
}
```

## 9. 记忆搜索API

### 9.0 获取短期记忆会话列表

**接口**: `GET /api/v1/memory/storage/sessions`

**描述**: 获取所有短期记忆会话列表

**请求参数**:

```json
{
  "limit": 20,
  "offset": 0
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "sessions": [
      {
        "session_id": "string",
        "user_id": "string",
        "agent_id": "string",
        "status": "active|closed",
        "message_count": 10,
        "created_at": "2024-01-01T00:00:00Z",
        "last_accessed_at": "2024-01-01T12:00:00Z"
      }
    ],
    "total": 3
  }
}
```

### 9.1 获取长期记忆列表

**接口**: `GET /api/v1/memory/search/ltm`

**描述**: 获取长期记忆条目列表

**请求参数**:

```json
{
  "limit": 20,
  "offset": 0
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "entries": [
      {
        "entry_id": "string",
        "title": "string",
        "content": "string",
        "content_type": "text",
        "source_type": "document",
        "quality_score": 0.92,
        "category": "技术文档",
        "created_at": "2024-01-01T00:00:00Z"
      }
    ],
    "total": 5,
    "limit": 20,
    "offset": 0
  }
}
```

### 9.2 获取知识图谱实体列表

**接口**: `GET /api/kg/entities`

**描述**: 获取知识图谱实体列表

**请求参数**:

```json
{
  "limit": 20,
  "offset": 0
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "entities": [
      {
        "entityId": "string",
        "entityName": "string",
        "entityType": "system",
        "description": "string"
      }
    ],
    "total": 5,
    "limit": 20,
    "offset": 0
  }
}
```

### 9.3 获取多模态记忆列表

**接口**: `GET /api/mm/list`

**描述**: 获取多模态记忆条目列表

**请求参数**:

```json
{
  "limit": 20,
  "offset": 0
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "entries": [
      {
        "entryId": "string",
        "sessionId": "string",
        "sourceId": "string",
        "modalityType": "text",
        "title": "string",
        "description": "string"
      }
    ],
    "total": 3,
    "limit": 20,
    "offset": 0
  }
}
```

### 9.3.1 KG/MM 路由说明

- KG canonical 路径：`/api/kg/*`
- MM canonical 路径：`/api/mm/*`
- 兼容性说明：`/api/v1/memory/kg/*` 与 `/api/v1/memory/mm/*` 为历史路径，新增客户端请使用 canonical 路径。

### 9.4 搜索短期记忆

**接口**: `POST /api/v1/memory/search/stm`

**描述**: 搜索短期记忆（会话历史）

**请求参数**:

```json
{
  "user_id": "string",
  "query": "string",
  "filter": {
    "session_id": "string",
    "start_time": "2024-01-01T00:00:00Z",
    "end_time": "2024-01-02T00:00:00Z",
    "roles": ["user", "assistant"]
  },
  "top_k": 10
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "message_id": "string",
        "session_id": "string",
        "role": "user|assistant|system",
        "content": "string",
        "timestamp": "2024-01-01T00:00:00Z",
        "score": 0.95
      }
    ]
  }
}
```

### 9.2 搜索长期记忆

**接口**: `POST /api/v1/memory/search/ltm`

**描述**: 搜索长期记忆

**请求参数**:

```json
{
  "query": "string",
  "user_id": "string",
  "filter": {
    "domain": "string",
    "tags": ["string"],
    "start_time": "2024-01-01T00:00:00Z",
    "end_time": "2024-01-02T00:00:00Z"
  },
  "top_k": 10,
  "include_embedding": false
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "entry_id": "string",
        "content": "string",
        "source": "stm|manual",
        "timestamp": "2024-01-01T00:00:00Z",
        "metadata": {},
        "score": 0.92
      }
    ]
  }
}
```

### 9.3 获取长期记忆条目

**接口**: `GET /api/v1/memory/search/ltm/{entry_id}`

**描述**: 获取指定ID的长期记忆条目

**响应参数**:

```json
{
  "success": true,
  "data": {
    "entry_id": "string",
    "content": "string",
    "user_id": "string",
    "source": "stm|manual",
    "timestamp": "2024-01-01T00:00:00Z",
    "metadata": {},
    "embedding": [0.1, 0.2, 0.3],
    "access_count": 5
  }
}
```

### 9.4 混合搜索

**接口**: `POST /api/v1/memory/search/hybrid`

**描述**: 同时搜索短期记忆和长期记忆

**请求参数**:

```json
{
  "query": "string",
  "user_id": "string",
  "memory_types": ["stm", "ltm", "kg"],
  "filter": {
    "start_time": "2024-01-01T00:00:00Z",
    "end_time": "2024-01-02T00:00:00Z",
    "roles": ["user", "assistant"]
  },
  "top_k": 10,
  "weights": {
    "keyword": 0.5,
    "semantic": 0.5
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "id": "string",
        "type": "stm|ltm|kg",
        "content": "string",
        "source": "string",
        "timestamp": "2024-01-01T00:00:00Z",
        "score": 0.95,
        "metadata": {}
      }
    ]
  }
}
```

### 9.5 基于实体搜索

**接口**: `POST /api/v1/memory/search/entity`

**描述**: 基于实体搜索相关记忆

**请求参数**:

```json
{
  "entity_id": "string",
  "entity_name": "string",
  "entity_type": "string",
  "top_k": 10,
  "include_related_entities": true
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "id": "string",
        "type": "ltm|kg",
        "content": "string",
        "score": 0.92,
        "related_entities": ["string"]
      }
    ]
  }
}
```

## 10. 系统管理API

### 10.1 健康检查

**接口**: `GET /api/v1/memory/health`

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

### 10.2 获取系统配置

**接口**: `GET /api/v1/memory/config`

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

### 10.3 获取记忆配置列表

**接口**: `GET /api/v1/memory/configs`

**描述**: 获取记忆配置列表

**响应参数**:

```json
{
  "success": true,
  "data": {
    "configs": [
      {
        "config_id": "string",
        "name": "string",
        "description": "string",
        "memory_weights": {
          "stm": 1.0,
          "ltm": 0.8,
          "kg": 0.7,
          "mm": 0.0
        },
        "is_default": true,
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z"
      }
    ],
    "total": 1
  }
}
```

### 10.4 创建记忆配置

**接口**: `POST /api/v1/memory/configs`

**描述**: 创建新的记忆配置

**请求参数**:

```json
{
  "name": "string",
  "description": "string",
  "memory_weights": {
    "stm": 1.0,
    "ltm": 0.8,
    "kg": 0.7,
    "mm": 0.0
  },
  "is_default": false
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "config_id": "string",
    "name": "string",
    "created_at": "2024-01-01T00:00:00Z"
  },
  "message": "记忆配置创建成功"
}
```

## 11. 错误码说明

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

## 12. 使用示例

### 12.1 完整工作流程示例

```bash
# 1. 用户登录（将 <your-username> 和 <your-password> 替换为实际凭据）
curl -X POST "http://127.0.0.1:8008/api/login" \
  -H "Content-Type: application/json" \
  -d '{"username": "<your-username>", "password": "<your-password>"}'

# 2. 分析任务特征（将下方 token 替换为登录接口返回的 access_token）
token="<your-access-token>"
curl -X POST "http://127.0.0.1:8008/api/v1/memory/analyzer/task-characteristics" \
  -H "Authorization: Bearer $token" \
  -H "Content-Type: application/json" \
  -d '{"task_context": {"content": "分析这个复杂的多模态数据", "modality": ["text", "image"]}}'

# 3. 选择最优记忆配置
curl -X POST "http://127.0.0.1:8008/api/v1/memory/adaptive/select" \
  -H "Authorization: Bearer $token" \
  -H "Content-Type: application/json" \
  -d '{"task_context": {"task_id": "task_123", "complexity": 0.8, "modality_requirements": ["text", "image"], "reasoning_depth": "deep"}, "resource_constraints": {"max_memory_usage_mb": 1024, "max_response_time_ms": 2000}}'

# 4. 监控系统状态
curl -X GET "http://127.0.0.1:8008/api/v1/memory/monitor/resources" \
  -H "Authorization: Bearer $token"

# 5. 搜索记忆
curl -X POST "http://127.0.0.1:8008/api/v1/memory/search/hybrid" \
  -H "Authorization: Bearer $token" \
  -H "Content-Type: application/json" \
  -d '{"query": "多模态数据", "memory_types": ["ltm", "kg"], "top_k": 5}'
```

## 13. 开发工具

### 13.1 OpenAPI文档

系统提供了OpenAPI文档，可以通过以下地址访问：

- **OpenAPI JSON**: `http://127.0.0.1:8008/api-doc/openapi.json`
- **Swagger UI**: `http://127.0.0.1:8008/scalar`

### 13.2 示例代码

后端目录下的`examples`文件夹中包含了一些示例代码，可以参考：

- `test_memory_search.rs`: 记忆搜索示例
- `conversation_history_example.rs`: 会话历史集成示例
- `hash_password.rs`: 密码哈希示例

---

**版本**: 1.0.0  
**最后更新**: 2025-12-30
