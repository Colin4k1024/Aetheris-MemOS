//! Database adapters: namespace for future multi-backend support.
//!
//! - **SQLite**: Current implementation lives in parent `db` modules (memory, performance, weights, etc.).
//!   This is the default adapter for local and demo use.
//! - **PostgreSQL / MySQL**: Planned; see docs/ROADMAP.md.

// Placeholder for future sqlite adapter module (current code uses crate::db::pool() directly).
// pub mod sqlite;

// Placeholder for future PostgreSQL adapter.
// pub mod postgres;
