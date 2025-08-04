#![deny(unsafe_code)]
#![warn(
    rust_2018_idioms,
    clippy::all
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::multiple_crate_versions,
    clippy::cognitive_complexity,
    clippy::cast_possible_truncation,
    clippy::explicit_iter_loop,
    clippy::items_after_statements,
    clippy::single_match_else,
    clippy::option_if_let_else
)]

//! # `poe_api_process`
//!
//! A Rust client library for interacting with the Poe.com API.
//!
//! This library provides streaming responses, tool calls support, and file upload capabilities.

/// Client module for Poe API interactions
pub mod client;
/// Error types for the library
pub mod error;
/// Type definitions for API requests and responses
pub mod types;
#[cfg(feature = "trace")]
/// Logging utilities for configuring tracing output
pub mod logging;


pub use client::{PoeClient, get_model_list};
pub use error::PoeError;
pub use types::*;
#[cfg(feature = "trace")]
pub use logging::{init_tracing, init_tracing_with_filter};
