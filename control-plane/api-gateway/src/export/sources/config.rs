use sea_orm::{AccessMode, DbConn, IsolationLevel, TransactionTrait};

use crate::export::{
    engine::{Exporter, Reader},
    tables::{self, TABLES},
    ExportError,
};

pub async fn export(db: &DbConn, exporter: &mut Exporter) -> Result<(), ExportError> {
    let snapshot = db
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await?;
    let mut dumps = Vec::with_capacity(TABLES.len());
    for table in &TABLES {
        dumps.push(tables::dump(&snapshot, table).await?);
    }
    snapshot.commit().await?;
    for (table, bytes) in TABLES.iter().zip(dumps) {
        exporter
            .add_bytes(table.artifact_name(), "table", None, bytes)
            .await?;
    }
    Ok(())
}

pub async fn restore(db: &DbConn, reader: &Reader) -> Result<(), ExportError> {
    let mut payloads = Vec::with_capacity(TABLES.len());
    for table in &TABLES {
        let bytes = reader.bytes(&table.artifact_name()).await?;
        let json = String::from_utf8(bytes).map_err(|_| ExportError::MalformedArtifact)?;
        payloads.push(json);
    }
    let transaction = db.begin().await?;
    for (table, json) in TABLES.iter().zip(payloads) {
        tables::upsert(&transaction, table, json).await?;
    }
    transaction.commit().await?;
    Ok(())
}
