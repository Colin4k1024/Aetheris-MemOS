# Observability Design: Distributed Tracing & Service Monitoring

**Date:** 2026-07-03  
**Status:** Approved  
**Scope:** Backend (Rust/Axum) + Docker Compose monitoring stack

---

## 1. Architecture

```
Backend (Rust/Axum)
  ├─ tracing-opentelemetry subscriber layer
  │    └─ OTLP gRPC → otel-collector :4317
  └─ /metrics (Prometheus text format)
       └─ Prometheus scrape every 15s

OpenTelemetry Collector
  ├─ receiver: otlp (gRPC :4317)
  └─ exporter: jaeger (otlp :4318 / :14250)

Jaeger all-in-one   :16686  (trace UI)
Prometheus          :9090   (metrics store)
Grafana             :3000   (dashboard)
```

## 2. Backend Changes

### 2.1 `src/otel/mod.rs` — replace placeholder
- Implement `init_telemetry(endpoint: &str, service_name: &str)` using:
  - `opentelemetry-otlp` gRPC exporter targeting `otel-collector:4317`
  - `opentelemetry_sdk::trace::BatchSpanProcessor`
  - `tracing_opentelemetry::OpenTelemetryLayer` registered into `tracing_subscriber`
- `shutdown_telemetry()` calls `opentelemetry::global::shutdown_tracer_provider()`
- Feature-gated behind `[otel]` config `enabled` flag (graceful no-op when disabled)

### 2.2 `src/axum_routers/mod.rs` — add TraceLayer
- Add `tower_http::trace::TraceLayer` to the router
- Records span per request: method, path, status, latency
- Connects to the global tracer so spans propagate to Jaeger

### 2.3 `src/main.rs`
- Call `otel::init_telemetry(...)` early in `main()` using config values
- Call `otel::shutdown_telemetry()` in graceful shutdown

### 2.4 `backend/config.toml` + `docker.toml`
New `[otel]` section:
```toml
[otel]
enabled = true
endpoint = "http://otel-collector:4317"
service_name = "aetheris-memos-backend"
```

## 3. Docker Compose Additions

| Service | Image | Ports | Purpose |
|---------|-------|-------|---------|
| otel-collector | otel/opentelemetry-collector-contrib | 4317 (gRPC) | OTLP receiver → Jaeger |
| jaeger | jaegertracing/all-in-one | 16686 (UI), 4318 (OTLP HTTP) | Trace storage + UI |
| prometheus | prom/prometheus | 9090 | Metrics scrape + store |
| grafana | grafana/grafana | 3000 | Dashboards (Prometheus ds) |

Config files added under `monitoring/`:
- `otel-collector-config.yaml`
- `prometheus.yml`
- `grafana/provisioning/datasources/prometheus.yaml`

## 4. Success Criteria

- `docker compose up` brings up full stack
- Backend spans visible in Jaeger UI at http://localhost:16686
- `/metrics` endpoint scraped by Prometheus at http://localhost:9090
- Grafana at http://localhost:3000 shows pre-provisioned Prometheus datasource
- No compilation errors; feature degrades gracefully when `[otel] enabled = false`
