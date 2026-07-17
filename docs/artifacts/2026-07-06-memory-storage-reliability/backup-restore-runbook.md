# Backup & Restore Runbook: Memory Storage

| Field | Value |
|---|---|
| Artifact | `backup-restore-runbook.md` |
| Source requirement | ADR-0003 §1 (备份恢复), `deployment-context.md` §恢复能力 |
| Work item | W1.7 |
| Audience | on-call operators, DevOps engineers, QA engineers running drills |
| Status | operational draft, awaiting first drill evidence |
| Last reviewed | 2026-07-17 |

This runbook covers backup, restore, and rebuild procedures for the three durable storage backends in the MemOS stack: PostgreSQL, Qdrant, and Neo4j. It is the working operator reference for restore drills required by ADR-0003 before the system can be declared enterprise-release-ready.

## Prerequisites

| Requirement | Detail |
|---|---|
| Network access | Reachable hosts for postgres (5432), qdrant (6333, 6334), neo4j (7474, 7687). For Docker Compose dev, all are on `localhost`. |
| Client tools | `psql` (16+), `pg_dump`, `pg_basebackup`, `pg_waldump`, `curl`, `jq`, `docker`, `docker compose`, `neo4j-admin`. |
| Backup storage | A writable path mounted into each container (`/backups/postgres`, `/backups/qdrant`, `/backups/neo4j`) and an off-host target (S3, GCS, NFS, or a managed archive). The path must be outside the live data volume so a corrupt volume can be restored without losing the backup. |
| Credentials | Source from the secret manager in any non-dev environment. The dev defaults in `docker-compose.yml` (`memory/memory` for PostgreSQL, `neo4j/password` for Neo4j) are acceptable for local/dev drills only. Production must not use default credentials (see `deployment-context.md` §配置与密钥). |
| RPO / RTO targets | Pending confirmation by tech-lead per ADR-0003 §后续动作. Record the confirmed targets in the Appendix at the end of this runbook before the first production drill. |

All commands assume the operator is in the repository root unless stated otherwise. Replace `{collection}` with the configured Qdrant collection name (see `backend/config.toml` `[qdrant] collection_name`, typically `memory_ltm`).

## Section 1: PostgreSQL Backup & Restore

PostgreSQL is the system of record. LTM content (`knowledge_entries`, `knowledge_relations`), STM sessions, KG entities, MM entries, the vector outbox, reconciliation runs, and audit events all live here. Treat every other backend as rebuildable from PostgreSQL.

### 1.1 Full logical backup (daily baseline)

Use a custom-format `pg_dump` for the daily baseline. It is parallel-restore capable and supports selective table restore.

```bash
# From the host with network access to PostgreSQL (5432)
export PGUSER=memory
export PGPASSWORD="$(secret-manager get pg/password)"   # dev: memory
export PGHOST=localhost
export PGPORT=5432
export PGDATABASE=memory

BACKUP_DIR=/backups/postgres
mkdir -p "$BACKUP_DIR"
BACKUP_FILE="$BACKUP_DIR/memory_$(date -u +%Y%m%dT%H%M%SZ).dump"

pg_dump -U "$PGUSER" -d "$PGDATABASE" -F c -Z 6 -f "$BACKUP_FILE"

# Record size and checksum for integrity verification on restore
sha256sum "$BACKUP_FILE" > "$BACKUP_FILE.sha256"
ls -lh "$BACKUP_FILE"
```

Schedule this once per 24 hours via cron or the platform scheduler. The dump captures a consistent snapshot of all tables, RLS policies, indexes, and migration history. Keep at least 7 daily, 4 weekly, and 12 monthly generations off-host.

For Docker Compose dev:

```bash
docker compose exec -T postgres pg_dump -U memory -d memory -F c -f /backups/postgres/memory_$(date -u +%Y%m%dT%H%M%SZ).dump
docker compose cp postgres:/backups/postgres/. ./backups/postgres/
```

### 1.2 PITR via WAL archiving (point-in-time recovery)

Logical backups cap RPO at 24 hours. To meet a tighter RPO, run continuous WAL archiving with a periodic base backup. This is the configuration ADR-0003 requires for production.

Enable WAL archiving in `postgresql.conf` (or via `ALTER SYSTEM`):

```sql
ALTER SYSTEM SET wal_level = 'replica';
ALTER SYSTEM SET archive_mode = 'on';
ALTER SYSTEM SET archive_timeout = '60s';   -- forces a segment switch every minute when idle
ALTER SYSTEM SET max_wal_senders = 5;
-- archive_command writes completed WAL segments to the archive. The test prevents
-- clobbering and returns non-zero on failure so PostgreSQL retries.
ALTER SYSTEM SET archive_command = 'test ! -f /backups/postgres/wal/%f && cp %p /backups/postgres/wal/%f';
SELECT pg_reload_conf();
```

Take a base backup. The directory naming keeps one base per day so the restore target is unambiguous:

```bash
BASE_DIR=/backups/postgres/base/$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p "$BASE_DIR"
pg_basebackup -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -D "$BASE_DIR" -F plain -X stream -S pitr_backup --progress
echo "$(date -u +%Y%m%dT%H%M%SZ) $BASE_DIR" >> /backups/postgres/base/index.txt
```

Verify archiving is live. The WAL directory should grow and `pg_stat_archiver` should show zero or low `failed_count`:

```sql
SELECT last_archived_wal, last_archived_time, failed_count, last_failed_wal
FROM pg_stat_archiver;
```

### 1.3 Restore procedure (PITR)

Use this when a production cluster must be rolled back to a point in time. For pure logical restores (replace all data from a dump), skip to 1.3.4.

**1.3.1 Quiesce the application.** Stop the backend so no new writes hit PostgreSQL. Leave the database running long enough for a final WAL flush, then stop it.

```bash
# Stop application writers
docker compose stop backend

# Let PostgreSQL flush WAL, then stop the DB
docker compose stop postgres
```

**1.3.2 Preserve the existing data directory.** Never overwrite the live data directory in place. Move it aside so the restore can be rolled back if it fails.

```bash
docker compose exec postgres bash -c '
  mv /var/lib/postgresql/data /var/lib/postgresql/data.precrash.$(date -u +%Y%m%dT%H%M%SZ)
  mkdir -p /var/lib/postgresql/data
  chown -R postgres:postgres /var/lib/postgresql/data
  chmod 700 /var/lib/postgresql/data
'
```

**1.3.3 Lay down the base backup and configure recovery.** Copy the chosen base backup into the empty data directory, then write `postgresql.auto.conf` and `recovery.signal`.

```bash
BASE=/backups/postgres/base/<chosen-timestamp>     # pick the base from index.txt
TARGET_T=$(date -u -d '2026-07-17 14:30:00' +%Y-%m-%dT%H:%M:%SZ)   # PITR target, UTC

docker compose cp "$BASE/." postgres:/var/lib/postgresql/data/

docker compose exec postgres bash -c "
  cat >> /var/lib/postgresql/data/postgresql.auto.conf <<'CONF'
restore_command = 'cp /backups/postgres/wal/%f %p'
recovery_target_time = '${TARGET_T}'
recovery_target_action = 'promote'
recovery_target_inclusive = true
CONF
  touch /var/lib/postgresql/data/recovery.signal
  chown -R postgres:postgres /var/lib/postgresql/data
"
```

**1.3.4 Start PostgreSQL and watch recovery.**

```bash
docker compose up -d postgres
docker compose logs -f postgres | grep -E 'restore|recovery|consistent|archive'
```

Watch for `recovery stopping before committed transaction`, `consistent recovery state reached at`, and `archive recovery complete` (or `database system is ready to accept read write connections` after promotion).

**1.3.5 Logical restore fallback (if PITR is unavailable).** Used when WAL archive is missing or the base backup is corrupt. This reverts the cluster to the last good dump, losing every write after the dump.

```bash
# Drop and recreate the database so the restore starts from a clean slate.
docker compose exec -T postgres psql -U memory -d postgres -c "ALTER DATABASE memory CONNECTION LIMIT 0;"
docker compose exec -T postgres psql -U memory -d postgres -c "
  SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='memory' AND pid <> pg_backend_pid();
"
docker compose exec -T postgres psql -U memory -d postgres -c "DROP DATABASE IF EXISTS memory;"
docker compose exec -T postgres psql -U memory -d postgres -c "CREATE DATABASE memory;"

# Restore from the dump, then re-apply migrations in lock-step with the code version.
docker compose cp ./backups/postgres/memory_<timestamp>.dump postgres:/tmp/restore.dump
docker compose exec -T postgres pg_restore -U memory -d memory -v -j 4 /tmp/restore.dump
```

### 1.4 Verification

Run these against the restored cluster before opening traffic. Row counts are the primary check. Save the output alongside the drill record. The pre-restore baseline should be captured from the running system before the drill starts.

```sql
-- Capture this snapshot before the drill (baseline) and after the restore (verification).
\echo === ROW COUNTS ===
SELECT 'knowledge_entries'           AS table, count(*) FROM knowledge_entries
UNION ALL SELECT 'knowledge_relations', count(*) FROM knowledge_relations
UNION ALL SELECT 'context_sessions',     count(*) FROM context_sessions
UNION ALL SELECT 'context_messages',     count(*) FROM context_messages
UNION ALL SELECT 'session_messages',     count(*) FROM session_messages
UNION ALL SELECT 'entities',              count(*) FROM entities
UNION ALL SELECT 'entity_versions',       count(*) FROM entity_versions
UNION ALL SELECT 'relations',             count(*) FROM relations
UNION ALL SELECT 'multimodal_entries',    count(*) FROM multimodal_entries
UNION ALL SELECT 'modality_relations',    count(*) FROM modality_relations
UNION ALL SELECT 'memory_configurations', count(*) FROM memory_configurations
UNION ALL SELECT 'memory_audit_events',   count(*) FROM memory_audit_events
UNION ALL SELECT 'users',                 count(*) FROM users
ORDER BY 1;

\echo === OUTBOX STATE ===
SELECT status, count(*) AS count, min(created_at) AS oldest, max(created_at) AS newest
FROM memory_vector_outbox
GROUP BY status
ORDER BY 1;

\echo === TENANT DISTRIBUTION ===
SELECT tenant_id, count(*) AS entry_count
FROM knowledge_entries
GROUP BY tenant_id
ORDER BY entry_count DESC
LIMIT 20;

\echo === LATEST AUDIT EVENTS ===
SELECT event_type, count(*), max(created_at) AS last_seen
FROM memory_audit_events
GROUP BY event_type
ORDER BY last_seen DESC
LIMIT 10;
```

Drill pass criteria:
- The post-restore counts match the pre-restore baseline for every table. Any divergence must be explained by WAL replay to `recovery_target_time` (acceptable if the target time intentionally excluded the missing rows).
- The outbox has no `dead_letter` rows that were not present in the baseline. An uptick here signals writes that hit PostgreSQL but never reached Qdrant.
- Tenant distribution matches baseline. Drift here signals a tenant isolation regression and blocks drill pass.

## Section 2: Qdrant Backup & Restore

Qdrant holds the vector index. PostgreSQL is the source of truth, so Qdrant can always be rebuilt from the outbox. The snapshot path is faster for full-cluster restores; the outbox path is the canonical recovery when only a subset of vectors is missing or the index format changed.

### 2.1 Snapshot creation

Qdrant v1.9.4 supports per-collection snapshots via the REST API on port 6333.

```bash
QDRANT=http://localhost:6333
COLLECTION={collection}     # e.g. memory_ltm

# Create a snapshot
curl -sS -X POST "$QDRANT/collections/$COLLECTION/snapshots" | jq

# List snapshots to confirm creation and capture the snapshot name
curl -sS "$QDRANT/collections/$COLLECTION/snapshots" | jq '.result.snapshots[] | {name, size, creation_time_seconds}'

# Download the chosen snapshot off the Qdrant container
SNAPSHOT=<snapshot-name>
mkdir -p /backups/qdrant
curl -sS -o "/backups/qdrant/${COLLECTION}_${SNAPSHOT}" "$QDRANT/collections/$COLLECTION/snapshots/$SNAPSHOT"
sha256sum "/backups/qdrant/${COLLECTION}_${SNAPSHOT}" > "/backups/qdrant/${COLLECTION}_${SNAPSHOT}.sha256"
```

For Docker Compose dev, snapshots land under `/qdrant/snapshots` inside the container. Copy them out:

```bash
docker compose exec qdrant ls -lh /qdrant/snapshots
docker compose cp qdrant:/qdrant/snapshots/. ./backups/qdrant/
```

Run the snapshot after the PostgreSQL base backup, so the Qdrant snapshot corresponds to a known PostgreSQL state.

### 2.2 Snapshot restore

Qdrant does not overwrite a live collection via the API. Stop the container, drop the storage directory, place the snapshot, and restart.

```bash
COLLECTION={collection}
SNAPSHOT=<snapshot-name>

# Stop the writer (backend) so no upserts race the restore
docker compose stop backend

# Stop Qdrant
docker compose stop qdrant

# Wipe the live storage and replace with the snapshot contents.
# qdrant expects the snapshot unpacked under /qdrant/storage/collections/<collection>/.
docker compose run --rm --no-deps qdrant bash -c "
  rm -rf /qdrant/storage/collections/${COLLECTION}
  mkdir -p /qdrant/storage/collections/${COLLECTION}
  tar -xOf /backups/qdrant/${COLLECTION}_${SNAPSHOT} -C /qdrant/storage/collections/${COLLECTION}
  chown -R qdrant:qdrant /qdrant/storage
"

docker compose up -d qdrant
sleep 5
curl -sS "$QDRANT/collections/$COLLECTION" | jq '.result | {vectors_count, status}'
```

Verify the post-restore `vectors_count` matches the `points_count` from `memory_vector_outbox` where `status='applied'` for the matching tenants (see the verification query in 2.3).

### 2.3 Rebuild from PostgreSQL outbox (canonical recovery)

This is the procedure referenced by ADR-0003 and `deployment-context.md` §Qdrant 回退. Use it when no snapshot exists, when a tenant's vectors are corrupt, or when the vector dimension or model signature changed and old snapshots are no longer loadable.

The rebuild re-queues outbox rows that the worker has already applied. PostgreSQL is the source of truth; the outbox replays the canonical writes.

**2.3.1 Confirm the reconciliation baseline is dry.** Reconciliation drift must be measured before the rebuild so the drill can prove drift returns to zero after replay.

```sql
-- Capture pre-rebuild drift baseline. Reconciliation scanner is expected to land
-- later per deployment-context.md; until then, use this manual snapshot.
SELECT
  count(*) FILTER (WHERE status = 'applied')            AS applied,
  count(*) FILTER (WHERE status = 'pending')           AS pending,
  count(*) FILTER (WHERE status = 'failed')            AS failed,
  count(*) FILTER (WHERE status = 'dead_letter')       AS dead_letter,
  count(*) FILTER (WHERE status = 'processing')       AS processing
FROM memory_vector_outbox
WHERE tenant_id = '<tenant>';
```

**2.3.2 Pause the outbox worker.** Confirm the worker is idle before mutating outbox state.

```bash
# Toggle provided by the outbox worker admin API or by setting the runtime flag.
# Until the admin API lands, pause via feature flag:
# vector_outbox_worker_enabled = false
curl -sS -X POST "$ADMIN_API/admin/outbox/pause" | jq    # when available

# Verify no row is in 'processing' state (no worker holding a lock)
docker compose exec -T postgres psql -U memory -d memory -c "
  SELECT count(*) AS in_flight FROM memory_vector_outbox WHERE status = 'processing';
"
```

**2.3.3 Reset applied rows back to pending for the affected scope.** Scope to a single tenant when possible; use the full-table variant only for whole-cluster rebuilds.

```sql
-- Tenant-scoped rebuild
UPDATE memory_vector_outbox
SET status = 'pending',
    attempt_count = 0,
    next_retry_at = now(),
    locked_at = NULL,
    locked_by = NULL,
    applied_at = NULL
WHERE tenant_id = '<tenant>'
  AND status IN ('applied', 'failed', 'dead_letter');

-- Full rebuild (admin confirmed only)
UPDATE memory_vector_outbox
SET status = 'pending',
    attempt_count = 0,
    next_retry_at = now(),
    locked_at = NULL,
    locked_by = NULL,
    applied_at = NULL
WHERE status IN ('applied', 'failed', 'dead_letter');
```

**2.3.4 Optionally wipe the Qdrant collection.** Do this only for a full rebuild; skip for tenant-scoped repair so other tenants keep serving traffic.

```bash
curl -sS -X DELETE "$QDRANT/collections/$COLLECTION?timeout=60" | jq
# The backend will recreate the collection on next write; or trigger creation manually:
curl -sS -X PUT "$QDRANT/collections/$COLLECTION" \
  -H 'Content-Type: application/json' \
  -d @ /path/to/collection-config.json | jq
```

**2.3.5 Resume the worker and monitor lag.**

```bash
# Set vector_outbox_worker_enabled = true (or call the admin resume endpoint when available)
curl -sS -X POST "$ADMIN_API/admin/outbox/resume" | jq

# Watch lag drain
watch -n 5 "docker compose exec -T postgres psql -U memory -d memory -c \"
  SELECT status, count(*), min(created_at) AS oldest
  FROM memory_vector_outbox
  GROUP BY status ORDER BY 1;\""
```

Drain is complete when `pending` reaches zero, `processing` reaches zero, and `dead_letter` count matches the pre-rebuild baseline (or has been triaged and re-queued with a documented root cause).

**2.3.6 Post-rebuild verification.**

```sql
-- Applied count must match the count of LTM entries for the tenant in PostgreSQL.
SELECT
  (SELECT count(*) FROM knowledge_entries WHERE tenant_id = '<tenant>') AS ltm_rows,
  (SELECT count(*) FROM memory_vector_outbox WHERE tenant_id = '<tenant>' AND status = 'applied') AS applied,
  (SELECT count(*) FROM memory_vector_outbox WHERE tenant_id = '<tenant>' AND status = 'dead_letter') AS dead_letter;
```

```bash
# Qdrant side: count points in the collection, scoped to the tenant payload filter.
curl -sS -X POST "$QDRANT/collections/$COLLECTION/points/count" \
  -H 'Content-Type: application/json' \
  -d '{"filter":{"must":[{"key":"tenantId","match":{"value":"<tenant>"}}]},"exact":true}' | jq
```

The Qdrant point count must equal `applied` for the tenant. A mismatch is drift; re-run reconciliation in dry-run mode to identify missing or orphaned points.

## Section 3: Neo4j Backup & Restore

Neo4j is production-mandatory per `deployment-context.md` §Neo4j. The KG backend is the only place certain entity and relation traversals live. Back it up with `neo4j-admin` dump.

### 3.1 Dump

`neo4j-admin database dump` produces an offline-consistent artifact. Stop the database (or run against a stopped instance) to avoid a partial dump.

```bash
# Pick a timestamp
TS=$(date -u +%Y%m%dT%H%M%SZ)
BACKUP_DIR=/backups/neo4j
mkdir -p "$BACKUP_DIR"

# For Docker Compose dev, the neo4j container holds the dump inside /backups
docker compose exec neo4j neo4j-admin database dump memory --to-path=/backups/ --overwrite-destination=true

# Pull the dump off the container
docker compose cp "neo4j:/backups/memory.dump" "$BACKUP_DIR/memory_${TS}.dump"
sha256sum "$BACKUP_DIR/memory_${TS}.dump" > "$BACKUP_DIR/memory_${TS}.dump.sha256"
ls -lh "$BACKUP_DIR/memory_${TS}.dump"
```

For online backups in production, run the dump against a read replica or use `neo4j-admin backup --type=full` followed by incremental backups. The dump file is the canonical restore artifact for the drill.

### 3.2 Load

Stop Neo4j, load the dump, restart, and confirm.

```bash
TS=<chosen-timestamp>
docker compose stop neo4j

docker compose run --rm --no-deps neo4j bash -c "
  rm -rf /data/databases/memory /data/transactions/memory
  neo4j-admin database load memory --from-path=/backups/ --overwrite-destination=true
"

docker compose up -d neo4j

# Wait for Neo4j to accept Bolt connections
for i in $(seq 1 30); do
  if docker compose exec -T neo4j cypher-shell -u neo4j -p "$(secret-manager get neo4j/password)" \
      "RETURN 1 AS ok;" 2>/dev/null | grep -q '1'; then
    echo "Neo4j ready"
    break
  fi
  sleep 2
done
```

For Docker Compose dev, the dev password is `password` (set via `NEO4J_AUTH=neo4j/password` in `docker-compose.yml`). Production must source the password from the secret manager.

### 3.3 Verification

```cypher
// Node count, grouped by label
MATCH (n)
RETURN labels(n)[0] AS label, count(n) AS nodes
ORDER BY nodes DESC;

// Total node count (must match baseline)
MATCH (n) RETURN count(n) AS total_nodes;

// Total relationship count (must match baseline)
MATCH ()-[r]->() RETURN count(r) AS total_relationships;

// Relationship count by type
MATCH ()-[r]->()
RETURN type(r) AS type, count(r) AS relationships
ORDER BY relationships DESC;

// Tenant isolation sanity: any node missing a tenantId property is suspect.
MATCH (n)
WHERE n.tenantId IS NULL
RETURN labels(n)[0] AS label, count(n) AS orphan_count;
```

Cross-check the KG totals against PostgreSQL `entities` and `relations` tables:

```sql
SELECT count(*) AS pg_entities FROM entities;
SELECT count(*) AS pg_relations FROM relations;
```

The Neo4j node count and PostgreSQL `entities` count should match (or the diff must be explained by a known KG exclusion such as a soft-deleted entity). The same applies to relationships versus `relations`.

## Section 4: Recovery Drill Checklist

ADR-0003 §测试和证据 requires operational drills with recorded date, environment, commands, results, failures, residual risk, and owner. This is the template for that evidence.

| Item | Required |
|---|---|
| Frequency | Quarterly at minimum. Additional drills before any major release that touches storage, RLS, outbox, or reconciliation. |
| Environment | `production-like` per `deployment-context.md`. Dev drills are allowed for procedure validation but do not satisfy the readiness gate. |
| Owner | DevOps engineer executes, QA engineer witnesses and records, tech-lead signs off. |
| Success criteria | All checks below pass; failures are root-caused and either fixed or accepted with a documented residual risk. |

### Drill sequence

Run all three backends in one drill session so cross-backend consistency is verifiable end to end.

1. **Capture baseline.** Run the verification queries in §1.4, §2.3, and §3.3 against the running system. Save outputs as `drill-<date>-baseline.txt`.
2. **Inject a small write batch.** Write a known number of LTM entries with a tagged tenant ID across two tenants. Record the inserted row counts and entry IDs.
3. **Trigger PostgreSQL restore.** Either PITR to a target time after the injected writes (full recovery) or to a target time before the writes (rollback). Capture the restore command output and duration.
4. **Trigger Qdrant rebuild.** Re-queue the outbox for the tagged tenant and run §2.3.5. Capture lag drain duration.
5. **Trigger Neo4j restore.** Run §3.2 against a known-good dump. Capture the load duration.
6. **Run post-restore verification.** Save outputs as `drill-<date>-postrestore.txt`.
7. **Diff baseline against post-restore.** Every table count and Neo4j label count must match unless the diff is explained by the PITR target time.

### Success criteria

| Criterion | How measured | Pass condition |
|---|---|---|
| PostgreSQL data consistency | Pre- and post-restore row counts from §1.4 | All counts match baseline within PITR window |
| Application functionality | Smoke test: write LTM entry, hybrid search returns it, KG entity reachable, MM entry retrievable | All four operations succeed against restored cluster |
| Tenant isolation intact | Two-tenant isolation test: tenant A cannot read tenant B's LTM entries through the API or via direct SQL under the application role | Zero cross-tenant rows visible to either tenant |
| Outbox lag drained | `memory_vector_outbox.status='pending'` reaches zero after rebuild | Pending count is zero, dead_letter count unchanged from baseline |
| Qdrant point count matches PostgreSQL | Applied outbox count equals Qdrant point count per tenant | Per-tenant diff is zero |
| Neo4j counts match PostgreSQL KG tables | §3.3 outputs match §1.4 `entities` and `relations` counts | Diff is zero or explained |

### Evidence to collect

File every artifact under `docs/artifacts/2026-07-06-memory-storage-reliability/drills/<YYYY-MM-DD>/`:

| Artifact | Source |
|---|---|
| `drill-plan.md` | The chosen scenarios, target times, tenant IDs, owner, witness |
| `drill-<date>-baseline.txt` | Verification query outputs before the drill |
| `drill-<date>-postrestore.txt` | Verification query outputs after the restore |
| `restore-commands.sh` | The exact commands run, captured from the shell |
| `pg-dump.log`, `pg-restore.log`, `pg-recovery.log` | PostgreSQL restore logs |
| `qdrant-rebuild.log` | Outbox lag drain output |
| `neo4j-load.log` | `neo4j-admin database load` output |
| `tenant-isolation-test.txt` | Two-tenant cross-read attempt results |
| `drill-summary.md` | Date, environment, durations, pass/fail per criterion, residual risks, signoff |

Required summary fields: backup file sizes, restore durations (PostgreSQL PITR, Qdrant rebuild, Neo4j load), row count diffs (zero or explained), tenant isolation test result (pass/fail per tenant pair).

### Failure handling

A drill is a pass only if every success criterion is met. If a criterion fails:
1. Stop the drill. Do not chain a second failure on top.
2. Capture the failure output into the drill directory.
3. File a tracking issue referencing the failed criterion and the captured evidence.
4. The release is blocked until the criterion passes in a re-run, or tech-lead explicitly accepts the residual risk in `launch-acceptance.md`.

## Section 5: Alerting Recommendations

ADR-0003 §告警闭环 and `deployment-context.md` §监控 require these alerts before a production release. The current deployment-context records that alerting is not in the handoff-ready gate; these recommendations are the production-readiness entry, not a current obligation.

Each alert has an owner, a severity, and an escalation path. Replace the owner placeholder with the confirmed on-call rotation when the readiness gate is entered.

### 5.1 Backup age alert

Fires when no successful backup has landed in the last 24 hours.

| Field | Value |
|---|---|
| Metric | `last_backup_age_seconds` per backend, computed from the newest file under each backup path or from a `backup_completed` audit event |
| Threshold | Postgres > 25h (1h grace over the daily schedule), Qdrant > 25h, Neo4j > 25h |
| Severity | High |
| Owner | on-call DevOps (PagerDuty rotation, pending confirmation) |
| Escalation | Page on-call immediately. If unacknowledged for 15 minutes, escalate to tech-lead. |
| Runbook link | §1.1, §2.1, §3.1 of this file |

Prometheus rule (sketch):

```yaml
- alert: PostgresBackupAgeTooHigh
  expr: time() - last_postgres_backup_timestamp_seconds > 90000   # 25h
  for: 10m
  labels: { severity: high, service: postgres, owner: devops-oncall }
  annotations:
    summary: "PostgreSQL backup is older than 25 hours"
    description: "No new backup file under /backups/postgres for {{ $value | humanizeDuration }}. See backup-restore-runbook §1.1."
```

### 5.2 Restore failure alert

Fires when a restore operation exits non-zero or does not produce a verifiable post-restore state.

| Field | Value |
|---|---|
| Metric | `restore_failed_total` per backend, incremented by a restore wrapper script |
| Threshold | Any non-zero value within the last drill window |
| Severity | Critical |
| Owner | on-call DevOps |
| Escalation | Page on-call. If the failure is during a production recovery, also page tech-lead. |
| Runbook link | The failing section of this runbook (§1.3, §2.2, §2.3, or §3.2) |

The restore wrapper script must emit a `restore_completed` or `restore_failed` audit event with backend name, target, duration, and exit code. Alert on the failed variant.

### 5.3 Disk space alert

Backups fail silently when the archive volume fills. Catch it before the backup job does.

| Field | Value |
|---|---|
| Metric | `node_filesystem_avail_bytes` on the backup volume, divided by `node_filesystem_size_bytes` |
| Threshold | Warning at < 20% free, Critical at < 10% free |
| Severity | Warning / Critical |
| Owner | on-call DevOps |
| Escalation | Warning: notify Slack #ops-alerts. Critical: page on-call. |
| Runbook link | This runbook §5.3 mitigation |

Mitigation when alert fires:
1. Identify the oldest backup generation eligible for pruning (per the retention policy in §1.1).
2. Verify the off-host copy exists and checksum matches.
3. Prune the local generation. Do not prune off-host copies.
4. If pruning does not free enough space, page tech-lead; do not disable the backup job.

Prometheus rule (sketch):

```yaml
- alert: BackupVolumeLowSpace
  expr: |
    (node_filesystem_avail_bytes{mountpoint="/backups"}
    / node_filesystem_size_bytes{mountpoint="/backups"}) * 100 < 20
  for: 5m
  labels: { severity: warning, owner: devops-oncall }
  annotations:
    summary: "Backup volume has < 20% free space"
    description: "Free space on /backups is {{ $value }}%. Prune old generations per backup-restore-runbook §5.3."

- alert: BackupVolumeCriticalSpace
  expr: |
    (node_filesystem_avail_bytes{mountpoint="/backups"}
    / node_filesystem_size_bytes{mountpoint="/backups"}) * 100 < 10
  for: 5m
  labels: { severity: critical, owner: devops-oncall }
  annotations:
    summary: "Backup volume has < 10% free space"
    description: "Free space on /backups is {{ $value }}%. Page on-call and prune immediately."
```

### 5.4 Alert coverage summary

| Backend | Backup age | Restore failure | Disk space | Outbox backlog | Reconciliation drift |
|---|---|---|---|---|---|
| PostgreSQL | §5.1 | §5.2 | §5.3 | n/a (PostgreSQL is the outbox source) | n/a |
| Qdrant | §5.1 | §5.2 | §5.3 | required by ADR-0003, tracked separately | required by ADR-0003, tracked separately |
| Neo4j | §5.1 | §5.2 | §5.3 | n/a | n/a |

Outbox backlog and reconciliation drift alerts are tracked in the monitoring rollout, not this runbook. They are listed here so operators know the complete alert surface ADR-0003 requires.

## Appendix: RPO and RTO targets

Targets are pending confirmation by tech-lead per ADR-0003 §后续动作. When confirmed, record them here and update the drill summary template.

| Backend | RPO target | RTO target | Confirmed on |
|---|---|---|---|
| PostgreSQL | TBD (proposed: PITR to within 5 minutes of failure) | TBD (proposed: 1 hour for full PITR restore) | pending |
| Qdrant | TBD (PostgreSQL is source of truth, so RPO is bounded by PostgreSQL) | TBD (proposed: 30 minutes for snapshot restore, 2 hours for full outbox rebuild) | pending |
| Neo4j | TBD (proposed: 24 hours, bounded by dump cadence) | TBD (proposed: 1 hour for load) | pending |

## References

- `docs/adr/ADR-0003-memory-storage-operational-readiness.md` §决策结果 and §后续动作
- `docs/artifacts/2026-07-06-memory-storage-reliability/deployment-context.md` §恢复能力 and §配置与密钥
- `docs/artifacts/2026-07-06-memory-storage-reliability/test-plan.md` (operational drill acceptance)
- `docker-compose.yml` (service names, ports, volumes, dev credentials)
- `backend/migrations/20260706000100_memory_storage_tenant_foundation.sql` (outbox and audit table schema)
