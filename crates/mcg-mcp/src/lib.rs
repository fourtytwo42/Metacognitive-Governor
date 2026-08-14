//! Model Context Protocol client contracts and transports.
//!
//! Persist [`McpConnection`] and [`McpToolCatalog`] values, but keep bearer
//! credentials in an operating-system protected store. Remote tools remain
//! data contracts here; MCG's runtime and policy broker decide whether an agent
//! may see or execute them.

mod client;
mod contracts;
mod credentials;
mod legacy;
mod negotiation;

pub use client::*;
pub use contracts::*;
pub use credentials::*;
pub use legacy::*;
pub use negotiation::*;
