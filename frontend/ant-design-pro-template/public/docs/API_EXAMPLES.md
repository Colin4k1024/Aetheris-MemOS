# Adaptive Memory System API Examples

Comprehensive practical examples for the Adaptive Memory Management System API. The backend runs on `http://127.0.0.1:8008`.

## Table of Contents

1. [Authentication](#1-authentication)
2. [Adaptive Memory Selection](#2-adaptive-memory-selection)
3. [Memory Storage](#3-memory-storage)
4. [Memory Search](#4-memory-search)
5. [Knowledge Graph](#5-knowledge-graph)
6. [Multimodal Memory](#6-multimodal-memory)
7. [Weight Management](#7-weight-management)
8. [System and Health](#8-system-and-health)

---

## 1. Authentication

### 1.1 Register a New User

```bash
curl -X POST "http://127.0.0.1:8008/api/register" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice_developer",
    "password": "SecureP@ssw0rd!"
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/register",
    json={
        "username": "alice_developer",
        "password": "SecureP@ssw0rd!"
    }
)
print(response.json())
```

**Response:**
```json
{
  "id": "01ARH3MMMMM50VNTVRENS9RVQD",
  "username": "alice_developer",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "exp": 1743648000
}
```

### 1.2 Login with Username and Password

```bash
curl -X POST "http://127.0.0.1:8008/api/login" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice_developer",
    "password": "SecureP@ssw0rd!"
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/login",
    json={
        "username": "alice_developer",
        "password": "SecureP@ssw0rd!"
    }
)
print(response.json())
# Save token: response.json()["token"]
```

**Response:**
```json
{
  "id": "01ARH3MMMMM50VNTVRENS9RVQD",
  "username": "alice_developer",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "exp": 1743648000
}
```

### 1.3 Login with Existing Token

```bash
curl -X POST "http://127.0.0.1:8008/api/login/account?token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

```python
import requests

token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
response = requests.post(
    f"http://127.0.0.1:8008/api/login/account?token={token}"
)
print(response.json())
```

**Response:**
```json
{
  "id": "",
  "username": "",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "exp": 0
}
```

### 1.4 Verify Token Validity

```bash
curl -X GET "http://127.0.0.1:8008/api/login/account?token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

```python
import requests

token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
response = requests.get(
    f"http://127.0.0.1:8008/api/login/account?token={token}"
)
print(response.json())
```

**Response:**
```json
{
  "valid": true,
  "message": "Token is valid"
}
```

### 1.5 Get Current User Info

```bash
curl -X GET "http://127.0.0.1:8008/api/currentUser" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

headers = {"Authorization": f"Bearer {TOKEN}"}
response = requests.get(
    "http://127.0.0.1:8008/api/currentUser",
    headers=headers
)
print(response.json())
```

**Response:**
```json
{
  "name": "alice_developer",
  "avatar": null,
  "userid": "01ARH3MMMMM50VNTVRENS9RVQD",
  "email": null,
  "signature": null,
  "title": null,
  "group": null,
  "tags": null,
  "notify_count": 0,
  "unread_count": 0,
  "country": null,
  "access": "admin",
  "geographic": null,
  "address": null,
  "phone": null
}
```

### Error Handling - Authentication

```python
import requests

# Invalid credentials
response = requests.post(
    "http://127.0.0.1:8008/api/login",
    json={"username": "alice", "password": "wrong_password"}
)
if response.status_code == 401:
    print("Authentication failed:", response.json())
# {"error": "Account not exist or password is incorrect."}

# Expired/invalid token
response = requests.get(
    "http://127.0.0.1:8008/api/currentUser",
    headers={"Authorization": "Bearer invalid_token"}
)
if response.status_code == 401:
    print("Unauthorized:", response.json())
```

---

## 2. Adaptive Memory Selection

The core API that automatically selects the optimal memory configuration based on task characteristics.

### 2.1 Select Memory Configuration (Full Request)

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/adaptive/select" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "task_id": "task_7x9k2Lm4",
      "task_type": "conversation",
      "complexity": 0.75,
      "modality_requirements": ["text"],
      "temporal_scope": "medium",
      "reasoning_depth": "deep",
      "context_dependency": 0.85,
      "user_id": "user_001",
      "agent_id": "agent_001"
    },
    "resource_constraints": {
      "max_memory_usage_mb": 1024,
      "max_cpu_usage_percent": 80,
      "max_response_time_ms": 2000,
      "storage_quota_percent": 75
    },
    "preferences": {
      "prioritize_efficiency": true,
      "prioritize_coherence": false,
      "enable_multimodal": false,
      "enable_reasoning": true
    },
    "explain": true
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/adaptive/select",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "task_context": {
            "task_id": "task_7x9k2Lm4",
            "task_type": "conversation",
            "complexity": 0.75,
            "modality_requirements": ["text"],
            "temporal_scope": "medium",
            "reasoning_depth": "deep",
            "context_dependency": 0.85,
            "user_id": "user_001",
            "agent_id": "agent_001"
        },
        "resource_constraints": {
            "max_memory_usage_mb": 1024,
            "max_cpu_usage_percent": 80,
            "max_response_time_ms": 2000,
            "storage_quota_percent": 75
        },
        "preferences": {
            "prioritize_efficiency": True,
            "prioritize_coherence": False,
            "enable_multimodal": False,
            "enable_reasoning": True
        },
        "explain": True
    }
)
print(response.json())
```

**Response:**
```json
{
  "memory_config": {
    "primary_memory": "stm",
    "secondary_memory": ["ltm", "kg"],
    "memory_weights": {
      "stm": 1.0,
      "ltm": 0.6,
      "kg": 0.3,
      "mm": 0.0
    },
    "reasoning_depth": "deep",
    "enable_multimodal": false
  },
  "performance_prediction": {
    "efficiency_gain": 0.82,
    "coherence_gain": 0.78,
    "resource_cost": 0.45,
    "cost_benefit_ratio": 1.82,
    "confidence_score": 0.89
  },
  "resource_requirements": {
    "estimated_memory_mb": 512,
    "estimated_cpu_percent": 45,
    "estimated_response_time_ms": 850
  },
  "trace": {
    "task_id": "task_7x9k2Lm4",
    "timestamp": "2026-03-29T10:30:00Z",
    "selected_config": {...},
    "candidate_configs": [...],
    "reasoning": "Selected STM primary due to high context dependency..."
  }
}
```

### 2.2 Select Memory with What-If Analysis

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/adaptive/select" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "task_id": "task_abc123",
      "task_type": "query",
      "complexity": 0.5,
      "modality_requirements": ["text"],
      "temporal_scope": "short",
      "reasoning_depth": "shallow",
      "context_dependency": 0.3,
      "user_id": "user_001",
      "agent_id": "agent_001"
    },
    "resource_constraints": {
      "max_memory_usage_mb": 512,
      "max_cpu_usage_percent": 50,
      "max_response_time_ms": 1000,
      "storage_quota_percent": 50
    },
    "preferences": {
      "prioritize_efficiency": true,
      "prioritize_coherence": true,
      "enable_multimodal": false,
      "enable_reasoning": false
    },
    "what_if_constraints": {
      "max_memory_usage_mb": 2048,
      "max_cpu_usage_percent": 90,
      "max_response_time_ms": 3000,
      "storage_quota_percent": 90
    }
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/adaptive/select",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "task_context": {
            "task_id": "task_abc123",
            "task_type": "query",
            "complexity": 0.5,
            "modality_requirements": ["text"],
            "temporal_scope": "short",
            "reasoning_depth": "shallow",
            "context_dependency": 0.3,
            "user_id": "user_001",
            "agent_id": "agent_001"
        },
        "resource_constraints": {
            "max_memory_usage_mb": 512,
            "max_cpu_usage_percent": 50,
            "max_response_time_ms": 1000,
            "storage_quota_percent": 50
        },
        "preferences": {
            "prioritize_efficiency": True,
            "prioritize_coherence": True,
            "enable_multimodal": False,
            "enable_reasoning": False
        },
        "what_if_constraints": {
            "max_memory_usage_mb": 2048,
            "max_cpu_usage_percent": 90,
            "max_response_time_ms": 3000,
            "storage_quota_percent": 90
        }
    }
)
# Compare actual vs hypothetical results
data = response.json()
print("Actual:", data["memory_config"])
print("What-If:", data.get("what_if_result", {}).get("memory_config"))
```

### 2.3 Dry Run Mode (No Persistence)

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/adaptive/select" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "task_id": "dry_run_task_001",
      "task_type": "task",
      "complexity": 0.65,
      "modality_requirements": ["text"],
      "temporal_scope": "medium",
      "reasoning_depth": "medium",
      "context_dependency": 0.6,
      "user_id": "user_test",
      "agent_id": "agent_test"
    },
    "resource_constraints": {
      "max_memory_usage_mb": 768,
      "max_cpu_usage_percent": 60,
      "max_response_time_ms": 1500,
      "storage_quota_percent": 60
    },
    "preferences": {
      "prioritize_efficiency": false,
      "prioritize_coherence": true,
      "enable_multimodal": false,
      "enable_reasoning": true
    },
    "dry_run": true
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/adaptive/select",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "task_context": {
            "task_id": "dry_run_task_001",
            "task_type": "task",
            "complexity": 0.65,
            "modality_requirements": ["text"],
            "temporal_scope": "medium",
            "reasoning_depth": "medium",
            "context_dependency": 0.6,
            "user_id": "user_test",
            "agent_id": "agent_test"
        },
        "resource_constraints": {
            "max_memory_usage_mb": 768,
            "max_cpu_usage_percent": 60,
            "max_response_time_ms": 1500,
            "storage_quota_percent": 60
        },
        "preferences": {
            "prioritize_efficiency": False,
            "prioritize_coherence": True,
            "enable_multimodal": False,
            "enable_reasoning": True
        },
        "dry_run": True
    }
)
print(response.json())
```

### 2.4 Get Memory Status

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/adaptive/status" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/adaptive/status",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "current_config": {
    "primary_memory": "stm",
    "secondary_memory": ["ltm"],
    "memory_weights": {
      "stm": 1.0,
      "ltm": 0.6,
      "kg": 0.0,
      "mm": 0.0
    },
    "reasoning_depth": "medium",
    "enable_multimodal": false
  },
  "performance_metrics": {
    "efficiency_score": 0.85,
    "coherence_score": 0.92,
    "response_time_ms": 850,
    "memory_usage_mb": 256,
    "cpu_usage_percent": 35
  },
  "resource_status": {
    "memory_usage_mb": 384,
    "memory_usage_percent": 38,
    "cpu_usage_percent": 42,
    "response_time_ms": 720,
    "storage_usage_percent": 25
  }
}
```

### 2.5 Get Decision Traces

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/traces?task_id=task_7x9k2Lm4&limit=10" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/traces",
    params={"task_id": "task_7x9k2Lm4", "limit": 10},
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "traces": [
    {
      "trace_id": "trace_01HXYZ...",
      "task_id": "task_7x9k2Lm4",
      "created_at": "2026-03-29T10:30:00Z",
      "trace": {
        "task_id": "task_7x9k2Lm4",
        "timestamp": "2026-03-29T10:30:00Z",
        "selected_config": {...},
        "candidate_configs": [...],
        "reasoning": "..."
      }
    }
  ]
}
```

### 2.6 Analyze Task Characteristics

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/analyzer/task-characteristics" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "content": "Analyze the performance metrics from Q1 2026 and provide insights on customer retention patterns",
      "modality": ["text"],
      "context_history": [
        {
          "role": "user",
          "content": "Show me the quarterly report",
          "timestamp": "2026-03-28T14:00:00Z"
        }
      ],
      "task_metadata": {
        "domain": "business_analytics",
        "complexity_hint": "high",
        "expected_duration": "medium"
      }
    }
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/analyzer/task-characteristics",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "task_context": {
            "content": "Analyze the performance metrics from Q1 2026 and provide insights on customer retention patterns",
            "modality": ["text"],
            "context_history": [
                {
                    "role": "user",
                    "content": "Show me the quarterly report",
                    "timestamp": "2026-03-28T14:00:00Z"
                }
            ],
            "task_metadata": {
                "domain": "business_analytics",
                "complexity_hint": "high",
                "expected_duration": "medium"
            }
        }
    }
)
print(response.json())
```

**Response:**
```json
{
  "characteristics": {
    "complexity": 0.78,
    "modality_count": 1,
    "temporal_scope": "long",
    "reasoning_depth": 0.85,
    "context_dependency": 0.72
  },
  "memory_strategy": {
    "primary_memory": "ltm",
    "secondary_memory": ["kg", "stm"],
    "enable_multimodal": false,
    "reasoning_depth": "deep"
  },
  "confidence_score": 0.87
}
```

### 2.7 Batch Task Analysis

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/analyzer/batch-characteristics" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "tasks": [
      {
        "task_id": "batch_task_001",
        "task_context": {
          "content": "Simple factual question about Python",
          "modality": ["text"],
          "context_history": [],
          "task_metadata": null
        }
      },
      {
        "task_id": "batch_task_002",
        "task_context": {
          "content": "Debug this complex multi-threaded code",
          "modality": ["text"],
          "context_history": [
            {"role": "user", "content": "The code hangs here", "timestamp": "2026-03-29T09:00:00Z"}
          ],
          "task_metadata": {"domain": "programming", "complexity_hint": "high"}
        }
      },
      {
        "task_id": "batch_task_003",
        "task_context": {
          "content": "Generate an image of a sunset",
          "modality": ["text", "image"],
          "context_history": [],
          "task_metadata": null
        }
      }
    ]
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/analyzer/batch-characteristics",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "tasks": [
            {
                "task_id": "batch_task_001",
                "task_context": {
                    "content": "Simple factual question about Python",
                    "modality": ["text"],
                    "context_history": [],
                    "task_metadata": None
                }
            },
            {
                "task_id": "batch_task_002",
                "task_context": {
                    "content": "Debug this complex multi-threaded code",
                    "modality": ["text"],
                    "context_history": [
                        {"role": "user", "content": "The code hangs here", "timestamp": "2026-03-29T09:00:00Z"}
                    ],
                    "task_metadata": {"domain": "programming", "complexity_hint": "high"}
                }
            },
            {
                "task_id": "batch_task_003",
                "task_context": {
                    "content": "Generate an image of a sunset",
                    "modality": ["text", "image"],
                    "context_history": [],
                    "task_metadata": None
                }
            }
        ]
    }
)
print(response.json())
```

**Response:**
```json
{
  "results": [
    {
      "task_id": "batch_task_001",
      "characteristics": {
        "complexity": 0.2,
        "modality_count": 1,
        "temporal_scope": "short",
        "reasoning_depth": 0.15,
        "context_dependency": 0.1
      },
      "memory_strategy": {
        "primary_memory": "stm",
        "secondary_memory": [],
        "enable_multimodal": false,
        "reasoning_depth": "shallow"
      }
    },
    {
      "task_id": "batch_task_002",
      "characteristics": {
        "complexity": 0.85,
        "modality_count": 1,
        "temporal_scope": "medium",
        "reasoning_depth": 0.9,
        "context_dependency": 0.75
      },
      "memory_strategy": {
        "primary_memory": "stm",
        "secondary_memory": ["ltm", "kg"],
        "enable_multimodal": false,
        "reasoning_depth": "deep"
      }
    },
    {
      "task_id": "batch_task_003",
      "characteristics": {
        "complexity": 0.55,
        "modality_count": 2,
        "temporal_scope": "short",
        "reasoning_depth": 0.3,
        "context_dependency": 0.2
      },
      "memory_strategy": {
        "primary_memory": "mm",
        "secondary_memory": ["stm"],
        "enable_multimodal": true,
        "reasoning_depth": "shallow"
      }
    }
  ],
  "batch_metrics": {
    "total_tasks": 3,
    "processed_tasks": 3,
    "average_complexity": 0.53,
    "processing_time_ms": 45
  }
}
```

### 2.8 Predict Performance

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/predictor/performance" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "task_profile": {
      "complexity": 0.7,
      "modality_count": 1,
      "temporal_scope": "medium",
      "reasoning_depth": 0.75,
      "context_dependency": 0.65
    },
    "memory_config": {
      "primary_memory": "stm",
      "secondary_memory": ["ltm", "kg"],
      "memory_weights": {
        "stm": 1.0,
        "ltm": 0.7,
        "kg": 0.4,
        "mm": 0.0
      },
      "reasoning_depth": "deep",
      "enable_multimodal": false
    }
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/predictor/performance",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "task_profile": {
            "complexity": 0.7,
            "modality_count": 1,
            "temporal_scope": "medium",
            "reasoning_depth": 0.75,
            "context_dependency": 0.65
        },
        "memory_config": {
            "primary_memory": "stm",
            "secondary_memory": ["ltm", "kg"],
            "memory_weights": {
                "stm": 1.0,
                "ltm": 0.7,
                "kg": 0.4,
                "mm": 0.0
            },
            "reasoning_depth": "deep",
            "enable_multimodal": False
        }
    }
)
print(response.json())
```

**Response:**
```json
{
  "predicted_performance": {
    "efficiency_gain": 0.81,
    "coherence_gain": 0.76,
    "resource_cost": 0.52,
    "cost_benefit_ratio": 1.56,
    "confidence_score": 0.85
  },
  "synergy_factor": 1.15,
  "decay_factor": 0.92,
  "performance_breakdown": {
    "stm_contribution": 0.45,
    "ltm_contribution": 0.32,
    "kg_contribution": 0.18,
    "mm_contribution": 0.0
  }
}
```

### 2.9 Get Performance Baselines

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/predictor/baselines" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/predictor/baselines",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "performance_baselines": {
    "stm": {
      "efficiency_gain": 0.95,
      "coherence_gain": 0.7,
      "resource_cost": 0.2
    },
    "ltm": {
      "efficiency_gain": 0.75,
      "coherence_gain": 0.9,
      "resource_cost": 0.4
    },
    "kg": {
      "efficiency_gain": 0.6,
      "coherence_gain": 0.85,
      "resource_cost": 0.5
    },
    "mm": {
      "efficiency_gain": 0.5,
      "coherence_gain": 0.8,
      "resource_cost": 0.7
    }
  },
  "marginal_decay_factors": {
    "stm_to_ltm": 0.85,
    "ltm_to_kg": 0.75,
    "kg_to_mm": 0.65
  }
}
```

### Error Handling - Adaptive Selection

```python
import requests

# Missing required field
response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/adaptive/select",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "task_context": {
            # Missing task_id and other required fields
            "complexity": 0.5
        },
        "resource_constraints": {},
        "preferences": {}
    }
)
if response.status_code == 400:
    print("Validation error:", response.json())

# Invalid task_type
response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/adaptive/select",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "task_context": {
            "task_id": "task_001",
            "task_type": "invalid_type",  # Must be: conversation, task, query
            "complexity": 0.5,
            "modality_requirements": ["text"],
            "temporal_scope": "short",
            "reasoning_depth": "shallow",
            "context_dependency": 0.3,
            "user_id": "user_001",
            "agent_id": "agent_001"
        },
        "resource_constraints": {
            "max_memory_usage_mb": 512,
            "max_cpu_usage_percent": 50,
            "max_response_time_ms": 1000,
            "storage_quota_percent": 50
        },
        "preferences": {
            "prioritize_efficiency": True,
            "prioritize_coherence": True,
            "enable_multimodal": False,
            "enable_reasoning": False
        }
    }
)
if response.status_code == 400:
    print("Invalid task_type:", response.json())
```

---

## 3. Memory Storage

### 3.1 Store Short-Term Memory (STM)

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/storage/stm" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "userId": "user_001",
    "agentId": "agent_001",
    "sessionType": "conversation",
    "role": "user",
    "content": "What is the difference between async and await in Python?",
    "maxContextLength": 4096,
    "retentionHours": 24
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/storage/stm",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "userId": "user_001",
        "agentId": "agent_001",
        "sessionType": "conversation",
        "role": "user",
        "content": "What is the difference between async and await in Python?",
        "maxContextLength": 4096,
        "retentionHours": 24
    }
)
print(response.json())
```

**Response:**
```json
{
  "sessionId": "01HXYZ123AB...",
  "messageId": "01HXYZ456CD..."
}
```

### 3.2 Store Assistant Response in STM

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/storage/stm" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "userId": "user_001",
    "agentId": "agent_001",
    "sessionType": "conversation",
    "role": "assistant",
    "content": "async/await are used for asynchronous programming in Python. async def defines a coroutine function, and await suspends execution until the coroutine completes. This allows other tasks to run while waiting for I/O operations.",
    "maxContextLength": 4096,
    "retentionHours": 24
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/storage/stm",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "userId": "user_001",
        "agentId": "agent_001",
        "sessionType": "conversation",
        "role": "assistant",
        "content": "async/await are used for asynchronous programming in Python. async def defines a coroutine function, and await suspends execution until the coroutine completes.",
        "maxContextLength": 4096,
        "retentionHours": 24
    }
)
print(response.json())
```

### 3.3 List STM Sessions

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/storage/sessions?user_id=user_001&status=active&limit=20" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/storage/sessions",
    params={"user_id": "user_001", "status": "active", "limit": 20},
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "sessions": [
    {
      "session_id": "01HXYZ123AB...",
      "user_id": "user_001",
      "agent_id": "agent_001",
      "session_type": "conversation",
      "status": "active",
      "message_count": 24,
      "created_at": "2026-03-28T09:00:00Z",
      "updated_at": "2026-03-29T10:30:00Z"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

### 3.4 Get Session Messages

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/storage/stm/01HXYZ123AB...?limit=50" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

session_id = "01HXYZ123AB..."
response = requests.get(
    f"http://127.0.0.1:8008/api/v1/memory/storage/stm/{session_id}",
    params={"limit": 50},
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "messages": [
    {
      "message_id": "01HXYZ111AA...",
      "session_id": "01HXYZ123AB...",
      "role": "user",
      "content": "What is the difference between async and await in Python?",
      "created_at": "2026-03-29T10:25:00Z"
    },
    {
      "message_id": "01HXYZ222BB...",
      "session_id": "01HXYZ123AB...",
      "role": "assistant",
      "content": "async/await are used for asynchronous programming...",
      "created_at": "2026-03-29T10:25:05Z"
    }
  ]
}
```

### 3.5 Store Long-Term Memory (LTM)

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/storage/ltm" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sourceId": "doc_python_async_001",
    "sourceType": "document",
    "title": "Python Async/Await Programming Guide",
    "content": "Asynchronous programming in Python allows you to write concurrent code using the async/await syntax. async def defines a coroutine function that can use await to suspend execution. This is particularly useful for I/O-bound operations like network requests, file I/O, and database queries. The asyncio module provides the foundation for async programming in Python's standard library."
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/storage/ltm",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "sourceId": "doc_python_async_001",
        "sourceType": "document",
        "title": "Python Async/Await Programming Guide",
        "content": "Asynchronous programming in Python allows you to write concurrent code using the async/await syntax..."
    }
)
print(response.json())
```

**Response:**
```json
{
  "entryId": "01HXYZ789LM..."
}
```

### 3.6 Batch Store LTM

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/storage/batch-ltm" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "entries": [
      {
        "sourceId": "doc_rust_001",
        "sourceType": "documentation",
        "title": "Rust Ownership Model",
        "content": "Rust's ownership system is its most distinctive feature, managing memory through a set of rules that the compiler checks. Every value has a single owner, and when the owner goes out of scope, the value is dropped."
      },
      {
        "sourceId": "doc_rust_002",
        "sourceType": "documentation",
        "title": "Rust Borrowing and References",
        "content": "Borrowing allows you to use values without taking ownership. References come in two forms: immutable (&T) and mutable (&mut T). The borrow checker enforces that references must always be valid."
      },
      {
        "sourceId": "doc_rust_003",
        "sourceType": "documentation",
        "title": "Rust Lifetimes",
        "content": "Lifetimes are Rust's way of ensuring that references are valid for as long as they are used. The compiler uses lifetime annotations to ensure no dangling references exist."
      },
      {
        "sourceId": "doc_rust_004",
        "sourceType": "documentation",
        "title": "Rust Traits and Generics",
        "content": "Traits define shared behavior similar to interfaces in other languages. Generics allow you to write flexible, reusable functions and types that work with different data types."
      }
    ]
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/storage/batch-ltm",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "entries": [
            {
                "sourceId": "doc_rust_001",
                "sourceType": "documentation",
                "title": "Rust Ownership Model",
                "content": "Rust's ownership system is its most distinctive feature..."
            },
            {
                "sourceId": "doc_rust_002",
                "sourceType": "documentation",
                "title": "Rust Borrowing and References",
                "content": "Borrowing allows you to use values without taking ownership..."
            },
            {
                "sourceId": "doc_rust_003",
                "sourceType": "documentation",
                "title": "Rust Lifetimes",
                "content": "Lifetimes are Rust's way of ensuring that references are valid..."
            },
            {
                "sourceId": "doc_rust_004",
                "sourceType": "documentation",
                "title": "Rust Traits and Generics",
                "content": "Traits define shared behavior similar to interfaces..."
            }
        ]
    }
)
print(response.json())
```

**Response:**
```json
{
  "entryIds": [
    "01HXYZaabb01",
    "01HXYZaabb02",
    "01HXYZaabb03",
    "01HXYZaabb04"
  ]
}
```

### 3.7 Transfer STM to LTM

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/storage/transfer" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sessionId": "01HXYZ123AB...",
    "messageCountThreshold": 50
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/storage/transfer",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "sessionId": "01HXYZ123AB...",
        "messageCountThreshold": 50
    }
)
print(response.json())
```

**Response:**
```json
{
  "entryIds": [
    "01HXYZtransfer01",
    "01HXYZtransfer02",
    "01HXYZtransfer03"
  ]
}
```

### 3.8 Compress Session Context

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/storage/compress/session" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "session_id": "01HXYZ123AB...",
    "strategy": "llm_summary",
    "token_budget": 2048,
    "window_size": 20,
    "hierarchical_recent_k": 10
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/storage/compress/session",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "session_id": "01HXYZ123AB...",
        "strategy": "llm_summary",
        "token_budget": 2048,
        "window_size": 20,
        "hierarchical_recent_k": 10
    }
)
print(response.json())
```

**Response:**
```json
{
  "original_token_count": 4500,
  "compressed_token_count": 1850,
  "compression_ratio": 0.41,
  "strategy_used": "llm_summary",
  "compressed_content": "This session covered Python async programming. Key points: async/await syntax for coroutines, asyncio module usage, concurrent task handling with gather(), and best practices for avoiding blocking operations..."
}
```

### 3.9 Compress Arbitrary Messages

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/storage/compress/messages" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [
      {"role": "user", "content": "Hello, I need help with my Python project", "timestamp": "2026-03-29T09:00:00Z"},
      {"role": "assistant", "content": "Hi! Id be happy to help. What specific issue are you facing?", "timestamp": "2026-03-29T09:00:05Z"},
      {"role": "user", "content": "Im getting a TypeError when calling my async function", "timestamp": "2026-03-29T09:01:00Z"},
      {"role": "assistant", "content": "This usually happens when you forget to await an async function. Can you share the code snippet?", "timestamp": "2026-03-29T09:01:10Z"},
      {"role": "user", "content": "Here it is: result = my_async_func()", "timestamp": "2026-03-29T09:02:00Z"}
    ],
    "strategy": "hierarchical",
    "token_budget": 500,
    "window_size": 10,
    "hierarchical_recent_k": 5
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/storage/compress/messages",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "messages": [
            {"role": "user", "content": "Hello, I need help with my Python project", "timestamp": "2026-03-29T09:00:00Z"},
            {"role": "assistant", "content": "Hi! Id be happy to help. What specific issue are you facing?", "timestamp": "2026-03-29T09:00:05Z"},
            {"role": "user", "content": "Im getting a TypeError when calling my async function", "timestamp": "2026-03-29T09:01:00Z"},
            {"role": "assistant", "content": "This usually happens when you forget to await an async function.", "timestamp": "2026-03-29T09:01:10Z"},
            {"role": "user", "content": "Here it is: result = my_async_func()", "timestamp": "2026-03-29T09:02:00Z"}
        ],
        "strategy": "hierarchical",
        "token_budget": 500,
        "window_size": 10,
        "hierarchical_recent_k": 5
    }
)
print(response.json())
```

### Error Handling - Memory Storage

```python
import requests

# Session not found
response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/storage/stm/nonexistent_session",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
if response.status_code == 404:
    print("Session not found:", response.json())

# Invalid session type
response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/storage/stm",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "userId": "user_001",
        "agentId": "agent_001",
        "sessionType": "invalid_type",  # Must be: conversation, task, query
        "role": "user",
        "content": "Test message"
    }
)
if response.status_code == 400:
    print("Validation error:", response.json())

# Missing required fields
response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/storage/stm",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "userId": "user_001",
        # Missing: agentId, sessionType, role, content
    }
)
if response.status_code == 400:
    print("Missing required fields:", response.json())
```

---

## 4. Memory Search

### 4.1 Search STM

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/search/stm" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "userId": "user_001",
    "agentId": "agent_001",
    "sessionType": "conversation",
    "limit": 10
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/search/stm",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "userId": "user_001",
        "agentId": "agent_001",
        "sessionType": "conversation",
        "limit": 10
    }
)
print(response.json())
```

**Response:**
```json
{
  "messages": [
    {
      "message_id": "01HXYZ111AA...",
      "session_id": "01HXYZ123AB...",
      "role": "user",
      "content": "What is the difference between async and await in Python?",
      "created_at": "2026-03-29T10:25:00Z"
    },
    {
      "message_id": "01HXYZ222BB...",
      "session_id": "01HXYZ123AB...",
      "role": "assistant",
      "content": "async/await are used for asynchronous programming...",
      "created_at": "2026-03-29T10:25:05Z"
    }
  ]
}
```

### 4.2 List LTM Entries

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/search/ltm" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/search/ltm",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "entries": [
    {
      "entry_id": "01HXYZ789LM...",
      "title": "Python Async/Await Programming Guide",
      "content": "Asynchronous programming in Python...",
      "source_id": "doc_python_async_001",
      "source_type": "document",
      "created_at": "2026-03-28T15:30:00Z",
      "updated_at": "2026-03-28T15:30:00Z"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

### 4.3 Semantic Search LTM

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/search/ltm" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "asynchronous programming coroutines concurrency",
    "topK": 5,
    "enableRerank": true,
    "minScore": 0.5
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/search/ltm",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "query": "asynchronous programming coroutines concurrency",
        "topK": 5,
        "enableRerank": True,
        "minScore": 0.5
    }
)
print(response.json())
```

**Response:**
```json
{
  "results": [
    {
      "entry_id": "01HXYZ789LM...",
      "score": 0.92,
      "title": "Python Async/Await Programming Guide",
      "content": "Asynchronous programming in Python allows you to write concurrent code using the async/await syntax...",
      "metadata": {
        "source_id": "doc_python_async_001",
        "source_type": "document"
      }
    },
    {
      "entry_id": "01HXYZasync02...",
      "score": 0.78,
      "title": "Understanding JavaScript Promises",
      "content": "Promises represent values that may be available now, or in the future, or never...",
      "metadata": {}
    }
  ]
}
```

### 4.4 Get LTM Entry by ID

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/search/ltm/01HXYZ789LM..." \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

entry_id = "01HXYZ789LM..."
response = requests.get(
    f"http://127.0.0.1:8008/api/v1/memory/search/ltm/{entry_id}",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "entry_id": "01HXYZ789LM...",
  "title": "Python Async/Await Programming Guide",
  "content": "Asynchronous programming in Python allows you to write concurrent code using the async/await syntax. async def defines a coroutine function that can use await to suspend execution.",
  "source_id": "doc_python_async_001",
  "source_type": "document",
  "created_at": "2026-03-28T15:30:00Z",
  "updated_at": "2026-03-28T15:30:00Z",
  "tags": ["python", "async", "programming"]
}
```

### 4.5 Hybrid Search (Keyword + Vector)

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/search/hybrid" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Rust ownership borrowing lifetime",
    "topK": 5,
    "keywordWeight": 0.3,
    "vectorWeight": 0.7,
    "enableRerank": true,
    "minScore": 0.4
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/search/hybrid",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "query": "Rust ownership borrowing lifetime",
        "topK": 5,
        "keywordWeight": 0.3,
        "vectorWeight": 0.7,
        "enableRerank": True,
        "minScore": 0.4
    }
)
print(response.json())
```

**Response:**
```json
{
  "results": [
    {
      "entry_id": "01HXYZaabb01",
      "score": 0.89,
      "title": "Rust Ownership Model",
      "content": "Rust's ownership system is its most distinctive feature...",
      "metadata": {}
    },
    {
      "entry_id": "01HXYZaabb02",
      "score": 0.85,
      "title": "Rust Borrowing and References",
      "content": "Borrowing allows you to use values without taking ownership...",
      "metadata": {}
    },
    {
      "entry_id": "01HXYZaabb03",
      "score": 0.82,
      "title": "Rust Lifetimes",
      "content": "Lifetimes are Rust's way of ensuring that references are valid...",
      "metadata": {}
    }
  ]
}
```

### 4.6 Triple Hybrid Search (Vector + Keyword + Knowledge Graph)

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/search/triple" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "machine learning neural networks deep learning",
    "topK": 10,
    "vectorWeight": 0.5,
    "keywordWeight": 0.3,
    "graphWeight": 0.2,
    "enableRerank": true,
    "minScore": 0.3
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/search/triple",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "query": "machine learning neural networks deep learning",
        "topK": 10,
        "vectorWeight": 0.5,
        "keywordWeight": 0.3,
        "graphWeight": 0.2,
        "enableRerank": True,
        "minScore": 0.3
    }
)
print(response.json())
```

**Response:**
```json
{
  "results": [
    {
      "entry_id": "01HXYZml001...",
      "score": 0.94,
      "title": "Introduction to Deep Learning",
      "content": "Deep learning is a subset of machine learning...",
      "metadata": {"source": "textbook"}
    },
    {
      "entry_id": "01HXYZnn002...",
      "score": 0.91,
      "title": "Neural Network Architectures",
      "content": "Neural networks are inspired by biological neural networks...",
      "metadata": {"source": "lecture_notes"}
    },
    {
      "entry_id": "01HXYZkg003...",
      "score": 0.88,
      "title": "Convolutional Neural Networks",
      "content": "CNNs are specialized for processing grid-like data such as images...",
      "metadata": {"source": "research_paper"}
    }
  ]
}
```

### 4.7 Scored Search with Confidence

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/search/scored" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Python best practices testing",
    "topK": 5,
    "vectorWeight": 0.5,
    "keywordWeight": 0.3,
    "graphWeight": 0.2,
    "enableRerank": true,
    "minScore": 0.3,
    "confidence_config": {
      "quality_weight": 0.3,
      "relevance_weight": 0.4,
      "recency_weight": 0.15,
      "access_weight": 0.1,
      "completeness_weight": 0.05,
      "recency_half_life_days": 30
    }
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/search/scored",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "query": "Python best practices testing",
        "topK": 5,
        "vectorWeight": 0.5,
        "keywordWeight": 0.3,
        "graphWeight": 0.2,
        "enableRerank": True,
        "minScore": 0.3,
        "confidence_config": {
            "quality_weight": 0.3,
            "relevance_weight": 0.4,
            "recency_weight": 0.15,
            "access_weight": 0.1,
            "completeness_weight": 0.05,
            "recency_half_life_days": 30
        }
    }
)
print(response.json())
```

### 4.8 Search by Entity

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/search/entity" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "entity": "Python",
    "limit": 10
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/search/entity",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "entity": "Python",
        "limit": 10
    }
)
print(response.json())
```

### 4.9 Time Travel - Query LTM at Specific Time

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/search/ltm/time-travel" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "at": "2026-03-01T00:00:00Z",
    "limit": 10
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/search/ltm/time-travel",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "at": "2026-03-01T00:00:00Z",
        "limit": 10
    }
)
print(response.json())
```

### 4.10 Get Entry History

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/search/ltm/01HXYZ789LM.../history" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

entry_id = "01HXYZ789LM..."
response = requests.get(
    f"http://127.0.0.1:8008/api/v1/memory/search/ltm/{entry_id}/history",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "history": [
    {
      "entry_id": "01HXYZ789LM...",
      "title": "Python Async/Await Programming Guide",
      "content": "Asynchronous programming in Python...",
      "version": 3,
      "updated_at": "2026-03-28T15:30:00Z"
    },
    {
      "entry_id": "01HXYZ789LM...",
      "title": "Python Async Programming",
      "content": "Async programming in Python...",
      "version": 2,
      "updated_at": "2026-03-20T10:15:00Z"
    },
    {
      "entry_id": "01HXYZ789LM...",
      "title": "Async Python",
      "content": "Python async tutorial...",
      "version": 1,
      "updated_at": "2026-03-15T08:00:00Z"
    }
  ]
}
```

### Error Handling - Memory Search

```python
import requests

# Entry not found
response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/search/ltm/nonexistent_id",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
if response.status_code == 404:
    print("Entry not found:", response.json())

# Invalid time format for time travel
response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/search/ltm/time-travel",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "at": "not-a-valid-timestamp",
        "limit": 10
    }
)
if response.status_code == 400:
    print("Invalid time format:", response.json())

# Qdrant not available (for vector search)
response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/search/ltm",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "query": "test search",
        "topK": 5
    }
)
if response.status_code == 503:
    print("Vector service unavailable:", response.json())
```

---

## 5. Knowledge Graph

### 5.1 Create Entity

```bash
curl -X POST "http://127.0.0.1:8008/api/kg/entities" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "entityName": "Elon Musk",
    "entityType": "person",
    "description": "American entrepreneur and business magnate, founder of SpaceX and Tesla",
    "aliases": ["Elon Musk", "Musk", "E. Musk"]
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/kg/entities",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "entityName": "Elon Musk",
        "entityType": "person",
        "description": "American entrepreneur and business magnate, founder of SpaceX and Tesla",
        "aliases": ["Elon Musk", "Musk", "E. Musk"]
    }
)
print(response.json())
```

**Response:**
```json
{
  "entityId": "01HXYZkgentity001"
}
```

### 5.2 List Entities

```bash
curl -X GET "http://127.0.0.1:8008/api/kg/entities?entity_type=person&limit=20&offset=0" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/kg/entities",
    params={"entity_type": "person", "limit": 20, "offset": 0},
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "entities": [
    {
      "entityId": "01HXYZkgentity001",
      "entityName": "Elon Musk",
      "entityType": "person",
      "description": "American entrepreneur and business magnate, founder of SpaceX and Tesla"
    },
    {
      "entityId": "01HXYZkgentity002",
      "entityName": "Steve Jobs",
      "entityType": "person",
      "description": "American entrepreneur and industrial designer, co-founder of Apple"
    }
  ],
  "total": 2,
  "limit": 20,
  "offset": 0
}
```

### 5.3 Get Entity by Name

```bash
curl -X GET "http://127.0.0.1:8008/api/kg/entities/by-name/Elon%20Musk" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/kg/entities/by-name/Elon%20Musk",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "entityId": "01HXYZkgentity001",
  "entityName": "Elon Musk",
  "entityType": "person",
  "description": "American entrepreneur and business magnate, founder of SpaceX and Tesla"
}
```

### 5.4 Create Relation

```bash
curl -X POST "http://127.0.0.1:8008/api/kg/relations" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sourceEntityId": "01HXYZkgentity001",
    "targetEntityId": "01HXYZkgentity002",
    "relationType": "competitor",
    "weight": 0.85,
    "confidence": 0.92
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/kg/relations",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "sourceEntityId": "01HXYZkgentity001",
        "targetEntityId": "01HXYZkgentity002",
        "relationType": "competitor",
        "weight": 0.85,
        "confidence": 0.92
    }
)
print(response.json())
```

**Response:**
```json
{
  "relationId": "01HXYZkgrel001"
}
```

### 5.5 Get Related Entities

```bash
curl -X GET "http://127.0.0.1:8008/api/kg/entities/01HXYZkgentity001/related?limit=10" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

entity_id = "01HXYZkgentity001"
response = requests.get(
    f"http://127.0.0.1:8008/api/kg/entities/{entity_id}/related",
    params={"limit": 10},
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "relations": [
    {
      "relationId": "01HXYZkgrel001",
      "sourceEntityId": "01HXYZkgentity001",
      "targetEntityId": "01HXYZkgentity002",
      "relationType": "competitor",
      "weight": 0.85,
      "confidence": 0.92
    },
    {
      "relationId": "01HXYZkgrel002",
      "sourceEntityId": "01HXYZkgentity001",
      "targetEntityId": "01HXYZkgentity003",
      "relationType": "founder",
      "weight": 1.0,
      "confidence": 1.0
    }
  ]
}
```

### 5.6 Search Knowledge by Entity

```bash
curl -X POST "http://127.0.0.1:8008/api/kg/search" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Tesla",
    "limit": 10
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/kg/search",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "query": "Tesla",
        "limit": 10
    }
)
print(response.json())
```

**Response:**
```json
{
  "entities": [
    {
      "entityId": "01HXYZkgentity003",
      "entityName": "Tesla",
      "entityType": "company",
      "description": "American electric vehicle and clean energy company"
    },
    {
      "entityId": "01HXYZkgentity001",
      "entityName": "Elon Musk",
      "entityType": "person",
      "description": "Founder of Tesla"
    }
  ]
}
```

### 5.7 Time Travel - Query KG Entity at Specific Time

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/search/kg/01HXYZkgentity001/at?at=2026-01-01T00:00:00Z" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

entity_id = "01HXYZkgentity001"
response = requests.get(
    f"http://127.0.0.1:8008/api/v1/memory/search/kg/{entity_id}/at",
    params={"at": "2026-01-01T00:00:00Z"},
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

### 5.8 Get KG Entity History

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/search/kg/01HXYZkgentity001/history" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

entity_id = "01HXYZkgentity001"
response = requests.get(
    f"http://127.0.0.1:8008/api/v1/memory/search/kg/{entity_id}/history",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

### Error Handling - Knowledge Graph

```python
import requests

# Entity not found
response = requests.get(
    "http://127.0.0.1:8008/api/kg/entities/nonexistent",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
if response.status_code == 404:
    print("Entity not found:", response.json())

# Missing required fields
response = requests.post(
    "http://127.0.0.1:8008/api/kg/entities",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "entityName": "Test Entity"
        # Missing entityType
    }
)
if response.status_code == 400:
    print("Missing required fields:", response.json())

# Invalid relation (source equals target)
response = requests.post(
    "http://127.0.0.1:8008/api/kg/relations",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "sourceEntityId": "01HXYZkgentity001",
        "targetEntityId": "01HXYZkgentity001",  # Same as source
        "relationType": "equals"
    }
)
if response.status_code == 400:
    print("Invalid relation:", response.json())
```

---

## 6. Multimodal Memory

### 6.1 Store Image Memory

```bash
curl -X POST "http://127.0.0.1:8008/api/mm/store" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sessionId": "01HXYZ123AB...",
    "sourceId": "img_screenshot_001",
    "modalityType": "image",
    "title": "Architecture Diagram Screenshot",
    "description": "Screenshot of system architecture diagram showing components",
    "textContent": "The architecture consists of frontend, backend, and database layers with async message queue",
    "imageUrl": "https://example.com/images/architecture.png"
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/mm/store",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "sessionId": "01HXYZ123AB...",
        "sourceId": "img_screenshot_001",
        "modalityType": "image",
        "title": "Architecture Diagram Screenshot",
        "description": "Screenshot of system architecture diagram showing components",
        "textContent": "The architecture consists of frontend, backend, and database layers",
        "imageUrl": "https://example.com/images/architecture.png"
    }
)
print(response.json())
```

**Response:**
```json
{
  "entryId": "01HXYZmm001..."
}
```

### 6.2 Store Audio Memory

```bash
curl -X POST "http://127.0.0.1:8008/api/mm/store" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sessionId": "01HXYZ123AB...",
    "sourceId": "audio_meeting_001",
    "modalityType": "audio",
    "title": "Sprint Planning Meeting Recording",
    "description": "Audio recording of Q2 sprint planning session",
    "textContent": "Team agreed on 3 major features: user dashboard, API v2, and performance optimization. Timeline: 6 weeks.",
    "audioUrl": "https://example.com/audio/sprint_planning.mp3"
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/mm/store",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "sessionId": "01HXYZ123AB...",
        "sourceId": "audio_meeting_001",
        "modalityType": "audio",
        "title": "Sprint Planning Meeting Recording",
        "description": "Audio recording of Q2 sprint planning session",
        "textContent": "Team agreed on 3 major features: user dashboard, API v2, and performance optimization.",
        "audioUrl": "https://example.com/audio/sprint_planning.mp3"
    }
)
print(response.json())
```

### 6.3 Store Video Memory

```bash
curl -X POST "http://127.0.0.1:8008/api/mm/store" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sessionId": "01HXYZ123AB...",
    "sourceId": "video_demo_001",
    "modalityType": "video",
    "title": "Product Demo Video",
    "description": "Demo video showing new feature workflow",
    "textContent": "The demo shows how to use the new dashboard. Key steps: 1) Navigate to dashboard, 2) Select date range, 3) View analytics."
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/mm/store",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "sessionId": "01HXYZ123AB...",
        "sourceId": "video_demo_001",
        "modalityType": "video",
        "title": "Product Demo Video",
        "description": "Demo video showing new feature workflow",
        "textContent": "The demo shows how to use the new dashboard..."
    }
)
print(response.json())
```

### 6.4 Store Base64 Encoded Content

```bash
curl -X POST "http://127.0.0.1:8008/api/mm/store" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sessionId": "01HXYZ123AB...",
    "sourceId": "img_embedded_001",
    "modalityType": "image",
    "title": "Embedded Chart Image",
    "content": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
  }'
```

```python
import requests
import base64

# Read image file and encode as base64
with open("chart.png", "rb") as f:
    image_data = base64.b64encode(f.read()).decode()

response = requests.post(
    "http://127.0.0.1:8008/api/mm/store",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "sessionId": "01HXYZ123AB...",
        "sourceId": "img_embedded_001",
        "modalityType": "image",
        "title": "Embedded Chart Image",
        "content": image_data
    }
)
print(response.json())
```

### 6.5 Get Multimodal Entry

```bash
curl -X GET "http://127.0.0.1:8008/api/mm/entry/01HXYZmm001..." \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

entry_id = "01HXYZmm001..."
response = requests.get(
    f"http://127.0.0.1:8008/api/mm/entry/{entry_id}",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "entryId": "01HXYZmm001...",
  "sessionId": "01HXYZ123AB...",
  "sourceId": "img_screenshot_001",
  "modalityType": "image",
  "title": "Architecture Diagram Screenshot",
  "description": "Screenshot of system architecture diagram showing components"
}
```

### 6.6 Get Session Multimodal Entries

```bash
curl -X GET "http://127.0.0.1:8008/api/mm/session/01HXYZ123AB...?limit=20" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

session_id = "01HXYZ123AB..."
response = requests.get(
    f"http://127.0.0.1:8008/api/mm/session/{session_id}",
    params={"limit": 20},
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "entries": [
    {
      "entryId": "01HXYZmm001...",
      "sessionId": "01HXYZ123AB...",
      "sourceId": "img_screenshot_001",
      "modalityType": "image",
      "title": "Architecture Diagram Screenshot",
      "description": "Screenshot of system architecture diagram"
    },
    {
      "entryId": "01HXYZmm002...",
      "sessionId": "01HXYZ123AB...",
      "sourceId": "audio_meeting_001",
      "modalityType": "audio",
      "title": "Sprint Planning Meeting Recording",
      "description": "Audio recording of Q2 sprint planning"
    }
  ]
}
```

### 6.7 Get Multimodal by Modality Type

```bash
curl -X GET "http://127.0.0.1:8008/api/mm/modality/image?limit=10" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/mm/modality/image",
    params={"limit": 10},
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "entries": [
    {
      "entryId": "01HXYZmm001...",
      "sessionId": "01HXYZ123AB...",
      "sourceId": "img_screenshot_001",
      "modalityType": "image",
      "title": "Architecture Diagram Screenshot",
      "description": "Screenshot of system architecture diagram"
    },
    {
      "entryId": "01HXYZmm003...",
      "sessionId": "01HXYZ123AB...",
      "sourceId": "img_chart_002",
      "modalityType": "image",
      "title": "Q1 Performance Chart",
      "description": "Chart showing Q1 metrics"
    }
  ]
}
```

### 6.8 List All Multimodal Entries

```bash
curl -X GET "http://127.0.0.8008/api/mm/list?modality_type=video&limit=20&offset=0" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/mm/list",
    params={"modality_type": "video", "limit": 20, "offset": 0},
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "entries": [
    {
      "entryId": "01HXYZmm010...",
      "sessionId": "01HXYZ999ZZ...",
      "sourceId": "video_demo_001",
      "modalityType": "video",
      "title": "Product Demo Video",
      "description": "Demo video showing new feature workflow"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

### Error Handling - Multimodal

```python
import requests

# Entry not found
response = requests.get(
    "http://127.0.0.1:8008/api/mm/entry/nonexistent_id",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
if response.status_code == 404:
    print("Entry not found:", response.json())

# Invalid modality type
response = requests.post(
    "http://127.0.0.1:8008/api/mm/store",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "sourceId": "test_001",
        "modalityType": "invalid_modality",  # Must be: image, audio, video
        "title": "Test"
    }
)
if response.status_code == 400:
    print("Invalid modality type:", response.json())

# Base64 decode error
response = requests.post(
    "http://127.0.0.1:8008/api/mm/store",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "sourceId": "test_002",
        "modalityType": "image",
        "title": "Test",
        "content": "not-valid-base64!!!"
    }
)
if response.status_code == 400:
    print("Invalid base64:", response.json())
```

---

## 7. Weight Management

### 7.1 Get Current Weights Status

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/weights/status" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/weights/status",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "decay_lambda": 0.95,
  "active_weights": {
    "stm": 1.0,
    "ltm": 0.6,
    "kg": 0.0,
    "mm": 0.0
  }
}
```

### 7.2 Adjust Weights

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/weights/adjust" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "task_profile": {
      "complexity": 0.8,
      "modality_count": 1,
      "temporal_scope": "medium",
      "reasoning_depth": 0.85,
      "context_dependency": 0.7
    },
    "cost_benefit_ratio": 1.5,
    "current_weights": {
      "stm": 1.0,
      "ltm": 0.6,
      "kg": 0.3,
      "mm": 0.0
    }
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/weights/adjust",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "task_profile": {
            "complexity": 0.8,
            "modality_count": 1,
            "temporal_scope": "medium",
            "reasoning_depth": 0.85,
            "context_dependency": 0.7
        },
        "cost_benefit_ratio": 1.5,
        "current_weights": {
            "stm": 1.0,
            "ltm": 0.6,
            "kg": 0.3,
            "mm": 0.0
        }
    }
)
print(response.json())
```

**Response:**
```json
{
  "adjusted_weights": {
    "stm": 1.0,
    "ltm": 0.7,
    "kg": 0.4,
    "mm": 0.1
  },
  "adjustment_reasons": {
    "stm": "High context dependency (0.7) requires strong STM",
    "ltm": "Medium complexity (0.8) benefits from enhanced LTM",
    "kg": "Deep reasoning (0.85) benefits from knowledge graph",
    "mm": "Task profile suggests potential multimodal value"
  },
  "confidence_score": 0.88
}
```

### 7.3 Get Weight History

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/weights/history" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/weights/history",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "adjustment_history": [
    {
      "timestamp": "2026-03-29T10:30:00Z",
      "task_id": "task_7x9k2Lm4",
      "old_weights": {
        "stm": 1.0,
        "ltm": 0.6,
        "kg": 0.3,
        "mm": 0.0
      },
      "new_weights": {
        "stm": 1.0,
        "ltm": 0.7,
        "kg": 0.4,
        "mm": 0.1
      },
      "reason": "Complexity-driven adjustment for deep reasoning task",
      "performance_impact": 0.12
    },
    {
      "timestamp": "2026-03-28T14:20:00Z",
      "task_id": "task_abc123",
      "old_weights": {
        "stm": 1.0,
        "ltm": 0.5,
        "kg": 0.2,
        "mm": 0.0
      },
      "new_weights": {
        "stm": 1.0,
        "ltm": 0.6,
        "kg": 0.3,
        "mm": 0.0
      },
      "reason": "Resource optimization for memory efficiency",
      "performance_impact": 0.08
    }
  ],
  "summary": {
    "total_adjustments": 47,
    "average_performance_impact": 0.095,
    "most_common_adjustment": "ltm_increase"
  }
}
```

---

## 8. System and Health

### 8.1 Health Check

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/health"
```

```python
import requests

response = requests.get("http://127.0.0.1:8008/api/v1/memory/health")
print(response.json())
```

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2026-03-29T10:30:00Z",
  "components": {
    "scheduler": "healthy",
    "analyzer": "healthy",
    "predictor": "healthy",
    "monitor": "healthy",
    "weight_adjuster": "healthy",
    "database": "healthy",
    "database_backend": "postgres",
    "compute_backend": "apple-silicon"
  },
  "performance": {
    "avg_response_time_ms": 45,
    "success_rate": 0.998,
    "error_rate": 0.002
  }
}
```

### 8.2 Self-Healing Health Check

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/v1/health"
```

```python
import requests

response = requests.get("http://127.0.0.1:8008/api/v1/memory/v1/health")
print(response.json())
```

**Response:**
```json
{
  "status": "healthy",
  "checks": {
    "database": "passed",
    "qdrant": "passed",
    "ollama": "passed",
    "memory": "passed"
  },
  "uptime_seconds": 86400,
  "last_self_heal": "2026-03-29T08:00:00Z"
}
```

### 8.3 Get System Configuration

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/config" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/config",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "resource_limits": {
    "memory_usage": 0.8,
    "cpu_usage": 0.8,
    "response_time": 2.0,
    "storage_quota": 0.9
  },
  "performance_baselines": {
    "stm": {
      "efficiency_gain": 0.95,
      "coherence_gain": 0.7,
      "resource_cost": 0.2
    },
    "ltm": {
      "efficiency_gain": 0.75,
      "coherence_gain": 0.9,
      "resource_cost": 0.4
    },
    "kg": {
      "efficiency_gain": 0.6,
      "coherence_gain": 0.85,
      "resource_cost": 0.5
    },
    "mm": {
      "efficiency_gain": 0.5,
      "coherence_gain": 0.8,
      "resource_cost": 0.7
    }
  },
  "marginal_decay_factors": {
    "stm_to_ltm": 0.85,
    "ltm_to_kg": 0.75,
    "kg_to_mm": 0.65
  }
}
```

### 8.4 Get System Resources

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/monitor/resources" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/monitor/resources",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "current_status": {
    "memory_usage_mb": 384,
    "memory_usage_percent": 38,
    "cpu_usage_percent": 42,
    "response_time_ms": 720,
    "storage_usage_percent": 25
  },
  "resource_limits": {
    "memory_limit_mb": 1024,
    "cpu_limit_percent": 80,
    "response_time_limit_ms": 2000,
    "storage_limit_percent": 80
  },
  "status": "healthy",
  "alerts": []
}
```

### 8.5 Calculate Cost-Benefit

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/monitor/cost-benefit" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "performance_prediction": {
      "efficiency_gain": 0.82,
      "coherence_gain": 0.78,
      "resource_cost": 0.45,
      "cost_benefit_ratio": 1.82,
      "confidence_score": 0.89
    },
    "resource_status": {
      "memory_usage_mb": 384,
      "memory_usage_percent": 38,
      "cpu_usage_percent": 42,
      "response_time_ms": 720,
      "storage_usage_percent": 25
    }
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/monitor/cost-benefit",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "performance_prediction": {
            "efficiency_gain": 0.82,
            "coherence_gain": 0.78,
            "resource_cost": 0.45,
            "cost_benefit_ratio": 1.82,
            "confidence_score": 0.89
        },
        "resource_status": {
            "memory_usage_mb": 384,
            "memory_usage_percent": 38,
            "cpu_usage_percent": 42,
            "response_time_ms": 720,
            "storage_usage_percent": 25
        }
    }
)
print(response.json())
```

**Response:**
```json
{
  "cost_benefit_ratio": 1.82,
  "performance_score": 0.804,
  "resource_cost": 0.458,
  "recommendation": "optimal",
  "optimization_suggestions": [
    "Current configuration provides excellent cost-benefit ratio",
    "Consider monitoring LTM usage as complexity increases"
  ]
}
```

### 8.6 Optimize Configuration

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/monitor/optimize" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "current_config": {
      "primary_memory": "stm",
      "secondary_memory": ["ltm", "kg"],
      "memory_weights": {
        "stm": 1.0,
        "ltm": 0.6,
        "kg": 0.3,
        "mm": 0.0
      },
      "reasoning_depth": "medium",
      "enable_multimodal": false
    },
    "performance_goals": {
      "target_efficiency": 0.85,
      "target_coherence": 0.9,
      "max_resource_cost": 0.5
    }
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/monitor/optimize",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "current_config": {
            "primary_memory": "stm",
            "secondary_memory": ["ltm", "kg"],
            "memory_weights": {
                "stm": 1.0,
                "ltm": 0.6,
                "kg": 0.3,
                "mm": 0.0
            },
            "reasoning_depth": "medium",
            "enable_multimodal": False
        },
        "performance_goals": {
            "target_efficiency": 0.85,
            "target_coherence": 0.9,
            "max_resource_cost": 0.5
        }
    }
)
print(response.json())
```

### 8.7 Get Memory Configuration List

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/configs?user_id=user_001&agent_id=agent_001&page=1&pageSize=10" \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/configs",
    params={"user_id": "user_001", "agent_id": "agent_001", "page": 1, "pageSize": 10},
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "data": [
    {
      "config_id": "config_01HXYZ...",
      "user_id": "user_001",
      "agent_id": "agent_001",
      "config_name": "Default Configuration",
      "config_type": "standard",
      "stm_enabled": 1,
      "stm_max_length": 100,
      "stm_retention_hours": 24,
      "ltm_enabled": 1,
      "ltm_max_entries": 1000,
      "ltm_quality_threshold": 0.7,
      "kg_enabled": 1,
      "kg_max_entities": 500,
      "kg_confidence_threshold": 0.75,
      "mm_enabled": 0,
      "mm_max_entries": 100,
      "status": "active",
      "created_at": "2026-03-01T00:00:00Z",
      "updated_at": "2026-03-29T10:00:00Z"
    }
  ],
  "total": 1,
  "page": 1,
  "pageSize": 10
}
```

### 8.8 Create Memory Configuration

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/configs" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "userId": "user_001",
    "agentId": "agent_001",
    "configName": "High Performance Config",
    "configType": "high_performance",
    "stmEnabled": 1,
    "stmMaxLength": 200,
    "stmRetentionHours": 48,
    "ltmEnabled": 1,
    "ltmMaxEntries": 2000,
    "ltmQualityThreshold": 0.8,
    "kgEnabled": 1,
    "kgMaxEntities": 1000,
    "kgConfidenceThreshold": 0.8,
    "mmEnabled": 1,
    "mmMaxEntries": 200,
    "mmModalityTypes": "image,audio",
    "maxResponseTimeMs": 1000,
    "maxMemoryUsageMb": 512,
    "maxCpuUsagePercent": 70,
    "status": "active"
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/configs",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "userId": "user_001",
        "agentId": "agent_001",
        "configName": "High Performance Config",
        "configType": "high_performance",
        "stmEnabled": 1,
        "stmMaxLength": 200,
        "stmRetentionHours": 48,
        "ltmEnabled": 1,
        "ltmMaxEntries": 2000,
        "ltmQualityThreshold": 0.8,
        "kgEnabled": 1,
        "kgMaxEntities": 1000,
        "kgConfidenceThreshold": 0.8,
        "mmEnabled": 1,
        "mmMaxEntries": 200,
        "mmModalityTypes": "image,audio",
        "maxResponseTimeMs": 1000,
        "maxMemoryUsageMb": 512,
        "maxCpuUsagePercent": 70,
        "status": "active"
    }
)
print(response.json())
```

**Response:**
```json
{
  "config_id": "config_01HXYZnew001"
}
```

### 8.9 Update Memory Configuration

```bash
curl -X PUT "http://127.0.0.1:8008/api/v1/memory/configs/config_01HXYZ..." \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "configName": "Updated Configuration Name",
    "stmMaxLength": 150,
    "ltmQualityThreshold": 0.75
  }'
```

```python
import requests

config_id = "config_01HXYZ..."
response = requests.put(
    f"http://127.0.0.1:8008/api/v1/memory/configs/{config_id}",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "configName": "Updated Configuration Name",
        "stmMaxLength": 150,
        "ltmQualityThreshold": 0.75
    }
)
print(response.json())
```

**Response:**
```json
{
  "success": true
}
```

### 8.10 Delete Memory Configuration

```bash
curl -X DELETE "http://127.0.0.1:8008/api/v1/memory/configs/config_01HXYZ..." \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

config_id = "config_01HXYZ..."
response = requests.delete(
    f"http://127.0.0.1:8008/api/v1/memory/configs/{config_id}",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "success": true
}
```

### 8.11 Get Importance Score

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/importance/01HXYZ789LM..." \
  -H "Authorization: Bearer $TOKEN"
```

```python
import requests

entry_id = "01HXYZ789LM..."
response = requests.get(
    f"http://127.0.0.1:8008/api/v1/memory/importance/{entry_id}",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print(response.json())
```

**Response:**
```json
{
  "entry_id": "01HXYZ789LM...",
  "score": 0.78,
  "factors": {
    "uniqueness": 0.65,
    "emotional_intensity": 0.3,
    "goal_relevance": 0.85,
    "timeliness": 0.72
  }
}
```

### 8.12 Batch Importance Evaluation

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/importance/batch" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "entries": [
      {
        "id": "entry_001",
        "layer": "ltm",
        "content": "Python async programming tutorial content",
        "user_id": "user_001",
        "agent_id": "agent_001",
        "session_id": "session_001",
        "tags": ["python", "async", "tutorial"],
        "created_at": 1743264000,
        "access_count": 15,
        "last_accessed": 1743340000,
        "source": "documentation"
      },
      {
        "id": "entry_002",
        "layer": "stm",
        "content": "User asked about Rust ownership",
        "user_id": "user_001",
        "agent_id": "agent_001",
        "session_id": "session_002",
        "tags": ["rust", "programming"],
        "created_at": 1743350000,
        "access_count": 3,
        "last_accessed": 1743351000,
        "source": "conversation"
      }
    ]
  }'
```

```python
import requests

response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/importance/batch",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "entries": [
            {
                "id": "entry_001",
                "layer": "ltm",
                "content": "Python async programming tutorial content",
                "user_id": "user_001",
                "agent_id": "agent_001",
                "session_id": "session_001",
                "tags": ["python", "async", "tutorial"],
                "created_at": 1743264000,
                "access_count": 15,
                "last_accessed": 1743340000,
                "source": "documentation"
            },
            {
                "id": "entry_002",
                "layer": "stm",
                "content": "User asked about Rust ownership",
                "user_id": "user_001",
                "agent_id": "agent_001",
                "session_id": "session_002",
                "tags": ["rust", "programming"],
                "created_at": 1743350000,
                "access_count": 3,
                "last_accessed": 1743351000,
                "source": "conversation"
            }
        ]
    }
)
print(response.json())
```

**Response:**
```json
{
  "results": [
    {
      "entry_id": "entry_001",
      "score": 0.82,
      "factors": {
        "uniqueness": 0.7,
        "emotional_intensity": 0.2,
        "goal_relevance": 0.9,
        "timeliness": 0.65
      }
    },
    {
      "entry_id": "entry_002",
      "score": 0.55,
      "factors": {
        "uniqueness": 0.6,
        "emotional_intensity": 0.3,
        "goal_relevance": 0.7,
        "timeliness": 0.8
      }
    }
  ]
}
```

### Error Handling - System

```python
import requests

# Database unavailable
response = requests.get("http://127.0.0.1:8008/api/v1/memory/health")
if response.status_code == 200:
    data = response.json()
    if data["components"]["database"] != "healthy":
        print("Database degraded:", data["components"]["database"])

# Resource limits exceeded
response = requests.post(
    "http://127.0.0.1:8008/api/v1/memory/adaptive/select",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json={
        "task_context": {...},
        "resource_constraints": {
            "max_memory_usage_mb": 100000000,  # Unrealistic
            "max_cpu_usage_percent": 99,
            "max_response_time_ms": 1,
            "storage_quota_percent": 99
        },
        "preferences": {...}
    }
)
if response.status_code == 400:
    print("Invalid resource constraints:", response.json())

# Configuration not found
response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/configs/nonexistent_config",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
if response.status_code == 404:
    print("Config not found:", response.json())
```

---

## Common Error Codes

| Status Code | Meaning | Common Causes |
|-------------|---------|---------------|
| 400 | Bad Request | Missing required fields, invalid format, validation failure |
| 401 | Unauthorized | Missing or invalid JWT token, token expired |
| 403 | Forbidden | Insufficient permissions for the requested operation |
| 404 | Not Found | Resource (session, entry, config) does not exist |
| 409 | Conflict | Duplicate resource (e.g., username already exists) |
| 422 | Unprocessable Entity | Semantically invalid request (e.g., invalid relation) |
| 429 | Too Many Requests | Rate limit exceeded |
| 500 | Internal Server Error | Database error, service failure |
| 503 | Service Unavailable | Dependencies not available (Qdrant, Neo4j, Ollama) |

### Error Response Format

```json
{
  "error": "Error message description",
  "code": 400,
  "details": {
    "field": "specific field that caused the error"
  }
}
```

---

## Rate Limiting

The API implements rate limiting at 100 requests per 60 seconds per user. When rate limited, you will receive a `429 Too Many Requests` response.

```python
import time
import requests

# Check rate limit headers
response = requests.get(
    "http://127.0.0.1:8008/api/v1/memory/health",
    headers={"Authorization": f"Bearer {TOKEN}"}
)
print("Rate limit remaining:", response.headers.get("X-RateLimit-Remaining"))
print("Rate limit reset:", response.headers.get("X-RateLimit-Reset"))

# Handle rate limiting
if response.status_code == 429:
    reset_time = int(response.headers.get("X-RateLimit-Reset", time.time() + 60))
    wait_seconds = max(reset_time - time.time(), 0)
    print(f"Rate limited. Waiting {wait_seconds} seconds...")
    time.sleep(wait_seconds)
```

---

## Complete Workflow Example

```python
import requests

BASE_URL = "http://127.0.0.1:8008"

# Step 1: Register and get token
register_response = requests.post(
    f"{BASE_URL}/api/register",
    json={"username": "demo_user", "password": "demo_password"}
)
token = register_response.json()["token"]
headers = {"Authorization": f"Bearer {token}"}

# Step 2: Store conversation in STM
stm_response = requests.post(
    f"{BASE_URL}/api/v1/memory/storage/stm",
    headers=headers,
    json={
        "userId": "demo_user",
        "agentId": "demo_agent",
        "sessionType": "conversation",
        "role": "user",
        "content": "Explain how vector databases work for similarity search"
    }
)
session_id = stm_response.json()["sessionId"]

# Step 3: Store knowledge in LTM
requests.post(
    f"{BASE_URL}/api/v1/memory/storage/ltm",
    headers=headers,
    json={
        "sourceId": "doc_vector_db",
        "sourceType": "documentation",
        "title": "Vector Databases and Similarity Search",
        "content": "Vector databases store high-dimensional embeddings and enable efficient similarity search using metrics like cosine similarity or Euclidean distance..."
    }
)

# Step 4: Create knowledge graph entity
requests.post(
    f"{BASE_URL}/api/kg/entities",
    headers=headers,
    json={
        "entityName": "Vector Database",
        "entityType": "technology",
        "description": "Database optimized for similarity search using embeddings"
    }
)

# Step 5: Perform adaptive memory selection
selection_response = requests.post(
    f"{BASE_URL}/api/v1/memory/adaptive/select",
    headers=headers,
    json={
        "task_context": {
            "task_id": "task_demo_001",
            "task_type": "query",
            "complexity": 0.6,
            "modality_requirements": ["text"],
            "temporal_scope": "medium",
            "reasoning_depth": "medium",
            "context_dependency": 0.5,
            "user_id": "demo_user",
            "agent_id": "demo_agent"
        },
        "resource_constraints": {
            "max_memory_usage_mb": 512,
            "max_cpu_usage_percent": 50,
            "max_response_time_ms": 1000,
            "storage_quota_percent": 50
        },
        "preferences": {
            "prioritize_efficiency": True,
            "prioritize_coherence": True,
            "enable_multimodal": False,
            "enable_reasoning": True
        }
    }
)
selected_config = selection_response.json()["memory_config"]
print(f"Selected: {selected_config['primary_memory']} with weights {selected_config['memory_weights']}")

# Step 6: Search for relevant information
search_response = requests.post(
    f"{BASE_URL}/api/v1/memory/search/hybrid",
    headers=headers,
    json={
        "query": "vector database similarity search embeddings",
        "topK": 5,
        "keywordWeight": 0.3,
        "vectorWeight": 0.7
    }
)
for result in search_response.json()["results"]:
    print(f"- {result['title']} (score: {result['score']:.2f})")

# Step 7: Check system health
health = requests.get(f"{BASE_URL}/api/v1/memory/health").json()
print(f"System status: {health['status']}")
```

---

## Environment Variables

For production deployment, configure these environment variables:

```bash
# Database
DATABASE_URL=postgresql://user:password@localhost:5432/adaptive_memory
# or for SQLite
DATABASE_URL=sqlite:///./adaptive_memory.db

# JWT Secret (required for production)
JWT_SECRET=your-production-secret-key-min-32-chars

# Qdrant Vector Database (optional, for LTM semantic search)
QDRANT_URL=http://localhost:6334
QDRANT_API_KEY=your_qdrant_api_key

# Neo4j Graph Database (optional, for knowledge graph)
NEO4J_URI=bolt://localhost:7687
NEO4J_USER=neo4j
NEO4J_PASSWORD=your_neo4j_password

# Ollama LLM Service (optional, for embeddings and LLM)
OLLAMA_BASE_URL=http://localhost:11434
OLLAMA_MODEL=llama2

# Server
RUST_LOG=info
SERVER_PORT=8008
```
