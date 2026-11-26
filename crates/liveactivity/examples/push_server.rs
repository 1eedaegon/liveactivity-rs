//! A server's side of pushes, with axum: an application registers the targets its device reported, and the server
//! starts, updates and ends the activity by its key. The store is in memory, and there is no sign-in: a service
//! puts its own authentication and storage around the two handlers.
//!
//! ```sh
//! APNS_KEY=/path/to/AuthKey_ABC123DEFG.p8 APNS_KEY_ID=ABC123DEFG APNS_TEAM_ID=TEAM456789 \
//! APNS_BUNDLE_ID=dev.example.app APNS_SANDBOX=1 \
//! FCM_SERVICE_ACCOUNT=/path/to/service-account.json \
//!   cargo run -p liveactivity --features server --example push_server
//!
//! # Either platform alone works: leave out the other's variables.
//! # What the application sends when its device reports a token, an activity's on iOS, the installation's on
//! # Android (`{"target":{"type":"fcm","token":"dK3x:APA91b…"}}`, under no key):
//! curl -X POST localhost:8080/v1/push-targets -H 'content-type: application/json' \
//!   -d '{"activityKey":"delivery:7","target":{"type":"apnsActivity","token":"0a1b…"}}'
//! # A change on the server's side:
//! curl -X POST localhost:8080/v1/activities/delivery:7/update -H 'content-type: application/json' \
//!   -d '{"values":{"eta":"2 min"},"alert":{"title":"Delivery","body":"2 minutes"}}'
//! ```

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};
use liveactivity::activitykit::layout::{text, vstack};
use liveactivity::push::{Apns, ApnsKey, Fcm, FcmAccount, Message, SendError, Sender, Target};
use liveactivity::{
    Alert, Bound, EndPolicy, Island, IslandExpanded, Layout, LayoutSnapshot, LayoutStartInput,
    Value,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    sync::{Arc, Mutex, PoisonError},
    time::SystemTime,
};

/// What the application sends its server: the target, and the activity it belongs to. A push-to-start token
/// belongs to no activity.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Registration {
    activity_key: Option<String>,
    target: Target,
}

/// A change of the server's: the values, and an alert when the person should hear of it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Change {
    values: BTreeMap<String, Value>,
    alert: Option<Alert>,
}

#[derive(Default)]
struct Store {
    /// The targets of each activity key; those of an installation, push-to-start and FCM, under no key.
    targets: HashMap<Option<String>, Vec<Target>>,
    /// The last revision the server sent of each activity: the server owns them.
    revisions: HashMap<String, u64>,
}

#[derive(Clone)]
struct App {
    sender: Sender,
    store: Arc<Mutex<Store>>,
}

impl App {
    fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sends `message` to every target under `key`, and to the installations on Android, whose one FCM token is
    /// for all their activities; forgets those that are gone.
    async fn push(
        &self,
        key: Option<String>,
        message: Message,
    ) -> (StatusCode, Json<serde_json::Value>) {
        let targets: Vec<Target> = {
            let store = self.store();
            let own = store.targets.get(&key).cloned().unwrap_or_default();
            let installations = store.targets.get(&None).cloned().unwrap_or_default();
            let android = installations
                .into_iter()
                .filter(|target| key.is_some() && matches!(target, Target::Fcm { .. }));
            own.into_iter().chain(android).collect()
        };
        let (mut sent, mut failed, mut gone) = (0, Vec::new(), Vec::new());
        for target in targets {
            match self.sender.send(&message, &target).await {
                Ok(_) => sent += 1,
                // The target is gone. `BadTarget` is not: a development token sent to production is refused the same way.
                Err(SendError::Unregistered) => gone.push(target),
                Err(error) => failed.push(error.to_string()),
            }
        }
        for kept in self.store().targets.values_mut() {
            kept.retain(|target| !gone.contains(target));
        }
        let status = if failed.is_empty() {
            StatusCode::OK
        } else {
            StatusCode::BAD_GATEWAY
        };
        (
            status,
            Json(json!({ "sent": sent, "forgotten": gone.len(), "failed": failed })),
        )
    }

    fn next_revision(&self, key: &str) -> u64 {
        let mut store = self.store();
        let revision = store.revisions.entry(key.to_owned()).or_insert(0);
        *revision += 1;
        *revision
    }
}

async fn register(State(app): State<App>, Json(registration): Json<Registration>) -> StatusCode {
    let mut store = app.store();
    let targets = store.targets.entry(registration.activity_key).or_default();
    if !targets.contains(&registration.target) {
        targets.push(registration.target);
    }
    StatusCode::NO_CONTENT
}

async fn start(
    State(app): State<App>,
    Path(key): Path<String>,
    Json(change): Json<Change>,
) -> (StatusCode, Json<serde_json::Value>) {
    let layout = Layout::new(vstack([text(Bound::key("eta"))]))
        .island(Island::new().expanded(IslandExpanded::new().center(text(Bound::key("eta")))));
    let input = LayoutStartInput::new(key.as_str(), layout, LayoutSnapshot::new(0, change.values));
    app.store().revisions.insert(key.clone(), 0);
    // A new id for every start: the activity's handle on iOS says it.
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let message = Message::start(format!("{key}-{nanos}"), input).alert(
        change
            .alert
            .unwrap_or_else(|| Alert::new("Started", key.as_str())),
    );
    app.push(None, message).await
}

async fn update(
    State(app): State<App>,
    Path(key): Path<String>,
    Json(change): Json<Change>,
) -> (StatusCode, Json<serde_json::Value>) {
    let snapshot = LayoutSnapshot::new(app.next_revision(&key), change.values);
    let message = Message::update(key.as_str(), snapshot);
    let message = match change.alert {
        Some(alert) => message.alert(alert),
        None => message,
    };
    app.push(Some(key), message).await
}

async fn end(
    State(app): State<App>,
    Path(key): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let message = Message::end(key.as_str()).policy(EndPolicy::Immediate {});
    let answer = app.push(Some(key.clone()), message).await;
    // The targets stay until every end was delivered: a push that failed is sent again.
    if answer.0 == StatusCode::OK {
        app.store().targets.remove(&Some(key));
    }
    answer
}

/// An environment variable, or an error that names it.
fn variable(name: &str) -> Result<String, Box<dyn Error>> {
    std::env::var(name).map_err(|_| format!("set {name}").into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // The keys' text stays in this process: each is read from its file and never printed.
    let mut sender = Sender::builder();
    if let Ok(path) = std::env::var("APNS_KEY") {
        let key = ApnsKey::from_pem(
            std::fs::read(path)?,
            variable("APNS_KEY_ID")?,
            variable("APNS_TEAM_ID")?,
        )?;
        let apns = Apns::new(key, variable("APNS_BUNDLE_ID")?);
        let sandbox = std::env::var("APNS_SANDBOX").is_ok_and(|value| value == "1");
        sender = sender.apns(if sandbox { apns.sandbox() } else { apns });
    }
    if let Ok(path) = std::env::var("FCM_SERVICE_ACCOUNT") {
        sender = sender.fcm(Fcm::new(FcmAccount::from_json(std::fs::read(path)?)?));
    }
    let app = App {
        sender: sender.build()?,
        store: Arc::default(),
    };
    let router = Router::new()
        .route("/v1/push-targets", post(register))
        .route("/v1/activities/{key}/start", post(start))
        .route("/v1/activities/{key}/update", post(update))
        .route("/v1/activities/{key}/end", post(end))
        .with_state(app);
    let address = std::env::var("ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("pushes on http://{address}");
    axum::serve(listener, router).await?;
    Ok(())
}
