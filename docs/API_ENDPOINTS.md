# API Endpoints Documentation

## Base URL

```
http://127.0.0.1:8008
```

## Memory API

### Adaptive Memory Selection

| Method | Endpoint                         | Description                             |
| ------ | -------------------------------- | --------------------------------------- |
| POST   | `/api/v1/memory/adaptive/select` | Select optimal memory configuration     |
| GET    | `/api/v1/memory/adaptive/status` | Get memory status                       |
| POST   | `/api/v1/memory/adaptive/trace`  | Get decision trace                      |
| POST   | `/api/v1/memory/adaptive`        | Legacy alias for select (compatibility) |
| GET    | `/api/v1/memory/adaptive`        | Legacy alias for status (compatibility) |

### Task Analysis

| Method | Endpoint                                        | Description                  |
| ------ | ----------------------------------------------- | ---------------------------- |
| POST   | `/api/v1/memory/analyzer/task-characteristics`  | Analyze task characteristics |
| POST   | `/api/v1/memory/analyzer/batch-characteristics` | Batch analyze tasks          |

### Performance Prediction

| Method | Endpoint                               | Description               |
| ------ | -------------------------------------- | ------------------------- |
| POST   | `/api/v1/memory/predictor/performance` | Predict performance       |
| GET    | `/api/v1/memory/predictor/baselines`   | Get performance baselines |

### Resource Monitoring

| Method | Endpoint                              | Description            |
| ------ | ------------------------------------- | ---------------------- |
| GET    | `/api/v1/memory/monitor/resources`    | Get resource status    |
| POST   | `/api/v1/memory/monitor/cost-benefit` | Calculate cost-benefit |
| POST   | `/api/v1/memory/monitor/optimize`     | Optimize resources     |

### Weight Management

| Method | Endpoint                         | Description           |
| ------ | -------------------------------- | --------------------- |
| POST   | `/api/v1/memory/weights/adjust`  | Adjust memory weights |
| GET    | `/api/v1/memory/weights/history` | Get weight history    |

### Decision Traces

| Method | Endpoint                | Description         |
| ------ | ----------------------- | ------------------- |
| GET    | `/api/v1/memory/traces` | Get decision traces |

### Health & Config

| Method | Endpoint                | Description       |
| ------ | ----------------------- | ----------------- |
| GET    | `/api/v1/memory/health` | Health check      |
| GET    | `/api/v1/memory/config` | Get configuration |

### Memory Configuration (Requires Auth)

| Method | Endpoint                      | Description          |
| ------ | ----------------------------- | -------------------- |
| GET    | `/api/v1/memory/configs`      | List configurations  |
| POST   | `/api/v1/memory/configs`      | Create configuration |
| GET    | `/api/v1/memory/configs/{id}` | Get configuration    |
| PUT    | `/api/v1/memory/configs/{id}` | Update configuration |
| DELETE | `/api/v1/memory/configs/{id}` | Delete configuration |

### Short-Term Memory (STM)

| Method | Endpoint                                  | Description          |
| ------ | ----------------------------------------- | -------------------- |
| POST   | `/api/v1/memory/storage/stm`              | Store STM            |
| GET    | `/api/v1/memory/storage/stm/{session_id}` | Get session messages |

### Long-Term Memory (LTM)

| Method | Endpoint                           | Description         |
| ------ | ---------------------------------- | ------------------- |
| POST   | `/api/v1/memory/storage/ltm`       | Store LTM           |
| POST   | `/api/v1/memory/storage/transfer`  | Transfer STM to LTM |
| POST   | `/api/v1/memory/storage/batch-ltm` | Batch store LTM     |

### Search

| Method | Endpoint                         | Description         |
| ------ | -------------------------------- | ------------------- |
| POST   | `/api/v1/memory/search/stm`      | Search STM          |
| POST   | `/api/v1/memory/search/ltm`      | Search LTM          |
| GET    | `/api/v1/memory/search/ltm/{id}` | Get LTM entry       |
| POST   | `/api/v1/memory/search/hybrid`   | Hybrid search       |
| POST   | `/api/v1/memory/search/entity`   | Entity-based search |

### Knowledge Graph (KG)

| Method | Endpoint                               | Description          |
| ------ | -------------------------------------- | -------------------- |
| GET    | `/api/kg/entities`                     | List entities        |
| POST   | `/api/kg/entities`                     | Create entity        |
| GET    | `/api/kg/entities/by-name/{name}`      | Get entity by name   |
| GET    | `/api/kg/entities/{entity_id}/related` | Get related entities |
| POST   | `/api/kg/relations`                    | Create relation      |
| POST   | `/api/kg/search`                       | Search by entity     |

### Multimodal Memory (MM)

| Method | Endpoint                  | Description            |
| ------ | ------------------------- | ---------------------- |
| POST   | `/api/mm/store`           | Store multimodal       |
| GET    | `/api/mm/entry/{id}`      | Get entry              |
| GET    | `/api/mm/session/{id}`    | Get session multimodal |
| GET    | `/api/mm/modality/{type}` | Get by modality        |
| GET    | `/api/mm/list`            | List multimodal        |

### KG/MM Compatibility Note

- Canonical routes are `/api/kg/*` and `/api/mm/*`.
- Legacy `/api/v1/memory/kg/*` and `/api/v1/memory/mm/*` paths are deprecated and should not be used by new clients.

### Session Management

| Method | Endpoint                          | Description         |
| ------ | --------------------------------- | ------------------- |
| GET    | `/api/v1/memory/storage/sessions` | List STM sessions  |

### Importance Scoring

| Method | Endpoint                               | Description             |
| ------ | -------------------------------------- | ----------------------- |
| GET    | `/api/v1/memory/importance/{entry_id}` | Get entry importance    |
| POST   | `/api/v1/memory/importance/batch`     | Batch importance query  |

---

## Bi-Temporal Tracking (Phase 2.4)

Query historical memory states and track entity evolution over time.

| Method | Endpoint                                         | Description                    |
| ------ | ------------------------------------------------ | ------------------------------ |
| GET    | `/api/v1/memory/search/ltm/{id}/at`             | Get LTM entry at specific time |
| GET    | `/api/v1/memory/search/ltm/{id}/history`        | Get LTM entry version history  |
| POST   | `/api/v1/memory/search/ltm/time-travel`         | Search LTM at specific time    |
| GET    | `/api/v1/memory/search/kg/{entity_id}/at`       | Get KG entity at specific time |
| GET    | `/api/v1/memory/search/kg/{entity_id}/history` | Get KG entity version history  |

---

## Context Snapshots (Phase 3.1 - Oris Integration)

Persist and restore task execution contexts with checkpoint support.

| Method | Endpoint                              | Description                     |
| ------ | ------------------------------------- | ------------------------------- |
| POST   | `/api/v1/memory/snapshot/task`        | Create task snapshot            |
| GET    | `/api/v1/memory/snapshot/task/{id}`   | Get task snapshot               |
| POST   | `/api/v1/memory/snapshot/create`      | Create context snapshot         |
| POST   | `/api/v1/memory/snapshot/restore`     | Restore from snapshot           |
| POST   | `/api/v1/memory/snapshot/checkpoint`  | Create checkpoint               |
| POST   | `/api/v1/memory/snapshot/rollback`    | Rollback to checkpoint          |
| GET    | `/api/v1/memory/snapshot/checkpoints/{task_id}` | List checkpoints    |

---

## Tenant Management (Phase 3.2 - Aetheris)

Multi-tenant management and quota control.

| Method | Endpoint                         | Description              |
| ------ | -------------------------------- | ------------------------ |
| POST   | `/api/v1/tenants`                | Create tenant            |
| GET    | `/api/v1/tenants`                | List tenants             |
| GET    | `/api/v1/tenants/{id}`           | Get tenant details       |
| PUT    | `/api/v1/tenants/{id}`           | Update tenant            |
| DELETE | `/api/v1/tenants/{id}`           | Delete tenant            |
| GET    | `/api/v1/tenants/{id}/quota`    | Get tenant quota         |

---

## Memory Pool (Phase 3.3 - Multi-Agent)

Collaborative memory sharing across multiple agents.

| Method | Endpoint                                       | Description                 |
| ------ | ---------------------------------------------- | --------------------------- |
| POST   | `/api/v1/memory/memory-pool/register`          | Register agent              |
| POST   | `/api/v1/memory/memory-pool/unregister/{id}`  | Unregister agent             |
| POST   | `/api/v1/memory/memory-pool/share/{owner_id}` | Share memory                |
| POST   | `/api/v1/memory/memory-pool/revoke/{owner_id}/{memory_id}` | Revoke memory      |
| GET    | `/api/v1/memory/memory-pool/visible/{agent_id}` | Get visible memories     |
| POST   | `/api/v1/memory/memory-pool/correlations`     | Add correlation             |
| GET    | `/api/v1/memory/memory-pool/correlations/{memory_id}` | Get correlations |
| GET    | `/api/v1/memory/memory-pool/network`          | Get network status          |
| GET    | `/api/v1/memory/memory-pool/agents`           | List registered agents      |

---

## Billing (Phase 4.3)

Usage tracking and quota management.

| Method | Endpoint                         | Description              |
| ------ | -------------------------------- | ------------------------ |
| POST   | `/api/v1/memory/billing/init`    | Initialize tenant billing |
| POST   | `/api/v1/memory/billing/usage`   | Get usage stats          |
| GET    | `/api/v1/memory/billing/usage/{tenant_id}` | Get current usage |
| GET    | `/api/v1/memory/billing/quota/{tenant_id}` | Get quota status  |
| POST   | `/api/v1/memory/billing/record`  | Record usage             |

---

## Enterprise (Phase 4.1)

Cluster management and data sharding for enterprise deployments.

### Cluster Management

| Method | Endpoint                              | Description            |
| ------ | ------------------------------------- | ---------------------- |
| POST   | `/api/v1/memory/enterprise/cluster/node`   | Register node         |
| GET    | `/api/v1/memory/enterprise/cluster/nodes` | Get all nodes         |
| GET    | `/api/v1/memory/enterprise/cluster/active`| Get active nodes      |
| GET    | `/api/v1/memory/enterprise/cluster/leader`| Get leader node       |
| POST   | `/api/v1/memory/enterprise/cluster/become-leader` | Become leader    |
| GET    | `/api/v1/memory/enterprise/cluster/is-leader` | Check if leader      |

### Data Sharding

| Method | Endpoint                               | Description          |
| ------ | -------------------------------------- | -------------------- |
| POST   | `/api/v1/memory/enterprise/shards`     | Create shard         |
| GET    | `/api/v1/memory/enterprise/shards`     | List shards          |
| GET    | `/api/v1/memory/enterprise/shards/{key}` | Get shard for key  |

---

## Visualization (Phase 4.2)

Data endpoints for frontend visualization widgets.

| Method | Endpoint                          | Description              |
| ------ | --------------------------------- | ------------------------ |
| GET    | `/api/v1/memory/visualization/timeline`    | Get timeline data        |
| GET    | `/api/v1/memory/visualization/graph`       | Get KG visualization    |
| GET    | `/api/v1/memory/visualization/heatmap`     | Get importance heatmap   |
| GET    | `/api/v1/memory/visualization/dashboard`   | Get dashboard stats      |

---

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
  "code": 1001,
  "message": "Invalid request parameters",
  "error": "Invalid request parameters"
}
```

```json
{
  "code": 1005,
  "message": "Rate limit exceeded. Please try again later.",
  "error": "Rate limit exceeded. Please try again later."
}
```

## OpenAPI Documentation

Access OpenAPI docs at:

- Scalar UI: `http://127.0.0.1:8008/scalar`
- OpenAPI JSON: `http://127.0.0.1:8008/api-doc/openapi.json`

---

**Version**: 2.0.0
**Last Updated**: 2026-03-17

**What's New**:
- Bi-temporal tracking endpoints (Phase 2.4)
- Context snapshot (Oris) endpoints (Phase 3.1)
- Tenant management endpoints (Phase 3.2)
- Memory pool endpoints (Phase 3.3)
- Billing endpoints (Phase 4.3)
- Enterprise cluster endpoints (Phase 4.1)
- Visualization endpoints (Phase 4.2)
