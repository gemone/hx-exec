//! hx-exec library: expansion, config loading, command building.

pub mod config;
pub mod expand;
pub mod platform;
pub mod presets;
pub mod runner;

/// Shared error type – a heap-allocated, sendable, sync error.
pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Shared result alias.
pub type Result<T> = std::result::Result<T, Error>;
