//! Core of Depiled: reads and edits Steam library collections through the running
//! client, applies user rules, and keeps snapshots so every change can be undone. The CLI and
//! the desktop UI are thin layers over `service`.

pub mod bundle;
pub mod cdp;
pub mod config;
pub mod live;
pub mod model;
pub mod ops;
pub mod rules;
pub mod service;
pub mod settings;
pub mod smart;
pub mod snapshot;
pub mod steam;
pub mod watcher;
