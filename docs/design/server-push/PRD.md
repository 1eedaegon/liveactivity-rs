# Pushes from the application's server, on both platforms: product requirements

## Problem

An activity often changes where the application does not run: a server knows that the courier turned the corner.
The engines have a part of a push each, in two shapes, and nothing joins them.

| | activitykit (iOS) | liveupdate (Android) |
| --- | --- | --- |
| What there is | `activitykit::push::Payload`: the JSON of a start, an update or an end for APNs, checked as the kit checks the call; `topic`, `PUSH_TYPE`, the priorities, 4096 bytes | Nothing yet. Its push path (a data message that the application's messaging service hands to the AAR, with the Rust lifecycle applying it) is proposed to liveupdate and being designed there |
| Where a push goes | A token of the activity's own, the app's push-to-start token (iOS 17.2) or a broadcast channel (iOS 18) | The installation's FCM registration token, the activity named by its `activity_key` in the data |
| How the server signs in | A provider token: an ES256 JWT of the team's `.p8` key, refreshed between 20 and 60 minutes | An OAuth access token for a service account's key (an RS256 JWT exchanged at Google), valid an hour |
| Sending | Left to the server: HTTP/2 to `api.push.apple.com` or the sandbox, with `apns-push-type`, `apns-topic`, `apns-priority` | FCM's HTTP v1 `projects/<id>/messages:send` |
| What a refusal says | `410 Unregistered`, `400 BadDeviceToken`, `429 TooManyRequests`, … | `404 UNREGISTERED`, `400 INVALID_ARGUMENT`, `429 QUOTA_EXCEEDED`, … |

A server that serves both platforms writes the same message twice, in two formats, signs in twice, and reads two
kinds of refusal, with checks that the engines already have and that it does not reach. The application learns what
a push did from iOS's `Event::Updated`, and from nothing on Android.

## Goal

One message, built from this crate's own contract types, that a server sends to any target of an activity: the
crate builds the request of each platform from it and checks it as the device would, and, with the feature
`server`, sends it and says in one vocabulary what came back. An application without the feature carries none of
it.

## Features

| Feature | What it gives | What it adds |
| --- | --- | --- |
| `app` (default) | `LiveActivity`, the backends, the events: all of the crate today | nothing new |
| always | `liveactivity::push`: `Message`, `Target`, the requests of each platform, their checks | nothing: serde and the two engines, as today |
| `server` | `liveactivity::push::Sender`: sign-in, HTTP/2, the answers | `reqwest` (HTTP/2, rustls) and `jsonwebtoken` (aws-lc-rs), both optional; no runtime of its own, it runs on the one `reqwest` brings |

```toml
liveactivity = "0.13"                                                                  # an application
liveactivity = { version = "0.13", default-features = false, features = ["server"] }  # a server
liveactivity = { version = "0.13", default-features = false }                         # shared types, wasm
```

`server` holds the sender only. A web handler (registration of targets, authentication, storage) is the
service's, and the example `push_server` shows one with axum.

## The interface

```rust
use liveactivity::push::{Application, Apns, ApnsKey, Message, Priority, SendError, Sender, Target};

// Shared by both platforms: the contract's own snapshot, named by the activity's key.
let message = Message::update("delivery:7", LayoutSnapshot::new(revision, values))
    .alert(Alert::new("On its way", "2 minutes"))
    .priority(Priority::Now);

// What the device reported, and the application sent to its server (serde, the registration's body).
let target: Target = serde_json::from_slice(&body)?;   // {"type":"apnsActivity","token":"0a1b…"}, "apnsPushToStart", "apnsChannel", "fcm"

// Without `server`: the request alone, for a server that sends with its own client.
let request = message.request(&target, &Application::new().bundle_id(bundle_id))?;   // path, headers, body; 4096 bytes checked

// With `server`: one sender for both, each platform chosen by the builder.
let sender = Sender::builder()
    .apns(Apns::new(ApnsKey::from_pem(&p8, key_id, team_id)?, bundle_id)) // .sandbox() for development builds
    .build()?;                       // at least one platform; FCM joins with liveupdate's push path
match sender.send(&message, &target).await {
    Ok(delivery) => { /* delivery.id(): apns-id or the FCM message's name */ }
    Err(SendError::Unregistered) => { /* delete the target */ }
    Err(SendError::TooManyRequests { retry_after }) => { /* wait */ }
    Err(other) => { /* log it */ }
}
```

| Message | iOS | Android |
| --- | --- | --- |
| `Message::start(id, LayoutStartInput)` with `.alert(..)` | to the push-to-start token (iOS 17.2); the alert is required, as activitykit says | to the FCM token; the activity is started by liveupdate's push path |
| `Message::update(key, LayoutSnapshot)` | to the activity's token or channel | to the FCM token with the key |
| `Message::end(key)`, `.snapshot(..)`, `.policy(EndPolicy)` | `dismiss_at` from the policy | the policy as it is |
| `.priority(Priority::Now \| Priority::Low)` | `apns-priority` 10 or 5 | FCM `high` or `normal` |
| `.alert`, a snapshot's `stale_at`, `relevance` | carried | carried where Android has them; `relevance` is iOS's alone and left out |
| A snapshot's `android` content | left out | carried |

## Requirements of this change

| Id | Requirement |
| --- | --- |
| REQ-P-001 | The feature `app` is the default and holds all of today's API; `liveactivity::push` is there with any features; `server` adds the sender and its two dependencies, which no build without it resolves. |
| REQ-P-002 | `Message` is built from the contract's types, named by the activity key, and checked by `request` as the backend checks the call it stands for: what the device would refuse is `INVALID_INPUT` on the server, and a request of more than 4096 bytes is refused before it is sent. |
| REQ-P-003 | `Target` is the JSON that an application sends to its server, serde both ways, `#[non_exhaustive]`: an activity's APNs token, the push-to-start token, a broadcast channel, an FCM token. A message that a target cannot take (a start to an activity's token, an update to the push-to-start token) is `INVALID_INPUT`. |
| REQ-P-004 | The APNs request: `POST /3/device/<token>`, or APNs's broadcast request with the channel's `apns-channel-id`; `apns-push-type: liveactivity`, `apns-topic: <bundle>.push-type.liveactivity`, `apns-priority`, the body by activitykit's `Payload`. |
| REQ-P-005 | The FCM request: HTTP v1's message with the token, `android.priority` and the data member that liveupdate's push path reads. Until liveupdate publishes that path, a message to an FCM target is `UNSUPPORTED_FEATURE`. |
| REQ-P-006 | `Sender::builder()` takes APNs and FCM each optionally and refuses to build with neither; a target of a platform the sender was not given is `UNSUPPORTED_FEATURE`. It signs in once and again before a token runs out (APNs between 20 and 60 minutes, FCM before its hour), shared by clones. |
| REQ-P-007 | `send` answers one vocabulary for both: `Unregistered` (APNs 410, FCM `UNREGISTERED`), `BadTarget`, `TooManyRequests { retry_after }`, `Rejected { status, reason }`, `Unavailable`, `Invalid` (the message's own error). A key never appears in `Debug`, `Display` or an error. |
| REQ-P-008 | The crate has no web handler and reads no environment variable: keys come in as text or bytes. `examples/push-server` (axum) registers targets and sends, keys from the environment. |

## Decisions

- **One message, a request per platform.** The message says what the activity becomes; the platform decides the
  address and the envelope. A server writes its domain once, and a platform that comes later is one more `Target`.
- **The activity's key, not its id.** The id is made on the device; the key is the server's own name for the
  activity. APNs addresses a token and carries no key; FCM carries it in the data.
- **The server owns the revisions** of an activity it pushes to. The device takes a revision above the one it shows
  and drops the rest, on both platforms, so an application that also updates the activity itself takes its
  revisions from the server.
- **A builder for the sender, each platform optional.** A service that has only iOS users gives only the APNs key.
  Both platforms need the same two dependencies, so there is no feature per platform: what a server does not use
  it does not configure, and pays one HTTP client for it.
- **The request without the sender.** `Message::request` answers the path, the headers and the body, `POST`, so that a server
  with its own client, a queue worker or another runtime sends it; the sender is one way to send it.
- **The sender takes a client.** `.client(reqwest::Client)` uses the service's own; otherwise the builder makes one
  with HTTP/2. `.apns_host` and `.fcm_host` point it at a test server.
- **`app` as a default feature.** Today's API moves under it, unchanged for a caller with default features. A caller
  that wrote `default-features = false` loses it: the change is a minor version with its line in the changelog.
  REQ-N-001 then says that the crate's own dependencies are the engines, serialization and errors, and `server`
  adds two.

- **No runtime of its own.** The sender's lock is the standard library's and it waits for nothing but its HTTP
  client; tokio comes with `reqwest`. The tests and the example use it as dev-dependencies.
- **One crypto provider.** jsonwebtoken and rustls both sign with aws-lc-rs. jsonwebtoken panics where a process has
  no provider of its own and both of its providers are on, as when a service's own jsonwebtoken turns on the other:
  the sender installs aws-lc-rs where none is installed, and takes the service's where one is. Pure Rust RSA, which
  FCM's sign-in would need, has an open timing advisory; aws-lc-rs has none.

## Order

1. The features, `push::Message` and `Target`, the APNs request, the sender for APNs, the example, with a test
   server in the tests and the user's APNs key on the user's iPhone at the end.
2. FCM, when liveupdate publishes its push path: the request, the sender's FCM part, and Android's event of what a
   push did mapped to `Event::Updated` and `Event::Started`.

## Not in this change

- A web handler, a store of targets, a queue, retries: the service's.
- APNs broadcast channels made or listed by the server (APNs's channel management API), FCM topics.
- A target that the device reports as one event: the application-side `PushTarget` follows FCM.
- Notifications that are not Live Activities or Live Updates.
