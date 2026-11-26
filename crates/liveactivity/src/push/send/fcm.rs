//! The FCM part of the sender: a service account's access token, exchanged for an RS256 JWT at its `token_uri`,
//! and FCM HTTP v1's answers.

use super::{Delivery, Request, SendError, bearer, crypto_provider, unreachable};
use crate::{Error, Result};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    sync::{Mutex, PoisonError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const HOST: &str = "https://fcm.googleapis.com";
/// The scope of FCM's sending.
const SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
/// How long the signed grant asks for, which Google takes at most.
const GRANT_LIFETIME: u64 = 3600;
/// How long before an access token runs out the sender makes a new one.
const MARGIN: Duration = Duration::from_secs(5 * 60);

/// A service account's JSON key, as the Firebase console gives it. Its key never shows: `Debug` names the email and
/// the project alone, and jsonwebtoken wipes its copy of the key when it is dropped.
#[derive(Clone)]
pub struct FcmAccount {
    client_email: String,
    project_id: String,
    token_uri: String,
    key_id: Option<String>,
    encoding: EncodingKey,
}

#[derive(Deserialize)]
struct AccountFile {
    #[serde(rename = "type")]
    kind: String,
    project_id: String,
    private_key: String,
    client_email: String,
    token_uri: String,
    #[serde(default)]
    private_key_id: Option<String>,
}

impl FcmAccount {
    /// The account of a service account's JSON key, signed with once to know that it signs. A file that is none, or
    /// a key that does not sign, is `INVALID_INPUT`; the message never holds the key's text.
    pub fn from_json(json: impl AsRef<[u8]>) -> Result<Self> {
        let refused = || {
            Error::new(
                "INVALID_INPUT",
                "The service account is not a JSON key of a service account with an RSA key.",
            )
        };
        let file: AccountFile = serde_json::from_slice(json.as_ref()).map_err(|_| refused())?;
        if file.kind != "service_account"
            || file.client_email.is_empty()
            || file.project_id.is_empty()
            || !super::web_host_or_path(&file.token_uri)
        {
            return Err(refused());
        }
        let encoding =
            EncodingKey::from_rsa_pem(file.private_key.as_bytes()).map_err(|_| refused())?;
        let account = Self {
            client_email: file.client_email,
            project_id: file.project_id,
            token_uri: file.token_uri,
            key_id: file.private_key_id,
            encoding,
        };
        account.grant(SystemTime::now()).map_err(|_| refused())?;
        Ok(account)
    }

    /// The Firebase project the account sends for.
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// The signed grant of an access token, made at `at`.
    fn grant(&self, at: SystemTime) -> std::result::Result<String, jsonwebtoken::errors::Error> {
        #[derive(Serialize)]
        struct Claims<'a> {
            iss: &'a str,
            scope: &'a str,
            aud: &'a str,
            iat: u64,
            exp: u64,
        }
        crypto_provider();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = self.key_id.clone();
        let iat = at
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        jsonwebtoken::encode(
            &header,
            &Claims {
                iss: &self.client_email,
                scope: SCOPE,
                aud: &self.token_uri,
                iat,
                exp: iat + GRANT_LIFETIME,
            },
            &self.encoding,
        )
    }
}

impl fmt::Debug for FcmAccount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FcmAccount")
            .field("client_email", &self.client_email)
            .field("project_id", &self.project_id)
            .finish_non_exhaustive()
    }
}

/// What the sender needs of FCM: the service account and the host, Google's unless [`host`](Self::host) says
/// otherwise.
#[derive(Debug, Clone)]
pub struct Fcm {
    account: FcmAccount,
    host: String,
}

impl Fcm {
    /// FCM's host, for the account's project.
    pub fn new(account: FcmAccount) -> Self {
        Self {
            account,
            host: HOST.into(),
        }
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

/// The sender's FCM, with the access token and the time it runs out.
pub(super) struct FcmSender {
    fcm: Fcm,
    token: Mutex<Option<(String, SystemTime)>>,
}

impl FcmSender {
    pub(super) fn new(fcm: Fcm) -> Self {
        Self {
            fcm,
            token: Mutex::new(None),
        }
    }

    pub(super) fn fcm(&self) -> &Fcm {
        &self.fcm
    }

    pub(super) fn project(&self) -> String {
        self.fcm.account.project_id.clone()
    }

    pub(super) async fn send(
        &self,
        client: &reqwest::Client,
        timeout: Duration,
        request: &Request,
    ) -> std::result::Result<Delivery, SendError> {
        let token = self.access(client, timeout, None).await?;
        let answer = self.post(client, timeout, request, &token).await?;
        // A token that FCM no longer takes is made again, once.
        let answer = if answer.status == 401 && answer.reason == "UNAUTHENTICATED" {
            let token = self.access(client, timeout, Some(&token)).await?;
            self.post(client, timeout, request, &token).await?
        } else {
            answer
        };
        answer.outcome()
    }

    async fn post(
        &self,
        client: &reqwest::Client,
        timeout: Duration,
        request: &Request,
        token: &str,
    ) -> std::result::Result<Answer, SendError> {
        let mut post = client
            .post(format!("{}{}", self.fcm.host, request.path))
            .header(reqwest::header::AUTHORIZATION, bearer("Bearer", token)?)
            .timeout(timeout)
            .body(request.body.clone());
        for (name, value) in &request.headers {
            post = post.header(*name, value);
        }
        let response = post.send().await.map_err(unreachable)?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|seconds| seconds.trim().parse().ok())
            .map(Duration::from_secs);
        let body: serde_json::Value = response.json_text().await;
        Ok(Answer {
            status,
            name: body
                .get("name")
                .and_then(|name| name.as_str())
                .map(str::to_owned),
            reason: reason(&body),
            retry_after,
        })
    }

    /// The access token: kept until [`MARGIN`] before it runs out, and made again when it is the token that FCM said
    /// it no longer takes.
    async fn access(
        &self,
        client: &reqwest::Client,
        timeout: Duration,
        expired: Option<&str>,
    ) -> std::result::Result<String, SendError> {
        {
            let held = self.token.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some((token, until)) = held.as_ref()
                && expired != Some(token.as_str())
                && SystemTime::now() < *until
            {
                return Ok(token.clone());
            }
        }
        let now = SystemTime::now();
        let grant = self.fcm.account.grant(now).map_err(|_| {
            SendError::Invalid(Error::new(
                "INVALID_INPUT",
                "The service account could not sign its grant.",
            ))
        })?;
        // The grant is a JWT, base64url and dots: it needs no escaping in the form.
        let form = format!(
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={grant}"
        );
        let response = client
            .post(&self.fcm.account.token_uri)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .timeout(timeout)
            .body(form)
            .send()
            .await
            .map_err(unreachable)?;
        let status = response.status().as_u16();
        let body: serde_json::Value = response.json_text().await;
        if status != 200 {
            let reason = body
                .get("error")
                .and_then(|error| error.as_str())
                .unwrap_or_default()
                .to_owned();
            return Err(if (500..=599).contains(&status) {
                SendError::Unavailable {
                    status: Some(status),
                    reason,
                }
            } else {
                SendError::Rejected { status, reason }
            });
        }
        let token = body
            .get("access_token")
            .and_then(|token| token.as_str())
            .ok_or_else(|| SendError::Rejected {
                status,
                reason: "no access token".into(),
            })?
            .to_owned();
        let lifetime = body
            .get("expires_in")
            .and_then(serde_json::Value::as_u64)
            .map_or(Duration::ZERO, Duration::from_secs);
        let until = now + lifetime.saturating_sub(MARGIN);
        *self.token.lock().unwrap_or_else(PoisonError::into_inner) = Some((token.clone(), until));
        Ok(token)
    }
}

/// FCM's code of an error: its `errorCode` where it gives one, else the status of Google's error.
fn reason(body: &serde_json::Value) -> String {
    let error = body.get("error");
    let detail = error
        .and_then(|error| error.get("details"))
        .and_then(serde_json::Value::as_array)
        .and_then(|details| {
            details
                .iter()
                .find_map(|detail| detail.get("errorCode").and_then(|code| code.as_str()))
        });
    detail
        .or_else(|| {
            error
                .and_then(|error| error.get("status"))
                .and_then(|status| status.as_str())
        })
        .unwrap_or_default()
        .to_owned()
}

/// One answer of FCM.
struct Answer {
    status: u16,
    name: Option<String>,
    reason: String,
    retry_after: Option<Duration>,
}

impl Answer {
    fn outcome(self) -> std::result::Result<Delivery, SendError> {
        if self.status == 200 {
            return Ok(Delivery { id: self.name });
        }
        Err(match self.reason.as_str() {
            "UNREGISTERED" => SendError::Unregistered,
            "SENDER_ID_MISMATCH" => SendError::BadTarget {
                reason: self.reason,
            },
            "QUOTA_EXCEEDED" => SendError::TooManyRequests {
                retry_after: self.retry_after,
            },
            _ if self.status == 429 => SendError::TooManyRequests {
                retry_after: self.retry_after,
            },
            _ if (500..=599).contains(&self.status) => SendError::Unavailable {
                status: Some(self.status),
                reason: self.reason,
            },
            _ => SendError::Rejected {
                status: self.status,
                reason: self.reason,
            },
        })
    }
}

/// The body of an answer as JSON, or `null` when it is none.
trait JsonText {
    async fn json_text(self) -> serde_json::Value;
}

impl JsonText for reqwest::Response {
    async fn json_text(self) -> serde_json::Value {
        let text = self.text().await.unwrap_or_default();
        serde_json::from_str(&text).unwrap_or_default()
    }
}
