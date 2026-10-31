use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum QosClass {
    BestEffort,
    Burstable,
    Guaranteed,
}

impl QosClass {
    pub fn classify(
        cpu_millicores: i32,
        cpu_limit_millicores: Option<i32>,
        memory_bytes: i64,
        memory_limit_bytes: Option<i64>,
    ) -> Self {
        if cpu_millicores == 0 && memory_bytes == 0 {
            return QosClass::BestEffort;
        }
        let cpu_fixed = cpu_limit_millicores.is_none_or(|limit| limit == cpu_millicores);
        let memory_fixed = memory_limit_bytes.is_none_or(|limit| limit == memory_bytes);
        if cpu_fixed && memory_fixed {
            QosClass::Guaranteed
        } else {
            QosClass::Burstable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_requests_and_limits() {
        assert_eq!(QosClass::classify(0, None, 0, None), QosClass::BestEffort);
        assert_eq!(
            QosClass::classify(500, None, 1024, None),
            QosClass::Guaranteed
        );
        assert_eq!(
            QosClass::classify(500, Some(500), 1024, Some(1024)),
            QosClass::Guaranteed
        );
        assert_eq!(
            QosClass::classify(500, Some(1000), 1024, Some(1024)),
            QosClass::Burstable
        );
        assert_eq!(
            QosClass::classify(500, None, 1024, Some(2048)),
            QosClass::Burstable
        );
    }

    #[test]
    fn orders_from_most_evictable_to_least() {
        assert!(QosClass::BestEffort < QosClass::Burstable);
        assert!(QosClass::Burstable < QosClass::Guaranteed);
    }
}
