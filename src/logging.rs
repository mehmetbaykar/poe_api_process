//! Logging utilities for configuring tracing output
//!
//! This module provides utilities for initializing and configuring
//! the tracing subscriber for debug logging.

#[cfg(feature = "trace")]
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initializes the global tracing subscriber with environment-based filtering
///
/// This function initializes the tracing subscriber with:
/// - Environment-based log level filtering via `RUST_LOG` environment variable
/// - Pretty printing format with timestamps
/// - Automatic log level detection from environment
///
/// # Environment Variables
///
/// - `RUST_LOG`: Controls log level filtering. Examples:
///   - `RUST_LOG=debug` - Enable debug logs for all modules
///   - `RUST_LOG=poe_api_process=debug` - Enable debug logs only for this crate
///   - `RUST_LOG=poe_api_process=trace,reqwest=debug` - Mixed log levels
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "trace")]
/// # {
/// // Initialize with default settings (reads RUST_LOG env var)
/// poe_api_process::init_tracing();
/// # }
/// ```
///
/// # Panics
///
/// This function will panic if called more than once, as the global
/// subscriber can only be set once.
#[cfg(feature = "trace")]
pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
}

/// Initializes the global tracing subscriber with a custom filter
///
/// This function allows you to programmatically set the log level filter
/// instead of relying on the `RUST_LOG` environment variable.
///
/// # Arguments
///
/// * `filter` - A string specifying the log level filter
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "trace")]
/// # {
/// // Enable debug logging for all modules
/// poe_api_process::init_tracing_with_filter("debug");
///
/// // Enable trace logging for poe_api_process, debug for everything else
/// poe_api_process::init_tracing_with_filter("debug,poe_api_process=trace");
/// # }
/// ```
///
/// # Panics
///
/// This function will panic if:
/// - Called more than once (global subscriber can only be set once)
/// - The filter string is invalid
#[cfg(feature = "trace")]
pub fn init_tracing_with_filter(filter: &str) {
    let env_filter = EnvFilter::try_new(filter)
        .expect("Invalid filter string");
    
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .init();
}