use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Authentication provider returned {kind}: {message}")]
    Provider { kind: String, message: String },
    #[error("Authentication service returned an unexpected response")]
    MalformedResponse,
    #[error("Authentication service request timed out")]
    Timeout,
    #[error("Authentication service could not be reached")]
    Network,
    #[error("Secure credential storage is unavailable; token was not saved")]
    SecureStorageUnavailable,
    #[error("This platform's secure credential storage is not implemented")]
    SecureStorageUnsupportedPlatform,
    #[error("Account was not found")]
    AccountNotFound,
    #[error("Invalid account data: {0}")]
    InvalidAccount(String),
    #[error("Local account data could not be read or written")]
    Storage,
}

impl From<reqwest::Error> for AuthError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout
        } else {
            Self::Network
        }
    }
}
