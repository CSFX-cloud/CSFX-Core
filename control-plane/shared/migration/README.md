# Migrations

Masters are updated one after another while the first one already migrates the schema, so
version N-1 binaries run against the schema of version N. Every migration must allow this.

## Expand/contract rule

- Add columns as nullable or with a DEFAULT. Never add a NOT NULL column without DEFAULT.
- Write both the old and the new column for one version (dual write).
- Remove a column or table only in the version after next, never in the version that stops using it.
- No renames of columns or tables. Add the new name, migrate, remove the old one later.
- No full-table `UPDATE` or `DELETE` backfills inside migrations. Backfill in batches from the service.
- Never create indexes on large tables (`logs`, `workloads`, `agent_metrics`) inside a migration. Add them to
  `CONCURRENT_INDEXES` in `src/concurrent_indexes.rs`; the runner builds them with `CREATE INDEX CONCURRENTLY`
  outside a transaction after the migrations and rebuilds an index left invalid by an aborted build.
- Migrations must not hold a table lock on those tables for more than a few seconds.

## Running

`migration::run_pending_migrations` takes a Postgres session advisory lock on a dedicated connection,
so concurrently started migrators serialize and the second one finds nothing pending. Migrations
are applied one at a time, each in its own transaction.

## CI check

`tests/compatibility.rs` scans every migration file newer than the cutoff constant (only the
`up` part) and fails on forbidden patterns. Existing migrations are not checked. Put rollback SQL in
`fn down` or in a constant named `DOWN` declared after the `up` statements; everything from there on is
skipped.
