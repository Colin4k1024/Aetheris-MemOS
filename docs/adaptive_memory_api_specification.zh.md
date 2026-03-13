# Adaptive Memory Management Algorithm API Specification

## Overview

Based on the core design of the adaptive memory management algorithm, this document provides a complete RESTful API specification for the agent memory system, supporting dynamic scheduling, performance monitoring, and resource optimization.

## Base Information

- **Base URL**: `https://api.agent-memory.com/api/v1`
- **Authentication**: Bearer Token
- **Data Format**: JSON
- **Character Encoding**: UTF-8

## 1. Adaptive Memory Scheduler API

### 1.1 Adaptive Memory Selection

**Endpoint**: `POST /memory/adaptive/select`

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

### 1.2 Get Current Memory Status

**Endpoint**: `GET /memory/adaptive/status`

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

## 2. Task Characteristic Analyzer API

### 2.1 Analyze Task Characteristics

**Endpoint**: `POST /memory/analyzer/task-characteristics`

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

### 2.2 Batch Analyze Task Characteristics

**Endpoint**: `POST /memory/analyzer/batch-characteristics`

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

## 3. Performance Prediction Model API

### 3.1 Predict Memory Performance

**Endpoint**: `POST /memory/predictor/performance`

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

### 3.2 Get Performance Baseline Data

**Endpoint**: `GET /memory/predictor/baselines`

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

## 4. Resource Monitor & Optimizer API

### 4.1 Get Resource Status

**Endpoint**: `GET /memory/monitor/resources`

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

### 4.2 Calculate Cost-Benefit Ratio

**Endpoint**: `POST /memory/monitor/cost-benefit`

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

### 4.3 Resource Optimization Suggestions

**Endpoint**: `POST /memory/monitor/optimize`

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

## 5. Dynamic Weight Adjuster API

### 5.1 Adjust Memory Weights

**Endpoint**: `POST /memory/weights/adjust`

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

### 5.2 Get Weight Adjustment History

**Endpoint**: `GET /memory/weights/history`

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

## 6. System Management API

### 6.1 Health Check

**Endpoint**: `GET /memory/health`

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

### 6.2 System Configuration

**Endpoint**: `GET /memory/config`

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

## Error Codes

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

## Usage Examples

### Complete Workflow Example

```bash
# 1. Analyze task characteristics
curl -X POST "https://api.agent-memory.com/api/v1/memory/analyzer/task-characteristics" \
  -H "Authorization: Bearer your-token" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "content": "Please help me analyze this complex multimodal data",
      "modality": ["text", "image"],
      "context_history": []
    }
  }'

# 2. Select optimal memory configuration
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

# 3. Monitor system status
curl -X GET "https://api.agent-memory.com/api/v1/memory/monitor/resources" \
  -H "Authorization: Bearer your-token"
```

This API specification provides a complete interface for the adaptive memory management algorithm, supporting dynamic scheduling, performance monitoring, and resource optimization for agent memory systems.
