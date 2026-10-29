//! Ely.by-specific facade over the provider's documented Yggdrasil protocol.
//! Ely.by token operations are implemented in `yggdrasil`; this module keeps
//! provider selection and endpoint identity explicit for the rest of the app.

pub use super::yggdrasil::{TokenResponse, YggdrasilClient};

pub type ElyByClient = YggdrasilClient;

pub const AUTH_SERVER: &str = super::yggdrasil::ELY_BY_AUTH_SERVER;
