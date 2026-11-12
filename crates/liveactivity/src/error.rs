use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

/// Every code of the contract, on every platform and backend.
pub const ERROR_CODES: [&str; 12] = [
    "UNSUPPORTED_PLATFORM",
    "UNSUPPORTED_FEATURE",
    "PERMISSION_DENIED",
    "NOT_FOREGROUND",
    "CONFIGURATION_REQUIRED",
    "INVALID_INPUT",
    "PAYLOAD_TOO_LARGE",
    "NOT_FOUND",
    "STALE_REVISION",
    "CONFLICT",
    "SYSTEM_LIMIT",
    "NATIVE_ERROR",
];

/// A failure of the contract: one of [`ERROR_CODES`] and a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{code}: {message}")]
pub struct Error {
    pub code: String,
    pub message: String,
}

impl Error {
    pub fn invalid() -> Self {
        Self::new(
            "INVALID_INPUT",
            "Request does not match the live activity contract",
        )
    }

    pub fn payload_too_large() -> Self {
        Self::new(
            "PAYLOAD_TOO_LARGE",
            "Layout, styles and values exceed the live activity payload limit",
        )
    }

    pub fn unsupported() -> Self {
        Self::new(
            "UNSUPPORTED_PLATFORM",
            "This release supports native activities on iOS and Android only",
        )
    }

    /// The backend does not provide what the request asks for.
    pub fn unsupported_feature(message: &str) -> Self {
        Self::new("UNSUPPORTED_FEATURE", message)
    }

    pub fn native() -> Self {
        Self::new(
            "NATIVE_ERROR",
            "The native activity backend could not complete the request",
        )
    }

    /// An error of `code`, which is one of [`ERROR_CODES`].
    pub fn new(code: &str, message: &str) -> Self {
        debug_assert!(ERROR_CODES.contains(&code), "no error code: {code}");
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// An error of activitykit keeps its code. The message is its text without the code: constant text, with the feature
/// and the iOS version of `UNSUPPORTED_FEATURE` and the scope of `SYSTEM_LIMIT`.
impl From<activitykit::Error> for Error {
    fn from(error: activitykit::Error) -> Self {
        let code = error.code();
        let text = error.to_string();
        let message = text
            .strip_prefix(code)
            .and_then(|rest| rest.strip_prefix(": "))
            .unwrap_or_else(|| error.message());
        Self::new(code, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use activitykit::{Error as Kit, LimitScope};

    #[test]
    fn activitykit_errors_keep_their_codes_and_their_details() {
        let cases = [
            (Kit::Unsupported, "UNSUPPORTED_PLATFORM"),
            (
                Kit::InvalidInput("Request does not match the live activity contract".into()),
                "INVALID_INPUT",
            ),
            (Kit::PayloadTooLarge("p".into()), "PAYLOAD_TOO_LARGE"),
            (Kit::NotFound("n".into()), "NOT_FOUND"),
            (Kit::Conflict("c".into()), "CONFLICT"),
            (Kit::StaleRevision("s".into()), "STALE_REVISION"),
            (Kit::PermissionDenied("d".into()), "PERMISSION_DENIED"),
            (Kit::NotForeground("f".into()), "NOT_FOREGROUND"),
            (
                Kit::ConfigurationRequired("r".into()),
                "CONFIGURATION_REQUIRED",
            ),
            (Kit::Native("x".into()), "NATIVE_ERROR"),
        ];
        for (error, code) in cases {
            let converted = Error::from(error.clone());
            assert_eq!(converted.code, code);
            assert_eq!(converted.message, error.message());
            assert_eq!(converted.to_string(), error.to_string());
        }
        let invalid = Kit::InvalidInput("Request does not match the live activity contract".into());
        assert_eq!(Error::from(invalid), Error::invalid());
        let feature = Error::from(Kit::UnsupportedFeature {
            feature: "transient".into(),
            minimum_ios: (18, 0),
        });
        assert_eq!(
            (feature.code.as_str(), feature.message.as_str()),
            ("UNSUPPORTED_FEATURE", "transient needs iOS 18.0")
        );
        let limit = Error::from(Kit::SystemLimit {
            scope: LimitScope::App,
        });
        assert_eq!(
            limit.to_string(),
            "SYSTEM_LIMIT: the app's limit of Live Activities is reached"
        );
        let device = Error::from(Kit::SystemLimit {
            scope: LimitScope::Device,
        });
        assert_eq!(
            device.message,
            "the device's limit of Live Activities is reached"
        );
    }
}
