use std::fs;
use std::path::Path;

const CUTOFF: &str = "m20261114_010000_partition_vault_audit.rs";
const HOT_TABLES: [&str; 3] = ["LOGS", "WORKLOADS", "AGENT_METRICS"];
const FORBIDDEN: [&str; 6] = [
    "RENAME COLUMN",
    "RENAME TO",
    "DROP COLUMN",
    "DROP TABLE",
    ".RENAME_COLUMN(",
    ".DROP_COLUMN(",
];

fn normalize(source: &str) -> String {
    let up = source.split("fn down").next().unwrap_or(source);
    up.to_uppercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn targets_hot_table(statement: &str) -> bool {
    HOT_TABLES.iter().any(|table| {
        let needle = format!(" ON {table}");
        statement.match_indices(&needle).any(|(index, _)| {
            statement[index + needle.len()..]
                .chars()
                .next()
                .is_none_or(|next| !(next.is_alphanumeric() || next == '_'))
        })
    })
}

fn statement_violation(statement: &str) -> Option<&'static str> {
    let is_mutation = statement.starts_with("UPDATE ") || statement.starts_with("DELETE FROM ");
    if is_mutation && !statement.contains(" WHERE ") {
        return Some("UPDATE or DELETE without WHERE");
    }
    let is_index = statement.contains("CREATE INDEX") || statement.contains("CREATE UNIQUE INDEX");
    if is_index && targets_hot_table(statement) {
        return Some("index on a large table belongs in concurrent_indexes.rs");
    }
    if statement.contains("ADD COLUMN")
        && statement.contains("NOT NULL")
        && !statement.contains("DEFAULT")
    {
        return Some("ADD COLUMN NOT NULL without DEFAULT");
    }
    None
}

fn violations(source: &str) -> Vec<&'static str> {
    let text = normalize(source);
    let mut found: Vec<&'static str> = FORBIDDEN
        .iter()
        .copied()
        .filter(|pattern| text.contains(pattern))
        .collect();
    found.extend(
        text.split([';', '"'])
            .map(str::trim)
            .filter_map(statement_violation),
    );
    found
}

fn checked_migrations() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<(String, String)> = fs::read_dir(dir)
        .expect("migration source directory must be readable")
        .map(|entry| entry.expect("directory entry must be readable").path())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_owned();
            let is_newer = name.starts_with('m') && name.as_str() > CUTOFF;
            is_newer.then(|| {
                (
                    name,
                    fs::read_to_string(&path).expect("file must be readable"),
                )
            })
        })
        .collect();
    files.sort();
    files
}

#[test]
fn new_migrations_are_backward_compatible() {
    let failures: Vec<String> = checked_migrations()
        .iter()
        .flat_map(|(name, source)| {
            violations(source)
                .into_iter()
                .map(move |reason| format!("{name}: {reason}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn accepts_additive_migration() {
    let source = r#"
        execute_unprepared("ALTER TABLE agents ADD COLUMN note TEXT NULL;
        ALTER TABLE agents ADD COLUMN flag BOOLEAN NOT NULL DEFAULT false;
        UPDATE agents SET note = '' WHERE note IS NULL;
        CREATE INDEX idx_other_x ON other_table (id)")
        async fn down() { DROP TABLE agents; }
    "#;
    assert!(violations(source).is_empty());
}

#[test]
fn rejects_renames_and_drops() {
    for sql in [
        "ALTER TABLE a RENAME COLUMN x TO y",
        "ALTER TABLE a RENAME TO b",
        "ALTER TABLE a DROP COLUMN x",
        "DROP TABLE a",
        "manager.alter_table(t).rename_column(a, b)",
        "manager.alter_table(t).drop_column(a)",
    ] {
        assert!(!violations(sql).is_empty(), "{sql}");
    }
}

#[test]
fn rejects_not_null_without_default() {
    assert!(!violations("\"ALTER TABLE a ADD COLUMN x UUID NOT NULL\"").is_empty());
}

#[test]
fn rejects_unbounded_mutations() {
    assert!(!violations("\"UPDATE logs SET seen = created\"").is_empty());
    assert!(!violations("\"DELETE FROM peers\"").is_empty());
    assert!(violations("\"DELETE FROM peers WHERE id = 1\"").is_empty());
}

#[test]
fn rejects_blocking_index_on_hot_tables() {
    assert!(!violations("\"CREATE INDEX i ON logs (a)\"").is_empty());
    assert!(!violations("\"CREATE UNIQUE INDEX i ON workloads(a)\"").is_empty());
    assert!(!violations("\"CREATE INDEX i ON agent_metrics (a)\"").is_empty());
    assert!(!violations("\"CREATE INDEX CONCURRENTLY i ON logs (a)\"").is_empty());
    assert!(violations("\"CREATE INDEX i ON logs_archive (a)\"").is_empty());
}
