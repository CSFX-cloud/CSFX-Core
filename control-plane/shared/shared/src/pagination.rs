use http::{HeaderMap, HeaderName, HeaderValue};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Select};
use serde::Deserialize;
use uuid::Uuid;

pub const MAX_LIMIT: u64 = 500;
pub const NEXT_CURSOR_HEADER: &str = "x-next-cursor";

#[derive(Debug, thiserror::Error)]
#[error("invalid cursor")]
pub struct InvalidCursor;

#[derive(Debug, Default, Deserialize)]
pub struct PageParams {
    pub limit: Option<u64>,
    pub cursor: Option<String>,
}

impl PageParams {
    pub fn limit(&self) -> Option<u64> {
        self.limit.map(|limit| limit.clamp(1, MAX_LIMIT))
    }

    pub fn fetch_limit(&self) -> Option<u64> {
        self.limit().map(|limit| limit + 1)
    }

    pub fn cursor_id(&self) -> Result<Option<Uuid>, InvalidCursor> {
        self.cursor
            .as_deref()
            .map(|raw| Uuid::parse_str(raw).map_err(|_| InvalidCursor))
            .transpose()
    }
}

pub fn keyset_by_id<E: EntityTrait>(
    query: Select<E>,
    id_column: E::Column,
    after: Option<Uuid>,
    params: &PageParams,
) -> Select<E> {
    let mut query = query.order_by_asc(id_column);
    if let Some(after) = after {
        query = query.filter(id_column.gt(after));
    }
    match params.fetch_limit() {
        Some(limit) => query.limit(limit),
        None => query,
    }
}

pub fn split_page<T>(
    mut rows: Vec<T>,
    params: &PageParams,
    cursor_of: impl Fn(&T) -> String,
) -> (Vec<T>, Option<String>) {
    let Some(limit) = params.limit() else {
        return (rows, None);
    };
    if rows.len() as u64 <= limit {
        return (rows, None);
    }
    rows.truncate(limit as usize);
    let next = rows.last().map(cursor_of);
    (rows, next)
}

pub fn cursor_headers(next_cursor: Option<String>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let value = next_cursor.and_then(|cursor| HeaderValue::from_str(&cursor).ok());
    if let Some(value) = value {
        headers.insert(HeaderName::from_static(NEXT_CURSOR_HEADER), value);
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(limit: Option<u64>) -> PageParams {
        PageParams {
            limit,
            cursor: None,
        }
    }

    #[test]
    fn limit_is_clamped() {
        assert_eq!(params(Some(0)).limit(), Some(1));
        assert_eq!(params(Some(100_000)).limit(), Some(MAX_LIMIT));
        assert_eq!(params(None).limit(), None);
    }

    #[test]
    fn split_returns_cursor_only_with_next_page() {
        let (rows, next) = split_page(vec![1, 2, 3], &params(Some(2)), |n| n.to_string());
        assert_eq!((rows, next), (vec![1, 2], Some("2".to_string())));
        let (rows, next) = split_page(vec![1, 2], &params(Some(2)), |n| n.to_string());
        assert_eq!((rows, next), (vec![1, 2], None));
        let (rows, next) = split_page(vec![1, 2, 3], &params(None), |n| n.to_string());
        assert_eq!((rows, next), (vec![1, 2, 3], None));
    }

    #[test]
    fn cursor_id_rejects_garbage() {
        let bad = PageParams {
            limit: None,
            cursor: Some("nope".to_string()),
        };
        assert!(bad.cursor_id().is_err());
        assert_eq!(params(None).cursor_id().unwrap(), None);
    }

    #[test]
    fn headers_empty_without_cursor() {
        assert!(cursor_headers(None).is_empty());
        assert!(cursor_headers(Some("abc".into())).contains_key(NEXT_CURSOR_HEADER));
    }
}
