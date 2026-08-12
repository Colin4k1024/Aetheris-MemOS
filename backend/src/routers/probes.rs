//! Kubernetes-style liveness and readiness probes.
//!
//! These sit at the **root** path (`/livez`, `/readyz`), unauthenticated, next
//! to `/metrics` — an orchestrator cannot present a JWT, and a probe behind
//! auth is a probe that always fails.
//!
//! # Why two endpoints and not one
//!
//! The split is the whole point, and getting it backwards is actively harmful:
//!
//! - **`/livez`** answers "is this process wedged?" and deliberately checks
//!   **nothing external**. A failed liveness probe gets the container
//!   **killed and restarted**. If it probed PostgreSQL, then a database blip
//!   would restart every replica at once — turning a recoverable dependency
//!   outage into a self-inflicted outage. Restarting the process cannot fix a
//!   down database, so liveness must not depend on one.
//!
//! - **`/readyz`** answers "can this instance serve traffic right now?" and
//!   **does** probe dependencies. A failed readiness probe only pulls the
//!   instance out of the load-balancer rotation; it recovers by itself when
//!   the dependency returns. That is the correct response to a dependency
//!   outage.
//!
//! # Relationship to the existing health endpoints
//!
//! - `/api/v1/memory/health` — a real `SELECT 1` DB probe, but it lives behind
//!   `/api` and returns a large payload. Kept for humans and dashboards.
//! - `/api/v1/memory/v1/health` — reports `self_healing` layer status that is
//!   **hardcoded** (`healthy: true` plus fabricated 1–4 ms latencies). It is
//!   not a probe and must not be used as one; see `services/self_healing.rs`.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::db::{DatabasePool, DATABASE_POOL};

/// Result of probing one dependency.
#[derive(Serialize)]
pub struct DependencyStatus {
    /// Dependency name (`database`, `qdrant`).
    pub name: String,
    /// Whether the probe succeeded.
    pub ready: bool,
    /// How long the probe took.
    pub latency_ms: u64,
    /// Failure reason, when `ready` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct ReadyResponse {
    /// `"ready"` when every required dependency answered, else `"not_ready"`.
    pub status: &'static str,
    pub dependencies: Vec<DependencyStatus>,
}

#[derive(Serialize)]
pub struct LiveResponse {
    pub status: &'static str,
}

/// `GET /livez` — process liveness. Always `200` while the process can serve.
///
/// Checks nothing external by design: see the module docs for why probing
/// dependencies here would amplify a dependency outage into a restart storm.
pub async fn livez() -> impl IntoResponse {
    (StatusCode::OK, Json(LiveResponse { status: "alive" }))
}

/// `GET /readyz` — dependency readiness.
///
/// `200` when every required dependency answers, `503` otherwise, so an
/// orchestrator can drain this instance without restarting it. The body always
/// lists per-dependency detail, including on failure — a bare 503 gives the
/// operator nothing to act on.
pub async fn readyz() -> impl IntoResponse {
    let mut dependencies = vec![probe_database().await];

    // Qdrant is required for vector search and for outbox delivery, so an
    // unreachable Qdrant means this instance cannot serve its core workload.
    dependencies.push(probe_qdrant().await);

    // Embedding is a hard dependency (decided 2026-08-11): vectors are generated
    // on the hot path of both LTM write and search, so an unreachable embedding
    // backend makes both unusable. Contrast Neo4j, which is genuinely optional
    // and is deliberately NOT probed here — failing readiness on an optional
    // dependency would drain healthy instances from the load balancer.
    dependencies.push(probe_embedding().await);

    let all_ready = dependencies.iter().all(|d| d.ready);
    let status_code = if all_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(ReadyResponse {
            status: if all_ready { "ready" } else { "not_ready" },
            dependencies,
        }),
    )
}

async fn probe_database() -> DependencyStatus {
    let started = std::time::Instant::now();

    // Mirrors `routers::memory::check_database_health` — `SELECT 1` on the
    // live pool, which exercises checkout + round-trip rather than merely
    // asserting the pool struct exists.
    let result: Result<(), String> = match DATABASE_POOL.get() {
        Some(DatabasePool::Postgres(pool)) => sqlx::query("SELECT 1")
            .fetch_optional(pool)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string()),
        Some(DatabasePool::Sqlite(pool)) => sqlx::query("SELECT 1")
            .fetch_optional(pool)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string()),
        None => Err("database pool not initialised".to_string()),
    };

    let latency_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(()) => DependencyStatus {
            name: "database".to_string(),
            ready: true,
            latency_ms,
            error: None,
        },
        Err(e) => DependencyStatus {
            name: "database".to_string(),
            ready: false,
            latency_ms,
            error: Some(e),
        },
    }
}

async fn probe_qdrant() -> DependencyStatus {
    let started = std::time::Instant::now();

    // `get_qdrant_client()` lazily builds the client, and `QdrantClient::new()`
    // reads `config::get()`, which **panics** via `expect` when the config
    // OnceLock is unset. A probe that panics is worse than one that reports a
    // failure: the caller gets a dropped connection instead of an actionable
    // 503. Treat "config not initialised" as not-ready and return early.
    let result = if crate::config::CONFIG.get().is_none() {
        Err("config not initialised".to_string())
    } else {
        match crate::services::qdrant::get_qdrant_client() {
            Ok(client) => client.probe().await.map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    };

    let latency_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(()) => DependencyStatus {
            name: "qdrant".to_string(),
            ready: true,
            latency_ms,
            error: None,
        },
        Err(e) => DependencyStatus {
            name: "qdrant".to_string(),
            ready: false,
            latency_ms,
            error: Some(e),
        },
    }
}

/// Probe embedding backend reachability.
///
/// Uses [`EmbeddingService::probe`], which hits the backend's model-list
/// endpoint rather than generating a real embedding: readiness is polled on the
/// order of seconds, and running inference per poll would both load the backend
/// continuously and make the probe's own latency (100 ms–1 s for inference) a
/// source of spurious failures.
async fn probe_embedding() -> DependencyStatus {
    let started = std::time::Instant::now();

    // Same guard as `probe_qdrant`: `get_embedding_service()` builds the service
    // lazily and reads `config::get()`, which panics via `expect` when the
    // config OnceLock is unset. A probe must return an actionable 503, never
    // panic and drop the connection.
    let result = if crate::config::CONFIG.get().is_none() {
        Err("config not initialised".to_string())
    } else {
        match crate::services::embedding::get_embedding_service() {
            Ok(service) => service.probe().await.map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    };

    let latency_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(()) => DependencyStatus {
            name: "embedding".to_string(),
            ready: true,
            latency_ms,
            error: None,
        },
        Err(e) => DependencyStatus {
            name: "embedding".to_string(),
            ready: false,
            latency_ms,
            error: Some(e),
        },
    }
}

/// Probe routes, mounted at the root (unauthenticated).
pub fn router() -> Router {
    Router::new()
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Liveness must not depend on any external system — that is the entire
    /// reason it is separate from readiness. This test pins the property: with
    /// no DB pool and no Qdrant initialised, `/livez` still reports alive.
    #[tokio::test]
    async fn livez_is_independent_of_dependencies() {
        let response = livez().await.into_response();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "liveness must not fail when dependencies are absent — a failing \
             liveness probe restarts the container, which cannot fix a \
             dependency outage"
        );
    }

    /// A readiness probe that returns 200 while a dependency is down is worse
    /// than no probe: the load balancer keeps sending traffic to an instance
    /// that cannot serve it.
    ///
    /// This test also pins that `readyz` **returns** rather than panics. It
    /// originally failed with a panic from `config::get()`'s `expect` deep
    /// inside `QdrantClient::new()` — a panicking probe hands the orchestrator
    /// a dropped connection instead of an actionable 503.
    #[tokio::test]
    async fn readyz_reports_503_when_dependencies_unavailable() {
        // In unit-test context neither DATABASE_POOL nor the config OnceLock is
        // initialised, so this exercises the real failure path.
        let response = readyz().await.into_response();
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "readiness must fail closed when dependencies are unreachable"
        );
    }

    #[tokio::test]
    async fn readyz_body_names_the_failing_dependency() {
        let response = readyz().await.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let parsed: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

        assert_eq!(parsed["status"], "not_ready");

        let deps = parsed["dependencies"]
            .as_array()
            .expect("dependencies array");
        let names: Vec<&str> = deps.iter().filter_map(|d| d["name"].as_str()).collect();
        assert!(
            names.contains(&"database"),
            "readiness payload must name each dependency so an operator can \
             act on a 503; got {names:?}"
        );

        // Every failing dependency must carry a reason — a bare `ready: false`
        // tells the operator nothing.
        for dep in deps {
            if dep["ready"] == false {
                assert!(
                    dep["error"].is_string(),
                    "failing dependency {} must include an error string",
                    dep["name"]
                );
            }
        }
    }

    /// Pins which dependencies are treated as *required*.
    ///
    /// `embedding` must be present: it is a hard dependency (vectors are
    /// generated on the hot path of both LTM write and search), so an instance
    /// without it cannot serve its core workload and should be drained.
    ///
    /// `neo4j` must be **absent**: it is genuinely optional, and failing
    /// readiness on an optional dependency would pull healthy instances out of
    /// the load balancer when only the knowledge-graph extra is down. See
    /// `db::neo4j::Neo4jStatus`, which carries the same note.
    #[tokio::test]
    async fn readyz_probes_required_dependencies_only() {
        let response = readyz().await.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let parsed: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        let names: Vec<&str> = parsed["dependencies"]
            .as_array()
            .expect("dependencies array")
            .iter()
            .filter_map(|d| d["name"].as_str())
            .collect();

        for required in ["database", "qdrant", "embedding"] {
            assert!(
                names.contains(&required),
                "{required} is a hard dependency and must be probed; got {names:?}"
            );
        }
        assert!(
            !names.contains(&"neo4j"),
            "neo4j is optional and must NOT gate readiness — failing on it would \
             drain healthy instances; got {names:?}"
        );
    }
}
