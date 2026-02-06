// etcd Module für High Availability State Management
//
// Module Organisation:
// 📦 core/    - Basis-Komponenten (client, config, error)
// 📦 state/   - State Management (storage, manager, types)
// 📦 ha/      - High Availability (leader election, health)
// 📦 sync/    - Synchronisation (locks, watchers)
// 📦 init/    - Initialisierung

pub mod core;
pub mod ha;
pub mod init;
pub mod state;
pub mod sync;

// Re-exports für einfachen Zugriff
pub use core::{EtcdClient, EtcdConfig, EtcdError};
pub use ha::{HealthChecker, LeaderElection};
pub use state::StateManager;
pub use sync::{DistributedLock, StateWatcher};
