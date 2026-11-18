use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement};

pub struct ConfigTable {
    pub name: &'static str,
    pub key: &'static [&'static str],
    pub filter: &'static str,
}

const fn table(name: &'static str) -> ConfigTable {
    ConfigTable {
        name,
        key: &["id"],
        filter: "TRUE",
    }
}

pub const TABLES: [ConfigTable; 14] = [
    ConfigTable {
        name: "system_settings",
        key: &["key"],
        filter: "key NOT LIKE 'export.%'",
    },
    table("organization"),
    table("permission"),
    table("user"),
    table("role"),
    ConfigTable {
        name: "role_permission",
        key: &["role_id", "permission_id"],
        filter: "TRUE",
    },
    table("user_organization"),
    table("identity_providers"),
    table("idp_group_mappings"),
    table("user_identities"),
    table("user_ssh_keys"),
    table("resource_groups"),
    table("networks"),
    table("network_policies"),
];

impl ConfigTable {
    pub fn artifact_name(&self) -> String {
        format!("config/{}.json", self.name)
    }
}

pub async fn dump(db: &impl ConnectionTrait, table: &ConfigTable) -> Result<Vec<u8>, DbErr> {
    let sql = format!(
        r#"SELECT COALESCE(jsonb_agg(t), '[]'::jsonb)::text AS data FROM "{}" t WHERE {}"#,
        table.name, table.filter
    );
    let row = db
        .query_one(Statement::from_string(DbBackend::Postgres, sql))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound(table.name.to_string()))?;
    Ok(row.try_get::<String>("", "data")?.into_bytes())
}

async fn update_columns(
    db: &impl ConnectionTrait,
    table: &ConfigTable,
) -> Result<Vec<String>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT column_name FROM information_schema.columns \
             WHERE table_schema = current_schema() AND table_name = $1 \
             ORDER BY ordinal_position",
            [table.name.into()],
        ))
        .await?;
    let mut columns = Vec::with_capacity(rows.len());
    for row in rows {
        let column = row.try_get::<String>("", "column_name")?;
        if !table.key.contains(&column.as_str()) {
            columns.push(column);
        }
    }
    Ok(columns)
}

fn upsert_sql(table: &ConfigTable, columns: &[String]) -> String {
    let keys = table
        .key
        .iter()
        .map(|column| format!("\"{column}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let action = if columns.is_empty() {
        "DO NOTHING".to_string()
    } else {
        let assignments = columns
            .iter()
            .map(|column| format!("\"{column}\" = EXCLUDED.\"{column}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!("DO UPDATE SET {assignments}")
    };
    format!(
        r#"INSERT INTO "{name}" SELECT * FROM jsonb_populate_recordset(NULL::"{name}", $1::jsonb) ON CONFLICT ({keys}) {action}"#,
        name = table.name
    )
}

pub async fn upsert(
    db: &impl ConnectionTrait,
    table: &ConfigTable,
    json: String,
) -> Result<(), DbErr> {
    let columns = update_columns(db, table).await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        upsert_sql(table, &columns),
        [json.into()],
    ))
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_updates_non_key_columns() {
        let sql = upsert_sql(
            &table("role"),
            &["name".to_string(), "description".to_string()],
        );
        assert!(sql.contains(r#"ON CONFLICT ("id") DO UPDATE SET "name" = EXCLUDED."name", "description" = EXCLUDED."description""#));
    }

    #[test]
    fn upsert_without_value_columns_does_nothing() {
        let sql = upsert_sql(&TABLES[5], &[]);
        assert!(sql.contains(r#"ON CONFLICT ("role_id", "permission_id") DO NOTHING"#));
    }

    #[test]
    fn parents_precede_children() {
        let position = |name: &str| TABLES.iter().position(|t| t.name == name).unwrap();
        for (parent, child) in [
            ("organization", "role"),
            ("role", "user_organization"),
            ("user", "user_organization"),
            ("identity_providers", "user_identities"),
            ("user", "user_identities"),
            ("organization", "resource_groups"),
            ("resource_groups", "networks"),
            ("networks", "network_policies"),
            ("permission", "role_permission"),
        ] {
            assert!(
                position(parent) < position(child),
                "{parent} before {child}"
            );
        }
    }
}
