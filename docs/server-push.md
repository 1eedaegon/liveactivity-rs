# Pushes from a server

A server starts, updates and ends an application's activities while the application does not run. The device
reports where pushes go; the application sends that to its server; the server makes one message of this crate's
own types and sends it. On iOS the system draws the content without the application. Android's pushes wait for
liveupdate's push path: a message to an FCM target is `UNSUPPORTED_FEATURE` until then.

```toml
# The server: the sender, without the application's side.
liveactivity = { version = "0.13", default-features = false, features = ["server"] }
# A crate that shares the types with the application or a page: no I/O, no new dependency.
liveactivity = { version = "0.13", default-features = false }
```

## 1. What the server needs

| What | Where | A secret |
| --- | --- | --- |
| An APNs key (`AuthKey_<id>.p8`), its key id and the team id | the Apple Developer account, Keys | **the key is**: on the server only, from a file or a secret manager, never in the repository or a log |
| The application's bundle id | the app target | no |
| Sandbox or production | a development build gets sandbox tokens; App Store and TestFlight builds production ones | no |

The application needs the Push Notifications capability ([docs/ios-setup.md](ios-setup.md#7-pushes)). A key may be
limited to some topics and to the sandbox in the developer account: APNs then refuses the others.

## 2. The application sends its targets

The device reports a token as an event; the application sends it to its server, which keeps it beside the
activity's key. `push::Target` is that JSON, read with serde on the server.

| Event | Target | For |
| --- | --- | --- |
| `Event::PushToken` (`activity_key`, `token`), when a start asked for `LayoutPush::Token` | `{"type":"apnsActivity","token":"0a1b…"}` | the updates and the end of that activity |
| `Event::PushToStartToken` (`token`), with `push_to_start(true)`, at each launch and when it changes | `{"type":"apnsPushToStart","token":"…"}` | starts (iOS 17.2) |
| a channel the server made at APNs, given to the start as `LayoutPush::Channel` | `{"type":"apnsChannel","channel":"…"}` | the updates and ends of every activity on it (iOS 18) |
| the FCM registration token | `{"type":"fcm","token":"…"}` | Android, when liveupdate has its push path |

```json
POST /v1/push-targets
{ "activityKey": "delivery:7", "target": { "type": "apnsActivity", "token": "0a1b…" } }
```

The registration's endpoint, its authentication and the store are the service's: the crate has no web handler.

## 3. The server makes a message

```rust
use liveactivity::push::{Message, Priority};
use liveactivity::{Alert, EndPolicy, LayoutSnapshot, LayoutStartInput};

let update = Message::update("delivery:7", LayoutSnapshot::new(revision, values))
    .alert(Alert::new("Delivery", "2 minutes"));
let quiet = Message::update("delivery:7", LayoutSnapshot::new(revision, values)).priority(Priority::Low);
let end = Message::end("delivery:7")
    .snapshot(LayoutSnapshot::new(revision, last_values))
    .policy(EndPolicy::After { at: in_ten_minutes });
let start = Message::start(uuid, LayoutStartInput::new("delivery:7", layout, LayoutSnapshot::new(0, values)))
    .alert(Alert::new("Delivery", "On its way"));
```

| Message | To | Rules |
| --- | --- | --- |
| `start(id, input)` | `apnsPushToStart` | an alert is required; `id` is new for every start and is what the activity's handle says; a scheduled or transient start is refused |
| `update(key, snapshot)` | `apnsActivity`, `apnsChannel` | the whole content at the next revision |
| `end(key)` | `apnsActivity`, `apnsChannel` | `.snapshot` the last content, `.policy` when it leaves the screen |

- **The server owns the revisions** of an activity it pushes to. The device takes a revision above the one it
  shows and drops the rest; an application that also updates the activity takes its revisions from the server.
- **The message is checked as the device checks the call**: what the device would refuse is `INVALID_INPUT` on the
  server, more than 4096 bytes is `PAYLOAD_TOO_LARGE`, and a message to a target that does not take it is
  `INVALID_INPUT`.
- **`Priority::Now`**, the default, shows the push at once and counts toward the application's budget of pushes;
  `Priority::Low` may wait.

## 4. The server sends it

```rust
use liveactivity::push::{Apns, ApnsKey, SendError, Sender};

let key = ApnsKey::from_pem(std::fs::read(key_path)?, key_id, team_id)?;
let sender = Sender::builder()
    .apns(Apns::new(key, "dev.example.app").sandbox()) // production without .sandbox()
    .timeout(std::time::Duration::from_secs(10))
    .build()?;

match sender.send(&update, &target).await {
    Ok(delivery) => { /* delivery.id: APNs's apns-id */ }
    Err(SendError::Unregistered) => { /* forget the target */ }
    Err(SendError::BadTarget { reason }) => { /* check the host and the topic: see below */ }
    Err(SendError::TooManyRequests { retry_after }) => { /* wait, then send again */ }
    Err(SendError::Unavailable { .. }) => { /* send again later */ }
    Err(SendError::Invalid(error)) => { /* the message is wrong: nothing was sent */ }
    Err(error) => { /* SendError::Rejected { status, reason }: log it */ }
}
```

The sender signs a provider token (ES256) once and again after 50 minutes, and once more when APNs says that it
expired; its clones share the token and the HTTP client. It takes the service's own client with `.client(..)`,
of the version `liveactivity::push::reqwest` names. It runs on the runtime its client brings: tokio. `Debug` of a
key, of `Apns` and of the sender names the ids alone, and no error holds the key.

| `SendError` | APNs said | Do |
| --- | --- | --- |
| `Unregistered` | 410 | forget the target |
| `BadTarget { reason }` | 400 `BadDeviceToken`, `DeviceTokenNotForTopic`, `MissingDeviceToken` | check the host and the topic first: APNs refuses a development build's token in production the same way |
| `TooManyRequests { retry_after }` | 429 | wait |
| `Unavailable { status, reason }` | 5xx, or no answer | send again later |
| `Rejected { status, reason }` | anything else | log it: the reason is APNs's |
| `Invalid(error)` | nothing: not sent | fix the message or the target |

## 5. Without the sender

`message.request(&target, &application)` answers the request alone: `POST` to `request.path` on
`https://api.push.apple.com` (or `api.sandbox.push.apple.com`), with `request.headers`, the provider token as
`authorization: bearer <jwt>`, and `request.body`. A server that has its own HTTP client, a queue worker, or a
language other than Rust takes it from there.

```rust
use liveactivity::push::Application;

let request = update.request(&target, &Application::new().bundle_id("dev.example.app"))?;
```

## 6. An example

The example `push_server` is a server with axum: it registers targets and starts, updates and ends activities
by their key, with the store in memory and the key from the environment.

```sh
APNS_KEY=/path/to/AuthKey_ABC123DEFG.p8 APNS_KEY_ID=ABC123DEFG APNS_TEAM_ID=TEAM456789 \
APNS_BUNDLE_ID=dev.example.app APNS_SANDBOX=1 \
  cargo run -p liveactivity --features server --example push_server
```
