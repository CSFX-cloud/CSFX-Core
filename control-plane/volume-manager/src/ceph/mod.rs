pub mod core;
pub mod ops;
pub mod storage;

// Re-export häufig verwendete Typen
pub use core::{CephClient, CephConfig, CephError, Result};
pub use ops::{create_postgres_volumes, init_ceph, CephManager};
pub use storage::types::*;
pub use storage::{PoolManager, RbdManager};
