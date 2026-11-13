# Framework integration

This is for the author of a framework plugin or adapter: code that carries `liveactivity` over a
framework's own commands and events. A plugin carries the contract and adds nothing to it.

[`examples/adapter.rs`](../crates/liveactivity/examples/adapter.rs) is a whole adapter in one file.
It runs anywhere:

```sh
cargo run -p liveactivity --example adapter --features testkit
```

A framework gives a plugin three things, and the adapter has one part for each:

1. commands that arrive by name with a JSON payload;
2. an event API that sends JSON to the application;
3. sometimes a native side of its own.

## The types

Re-export `liveactivity::contract::*` as the plugin's types. The requests and records are serde
types with camelCase JSON, ready for the framework's IPC. `Error` serializes as
`{ "code", "message" }`. The JSON shapes are in [the contract](contract.md).

## Part 1: commands

Each command parses its payload and awaits the call of the same name. `parse_request` reads the
JSON into the typed input and checks it against the contract, so a wrong request is `INVALID_INPUT`
before any platform is asked.

```rust
use liveactivity::{Error, LiveActivity, PermissionKind, Result, parse_request};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
struct PermissionRequest {
    kind: PermissionKind,
}

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

struct Plugin {
    live: LiveActivity,
}

impl Plugin {
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
}
```

Use `parse_request` rather than `serde_json::from_value` for the requests. For the three layout
commands it applies the strict check of the contract first: an unknown member or a `null` anywhere
in the request is refused.

## Part 2: events

Give the builder a sink that forwards each event to the framework's event API. The names are the
plugin's choice.

```rust
use liveactivity::Event;
use serde_json::Value;
use std::sync::Arc;

/// The framework's event API, as the adapter needs it.
trait Framework: Send + Sync + 'static {
    fn emit(&self, event: &str, payload: Value);
}

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
            Event::Started(started) => ("live-activity://started", serde_json::to_value(started)),
            Event::State(state) => ("live-activity://state", serde_json::to_value(state)),
            Event::Permissions(permissions) => (
                "live-activity://permissions",
                serde_json::to_value(permissions),
            ),
            // A kind of a later version: nothing to forward yet.
            _ => return,
        };
        if let Ok(payload) = payload {
            framework.emit(name, payload);
        }
    }
}
```

`Event` is `#[non_exhaustive]`, so the `_` arm is required. Any closure
`Fn(Event) + Send + Sync + 'static` is an `EventSink`; a type can implement the trait instead.

A page that subscribes late has missed the push tokens. `push_tokens` answers the ones the backend
holds, so expose it as a command and call it after subscribing.

## Part 3: a native side of the framework's own

Name a backend only when the framework has a native side of its own. There are two cases.

**Only the transport differs.** The framework reaches Android through its own native call. Wrap
that call as a `liveupdate::Bridge` and keep the default backend:

```rust
let backend = LiveUpdateBackend::over_bridge(my_bridge).with_files(files);
```

**The framework does more than the default.** Android's permission dialog needs an Activity, which
a framework has and the default backend has not. The framework's backend answers that part and
hands every other call on:

```rust
use liveactivity::{
    ActivityRecord, BackendContext, BackendFuture, Capabilities, EndInput, LayoutActivityRecord,
    LayoutEndInput, LayoutStartInput, LayoutUpdateInput, LiveActivityBackend, PermissionKind,
    Permissions, PushTokens, Result, StartInput, UpdateInput,
};

/// The framework's dialog, as the adapter needs it.
trait Dialog: Send + Sync + 'static {
    fn ask(&self, kind: PermissionKind) -> Result<()>;
}

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
```

### Rules of composing

- **Delegate every method.** The trait has defaults for `attach`, `request_permissions`, the four
  layout methods, `write_image`, `push_tokens` and `sync_events`. A wrapper that leaves one out gets
  the default, not the inner backend's answer: layouts become `UNSUPPORTED_FEATURE`, and without
  `attach` the inner backend never gets the sink and no tap arrives.
- **`attach` runs once**, in `build()`, before any command. Hand the `BackendContext` to the inner
  backend, or keep it and call `events.emit(..)` when the framework's own native side reports a
  tap.
- **`Arc<B>` is a backend** when `B` is one. Keep a clone of the `Arc` to reach the backend's own
  methods after the builder took it, such as `LiveUpdateBackend::pump_events` or the testkit's
  `MemoryBackend::tap`.
- **Requests are already checked.** `LiveActivity` validates before it calls the backend, so a
  backend sees valid requests only.
- **Say what the backend has.** `capabilities()` is what the application reads. When the wrapper
  adds a feature, report it there.

## Lifecycle

### Build once, at setup

Build one `LiveActivity` when the framework sets the plugin up, and keep it in the framework's
application state:

```rust
impl Plugin {
    fn setup(framework: Arc<dyn Framework>, backend: impl LiveActivityBackend) -> Result<Self> {
        let live = LiveActivity::builder()
            .backend(backend)
            .event_sink(sink(framework))
            .build()?;
        Ok(Self { live })
    }
}
```

With the platform's default backend this is
`LiveActivity::builder().event_sink(sink(framework)).build()`.

Do not build a second one. On iOS the backend starts one thread that reads activitykit's events,
and activitykit's settings hold for the whole process. On Android one backend is the serialization
boundary of all operations, and the Android backend keeps the first sink it was attached to.

Build early. On iOS the system can launch the app in the background for a tap, and the tap waits
for the backend to subscribe. Set the plugin up in a place that also runs on such a launch.

`LiveActivity` is `Clone`, `Send` and `Sync`. It holds an `Arc` of the backend, so a clone is cheap
and every clone talks to the same backend. Hand clones to command handlers and worker threads
freely.

### Call sync_events at startup and on every resume

Android stores taps, also while the application does not run. Nothing reaches the sink until the
backend is asked:

```rust
impl Plugin {
    /// The framework's resume hook, and once after setup.
    async fn resumed(&self) -> Result<()> {
        self.live.sync_events().await
    }
}
```

Call it:

- once at startup, after `build()`;
- on every resume of the application;
- when the Android side signals a stored event (liveupdate's AAR calls a listener after each one:
  see [Android setup](android-setup.md#6-deliver-taps-and-states));
- at the time the default Android backend names with `next_check`, when the framework has a thread
  to wait on: Android tells nobody that a stale time has come, and `sync_events` is what reports it.

On iOS `sync_events` does nothing, so the same code runs on both.

### iOS: never block the main thread

On iOS the Swift layer runs each request on the main thread and completes the future from there. A
`block_on` on the main thread waits for an answer that only the main thread can give: a deadlock.

Wrong, from a UI callback or any code on the main thread:

```rust
let record = liveactivity::block_on(live.start(input))?; // never returns on iOS
```

Right: await the call on the framework's async runtime.

```rust
let live = live.clone();
runtime.spawn(async move {
    let result = live.start(input).await;
    // hand `result` back through the framework
});
```

Or, where there is no runtime, drive it on a worker thread:

```rust
let live = live.clone();
std::thread::spawn(move || {
    let result = liveactivity::block_on(live.start(input));
    // hand `result` back through the framework
});
```

`runtime` stands for the framework's own executor. The calls are plain futures for any executor. A
framework whose command handlers already run off the main thread needs nothing more than `.await`.

For a blocking API, use `liveactivity::wait` and not `block_on`. It is `block_on` with a guard: on
the main thread of an Apple platform it answers `NATIVE_ERROR` at once, with a message that says
why, where `block_on` would hang.

```rust
pub fn start(&self, input: StartInput) -> Result<ActivityRecord> {
    liveactivity::wait(self.live.start(input))
}
```

### iOS: the sink only forwards

On iOS the backend calls the sink from a thread of its own, without holding a lock. A tap keeps its
intent, and with it an app that the system launched for the tap, until the sink has returned, for
at most about 4 seconds. Taps come one at a time, so a slow sink also holds the taps behind it.

So the sink hands the event over and returns soon. If the application's handlers must finish before
the system may suspend the app again, the sink may wait for them, but never longer than those 4
seconds. The push events wait for nothing, but they come through the same thread.

### Android: a call runs on the polling thread

On Android the engine calls its driver synchronously. A call completes on its first poll and runs
the native call, a storage write and a framework call, on the thread that polls it. Poll it off the
UI thread.

The sink is called by a thread that asked: the one that runs `sync_events` (or `pump_events`), or
a call of the application that made a state due. It receives one event at a time, in the order the
events were taken, and no lock is held while it runs, so it may call the service. When two threads
ask at once, one of them hands over what both took. A slow sink then also holds the events of the
other thread's call behind it, though that call has returned.

## Testing an adapter

The `testkit` feature has a backend without a platform and the checks of the interface:

```toml
[dev-dependencies]
liveactivity = { git = "https://github.com/1eedaegon/liveactivity-rs", features = ["testkit"] }
```

`testkit::MemoryBackend` keeps its activities in memory and shows nothing. It runs the progress and
timer templates under the rules of the contract, stores the names of written images
(`images()`), and reports a tap to the sink when asked (`tap(id, action_id)`). With it an adapter's
commands and events run in an ordinary test:

```rust
use liveactivity::testkit::MemoryBackend;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Recording(Mutex<Vec<(String, Value)>>);

impl Framework for Recording {
    fn emit(&self, event: &str, payload: Value) {
        self.0.lock().unwrap().push((event.to_owned(), payload));
    }
}

#[test]
fn a_tap_reaches_the_frameworks_events() {
    let memory = Arc::new(MemoryBackend::new());
    let framework = Arc::new(Recording::default());
    let plugin = Plugin::setup(framework.clone(), memory.clone()).unwrap();

    liveactivity::block_on(async {
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
            .await
            .unwrap();
        let id = started["id"].as_str().unwrap();

        // A request outside the contract never reaches the backend.
        let refused = plugin.command("update", json!({ "id": id, "snapshot": {} })).await;
        assert_eq!(refused.unwrap_err().code, "INVALID_INPUT");

        // A tap on the platform comes out of the framework's event API.
        memory.tap(id, "track").unwrap();
    });

    let events = framework.0.lock().unwrap();
    assert_eq!(events[0].0, "live-activity://action");
    assert_eq!(events[0].1["actionId"], "track");
}
```

`testkit::conformance` runs the template lifecycle on a connected `LiveActivity` and reports each
rule that makes a backend exchangeable with another. Run it on a backend of the framework's own:

```rust
let live = LiveActivity::builder().backend(MyBackend::new()).build()?;
liveactivity::testkit::conformance(&live).await.assert_passed();
```

The `Report` also has `passed()` and `failures()`. The run ends the activities it starts. It covers
the templates: capabilities and permissions, a start with its key and content, a repeated start, a
conflicting start, an invalid request, newer, equal, older and conflicting revisions, an unknown
handle, the list, a timer beside a progress activity, the end, a repeated end, a late update, a
start after the end, and `sync_events`.

## Checklist

- [ ] The plugin's types are `liveactivity::contract::*`, not copies.
- [ ] Every command of the contract has a handler: `capabilities`, `permissions`,
      `request_permissions`, `start`, `update`, `end`, `list`, `start_layout`, `update_layout`,
      `end_layout`, `list_layout`, `write_image`, `push_tokens`, `sync_events`.
- [ ] Requests that arrive as JSON go through `parse_request`.
- [ ] Errors reach the application with their `code` and `message` unchanged.
- [ ] One `LiveActivity` is built at setup, with the sink, and kept in application state.
- [ ] Setup also runs when the system launches the app in the background.
- [ ] The sink forwards every `Event` variant, has a `_` arm, and returns at once.
- [ ] `sync_events()` runs at startup, on every resume, and when Android signals a stored event.
- [ ] No call is blocked on from the main thread on iOS; no call is polled on the UI thread on
      Android.
- [ ] A backend of the framework's own delegates every method it does not answer, `attach` and
      `sync_events` included, and reports what it has in `capabilities()`.
- [ ] On Android the backend has the app's files (`with_files`) when the plugin offers layouts,
      images or keyed taps, and the host shows the notification permission dialog.
- [ ] The host app is set up as [iOS setup](ios-setup.md) and [Android setup](android-setup.md)
      say.
- [ ] The adapter's tests run on `testkit::MemoryBackend`, and a backend of the framework's own
      passes `testkit::conformance`.
