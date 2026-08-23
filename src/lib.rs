//! Native Rust port of [uFBT](https://github.com/flipperdevices/flipperzero-ufbt), the
//! Flipper Zero micro build tool.
//!
//! The crate deploys a Flipper Zero SDK and an ARM GCC toolchain into a local uFBT home
//! directory and builds `.fap` / `.fal` applications from an `application.fam` manifest,
//! without requiring Python, SCons or a shell.
//!
//! Every long-running operation reports structured [`log`] events instead of writing to
//! standard output, so a GUI can render progress rather than parse text.
//!
//! # Modules
//!
//! - [`paths`] — layout of the uFBT home directory.
//! - [`state`] — persisted deployment state (`ufbt_state.json`).
//! - [`log`] — structured logging: levels, progress, events, sinks.
//! - [`net`] — HTTP downloads with progress reporting.
//! - [`sdk`] — SDK channel resolution and deployment.
//! - [`toolchain`] — ARM GCC toolchain deployment.
//! - [`build`] — manifest parsing, asset compilation and application linking.
//! - [`installer`] — top-level orchestration of SDK and toolchain installation.
//!
//! # Errors
//!
//! Each module defines its own error enum and every operation returns the narrowest one
//! that fits. [`Error`] aggregates them all and every module error converts into it with
//! `?`, so callers that drive several operations can propagate a single type.

#![deny(missing_docs)]

pub mod build;
mod error;
pub mod installer;
pub mod log;
pub mod net;
pub mod paths;
pub mod sdk;
pub mod state;
pub mod toolchain;

pub use error::{Error, Result};
pub use log::{ConsoleSink, LogEvent, LogLevel, LogSink, Logger};
pub use paths::Paths;
