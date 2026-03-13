# Adaptive Memory System API Usage Guide

## 1. Basic Information

### 1.1 Base URLs

- **Development Environment**: `http://127.0.0.1:8008`
- **Production Environment**: `https://api.example.com`
- **API Version**: `v1`

### 1.2 Authentication

The system uses Bearer Token authentication. Include in the request header:

```
Authorization: Bearer <your-access-token>
```

### 1.3 Data Format

- **Request Format**: JSON
- **Response Format**: JSON
- **Character Encoding**: UTF-8

### 1.4 Response Structure

```json
{
  "success": true, // Whether the request succeeded
  "data": {}, // Response data
  "message": "" // Response message
}
```

## 2. Authentication API

### 2.1 User Login

**Endpoint**: `POST /api/login`

**Description**: User login to obtain authentication token

**Request Parameters**:

```json
{
  "username": "<your-username>",
  "password": "<your-password>"
}
```

**Response Parameters**:

```json
{
  "success": true,
  "data": {
    "access_token": "<token-returned-by-login>",
    "token_type": "bearer",
    "expires_in": 3600,
    "user": {
      "id": "1",
      "name": "Administrator",
      "username": "admin",
      "email": "admin@example.com",
      "avatar": "",
      "role": "admin"
    }
  },
  "message": "Login successful"
}
```

### 2.2 Get Current User Info

**Endpoint**: `GET /api/currentUser`

**Description**: Get current logged-in user's information

**Request Headers**:

```
Authorization: Bearer <your-access-token>
```

**Response Parameters**:

```json
{
  "success": true,
  "data": {
    "id": "1",
    "name": "Administrator",
    "username": "admin",
    "email": "admin@example.com",
    "avatar": "",
    "role": "admin"
  },
  "message": "Retrieved successfully"
}
```

## 3. Adaptive Memory Scheduler API

### 3.1 Select Optimal Memory Configuration

**Endpoint**: `POST /api/v1/memory/adaptive/select`

**Description**: Select optimal memory configuration based on task context and resource constraints

**Request Parameters**:

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

**Response Parameters**:

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

### 3.2 Get Current Memory Status

**Endpoint**: `GET /api/v1/memory/adaptive/status`

**Description**: Get current memory system operational status

**Request Parameters**:

```json
{
  "session_id": "string",
  "include_metrics": true
}
```

**Response Parameters**:

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

## 4. Task Characteristic Analyzer API

### 4.1 Analyze Task Characteristics

**Endpoint**: `POST /api/v1/memory/analyzer/task-characteristics`

**Description**: Analyze task characteristics to determine memory requirements

**Request Parameters**:

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

**Response Parameters**:

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

### 4.2 Batch Analyze Task Characteristics

**Endpoint**: `POST /api/v1/memory/analyzer/batch-characteristics`

**Description**: Batch analyze characteristics of multiple tasks

**Request Parameters**:

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

**Response Parameters**:

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

## 5. Performance Prediction Model API

### 5.1 Predict Memory Performance

**Endpoint**: `POST /api/v1/memory/predictor/performance`

**Description**: Predict performance for a specific memory configuration

**Request Parameters**:

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

**Response Parameters**:

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

### 5.2 Get Performance Baseline Data

**Endpoint**: `GET /api/v1/memory/predictor/baselines`

**Description**: Get performance baseline data for each memory layer

**Response Parameters**:

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

## 6. Resource Monitor & Optimizer API

### 6.1 Get Resource Status

**Endpoint**: `GET /api/v1/memory/monitor/resources`

**Description**: Get current system resource usage status

**Request Parameters**:

```json
{
  "include_history": false,
  "time_range": "1h|24h|7d|30d"
}
```

**Response Parameters**:

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

### 6.2 Calculate Cost-Benefit Ratio

**Endpoint**: `POST /api/v1/memory/monitor/cost-benefit`

**Description**: Calculate cost-benefit ratio for a specific configuration

**Request Parameters**:

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

**Response Parameters**:

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

### 6.3 Resource Optimization Suggestions

**Endpoint**: `POST /api/v1/memory/monitor/optimize`

**Description**: Get resource optimization suggestions

**Request Parameters**:

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

**Response Parameters**:

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

## 7. Dynamic Weight Adjuster API

### 7.1 Adjust Memory Weights

**Endpoint**: `POST /api/v1/memory/weights/adjust`

**Description**: Dynamically adjust memory layer weights

**Request Parameters**:

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

**Response Parameters**:

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

### 7.2 Get Weight Adjustment History

**Endpoint**: `GET /api/v1/memory/weights/history`

**Description**: Get weight adjustment history records

**Request Parameters**:

```json
{
  "session_id": "string",
  "time_range": "1h|24h|7d|30d",
  "limit": 100
}
```

**Response Parameters**:

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

## 8. Memory Storage API

### 8.1 Store Short-Term Memory

**Endpoint**: `POST /api/v1/memory/storage/stm`

**Description**: Store short-term memory (session history)

**Request Parameters**:

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

**Response Parameters**:

```json
{
  "success": true,
  "data": {
    "session_id": "string",
    "message_count": 1
  },
  "message": "Short-term memory stored successfully"
}
```

### 8.2 Get Session Messages

**Endpoint**: `GET /api/v1/memory/storage/stm/{session_id}`

**Description**: Get message history for a specific session

**Response Parameters**:

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

### 8.3 Store Long-Term Memory

**Endpoint**: `POST /api/v1/memory/storage/ltm`

**Description**: Store long-term memory

**Request Parameters**:

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

**Response Parameters**:

```json
{
  "success": true,
  "data": {
    "entry_id": "string",
    "timestamp": "2024-01-01T00:00:00Z"
  },
  "message": "Long-term memory stored successfully"
}
```

### 8.4 Batch Store Long-Term Memory

**Endpoint**: `POST /api/v1/memory/storage/batch-ltm`

**Description**: Batch store long-term memory

**Request Parameters**:

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

**Response Parameters**:

```json
{
  "success": true,
  "data": {
    "processed_count": 1,
    "success_count": 1,
    "failed_count": 0,
    "entry_ids": ["string"]
  },
  "message": "Batch storage completed"
}
```

### 8.5 Transfer Short-Term to Long-Term Memory

**Endpoint**: `POST /api/v1/memory/storage/transfer`

**Description**: Transfer short-term memory to long-term memory

**Request Parameters**:

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

**Response Parameters**:

```json
{
  "success": true,
  "data": {
    "session_id": "string",
    "transferred_messages": 5,
    "ltm_entries_created": 3
  },
  "message": "Memory transfer successful"
}
```

## 9. Memory Search API

### 9.1 Search Short-Term Memory

**Endpoint**: `POST /api/v1/memory/search/stm`

**Description**: Search short-term memory (session history)

**Request Parameters**:

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

**Response Parameters**:

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

### 9.2 Search Long-Term Memory

**Endpoint**: `POST /api/v1/memory/search/ltm`

**Description**: Search long-term memory

**Request Parameters**:

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

**Response Parameters**:

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

### 9.3 Get Long-Term Memory Entry

**Endpoint**: `GET /api/v1/memory/search/ltm/{entry_id}`

**Description**: Get long-term memory entry by ID

**Response Parameters**:

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

### 9.4 Hybrid Search

**Endpoint**: `POST /api/v1/memory/search/hybrid`

**Description**: Search both short-term and long-term memory simultaneously

**Request Parameters**:

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

**Response Parameters**:

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

### 9.5 Entity-Based Search

**Endpoint**: `POST /api/v1/memory/search/entity`

**Description**: Search related memories based on entities

**Request Parameters**:

```json
{
  "entity_id": "string",
  "entity_name": "string",
  "entity_type": "string",
  "top_k": 10,
  "include_related_entities": true
}
```

**Response Parameters**:

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

## 10. System Management API

### 10.1 Health Check

**Endpoint**: `GET /api/v1/memory/health`

**Description**: Check memory system health status

**Response Parameters**:

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

### 10.2 Get System Configuration

**Endpoint**: `GET /api/v1/memory/config`

**Description**: Get system configuration information

**Response Parameters**:

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

### 10.3 Get Memory Configuration List

**Endpoint**: `GET /api/v1/memory/configs`

**Description**: Get memory configuration list

**Response Parameters**:

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

### 10.4 Create Memory Configuration

**Endpoint**: `POST /api/v1/memory/configs`

**Description**: Create a new memory configuration

**Request Parameters**:

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

**Response Parameters**:

```json
{
  "success": true,
  "data": {
    "config_id": "string",
    "name": "string",
    "created_at": "2024-01-01T00:00:00Z"
  },
  "message": "Memory configuration created successfully"
}
```

## 11. Error Codes

| Error Code | HTTP Status | Description                |
| ---------- | ----------- | -------------------------- |
| 1000       | 200         | Success                    |
| 1001       | 400         | Invalid request parameters |
| 1002       | 401         | Authentication failed      |
| 1003       | 403         | Insufficient permissions   |
| 1004       | 404         | Resource not found         |
| 1005       | 429         | Rate limit exceeded        |
| 1006       | 500         | Internal server error      |
| 1007       | 503         | Service unavailable        |

## 12. Usage Examples

### 12.1 Complete Workflow Example

```bash
# 1. User login (replace <your-username> and <your-password> with actual credentials)
curl -X POST "http://127.0.0.1:8008/api/login" \
  -H "Content-Type: application/json" \
  -d '{"username": "<your-username>", "password": "<your-password>"}'

# 2. Analyze task characteristics (replace token with access_token from login response)
token="<your-access-token>"
curl -X POST "http://127.0.0.1:8008/api/v1/memory/analyzer/task-characteristics" \
  -H "Authorization: Bearer $token" \
  -H "Content-Type: application/json" \
  -d '{"task_context": {"content": "Analyze this complex multimodal data", "modality": ["text", "image"]}}'

# 3. Select optimal memory configuration
curl -X POST "http://127.0.0.1:8008/api/v1/memory/adaptive/select" \
  -H "Authorization: Bearer $token" \
  -H "Content-Type: application/json" \
  -d '{"task_context": {"task_id": "task_123", "complexity": 0.8, "modality_requirements": ["text", "image"], "reasoning_depth": "deep"}, "resource_constraints": {"max_memory_usage_mb": 1024, "max_response_time_ms": 2000}}'

# 4. Monitor system status
curl -X GET "http://127.0.0.1:8008/api/v1/memory/monitor/resources" \
  -H "Authorization: Bearer $token"

# 5. Search memory
curl -X POST "http://127.0.0.1:8008/api/v1/memory/search/hybrid" \
  -H "Authorization: Bearer $token" \
  -H "Content-Type: application/json" \
  -d '{"query": "multimodal data", "memory_types": ["ltm", "kg"], "top_k": 5}'
```

## 13. Development Tools

### 13.1 OpenAPI Documentation

The system provides OpenAPI documentation accessible at:

- **OpenAPI JSON**: `http://127.0.0.1:8008/api-doc/openapi.json`
- **Swagger UI**: `http://127.0.0.1:8008/scalar`

### 13.2 Sample Code

The `examples` folder in the backend directory contains sample code for reference:

- `test_memory_search.rs`: Memory search example
- `conversation_history_example.rs`: Conversation history integration example
- `hash_password.rs`: Password hashing example

---

**Version**: 1.0.0  
**Last Updated**: 2025-12-30
