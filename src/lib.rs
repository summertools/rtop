//! `btop-rs` — a fast, elegant terminal system monitor.
//!
//! The crate is split into a platform layer that samples the operating
//! system, a small terminal drawing layer, and an application/UI layer.
//!
//! The binary entry point is intentionally thin: it parses arguments,
//! initialises the terminal, and runs [`app::App`]. Everything else is
//! available to library users and is covered by unit tests.

pub mod app;
pub mod cli;
pub mod config;
pub mod format;
pub mod platform;
pub mod term;
pub mod theme;
pub mod ui;

pub use app::App;
pub use config::Config;
pub use platform::Snapshot;

/// Minimum terminal size accepted by the interactive UI.
pub const MIN_WIDTH: u16 = 40;
/// Minimum terminal height accepted by the interactive UI.
pub const MIN_HEIGHT: u16 = 12;
