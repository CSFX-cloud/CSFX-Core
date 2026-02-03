// State Management Modul
//
// Hier kommt rein:
// - StateManager: Hauptschnittstelle für State-Operationen
// - State CRUD operations
// - State versioning

pub mod manager;
pub mod storage;
pub mod types;

pub use manager::StateManager;
pub use types::*;
