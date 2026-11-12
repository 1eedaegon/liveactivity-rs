use crate::CONTRACT_VERSION;
use serde::{Deserialize, Serialize};

/// Machine-readable error category shared by every backend.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// The request is invalid at the public boundary.
    InvalidRequest,
    /// The device or backend cannot provide the requested feature.
    Unsupported,
    /// The user or operating system denied permission.
    PermissionDenied,
    /// A backend or native bridge failed.
    BackendFailure,
    /// A framework event sink rejected an event.
    EventDelivery,
}

/// Stable error crossing from a backend to a framework adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{code:?}: {message}")]
pub struct Error {
    /// Contract version of this serialized error.
    pub contract_version: u16,
    /// Machine-readable category.
    pub code: ErrorCode,
    /// Human-readable, non-sensitive explanation.
    pub message: String,
}

impl Error {
    /// Creates an error under the current contract version.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            code,
            message: message.into(),
        }
    }
}

/// Result returned by liveactivity-core interfaces.
pub type Result<T> = std::result::Result<T, Error>;
