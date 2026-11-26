//! The sender of the feature `server`: it signs in to APNs with the team's key, sends a message's request over
//! HTTP/2, and says in one vocabulary what came back.

use super::{Application, Message, Platform, Request, Target, apns::valid_bundle_id};
use crate::{Error, Result};
use jsonwebtoken::{Algorithm, EncodingKey, Header, crypto::CryptoProvider};
use serde::Serialize;
use std::{
    fmt,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const PRODUCTION: &str = "https://api.push.apple.com";
const SANDBOX: &str = "https://api.sandbox.push.apple.com";
/// How long a provider token is used. APNs takes one for an hour and refuses a new one more often than every 20
/// minutes.
const TOKEN_LIFETIME: Duration = Duration::from_secs(50 * 60);
/// The longest key id and team id taken: Apple's are ten characters.
const MAX_ID: usize = 64;
/// How long one request may take unless the builder says otherwise.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the sender's own client waits for a connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// A team's APNs key: the `.p8` file's PEM text, its key id and the team's id. Its text never shows: `Debug` names
/// the two ids alone, and jsonwebtoken wipes its copy of the key when it is dropped. The PEM that the caller read
/// stays the caller's to drop.
#[derive(Clone)]
pub struct ApnsKey {
    encoding: EncodingKey,
    key_id: String,
    team_id: String,
}

impl ApnsKey {
    /// The key of a `.p8` file, signed with once to know that it signs. A key that does not, or an id that is not
    /// 1 to 64 letters and digits, is `INVALID_INPUT`; the message never holds the key's text.
    pub fn from_pem(
        pem: impl AsRef<[u8]>,
        key_id: impl Into<String>,
        team_id: impl Into<String>,
    ) -> Result<Self> {
        let refused = || Error::new("INVALID_INPUT", "The APNs key is not a P-256 key in PEM.");
        let (key_id, team_id) = (key_id.into(), team_id.into());
        if !valid_id(&key_id) || !valid_id(&team_id) {
            return Err(Error::new(
                "INVALID_INPUT",
                "The key id and the team id are 1 to 64 letters and digits.",
            ));
        }
        let encoding = EncodingKey::from_ec_pem(pem.as_ref()).map_err(|_| refused())?;
        let key = Self {
            encoding,
            key_id,
            team_id,
        };
        key.sign().map_err(|_| refused())?;
        Ok(key)
    }

    /// A provider token made now.
    fn sign(&self) -> std::result::Result<String, jsonwebtoken::errors::Error> {
        self.sign_at(SystemTime::now())
    }

    /// A provider token made at `at`, which is its `iat`.
    fn sign_at(&self, at: SystemTime) -> std::result::Result<String, jsonwebtoken::errors::Error> {
        #[derive(Serialize)]
        struct Claims<'a> {
            iss: &'a str,
            iat: u64,
        }
        crypto_provider();
        // The header APNs documents: the algorithm and the key's id.
        let mut header = Header::new(Algorithm::ES256);
        header.typ = None;
        header.kid = Some(self.key_id.clone());
        let iat = at
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        jsonwebtoken::encode(
            &header,
            &Claims {
                iss: &self.team_id,
                iat,
            },
            &self.encoding,
        )
    }
}

impl fmt::Debug for ApnsKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApnsKey")
            .field("key_id", &self.key_id)
            .field("team_id", &self.team_id)
            .finish_non_exhaustive()
    }
}

/// What the sender needs of APNs: the key, the application's bundle id and the host, production unless
/// [`sandbox`](Self::sandbox) or [`host`](Self::host) says otherwise.
#[derive(Debug, Clone)]
pub struct Apns {
    key: ApnsKey,
    bundle_id: String,
    host: String,
}

impl Apns {
    /// APNs's production host, which an application from the App Store or TestFlight gets its tokens from.
    pub fn new(key: ApnsKey, bundle_id: impl Into<String>) -> Self {
        Self {
            key,
            bundle_id: bundle_id.into(),
            host: PRODUCTION.into(),
        }
    }

    /// The sandbox, which a development build gets its tokens from.
    pub fn sandbox(self) -> Self {
        self.host(SANDBOX)
    }

    /// Another host, such as a test server: a URL without a path.
    pub fn host(self, url: impl Into<String>) -> Self {
        Self {
            host: url.into().trim_end_matches('/').to_owned(),
            ..self
        }
    }

    /// The host the requests go to.
    pub fn host_url(&self) -> &str {
        &self.host
    }
}

/// What a platform said when it took a push.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// The platform's id of the push: APNs's `apns-id`.
    pub id: Option<String>,
}

/// Why a push was not delivered, in one vocabulary for every platform. None holds a key.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    /// The message, or its target, is not one that the device or the sender takes: nothing was sent.
    #[error("the push was not sent: {0}")]
    Invalid(Error),
    /// The target is gone: the application was removed, or the activity ended. Forget the target.
    #[error("the target is no longer registered")]
    Unregistered,
    /// The target is not one the platform takes for this application: a token of another bundle id, or of the other
    /// host, since APNs refuses a development build's token in production the same way. Check the host before
    /// forgetting the target.
    #[error("the platform does not take the target: {reason}")]
    BadTarget {
        /// The platform's reason.
        reason: String,
    },
    /// Too many pushes: send again later, after `retry_after` when the platform says.
    #[error("too many pushes")]
    TooManyRequests {
        /// How long the platform asked to wait.
        retry_after: Option<Duration>,
    },
    /// The platform refused the push for another reason.
    #[error("the platform refused the push ({status}): {reason}")]
    Rejected {
        /// The HTTP status.
        status: u16,
        /// The platform's reason, empty without one.
        reason: String,
    },
    /// The platform could not be reached, or was unavailable: send again later.
    #[error("the platform is unavailable: {reason}")]
    Unavailable {
        /// The HTTP status, none when no answer came.
        status: Option<u16>,
        /// What happened.
        reason: String,
    },
}

/// Sends messages to the platforms it was given; its clones share the client and the sign-in.
#[derive(Clone)]
pub struct Sender {
    shared: Arc<Shared>,
}

struct Shared {
    client: reqwest::Client,
    timeout: Duration,
    apns: Option<ApnsSender>,
}

struct ApnsSender {
    apns: Apns,
    /// The provider token and the wall-clock time it was made at, which APNs reads as its `iat`.
    token: Mutex<Option<(String, SystemTime)>>,
}

impl fmt::Debug for Sender {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sender")
            .field("apns", &self.shared.apns.as_ref().map(|apns| &apns.apns))
            .field("timeout", &self.shared.timeout)
            .finish_non_exhaustive()
    }
}

/// Builds a [`Sender`]: each platform optional, at least one.
#[must_use]
#[derive(Debug, Default)]
pub struct SenderBuilder {
    apns: Option<Apns>,
    client: Option<reqwest::Client>,
    timeout: Option<Duration>,
}

impl SenderBuilder {
    /// Sends to APNs with this key.
    pub fn apns(self, apns: Apns) -> Self {
        Self {
            apns: Some(apns),
            ..self
        }
    }

    /// The service's own HTTP client; otherwise the sender makes one, with HTTP/2 and rustls.
    pub fn client(self, client: reqwest::Client) -> Self {
        Self {
            client: Some(client),
            ..self
        }
    }

    /// How long one request may take: 30 seconds unless set.
    pub fn timeout(self, timeout: Duration) -> Self {
        Self {
            timeout: Some(timeout),
            ..self
        }
    }

    /// The sender, or `INVALID_INPUT` without a platform, with a bundle id that is none, or with a host that is
    /// not an `https` or `http` URL.
    pub fn build(self) -> Result<Sender> {
        let Some(apns) = self.apns else {
            return Err(Error::new(
                "INVALID_INPUT",
                "A sender needs a platform to send to: give it APNs.",
            ));
        };
        if !valid_bundle_id(&apns.bundle_id) {
            return Err(Error::new(
                "INVALID_INPUT",
                "The bundle id is letters, digits, `.` and `-`.",
            ));
        }
        let host = reqwest::Url::parse(&apns.host).ok();
        let web = host.as_ref().is_some_and(|url| {
            matches!(url.scheme(), "https" | "http") && url.has_host() && url.path() == "/"
        });
        if !web {
            return Err(Error::new(
                "INVALID_INPUT",
                "The APNs host is an https URL without a path.",
            ));
        }
        let client = match self.client {
            Some(client) => client,
            None => reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .build()
                .map_err(|_| Error::native())?,
        };
        Ok(Sender {
            shared: Arc::new(Shared {
                client,
                timeout: self.timeout.unwrap_or(DEFAULT_TIMEOUT),
                apns: Some(ApnsSender {
                    apns,
                    token: Mutex::new(None),
                }),
            }),
        })
    }
}

impl Sender {
    /// A builder with no platform yet.
    pub fn builder() -> SenderBuilder {
        SenderBuilder::default()
    }

    /// Sends `message` to `target`, or says why it was not delivered. A message that the device would refuse, and
    /// a target of a platform that the sender was not given, are [`SendError::Invalid`] and send nothing.
    pub async fn send(
        &self,
        message: &Message,
        target: &Target,
    ) -> std::result::Result<Delivery, SendError> {
        match target.platform() {
            Platform::Apns => {
                let Some(apns) = &self.shared.apns else {
                    return Err(SendError::Invalid(Error::unsupported_feature(
                        "This sender was not given APNs.",
                    )));
                };
                let application = Application::new().bundle_id(apns.apns.bundle_id.clone());
                let request = message
                    .request(target, &application)
                    .map_err(SendError::Invalid)?;
                self.send_apns(apns, &request).await
            }
            Platform::Fcm => Err(SendError::Invalid(Error::unsupported_feature(
                "This sender sends to APNs; a push to Android waits for liveupdate's push path.",
            ))),
        }
    }

    async fn send_apns(
        &self,
        apns: &ApnsSender,
        request: &Request,
    ) -> std::result::Result<Delivery, SendError> {
        let (first, token) = self.post(apns, request, None).await?;
        // A token that APNs says has expired is made again, once: the clock of the server may have drifted, or the
        // process slept. Pushes that failed with the same token make one new token between them.
        let answer = if first.status == 403 && first.reason == "ExpiredProviderToken" {
            self.post(apns, request, Some(&token)).await?.0
        } else {
            first
        };
        answer.outcome()
    }

    /// One request with the provider token, and the token it was sent with. `expired` is a token that APNs said
    /// has expired: the sender makes a new one unless another push already has.
    async fn post(
        &self,
        apns: &ApnsSender,
        request: &Request,
        expired: Option<&str>,
    ) -> std::result::Result<(Answer, String), SendError> {
        let token = apns.token(expired)?;
        // Marked sensitive, so that a dump of the request at trace level does not show it.
        let mut bearer = reqwest::header::HeaderValue::from_str(&format!("bearer {token}"))
            .map_err(|_| SendError::Invalid(Error::native()))?;
        bearer.set_sensitive(true);
        let mut post = self
            .shared
            .client
            .post(format!("{}{}", apns.apns.host, request.path))
            .header(reqwest::header::AUTHORIZATION, bearer)
            .timeout(self.shared.timeout)
            .body(request.body.clone());
        for (name, value) in &request.headers {
            post = post.header(*name, value);
        }
        let response = post.send().await.map_err(|error| {
            if error.is_builder() {
                SendError::Invalid(Error::new(
                    "INVALID_INPUT",
                    "The request could not be made: check the host.",
                ))
            } else {
                SendError::Unavailable {
                    status: None,
                    reason: if error.is_timeout() {
                        "the request timed out".into()
                    } else {
                        "the host could not be reached".into()
                    },
                }
            }
        })?;
        let status = response.status().as_u16();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        // A broadcast's answer names its request by `apns-request-id`.
        let id = header("apns-id").or_else(|| header("apns-request-id"));
        let retry_after = header("retry-after")
            .and_then(|seconds| seconds.trim().parse().ok())
            .map(Duration::from_secs);
        let text = response.text().await.unwrap_or_default();
        let reason = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|body| body.get("reason")?.as_str().map(str::to_owned))
            .unwrap_or_default();
        Ok((
            Answer {
                status,
                id,
                retry_after,
                reason,
            },
            token,
        ))
    }
}

impl ApnsSender {
    /// The provider token: made once, then again [`TOKEN_LIFETIME`] after its `iat` by the wall clock, which APNs
    /// reads, or when it is the token that APNs said has `expired`.
    fn token(&self, expired: Option<&str>) -> std::result::Result<String, SendError> {
        let mut held = self.token.lock().unwrap_or_else(PoisonError::into_inner);
        let now = SystemTime::now();
        if let Some((token, made)) = held.as_ref()
            && expired != Some(token.as_str())
            && now
                .duration_since(*made)
                .is_ok_and(|age| age < TOKEN_LIFETIME)
        {
            return Ok(token.clone());
        }
        let token = self.apns.key.sign_at(now).map_err(|_| {
            SendError::Invalid(Error::new(
                "INVALID_INPUT",
                "The APNs key could not sign a provider token.",
            ))
        })?;
        *held = Some((token.clone(), now));
        Ok(token)
    }
}

/// One answer of a push service.
struct Answer {
    status: u16,
    id: Option<String>,
    retry_after: Option<Duration>,
    reason: String,
}

impl Answer {
    fn outcome(self) -> std::result::Result<Delivery, SendError> {
        match self.status {
            200 => Ok(Delivery { id: self.id }),
            410 => Err(SendError::Unregistered),
            400 if matches!(
                self.reason.as_str(),
                "BadDeviceToken" | "DeviceTokenNotForTopic" | "MissingDeviceToken"
            ) =>
            {
                Err(SendError::BadTarget {
                    reason: self.reason,
                })
            }
            429 => Err(SendError::TooManyRequests {
                retry_after: self.retry_after,
            }),
            500..=599 => Err(SendError::Unavailable {
                status: Some(self.status),
                reason: self.reason,
            }),
            status => Err(SendError::Rejected {
                status,
                reason: self.reason,
            }),
        }
    }
}

/// Signs with aws-lc-rs where the process has no provider of jsonwebtoken yet: a service whose own jsonwebtoken
/// turns on the other provider leaves the choice to the process, and jsonwebtoken would panic without one.
fn crypto_provider() {
    if CryptoProvider::try_get_default().is_none() {
        // Another thread may have installed one in between: either is a provider.
        let _ = jsonwebtoken::crypto::aws_lc::DEFAULT_PROVIDER.install_default();
    }
}

/// A key id or a team id: letters and digits.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID && id.bytes().all(|byte| byte.is_ascii_alphanumeric())
}
