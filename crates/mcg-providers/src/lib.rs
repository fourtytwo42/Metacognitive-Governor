//! Provider-neutral model contracts and provider adapters.
//!
//! Persist [`ProviderConnection`] values, but keep [`ProviderCredential`] values in
//! the operating-system credential store. Credentials deliberately cannot be
//! serialized and redact their `Debug` output.

mod compatible;
mod contracts;
mod oauth_browser;
mod oauth_device;
mod probes;
mod routing;

pub use compatible::{
    CHATGPT_CODEX_BASE_URL, OpenAiCompatibleProvider, OpenAiCompatibleProviderBuilder,
};
pub use contracts::*;
pub use oauth_browser::*;
pub use oauth_device::*;
pub use probes::*;
pub use routing::*;
