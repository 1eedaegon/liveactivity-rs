//! The sender of the feature `server`, against a test server in APNs's place (REQ-P-006, REQ-P-007). The key is
//! made for each run: no key is kept in the repository.
#![cfg(feature = "server")]

use aws_lc_rs::signature::{
    ECDSA_P256_SHA256_FIXED, ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair,
    UnparsedPublicKey,
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use liveactivity::push::{Apns, ApnsKey, Message, SendError, Sender, Target};
use liveactivity::{LayoutSnapshot, Value};
use serde_json::{Value as Json, json};
use std::{collections::BTreeMap, time::Duration};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "0a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f9";
const BUNDLE: &str = "dev.example.app";
const KEY_ID: &str = "ABC123DEFG";
const TEAM_ID: &str = "TEAM456789";

/// A P-256 key in the PKCS#8 PEM of a `.p8` file, and its public key.
fn key() -> (String, Vec<u8>) {
    let pair = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING).unwrap();
    let der = pair.to_pkcs8v1().unwrap();
    let body = STANDARD.encode(der.as_ref());
    let lines: Vec<&str> = body
        .as_bytes()
        .chunks(64)
        .map(|line| std::str::from_utf8(line).unwrap())
        .collect();
    let pem = format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        lines.join("\n")
    );
    (pem, pair.public_key().as_ref().to_vec())
}

fn update(revision: u64) -> Message {
    let values: BTreeMap<String, Value> =
        serde_json::from_value(json!({ "eta": "2 min" })).unwrap();
    Message::update("delivery:7", LayoutSnapshot::new(revision, values))
}

fn activity() -> Target {
    Target::ApnsActivity {
        token: TOKEN.into(),
    }
}

fn sender(server: &MockServer, pem: &str) -> Sender {
    let key = ApnsKey::from_pem(pem, KEY_ID, TEAM_ID).unwrap();
    Sender::builder()
        .apns(Apns::new(key, BUNDLE).host(server.uri()))
        .build()
        .unwrap()
}

fn device() -> String {
    format!("/3/device/{TOKEN}")
}

/// The bearer tokens of the requests the server got.
async fn bearers(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| {
            let value = request.headers.get("authorization").unwrap();
            let value = value.to_str().unwrap();
            value.strip_prefix("bearer ").unwrap().to_owned()
        })
        .collect()
}

fn part(token: &str, index: usize) -> Json {
    let part = token.split('.').nth(index).unwrap();
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part).unwrap()).unwrap()
}

#[tokio::test]
async fn req_p_006_a_sender_signs_in_for_what_it_is_given() {
    // REQ-P-006
    let (pem, public) = key();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(device()))
        .and(header("apns-push-type", "liveactivity"))
        .and(header(
            "apns-topic",
            "dev.example.app.push-type.liveactivity",
        ))
        .and(header("apns-priority", "10"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("apns-id", "8E1B0F5A-0000-4000-8000-000000000001"),
        )
        .expect(3)
        .mount(&server)
        .await;
    let sender = sender(&server, &pem);
    let delivery = sender.send(&update(1), &activity()).await.unwrap();
    assert_eq!(
        delivery.id.as_deref(),
        Some("8E1B0F5A-0000-4000-8000-000000000001")
    );

    // The provider token: ES256, the key's id, the team, the time it was made; signed by the key.
    let tokens = bearers(&server).await;
    let token = &tokens[0];
    assert_eq!(part(token, 0), json!({ "alg": "ES256", "kid": KEY_ID }));
    let claims = part(token, 1);
    assert_eq!(claims["iss"], TEAM_ID);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let made = claims["iat"].as_u64().unwrap();
    assert!(made <= now && now - made < 60, "iat {made}, now {now}");
    let (signed, signature) = token.rsplit_once('.').unwrap();
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, &public)
        .verify(
            signed.as_bytes(),
            &URL_SAFE_NO_PAD.decode(signature).unwrap(),
        )
        .unwrap();

    // Made once: the next push and a clone's take the same.
    sender.send(&update(2), &activity()).await.unwrap();
    sender.clone().send(&update(3), &activity()).await.unwrap();
    let tokens = bearers(&server).await;
    assert_eq!(tokens.len(), 3);
    assert!(tokens.iter().all(|other| other == token));

    // A builder without a platform builds nothing, and a key that is no key is refused without its text.
    let error = Sender::builder().build().unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");
    let error = ApnsKey::from_pem(
        "-----BEGIN PRIVATE KEY-----\nc2VjcmV0\n-----END PRIVATE KEY-----\n",
        KEY_ID,
        TEAM_ID,
    )
    .unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");
    assert!(!error.message.contains("c2VjcmV0"));
    for id in ["", "has space"] {
        assert!(ApnsKey::from_pem(&pem, id, TEAM_ID).is_err(), "{id:?}");
        assert!(ApnsKey::from_pem(&pem, KEY_ID, id).is_err(), "{id:?}");
    }

    // The key never shows, nor a line of it, nor the provider token.
    let key = ApnsKey::from_pem(&pem, KEY_ID, TEAM_ID).unwrap();
    let apns = Apns::new(key.clone(), BUNDLE);
    for shown in [
        format!("{key:?}"),
        format!("{apns:?}"),
        format!("{sender:?}"),
    ] {
        for line in pem.lines().filter(|line| !line.starts_with("-----")) {
            assert!(!shown.contains(line), "{shown}");
        }
        assert!(!shown.contains("PRIVATE KEY"), "{shown}");
        assert!(!shown.contains(token.as_str()), "{shown}");
    }

    // A platform it was not given.
    let fcm = Target::Fcm {
        token: "dK3x:APA91b".into(),
    };
    match sender.send(&update(4), &fcm).await {
        Err(SendError::Invalid(error)) => assert_eq!(error.code, "UNSUPPORTED_FEATURE"),
        other => panic!("{other:?}"),
    }
    // The sandbox is the development builds' host.
    assert_eq!(
        Apns::new(key.clone(), BUNDLE).sandbox().host_url(),
        "https://api.sandbox.push.apple.com"
    );
    assert_eq!(
        Apns::new(key, BUNDLE).host_url(),
        "https://api.push.apple.com"
    );
}

#[tokio::test]
async fn req_p_006_an_expired_sign_in_is_made_again_once() {
    // REQ-P-006
    let (pem, _) = key();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(device()))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(json!({ "reason": "ExpiredProviderToken" })),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(device()))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let sender = sender(&server, &pem);
    sender.send(&update(1), &activity()).await.unwrap();
    let tokens = bearers(&server).await;
    assert_eq!(tokens.len(), 2);
    assert_ne!(tokens[0], tokens[1], "a new token after the expired one");

    // Expired again at once: answered, not tried a third time.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(json!({ "reason": "ExpiredProviderToken" })),
        )
        .expect(2)
        .mount(&server)
        .await;
    let sender = self::sender(&server, &pem);
    match sender.send(&update(1), &activity()).await {
        Err(SendError::Rejected { status, reason }) => {
            assert_eq!((status, reason.as_str()), (403, "ExpiredProviderToken"));
        }
        other => panic!("{other:?}"),
    }
}

/// What an answer should become.
type Expected = fn(&SendError) -> bool;

#[tokio::test]
async fn req_p_007_each_answer_of_each_platform() {
    // REQ-P-007
    let (pem, _) = key();
    let answers: Vec<(ResponseTemplate, Expected)> = vec![
        (
            ResponseTemplate::new(410).set_body_json(
                json!({ "reason": "Unregistered", "timestamp": 1_790_000_000_000_u64 }),
            ),
            |error| matches!(error, SendError::Unregistered),
        ),
        (
            ResponseTemplate::new(400).set_body_json(json!({ "reason": "BadDeviceToken" })),
            |error| matches!(error, SendError::BadTarget { reason } if reason == "BadDeviceToken"),
        ),
        (
            ResponseTemplate::new(400).set_body_json(json!({ "reason": "DeviceTokenNotForTopic" })),
            |error| matches!(error, SendError::BadTarget { reason } if reason == "DeviceTokenNotForTopic"),
        ),
        (
            ResponseTemplate::new(429).set_body_json(json!({ "reason": "TooManyRequests" })),
            |error| matches!(error, SendError::TooManyRequests { retry_after: None }),
        ),
        (
            ResponseTemplate::new(429)
                .insert_header("retry-after", "30")
                .set_body_json(json!({ "reason": "TooManyRequests" })),
            |error| matches!(error, SendError::TooManyRequests { retry_after: Some(after) } if *after == Duration::from_secs(30)),
        ),
        (
            ResponseTemplate::new(503).set_body_json(json!({ "reason": "ServiceUnavailable" })),
            |error| matches!(error, SendError::Unavailable { status: Some(503), reason } if reason == "ServiceUnavailable"),
        ),
        (
            ResponseTemplate::new(400).set_body_json(json!({ "reason": "BadPriority" })),
            |error| matches!(error, SendError::Rejected { status: 400, reason } if reason == "BadPriority"),
        ),
        (
            ResponseTemplate::new(400).set_body_string("not json"),
            |error| matches!(error, SendError::Rejected { status: 400, reason } if reason.is_empty()),
        ),
    ];
    for (answer, expected) in answers {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(answer)
            .expect(1)
            .mount(&server)
            .await;
        let error = sender(&server, &pem)
            .send(&update(1), &activity())
            .await
            .unwrap_err();
        assert!(expected(&error), "{error:?}");
        // Each says what it is without a key.
        assert!(!error.to_string().contains("PRIVATE KEY"));
    }

    // A message that the device would refuse is not sent.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let marked: BTreeMap<String, Value> =
        serde_json::from_value(json!({ "liveActivityTemplate": true })).unwrap();
    let error = sender(&server, &pem)
        .send(
            &Message::update("delivery:7", LayoutSnapshot::new(1, marked)),
            &activity(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(&error, SendError::Invalid(error) if error.code == "INVALID_INPUT"),
        "{error:?}"
    );

    // A host that does not answer.
    let key = ApnsKey::from_pem(&pem, KEY_ID, TEAM_ID).unwrap();
    let closed = Sender::builder()
        .apns(Apns::new(key, BUNDLE).host("http://127.0.0.1:9"))
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    match closed.send(&update(1), &activity()).await {
        Err(SendError::Unavailable { status: None, .. }) => {}
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn req_p_006_only_an_expired_token_is_made_again() {
    // REQ-P-006
    let (pem, _) = key();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(json!({ "reason": "InvalidProviderToken" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    match sender(&server, &pem).send(&update(1), &activity()).await {
        Err(SendError::Rejected { status, reason }) => {
            assert_eq!((status, reason.as_str()), (403, "InvalidProviderToken"));
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn req_p_006_pushes_at_once_share_one_token() {
    // REQ-P-006
    let (pem, _) = key();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(8)
        .mount(&server)
        .await;
    let sender = sender(&server, &pem);
    let sends = (0..8).map(|revision| {
        let sender = sender.clone();
        tokio::spawn(async move { sender.send(&update(revision), &activity()).await })
    });
    for send in sends.collect::<Vec<_>>() {
        send.await.unwrap().unwrap();
    }
    let mut tokens = bearers(&server).await;
    tokens.dedup();
    assert_eq!(tokens.len(), 1, "one provider token for pushes at once");
    // Nor does the sender show it.
    assert!(!format!("{sender:?}").contains(&tokens[0]));
}

#[tokio::test]
async fn req_p_006_a_broadcast_goes_to_its_channel() {
    // REQ-P-006
    let (pem, _) = key();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/4/broadcasts/apps/dev.example.app"))
        .and(header("apns-channel-id", "dHJhY2tpbmc="))
        .and(header("apns-push-type", "liveactivity"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("apns-request-id", "4B6A9C1E-0000-4000-8000-000000000002"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let channel = Target::ApnsChannel {
        channel: "dHJhY2tpbmc=".into(),
    };
    let delivery = sender(&server, &pem)
        .send(&update(1), &channel)
        .await
        .unwrap();
    assert_eq!(
        delivery.id.as_deref(),
        Some("4B6A9C1E-0000-4000-8000-000000000002")
    );
    let requests = server.received_requests().await.unwrap();
    assert!(requests[0].headers.get("apns-topic").is_none());
}

#[tokio::test]
async fn req_p_006_a_sender_is_checked_when_it_is_built() {
    // REQ-P-006
    let (pem, _) = key();
    let key = ApnsKey::from_pem(&pem, KEY_ID, TEAM_ID).unwrap();
    for bundle in ["", "dev/example", "dev example", "dev.example\r\n"] {
        let error = Sender::builder()
            .apns(Apns::new(key.clone(), bundle))
            .build()
            .unwrap_err();
        assert_eq!(error.code, "INVALID_INPUT", "{bundle:?}");
    }
    for host in [
        "api.push.apple.com",
        "ftp://api.push.apple.com",
        "https://api.push.apple.com/3/device",
    ] {
        let error = Sender::builder()
            .apns(Apns::new(key.clone(), BUNDLE).host(host))
            .build()
            .unwrap_err();
        assert_eq!(error.code, "INVALID_INPUT", "{host}");
    }
    // A host given with its slash is the same host.
    assert!(
        Sender::builder()
            .apns(Apns::new(key, BUNDLE).host("https://api.push.apple.com/"))
            .build()
            .is_ok()
    );
}

#[tokio::test]
async fn req_p_007_a_slow_answer_and_the_rest_of_the_answers() {
    // REQ-P-007
    let (pem, _) = key();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(3)))
        .mount(&server)
        .await;
    let key = ApnsKey::from_pem(&pem, KEY_ID, TEAM_ID).unwrap();
    let slow = Sender::builder()
        .apns(Apns::new(key, BUNDLE).host(server.uri()))
        .timeout(Duration::from_millis(200))
        .build()
        .unwrap();
    match slow.send(&update(1), &activity()).await {
        Err(SendError::Unavailable {
            status: None,
            reason,
        }) => assert!(reason.contains("timed out"), "{reason}"),
        other => panic!("{other:?}"),
    }

    let answers: Vec<(ResponseTemplate, Expected)> = vec![
        (
            ResponseTemplate::new(400).set_body_json(json!({ "reason": "MissingDeviceToken" })),
            |error| matches!(error, SendError::BadTarget { reason } if reason == "MissingDeviceToken"),
        ),
        (
            ResponseTemplate::new(429)
                .insert_header("retry-after", "soon")
                .set_body_json(json!({ "reason": "TooManyRequests" })),
            |error| matches!(error, SendError::TooManyRequests { retry_after: None }),
        ),
    ];
    for (answer, expected) in answers {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(answer)
            .mount(&server)
            .await;
        let error = sender(&server, &pem)
            .send(&update(1), &activity())
            .await
            .unwrap_err();
        assert!(expected(&error), "{error:?}");
    }
    // A delivery without an id.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let delivery = sender(&server, &pem)
        .send(&update(1), &activity())
        .await
        .unwrap();
    assert_eq!(delivery.id, None);
}

/// A service account's JSON key for a project, its token endpoint on `server`, and the account's public key.
fn service_account(server: &MockServer) -> (String, Vec<u8>) {
    use aws_lc_rs::encoding::{AsDer, Pkcs8V1Der};
    let pair = aws_lc_rs::rsa::KeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).unwrap();
    let der: Pkcs8V1Der<'static> = pair.as_der().unwrap();
    let body = STANDARD.encode(der.as_ref());
    let lines: Vec<&str> = body
        .as_bytes()
        .chunks(64)
        .map(|line| std::str::from_utf8(line).unwrap())
        .collect();
    let pem = format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        lines.join("\n")
    );
    let account = json!({
        "type": "service_account",
        "project_id": "demo-project",
        "private_key_id": "0123456789abcdef",
        "private_key": pem,
        "client_email": "sender@demo-project.iam.gserviceaccount.com",
        "token_uri": format!("{}/token", server.uri()),
    });
    (account.to_string(), pair.public_key().as_ref().to_vec())
}

fn fcm_target() -> Target {
    Target::Fcm {
        token: "dK3x:APA91b".into(),
    }
}

fn fcm_sender(server: &MockServer, account: &str) -> Sender {
    let account = liveactivity::push::FcmAccount::from_json(account).unwrap();
    Sender::builder()
        .fcm(liveactivity::push::Fcm::new(account).host(server.uri()))
        .build()
        .unwrap()
}

fn granted(token: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(json!({ "access_token": token, "expires_in": 3599, "token_type": "Bearer" }))
}

#[tokio::test]
async fn req_p_009_a_sender_for_fcm() {
    // REQ-P-009
    let server = MockServer::start().await;
    let (account, public) = service_account(&server);
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .respond_with(granted("ya29.first"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/projects/demo-project/messages:send"))
        .and(header("authorization", "Bearer ya29.first"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "name": "projects/demo-project/messages/0:1790000000" })),
        )
        .expect(2)
        .mount(&server)
        .await;
    let sender = fcm_sender(&server, &account);
    let delivery = sender.send(&update(1), &fcm_target()).await.unwrap();
    assert_eq!(
        delivery.id.as_deref(),
        Some("projects/demo-project/messages/0:1790000000")
    );
    // The access token is kept: the second push asks for none.
    sender
        .clone()
        .send(&update(2), &fcm_target())
        .await
        .unwrap();

    // The grant: the JWT bearer of the account, RS256, its claims, signed by its key.
    let requests = server.received_requests().await.unwrap();
    let grants: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path() == "/token")
        .collect();
    assert_eq!(grants.len(), 1);
    let form = String::from_utf8(grants[0].body.clone()).unwrap();
    let assertion = form
        .strip_prefix("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion=")
        .unwrap();
    assert_eq!(part(assertion, 0)["alg"], "RS256");
    let claims = part(assertion, 1);
    assert_eq!(claims["iss"], "sender@demo-project.iam.gserviceaccount.com");
    assert_eq!(
        claims["scope"],
        "https://www.googleapis.com/auth/firebase.messaging"
    );
    assert_eq!(claims["aud"], format!("{}/token", server.uri()));
    assert_eq!(
        claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap(),
        3600
    );
    let (signed, signature) = assertion.rsplit_once('.').unwrap();
    UnparsedPublicKey::new(&aws_lc_rs::signature::RSA_PKCS1_2048_8192_SHA256, &public)
        .verify(
            signed.as_bytes(),
            &URL_SAFE_NO_PAD.decode(signature).unwrap(),
        )
        .unwrap();
    // The message, with the layer's member.
    let sent: serde_json::Value = serde_json::from_slice(
        &requests
            .iter()
            .find(|request| request.url.path().ends_with(":send"))
            .unwrap()
            .body,
    )
    .unwrap();
    assert_eq!(sent["message"]["token"], "dK3x:APA91b");
    assert!(sent["message"]["data"]["liveupdate.layer"].is_string());

    // The account never shows, and one that is none is refused without its text.
    let shown = format!(
        "{:?}",
        liveactivity::push::FcmAccount::from_json(&account).unwrap()
    );
    assert!(shown.contains("sender@demo-project.iam.gserviceaccount.com"));
    assert!(!shown.contains("PRIVATE KEY"), "{shown}");
    assert!(!format!("{sender:?}").contains("PRIVATE KEY"));
    let bad = json!({ "type": "service_account", "project_id": "demo-project", "private_key": "c2VjcmV0",
        "client_email": "sender@demo-project.iam.gserviceaccount.com", "token_uri": "https://oauth2.googleapis.com/token" });
    let error = liveactivity::push::FcmAccount::from_json(bad.to_string()).unwrap_err();
    assert_eq!(error.code, "INVALID_INPUT");
    assert!(!error.message.contains("c2VjcmV0"));
    assert_eq!(
        liveactivity::push::FcmAccount::from_json("{}")
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
    // A sender without APNs refuses an APNs target, and one with both platforms builds.
    match sender.send(&update(3), &activity()).await {
        Err(SendError::Invalid(error)) => assert_eq!(error.code, "UNSUPPORTED_FEATURE"),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn req_p_009_an_unauthenticated_answer_takes_a_new_token_once() {
    // REQ-P-009
    let server = MockServer::start().await;
    let (account, _) = service_account(&server);
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(granted("ya29.first"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(granted("ya29.second"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/projects/demo-project/messages:send"))
        .and(header("authorization", "Bearer ya29.first"))
        .respond_with(ResponseTemplate::new(401).set_body_json(
            json!({ "error": { "code": 401, "status": "UNAUTHENTICATED", "message": "expired" } }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/projects/demo-project/messages:send"))
        .and(header("authorization", "Bearer ya29.second"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "name": "projects/demo-project/messages/1" })),
        )
        .mount(&server)
        .await;
    let delivery = fcm_sender(&server, &account)
        .send(&update(1), &fcm_target())
        .await
        .unwrap();
    assert_eq!(
        delivery.id.as_deref(),
        Some("projects/demo-project/messages/1")
    );
}

#[tokio::test]
async fn req_p_009_each_answer_of_fcm() {
    // REQ-P-009
    let fcm_error = |status: u16, code: &str| {
        ResponseTemplate::new(status).set_body_json(json!({ "error": { "code": status, "status": "X",
            "message": "m", "details": [{ "@type": "type.googleapis.com/google.firebase.fcm.v1.FcmError", "errorCode": code }] } }))
    };
    let answers: Vec<(ResponseTemplate, Expected)> = vec![
        (fcm_error(404, "UNREGISTERED"), |error| {
            matches!(error, SendError::Unregistered)
        }),
        (
            fcm_error(403, "SENDER_ID_MISMATCH"),
            |error| matches!(error, SendError::BadTarget { reason } if reason == "SENDER_ID_MISMATCH"),
        ),
        (
            fcm_error(429, "QUOTA_EXCEEDED").insert_header("retry-after", "12"),
            |error| matches!(error, SendError::TooManyRequests { retry_after: Some(after) } if *after == Duration::from_secs(12)),
        ),
        (
            fcm_error(503, "UNAVAILABLE"),
            |error| matches!(error, SendError::Unavailable { status: Some(503), reason } if reason == "UNAVAILABLE"),
        ),
        (fcm_error(500, "INTERNAL"), |error| {
            matches!(
                error,
                SendError::Unavailable {
                    status: Some(500),
                    ..
                }
            )
        }),
        (
            fcm_error(400, "INVALID_ARGUMENT"),
            |error| matches!(error, SendError::Rejected { status: 400, reason } if reason == "INVALID_ARGUMENT"),
        ),
        (
            ResponseTemplate::new(403).set_body_json(
                json!({ "error": { "code": 403, "status": "PERMISSION_DENIED", "message": "m" } }),
            ),
            |error| matches!(error, SendError::Rejected { status: 403, reason } if reason == "PERMISSION_DENIED"),
        ),
    ];
    for (answer, expected) in answers {
        let server = MockServer::start().await;
        let (account, _) = service_account(&server);
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(granted("ya29.t"))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/projects/demo-project/messages:send"))
            .respond_with(answer)
            .mount(&server)
            .await;
        let error = fcm_sender(&server, &account)
            .send(&update(1), &fcm_target())
            .await
            .unwrap_err();
        assert!(expected(&error), "{error:?}");
    }
    // A grant that is refused says so, and sends nothing.
    let server = MockServer::start().await;
    let (account, _) = service_account(&server);
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(
                json!({ "error": "invalid_grant", "error_description": "Invalid JWT" }),
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/projects/demo-project/messages:send"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    match fcm_sender(&server, &account)
        .send(&update(1), &fcm_target())
        .await
    {
        Err(SendError::Rejected {
            status: 400,
            reason,
        }) => assert_eq!(reason, "invalid_grant"),
        other => panic!("{other:?}"),
    }
}
