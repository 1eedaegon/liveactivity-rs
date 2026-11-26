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
