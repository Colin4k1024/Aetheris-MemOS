//! Axum router assembly.
//!
//! This module is deliberately thin. It owns exactly two things — the outermost
//! observability/CORS layers and the `distributed` sub-agent-pool routes — and
//! delegates the entire API surface to [`crate::routers::root`].
//!
//! ## Why it is thin (backlog C-3 / PR-1)
//!
//! It used to also carry `auth`, `agent` and `demo` sub-modules that duplicated
//! handlers already served by `crate::routers`. None of them were reachable:
//! [`create_router`] only calls `crate::routers::root()`, and nothing else
//! referenced them, so they compiled into the binary and were never routed.
//!
//! The `auth` copy was the dangerous one — it carried its own `jwt::get_token`
//! call sites. A parallel, unreachable login path is not merely dead weight: the
//! next person to "fix" it reintroduces token minting that skips whatever the
//! live path has since learned (the `org` claim, in this case). Same failure mode
//! as the duplicate `web/jwt.rs` removed in backlog E-2.
//!
//! `submodule_set_is_pinned` below keeps this module from growing a third
//! parallel router by accident.

pub mod distributed;

use axum::Router;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::web::cors_layer;

/// Build the application router: [`crate::routers::root`] wrapped in the
/// outermost observability and CORS layers.
///
/// Route documentation lives with the routes themselves in `crate::routers`;
/// duplicating it here is how the previous version of this comment came to
/// describe a `protected::protected_router()` that does not exist in this module.
///
/// ## Layers (outermost → innermost)
/// 1. `TraceLayer` — per-request span with method/path/status/latency
/// 2. CORS
pub fn create_router() -> Router {
    let cors = cors_layer();

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(
            DefaultMakeSpan::new()
                .level(Level::INFO)
                .include_headers(false),
        )
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    crate::routers::root().layer(trace_layer).layer(cors)
}

#[cfg(test)]
mod tests {
    /// Pin the sub-module set so a parallel router cannot reappear unnoticed.
    ///
    /// The compiler will not object to a new `pub mod` here that nothing routes —
    /// that is exactly how `auth`, `agent` and `demo` survived as unreachable
    /// duplicates of live handlers. Adding a module is not automatically wrong,
    /// but it has to be a decision: either it gets mounted in [`super::create_router`]
    /// (or in `crate::routers`), or it should not exist.
    #[test]
    fn submodule_set_is_pinned() {
        let src = include_str!("mod.rs");
        let declared: Vec<&str> = src
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub mod "))
            .filter_map(|rest| rest.strip_suffix(';'))
            .collect();

        assert_eq!(
            declared,
            vec!["distributed"],
            "axum_routers must declare only `distributed` (mounted from main.rs). \
             Anything else is either unrouted dead code or belongs in crate::routers."
        );
    }
}
