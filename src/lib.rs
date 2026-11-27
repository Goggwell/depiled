//! Core of Depiled: reads Steam's library and collections from its files, applies user rules,
//! writes collections back while Steam is closed, and keeps snapshots so every change can be
//! undone. The CLI and
//! the desktop UI are thin layers over `service`.

pub mod appinfo;
pub mod bundle;
pub mod catalog;
pub mod cond;
pub mod config;
pub mod hunters;
pub mod licenses;
pub mod local;
pub mod model;
pub mod online;
pub mod ops;
pub mod presets;
pub mod report;
pub mod rules;
pub mod service;
pub mod settings;
pub mod smart;
pub mod snapshot;
pub mod steam;
pub mod vdf;
pub mod watcher;
