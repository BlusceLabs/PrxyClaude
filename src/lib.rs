//! PxyClaude - Middleware between Claude Code CLI (Anthropic API) and NVIDIA NIM
//! 
//! This library provides a Rust implementation of the Python-based PxyClaude proxy server.
//! It acts as a middleware between Claude Code CLI and various AI model providers,
//! primarily NVIDIA NIM, providing API compatibility, routing, and enhanced features.

pub mod api;
pub mod cli;
pub mod config;
pub mod core;
pub mod messaging;
pub mod providers;

pub use api::*;
pub use cli::*;
pub use config::*;
pub use core::anthropic::*;
pub use providers::*;

#[cfg(test)]
mod tests;

/// Re-export commonly used types and functions
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;