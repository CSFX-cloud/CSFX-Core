use entity::entities::{agent_api_keys, agents};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect, RelationTrait,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn hash_key(key: &str) -> String {
    Sha256::digest(key.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub async fn agent_for_key(db: &impl ConnectionTrait, key: &str) -> Result<Option<Uuid>, DbErr> {
    agent_api_keys::Entity::find()
        .join(
            sea_orm::JoinType::InnerJoin,
            agent_api_keys::Relation::Agent.def(),
        )
        .filter(agent_api_keys::Column::KeyHash.eq(hash_key(key)))
        .filter(agent_api_keys::Column::IsActive.eq(true))
        .select_only()
        .column(agents::Column::Id)
        .into_tuple::<Uuid>()
        .one(db)
        .await
}

#[cfg(test)]
mod tests {
    use super::hash_key;

    #[test]
    fn hash_is_lowercase_hex_sha256() {
        assert_eq!(
            hash_key("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
