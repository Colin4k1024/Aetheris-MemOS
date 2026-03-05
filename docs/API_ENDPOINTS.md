# API Endpoints Documentation

## Base URL
```
http://127.0.0.1:8008
```

## Memory API

### Adaptive Memory Selection
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/memory/adaptive` | Select optimal memory configuration |
| GET | `/api/v1/memory/adaptive` | Get memory status |
| POST | `/api/v1/memory/adaptive/trace` | Get decision trace |

### Task Analysis
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/memory/analyzer/task-characteristics` | Analyze task characteristics |
| POST | `/api/v1/memory/analyzer/batch-characteristics` | Batch analyze tasks |

### Performance Prediction
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/memory/predictor/performance` | Predict performance |
| GET | `/api/v1/memory/predictor/baselines` | Get performance baselines |

### Resource Monitoring
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/memory/monitor/resources` | Get resource status |
| POST | `/api/v1/memory/monitor/cost-benefit` | Calculate cost-benefit |
| POST | `/api/v1/memory/monitor/optimize` | Optimize resources |

### Weight Management
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/memory/weights/adjust` | Adjust memory weights |
| GET | `/api/v1/memory/weights/history` | Get weight history |

### Decision Traces
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/memory/traces` | Get decision traces |

### Health & Config
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/memory/health` | Health check |
| GET | `/api/v1/memory/config` | Get configuration |

### Memory Configuration (Requires Auth)
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/memory/configs` | List configurations |
| POST | `/api/v1/memory/configs` | Create configuration |
| GET | `/api/v1/memory/configs/{id}` | Get configuration |
| PUT | `/api/v1/memory/configs/{id}` | Update configuration |
| DELETE | `/api/v1/memory/configs/{id}` | Delete configuration |

### Short-Term Memory (STM)
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/memory/storage/stm` | Store STM |
| GET | `/api/v1/memory/storage/stm/{session_id}` | Get session messages |

### Long-Term Memory (LTM)
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/memory/storage/ltm` | Store LTM |
| POST | `/api/v1/memory/storage/transfer` | Transfer STM to LTM |
| POST | `/api/v1/memory/storage/batch-ltm` | Batch store LTM |

### Search
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/memory/search/stm` | Search STM |
| POST | `/api/v1/memory/search/ltm` | Search LTM |
| GET | `/api/v1/memory/search/ltm/{id}` | Get LTM entry |
| POST | `/api/v1/memory/search/hybrid` | Hybrid search |
| POST | `/api/v1/memory/search/entity` | Entity-based search |

### Knowledge Graph (KG)
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/memory/kg/entities` | Create entity |
| GET | `/api/v1/memory/kg/entities/by-name/{name}` | Get entity by name |
| GET | `/api/v1/memory/kg/entities/{id}/related` | Get related entities |
| POST | `/api/v1/memory/kg/relations` | Create relation |
| POST | `/api/v1/memory/kg/search` | Search by entity |

### Multimodal Memory (MM)
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/memory/mm/store` | Store multimodal |
| GET | `/api/v1/memory/mm/entry/{id}` | Get entry |
| GET | `/api/v1/memory/mm/session/{id}` | Get session multimodal |
| GET | `/api/v1/memory/mm/modality/{type}` | Get by modality |

## Authentication

Some endpoints require JWT authentication:
- `Authorization: Bearer <token>`

Login to get token:
```bash
POST /api/login
{
  "username": "admin",
  "password": "admin"
}
```

## Rate Limiting

API endpoints are rate-limited:
- Default: 100 requests per minute
- Headers returned: `X-RateLimit-Limit`, `X-RateLimit-Remaining`

## Response Format

All responses follow this format:
```json
{
  // Data payload
}
```

Error responses:
```json
{
  "code": 500,
  "message": "Error description"
}
```

## OpenAPI Documentation

Access OpenAPI docs at:
- Scalar UI: `http://127.0.0.1:8008/scalar`
- OpenAPI JSON: `http://127.0.0.1:8008/api-doc/openapi.json`
