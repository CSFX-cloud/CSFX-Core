use entity::entities::{dns_records, ingress_routes, site_domains};
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryFilter};

use crate::dns_sync;

const INTERNAL_SUFFIX: &str = ".internal";
const INTERNAL_SERVICE_MARKER: &str = ".svc.";

#[derive(Debug)]
pub enum HostnameRejection {
    Reserved,
    Taken,
    Database(DbErr),
}

impl HostnameRejection {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Reserved => "hostname is reserved by the platform",
            Self::Taken => "hostname is already in use",
            Self::Database(_) => "internal error",
        }
    }
}

impl From<DbErr> for HostnameRejection {
    fn from(error: DbErr) -> Self {
        Self::Database(error)
    }
}

pub fn is_reserved(hostname: &str, platform_hostname: &str) -> bool {
    is_platform_name(hostname, platform_hostname) || is_internal_service_name(hostname)
}

fn is_platform_name(hostname: &str, platform_hostname: &str) -> bool {
    hostname == platform_hostname
        || hostname
            .strip_suffix(platform_hostname)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn is_internal_service_name(hostname: &str) -> bool {
    hostname.ends_with(INTERNAL_SUFFIX) && hostname.contains(INTERNAL_SERVICE_MARKER)
}

pub async fn ensure_claimable(
    db: &DatabaseConnection,
    hostname: &str,
) -> Result<(), HostnameRejection> {
    let platform_hostname = dns_sync::platform_hostname(&dns_sync::zone_from_env());
    if is_reserved(hostname, &platform_hostname) {
        return Err(HostnameRejection::Reserved);
    }
    let routes = ingress_routes::Entity::find()
        .filter(ingress_routes::Column::Hostname.eq(hostname))
        .count(db)
        .await?;
    let domains = site_domains::Entity::find()
        .filter(site_domains::Column::Hostname.eq(hostname))
        .count(db)
        .await?;
    let records = dns_records::Entity::find()
        .filter(dns_records::Column::Name.eq(hostname))
        .count(db)
        .await?;
    if routes + domains + records > 0 {
        return Err(HostnameRejection::Taken);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_reserved;

    const PLATFORM: &str = "console.csfx.internal";

    #[test]
    fn platform_hostname_and_everything_below_it_is_reserved() {
        assert!(is_reserved("console.csfx.internal", PLATFORM));
        assert!(is_reserved("api.console.csfx.internal", PLATFORM));
        assert!(is_reserved("a.b.console.csfx.internal", PLATFORM));
    }

    #[test]
    fn lookalike_names_are_not_reserved() {
        assert!(!is_reserved("myconsole.csfx.internal", PLATFORM));
        assert!(!is_reserved("console.csfx.internal.example.com", PLATFORM));
        assert!(!is_reserved("app.csfx.internal", PLATFORM));
    }

    #[test]
    fn internal_service_names_are_reserved() {
        assert!(is_reserved("db.svc.0000.internal", PLATFORM));
        assert!(!is_reserved("svc.example.com", PLATFORM));
    }
}
