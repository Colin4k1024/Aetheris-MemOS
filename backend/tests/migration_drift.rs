//! Migration drift tests (#125): prove the migration baseline holds for BOTH
//! database backends.
//!
//! What is guarded and why:
//!
//! 1. **Empty-database full runs** - PostgreSQL AND SQLite must each be able to
//!    execute their *entire* migration set from an empty database. This is not
//!    theoretical: commit 952a899 dropped a SQLite-dialect file
//!    (`20260813_distillation_tables.sql`, `datetime('now')` defaults, 8-digit
//!    version prefix) into `migrations/`, which broke `sqlx migrate run` on any
//!    fresh PostgreSQL. Nothing caught it because CI's migration step had never
//!    run since (a separate actions-permission failure).
//! 2. **Dialect purity** - `migrations/` is PostgreSQL-only, `migrations_sqlite/`
//!    is SQLite-only. Cross-dialect files are how the 952a899 breakage happened,
//!    so the markers that bit us are now asserted absent.
//! 3. **No duplicate table definitions** - `skills` and `agent_equipment` were
//!    each defined twice with *different* schemas; the second `CREATE TABLE IF
//!    NOT EXISTS` silently masked the first. A table must be created by exactly
//!    one migration per directory.
//! 4. **Version prefix format** - every filename must carry a 14-digit timestamp
//!    version. The 8-digit `20260813` prefix sorted numerically *before* all
//!    14-digit versions (20 million < 20 trillion), silently reordering the
//!    migration chain. Enforcing 14 digits keeps lexicographic, numeric, and
//!    chronological order identical.
//!
//! Skip semantics: the PostgreSQL half is `#[ignore]`d and guarded on
//! `DATABASE_URL` exactly like `rls_*_pg.rs` - no false-green pass without a
//! live database, and CI opts back in with `--include-ignored`. The SQLite half
//! and all static checks run everywhere with no external dependencies.
//!
//!   DATABASE_URL=postgres://memory:memory@localhost:5433/memory \
//!     cargo test --test migration_drift -- --include-ignored --nocapture

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::migrate::Migrator;

/// Backend's `migrations/` (PostgreSQL dialect, single source of schema truth).
fn pg_migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

/// Backend's `migrations_sqlite/` (SQLite dialect, dev/test backend).
fn sqlite_migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations_sqlite")
}

/// All `.sql` files in a directory, sorted by filename.
fn sql_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read migration dir {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.expect("read dir entry").path();
            (path.extension().is_some_and(|ext| ext == "sql")).then_some(path)
        })
        .collect();
    files.sort();
    files
}

// ============================================================================
// 1. Empty-database full runs (both backends)
// ============================================================================

/// PostgreSQL: from a *freshly created* empty database, the whole migration set
/// must apply cleanly and record every migration as applied.
///
/// Creates (and always drops) a scratch database so the shared dev/CI `memory`
/// database is never touched by this test.
#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn postgres_runs_every_migration_from_empty_database() {
    let Ok(admin_url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP migration_drift (pg): DATABASE_URL not set");
        return;
    };

    let migrator = Migrator::new(pg_migrations_dir())
        .await
        .expect("build PostgreSQL migrator");
    let expected = migrator.migrations.len();

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let db_name = format!("aetheris_mig_drift_{suffix}");

    let admin = sqlx::PgPool::connect(&admin_url)
        .await
        .expect("connect to admin/owner database");

    // Idempotent scratch database (a leftover from a crashed run is safe to
    // drop: this test is its only possible owner).
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {db_name} WITH (FORCE)"))
        .execute(&admin)
        .await;
    sqlx::query(&format!("CREATE DATABASE {db_name}"))
        .execute(&admin)
        .await
        .expect("create scratch database (needs CREATEDB privilege)");

    // Run the full migration set against the EMPTY database. Any failure here
    // is the exact "cannot bootstrap from empty" class (#125).
    let target_opts: sqlx::postgres::PgConnectOptions =
        std::str::FromStr::from_str(&admin_url).expect("parse DATABASE_URL");
    let target = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(target_opts.database(&db_name))
        .await
        .expect("connect to scratch database");

    let run_result = migrator.run(&target).await;
    if let Err(e) = run_result {
        // Drop the scratch DB even on failure so reruns start clean.
        target.close().await;
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {db_name} WITH (FORCE)"))
            .execute(&admin)
            .await;
        panic!("PostgreSQL migrations must apply from an empty database: {e}");
    }

    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success")
        .fetch_one(&target)
        .await
        .expect("count applied migrations");
    target.close().await;

    sqlx::query(&format!("DROP DATABASE IF EXISTS {db_name} WITH (FORCE)"))
        .execute(&admin)
        .await
        .expect("drop scratch database");
    admin.close().await;

    assert_eq!(
        applied as usize, expected,
        "every migration file must be recorded as applied in the empty database \
         (expected {expected}, found {applied})"
    );
}

/// SQLite: from an empty database file, the whole `migrations_sqlite/` set must
/// apply cleanly. Runs everywhere - no external service needed.
#[tokio::test]
async fn sqlite_runs_every_migration_from_empty_database() {
    let migrator = Migrator::new(sqlite_migrations_dir())
        .await
        .expect("build SQLite migrator (dir must exist; see db::init_sqlite fail-fast)");
    let expected = migrator.migrations.len();
    assert!(expected > 0, "SQLite migration set must not be empty");

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let db_path = std::env::temp_dir().join(format!("aetheris_mig_drift_{suffix}.db"));

    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("open empty SQLite database");

    migrator
        .run(&pool)
        .await
        .expect("SQLite migrations must apply from an empty database");

    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success")
        .fetch_one(&pool)
        .await
        .expect("count applied SQLite migrations");
    pool.close().await;
    let _ = std::fs::remove_file(&db_path);

    assert_eq!(
        applied as usize, expected,
        "every SQLite migration file must be recorded as applied"
    );
}

// ============================================================================
// 2. Dialect purity
// ============================================================================

/// Tokens that only exist in (or only work in) SQLite DDL. Each one appeared in
/// the 952a899 breakage or is the same failure class.
const SQLITE_ONLY_MARKERS: &[&str] = &[
    "datetime('now')",
    "datetime(\"now\")",
    "AUTOINCREMENT",
    "PRAGMA ",
];

/// Tokens that only exist in PostgreSQL DDL and would fail on SQLite.
const POSTGRES_ONLY_MARKERS: &[&str] = &[
    "JSONB",
    "TIMESTAMPTZ",
    "CREATE POLICY",
    "CURRENT_SETTING",
    "GEN_RANDOM_UUID",
    "NOW()",
];

#[test]
fn postgres_migration_dir_contains_no_sqlite_dialect() {
    for file in sql_files(&pg_migrations_dir()) {
        let sql = std::fs::read_to_string(&file).expect("read migration file");
        for marker in SQLITE_ONLY_MARKERS {
            assert!(
                !sql.to_uppercase().contains(marker),
                "{}: contains SQLite-only token '{marker}' - the PostgreSQL \
                 migrations directory must stay pure PostgreSQL dialect (#125)",
                file.display()
            );
        }
    }
}

#[test]
fn sqlite_migration_dir_contains_no_postgres_dialect() {
    for file in sql_files(&sqlite_migrations_dir()) {
        let sql = std::fs::read_to_string(&file).expect("read migration file");
        for marker in POSTGRES_ONLY_MARKERS {
            assert!(
                !sql.to_uppercase().contains(marker),
                "{}: contains PostgreSQL-only token '{marker}' - the SQLite \
                 migrations directory must stay pure SQLite dialect (#125)",
                file.display()
            );
        }
    }
}

// ============================================================================
// 3. No duplicate table definitions
// ============================================================================

/// Extract `CREATE TABLE [IF NOT EXISTS] <name>` targets from SQL text.
/// Line-based on purpose: every migration in this repo writes the target on
/// the same line as `CREATE TABLE`, and if that shape ever changes this test
/// failing is exactly the signal we want.
fn created_tables(sql: &str) -> Vec<String> {
    sql.lines()
        .filter_map(|line| {
            let upper = line.trim().to_uppercase();
            let rest = upper.strip_prefix("CREATE TABLE ")?;
            let rest = rest.strip_prefix("IF NOT EXISTS ").unwrap_or(rest);
            let name = rest
                .split(|c: char| c.is_whitespace() || c == '(')
                .next()?
                .trim_matches('"');
            (!name.is_empty()).then(|| name.to_lowercase())
        })
        .collect()
}

fn assert_no_duplicate_table_definitions(dir: &Path, backend: &str) {
    let mut owner: HashMap<String, std::path::PathBuf> = HashMap::new();
    for file in sql_files(dir) {
        let sql = std::fs::read_to_string(&file).expect("read migration file");
        for table in created_tables(&sql) {
            if let Some(first) = owner.get(&table) {
                panic!(
                    "{backend} migration {} redefines table '{table}' already \
                     created by {}. The second CREATE TABLE IF NOT EXISTS would \
                     silently mask the first schema (the exact skills/\
                     agent_equipment duplication #125 removed); fold schema \
                     changes into a single file or an ALTER migration",
                    file.display(),
                    first.display()
                );
            }
            owner.insert(table, file.clone());
        }
    }
}

#[test]
fn postgres_migrations_define_each_table_exactly_once() {
    assert_no_duplicate_table_definitions(&pg_migrations_dir(), "PostgreSQL");
}

#[test]
fn sqlite_migrations_define_each_table_exactly_once() {
    assert_no_duplicate_table_definitions(&sqlite_migrations_dir(), "SQLite");
}

// ============================================================================
// 4. Version prefix format
// ============================================================================

/// Every migration filename must be `<14-digit timestamp>_<name>.sql`, versions
/// unique. A shorter prefix (the deleted `20260813_...`) sorts numerically
/// BEFORE all 14-digit versions and silently reorders the chain; 14 digits
/// keep lexicographic == numeric == chronological order.
#[test]
fn migration_filenames_use_14_digit_timestamp_versions() {
    for dir in [pg_migrations_dir(), sqlite_migrations_dir()] {
        let mut versions: Vec<String> = Vec::new();
        for file in sql_files(&dir) {
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .expect("utf-8 filename");
            let version = name
                .split_once('_')
                .expect("filename must be <version>_<description>.sql")
                .0;
            assert!(
                version.len() == 14 && version.chars().all(|c| c.is_ascii_digit()),
                "{}: version prefix must be a 14-digit timestamp (got '{version}')",
                file.display()
            );
            versions.push(version.to_string());
        }
        let mut sorted = versions.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            versions.len(),
            "duplicate migration versions in {}",
            dir.display()
        );
    }
}
