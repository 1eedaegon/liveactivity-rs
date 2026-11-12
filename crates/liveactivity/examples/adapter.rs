//! A whole adapter of a framework: what a framework plugin is, on the supporting side of the builder.
//!
//! A framework gives a plugin three things, and the adapter has one part for each:
//!
//! 1. commands that arrive by name with a JSON payload: [`Plugin::command`];
//! 2. an event API that sends JSON to the application: [`sink`];
//! 3. sometimes a native side of its own, here Android's permission dialog: [`WithPermissionDialog`].
//!
//! `Framework` and `Dialog` stand for the framework's own types. The example runs anywhere, on a backend without a
//! platform:
//!
//! ```sh
//! cargo run -p liveactivity --example adapter --features testkit
//! ```

use liveactivity::{
    ActivityRecord, BackendContext, BackendFuture, Capabilities, EndInput, Error, Event,
    LayoutActivityRecord, LayoutEndInput, LayoutStartInput, LayoutUpdateInput, LiveActivity,
    LiveActivityBackend, PermissionKind, Permissions, PushTokens, Result, StartInput, UpdateInput,
    parse_request,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

/// The framework's event API, as the adapter needs it.
trait Framework: Send + Sync + 'static {
    /// Sends `payload` to the application's listeners of `event`.
    fn emit(&self, event: &str, payload: Value);
}

/// Events: every event of the contract goes out under a name of the plugin's choosing, as JSON.
///
/// On iOS a tap keeps the app awake until this returns, so the sink hands the event over and does no work of its
/// own.
fn sink(framework: Arc<dyn Framework>) -> impl Fn(Event) + Send + Sync + 'static {
    move |event| {
        let (name, payload) = match event {
            Event::Action(tap) => ("live-activity://action", serde_json::to_value(tap)),
            Event::PushToken(token) => ("live-activity://push-token", serde_json::to_value(token)),
            Event::PushToStartToken(token) => (
                "live-activity://push-to-start-token",
                serde_json::to_value(token),
            ),
            Event::Updated(updated) => ("live-activity://updated", serde_json::to_value(updated)),
            // A kind of a later version: nothing to forward yet.
            _ => return,
        };
        if let Ok(payload) = payload {
            framework.emit(name, payload);
        }
    }
}

/// A native side of the framework's own: the system's permission dialog needs an Activity, which a framework
/// has and the default backend has not.
trait Dialog: Send + Sync + 'static {
    /// Shows the system's dialog for `kind` and waits for the answer.
    fn ask(&self, kind: PermissionKind) -> Result<()>;
}

/// A backend that asks through the framework's dialog and hands every other call to `inner`, which is a default
/// backend or any other.
struct WithPermissionDialog<B, D> {
    inner: B,
    dialog: D,
}

impl<B: LiveActivityBackend, D: Dialog> LiveActivityBackend for WithPermissionDialog<B, D> {
    fn attach(&self, events: BackendContext) -> Result<()> {
        self.inner.attach(events)
    }

    fn request_permissions(&self, kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        Box::pin(async move {
            self.dialog.ask(kind)?;
            self.inner.permissions().await
        })
    }

    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        self.inner.capabilities()
    }

    fn permissions(&self) -> BackendFuture<'_, Permissions> {
        self.inner.permissions()
    }

    fn start(&self, input: StartInput) -> BackendFuture<'_, ActivityRecord> {
        self.inner.start(input)
    }

    fn update(&self, input: UpdateInput) -> BackendFuture<'_, ActivityRecord> {
        self.inner.update(input)
    }

    fn end(&self, input: EndInput) -> BackendFuture<'_, ActivityRecord> {
        self.inner.end(input)
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        self.inner.list()
    }

    fn start_layout(&self, input: LayoutStartInput) -> BackendFuture<'_, LayoutActivityRecord> {
        self.inner.start_layout(input)
    }

    fn update_layout(&self, input: LayoutUpdateInput) -> BackendFuture<'_, LayoutActivityRecord> {
        self.inner.update_layout(input)
    }

    fn end_layout(&self, input: LayoutEndInput) -> BackendFuture<'_, LayoutActivityRecord> {
        self.inner.end_layout(input)
    }

    fn list_layout(&self) -> BackendFuture<'_, Vec<LayoutActivityRecord>> {
        self.inner.list_layout()
    }

    fn write_image(&self, name: String, png: Vec<u8>) -> BackendFuture<'_, ()> {
        self.inner.write_image(name, png)
    }

    fn push_tokens(&self) -> BackendFuture<'_, PushTokens> {
        self.inner.push_tokens()
    }

    fn sync_events(&self) -> BackendFuture<'_, ()> {
        self.inner.sync_events()
    }
}

/// The payload of `request_permissions`.
#[derive(Deserialize)]
struct PermissionRequest {
    kind: PermissionKind,
}

/// The payload of `write_image`.
#[derive(Deserialize)]
struct ImageRequest {
    name: String,
    png: Vec<u8>,
}

fn payload<T: for<'de> Deserialize<'de>>(input: Value) -> Result<T> {
    serde_json::from_value(input).map_err(|_| Error::invalid())
}

fn answer<T: Serialize>(value: T) -> Result<Value> {
    serde_json::to_value(value).map_err(|_| Error::native())
}

/// The plugin as the framework holds it, in its application state.
struct Plugin {
    live: LiveActivity,
}

impl Plugin {
    /// What the plugin does when the framework sets it up: it names the sink, and a backend only when it has one.
    /// With the default backend this is `LiveActivity::builder().event_sink(sink(framework)).build()`.
    fn setup(framework: Arc<dyn Framework>, backend: impl LiveActivityBackend) -> Result<Self> {
        let live = LiveActivity::builder()
            .backend(backend)
            .event_sink(sink(framework))
            .build()?;
        Ok(Self { live })
    }

    /// Commands: each parses its payload and awaits the call of the same name. `parse_request` checks the request
    /// against the contract, so a wrong one is `INVALID_INPUT` before any platform is asked.
    async fn command(&self, name: &str, input: Value) -> Result<Value> {
        let live = &self.live;
        match name {
            "capabilities" => answer(live.capabilities().await?),
            "permissions" => answer(live.permissions().await?),
            "request_permissions" => {
                let request: PermissionRequest = payload(input)?;
                answer(live.request_permissions(request.kind).await?)
            }
            "start" => answer(live.start(parse_request(name, input)?).await?),
            "update" => answer(live.update(parse_request(name, input)?).await?),
            "end" => answer(live.end(parse_request(name, input)?).await?),
            "list" => answer(live.list().await?),
            "start_layout" => answer(live.start_layout(parse_request(name, input)?).await?),
            "update_layout" => answer(live.update_layout(parse_request(name, input)?).await?),
            "end_layout" => answer(live.end_layout(parse_request(name, input)?).await?),
            "list_layout" => answer(live.list_layout().await?),
            "write_image" => {
                let image: ImageRequest = payload(input)?;
                answer(live.write_image(&image.name, &image.png).await?)
            }
            "push_tokens" => answer(live.push_tokens().await?),
            "sync_events" => answer(live.sync_events().await?),
            _ => Err(Error::invalid()),
        }
    }

    /// The framework's resume hook. Android stores taps while the application does not run; this delivers them.
    /// On iOS it does nothing.
    async fn resumed(&self) -> Result<()> {
        self.live.sync_events().await
    }
}

/// A framework for the example: it prints what it would send to the application.
struct Printing;

impl Framework for Printing {
    fn emit(&self, event: &str, payload: Value) {
        println!("event {event} {payload}");
    }
}

/// A dialog for the example: the person always agrees.
struct Agreeing;

impl Dialog for Agreeing {
    fn ask(&self, kind: PermissionKind) -> Result<()> {
        println!("dialog for {kind:?}");
        Ok(())
    }
}

fn main() -> Result<()> {
    // The backend is named in this one place. On a device it is the platform's default, or a default one inside
    // the framework's own, as here.
    let memory = Arc::new(liveactivity::testkit::MemoryBackend::new());
    let backend = WithPermissionDialog {
        inner: memory.clone(),
        dialog: Agreeing,
    };
    let plugin = Plugin::setup(Arc::new(Printing), backend)?;

    liveactivity::block_on(async {
        plugin.resumed().await?;
        let asked = plugin
            .command("request_permissions", json!({ "kind": "notifications" }))
            .await?;
        println!("permissions {asked}");

        // What the application's side of the framework sends: the contract's JSON.
        let started = plugin
            .command(
                "start",
                json!({
                    "activityKey": "order:42",
                    "schemaVersion": 1,
                    "template": "progress",
                    "fallback": "notification",
                    "snapshot": { "revision": 0, "title": "Order 42", "status": "Preparing", "progress": 0.1 },
                }),
            )
            .await?;
        println!("started {started}");
        let id = started["id"].as_str().unwrap_or_default().to_owned();

        // A request outside the contract never reaches the backend.
        let refused = plugin
            .command("update", json!({ "id": id, "snapshot": {} }))
            .await;
        println!("refused {:?}", refused.map_err(|error| error.code));

        // A tap on the platform comes out of the framework's event API.
        memory.tap(&id, "track")?;

        let ended = plugin
            .command(
                "end",
                json!({ "id": id, "policy": { "kind": "immediate" } }),
            )
            .await?;
        println!("ended {}", ended["state"]);
        Ok(())
    })
}
