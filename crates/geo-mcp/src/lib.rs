//! `geokernel-mcp`: an MCP server that drives the geokernel window without a
//! window. Agents "click" through the app (pointer, touch, keyboard, tools,
//! input bar) and read its exact state; every action goes through the
//! window's own frame loop (`geo_app::headless`).
//!
//! [`protocol`] is the stdio JSON-RPC layer (and the batch mode),
//! [`session`] the tools, [`report`] what they return.

mod base64;
pub mod protocol;
pub mod report;
mod screenshot;
pub mod session;

pub use base64::{decode as base64_decode, encode as base64_encode};
pub use protocol::Server;
pub use session::Session;
