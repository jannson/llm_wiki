//! Display-independent Wiki core. Public access is mediated by a trusted gateway.
#[cfg(feature = "server")]
pub mod engine;
pub mod limits;
#[cfg(feature = "server")]
pub mod provider;
#[cfg(feature = "server")]
pub mod store;
pub mod text;
