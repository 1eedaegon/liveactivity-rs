use crate::{CONTRACT_VERSION, Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};

/// A permission of the operating system that Live Activities depend on.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionKind {
    /// Posting notifications.
    Notifications,
    /// Showing Live Activities.
    LiveActivities,
    /// Promoting an ongoing notification to a Live Update.
    Promotion,
}

/// What the operating system currently allows.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionState {
    /// Allowed.
    Granted,
    /// Refused.
    Denied,
    /// Not asked yet; a request shows the system prompt.
    Prompt,
    /// Only the person can change it, in the system settings.
    SettingsRequired,
    /// The platform has no such permission.
    NotApplicable,
    /// The backend cannot tell.
    Unknown,
}

/// The permissions as the selected backend reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    /// Contract version of this response.
    pub contract_version: u16,
    /// Posting notifications.
    pub notifications: PermissionState,
    /// Showing Live Activities.
    pub live_activities: PermissionState,
    /// Promoting to a Live Update.
    pub promotion: PermissionState,
}

impl Permissions {
    /// Permissions with every kind in `state`, to set the known ones on.
    pub fn all(state: PermissionState) -> Self {
        Self {
            contract_version: CONTRACT_VERSION,
            notifications: state,
            live_activities: state,
            promotion: state,
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.contract_version == CONTRACT_VERSION {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCode::InvalidRequest,
                format!("unsupported contract version {}", self.contract_version),
            ))
        }
    }
}
