//! Pushes from the application's server: one [`Message`] for an activity, the [`Target`]s that an application
//! reports to its server, and the [`Request`] of each platform, checked as the device checks the call it stands for.
//!
//! This module has no I/O and no dependency of its own: a server builds the request here and sends it with any
//! HTTP client, and a crate that shares the server's types with an application or a page uses it as it is. With the
//! feature `server`, `Sender` sends it: it signs in to APNs and says in one vocabulary what came back. A web
//! handler, the registration of targets and their storage are the service's own.
//!
//! ```
//! use liveactivity::push::{Application, Message, Target};
//! use liveactivity::{Alert, LayoutSnapshot};
//!
//! # fn run(values: std::collections::BTreeMap<String, liveactivity::Value>) -> liveactivity::Result<()> {
//! // What the application sent its server when the activity got its token.
//! let target: Target = serde_json::from_str(r#"{"type":"apnsActivity","token":"0a1b2c3d"}"#)
//!     .map_err(|_| liveactivity::Error::invalid())?;
//! let message = Message::update("delivery:7", LayoutSnapshot::new(3, values))
//!     .alert(Alert::new("Delivery", "2 minutes"));
//! let request = message.request(&target, &Application::new().bundle_id("dev.example.app"))?;
//! // POST request.path to api.push.apple.com, with request.headers and the provider token, and request.body.
//! # Ok(())
//! # }
//! ```

mod apns;
#[cfg(feature = "server")]
mod send;

/// The HTTP client of the sender, so a server hands it a client of the same version.
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
#[doc(no_inline)]
pub use ::reqwest;
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
pub use send::{Apns, ApnsKey, Delivery, SendError, Sender, SenderBuilder};

use crate::{Alert, EndPolicy, Error, LayoutSnapshot, LayoutStartInput, Result};
use serde::{Deserialize, Serialize};

/// What a push does to an activity, named by the activity's key, which the server knows: the device makes the
/// activity's id. Each platform's [`Request`] is made of it.
#[must_use]
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    event: Event,
    key: String,
    alert: Option<Alert>,
    priority: Priority,
    final_content: Option<LayoutSnapshot>,
    policy: Option<EndPolicy>,
    sent_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
enum Event {
    Start {
        id: String,
        input: Box<LayoutStartInput>,
    },
    Update(Box<LayoutSnapshot>),
    End,
}

impl Message {
    fn new(event: Event, key: String) -> Self {
        Self {
            event,
            key,
            alert: None,
            priority: Priority::Now,
            final_content: None,
            policy: None,
            sent_at: None,
        }
    }

    /// A start, as the application would start the activity: on iOS sent to the push-to-start token (iOS 17.2),
    /// where it needs an [`alert`](Self::alert). `id` is new for every start, a UUID for example: it is what the
    /// activity's handle says on iOS. A scheduled or transient start is the application's own and is refused.
    /// The start's Android options and its fallback are Android's and the application's: APNs carries neither.
    pub fn start(id: impl Into<String>, input: LayoutStartInput) -> Self {
        let key = input.activity_key.clone();
        Self::new(
            Event::Start {
                id: id.into(),
                input: Box::new(input),
            },
            key,
        )
    }

    /// An update with the whole content again, at the next revision of the activity: the device takes a revision
    /// above the one it shows and drops the rest. The server owns the revisions of an activity it pushes to.
    pub fn update(key: impl Into<String>, snapshot: LayoutSnapshot) -> Self {
        Self::new(Event::Update(Box::new(snapshot)), key.into())
    }

    /// An end, with the content the activity shows unless [`snapshot`](Self::snapshot) gives the last, and the
    /// platform's time to leave the screen unless [`policy`](Self::policy) says otherwise.
    pub fn end(key: impl Into<String>) -> Self {
        Self::new(Event::End, key.into())
    }

    /// The last content of an end, at a revision like an update's.
    pub fn snapshot(self, snapshot: LayoutSnapshot) -> Self {
        Self {
            final_content: Some(snapshot),
            ..self
        }
    }

    /// When an ended activity leaves the screen: at once, at the platform's time, or at a time.
    pub fn policy(self, policy: EndPolicy) -> Self {
        Self {
            policy: Some(policy),
            ..self
        }
    }

    /// The alert the push brings: required for a start on iOS, optional otherwise.
    pub fn alert(self, alert: Alert) -> Self {
        Self {
            alert: Some(alert),
            ..self
        }
    }

    /// Whether the person should see the push at once, the default, or it may wait.
    pub fn priority(self, priority: Priority) -> Self {
        Self { priority, ..self }
    }

    /// When the push is sent, in epoch milliseconds: now unless set. The system orders pushes by it.
    pub fn sent_at(self, at: u64) -> Self {
        Self {
            sent_at: Some(at),
            ..self
        }
    }

    /// The key of the activity the message is for.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The request of `target`'s platform, checked as the backend checks the call it stands for: what the device
    /// would refuse is `INVALID_INPUT` here, and a request above the platform's size is `PAYLOAD_TOO_LARGE`.
    /// `application` names what the platform needs to address the application.
    pub fn request(&self, target: &Target, application: &Application) -> Result<Request> {
        if !matches!(self.event, Event::End)
            && (self.final_content.is_some() || self.policy.is_some())
        {
            return Err(Error::new(
                "INVALID_INPUT",
                "The last content and the policy belong to an end.",
            ));
        }
        match target {
            Target::ApnsActivity { .. }
            | Target::ApnsPushToStart { .. }
            | Target::ApnsChannel { .. } => apns::request(self, target, application),
            Target::Fcm { .. } => Err(Error::unsupported_feature(
                "A push to Android waits for liveupdate's push path.",
            )),
        }
    }
}

/// Whether the person should see a push at once or it may wait. On iOS a push of [`Now`](Self::Now) counts
/// toward the application's budget of pushes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Priority {
    /// At once: `apns-priority` 10.
    #[default]
    Now,
    /// When it suits the device: `apns-priority` 5.
    Low,
}

/// Where a push goes: the JSON that an application sends its server when the device reports a token, for example
/// `{"type":"apnsActivity","token":"0a1b…"}`. The server keeps it beside the activity's key.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Target {
    /// An activity's own APNs token, lowercase hex, as `Event::PushToken` brings it: for its updates and its end.
    ApnsActivity {
        /// The token, hex.
        token: String,
    },
    /// The application's push-to-start token, as `Event::PushToStartToken` brings it: for starts (iOS 17.2).
    ApnsPushToStart {
        /// The token, hex.
        token: String,
    },
    /// A broadcast channel the server made at APNs, base64: for the updates and ends of every activity that
    /// listens to it (iOS 18).
    ApnsChannel {
        /// The channel's id, base64.
        channel: String,
    },
    /// The installation's FCM registration token, the activity named by its key in the data. Android's push path
    /// comes with liveupdate; until then a request to it is `UNSUPPORTED_FEATURE`.
    Fcm {
        /// The registration token.
        token: String,
    },
}

impl Target {
    /// The platform the target is on.
    pub fn platform(&self) -> Platform {
        match self {
            Self::ApnsActivity { .. } | Self::ApnsPushToStart { .. } | Self::ApnsChannel { .. } => {
                Platform::Apns
            }
            Self::Fcm { .. } => Platform::Fcm,
        }
    }
}

/// The push services a [`Request`] goes to.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    /// Apple's push service.
    Apns,
    /// Firebase Cloud Messaging.
    Fcm,
}

/// What a platform needs to address the application: the bundle id for APNs's topic and broadcasts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Application {
    bundle_id: Option<String>,
}

impl Application {
    /// An application with nothing named yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The iOS application's bundle id, such as `dev.example.app`.
    pub fn bundle_id(self, bundle_id: impl Into<String>) -> Self {
        Self {
            bundle_id: Some(bundle_id.into()),
        }
    }
}

/// One HTTP request to a push service, `POST` to [`path`](Self::path) on the platform's host. The sign-in header
/// (`authorization`) is the sender's: it is not here.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The service the request goes to.
    pub platform: Platform,
    /// The path on the service's host, such as `/3/device/<token>`.
    pub path: String,
    /// The headers, in the order they are sent.
    pub headers: Vec<(&'static str, String)>,
    /// The JSON body.
    pub body: String,
}
