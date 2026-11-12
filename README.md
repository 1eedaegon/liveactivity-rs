# liveactivity-rs

Live Activities for Rust applications, behind one interface: iOS ActivityKit and
Android Live Updates by default, and any other backend in their place.

```rust
use liveactivity::{EndInput, LiveActivity, StartInput, UpdateInput};

let live = LiveActivity::builder().build()?;

let order = live
    .start(StartInput::progress("order:42", "Order 42", "Preparing", 0.1))
    .await?;
live.update(UpdateInput::progress(&order.id, 1, "Order 42", "On the way", 0.6))
    .await?;
live.end(EndInput::new(&order.id)).await?;
```

Status: pre-release. The contract may still change before the first published
version.

## Install

```toml
[dependencies]
liveactivity = { git = "https://github.com/1eedaegon/liveactivity-rs" }
```

The crate uses [activitykit](https://github.com/1eedaegon/activitykit-rs) for
iOS and [liveupdate](https://github.com/1eedaegon/liveupdate-rs) for Android as
they are. It compiles on every target; where there is no platform, every call
answers `UNSUPPORTED_PLATFORM`.

## The two sides

Everything goes through one builder. An application says where taps go; a
framework plugin or a platform says which backend runs the commands. Whatever
is not said is the default.

```rust
let live = LiveActivity::builder()
    .backend(backend)      // the supporting side; without it, the platform's default
    .event_sink(sink)      // the using side; without it, taps are dropped
    .build()?;
```

| You are | You write |
| --- | --- |
| an application | `.event_sink(..)`, then the calls below |
| a framework plugin | an `EventSink` that forwards taps to the framework's events, commands that call `LiveActivity`, and a backend only when the framework has a native side of its own |
| a platform | a `LiveActivityBackend` |

## Using it

```rust
use liveactivity::{ActionEvent, EndInput, LiveActivity, StartInput, UpdateInput};

let live = LiveActivity::builder()
    .event_sink(|tap: ActionEvent| println!("{} tapped {}", tap.activity_id, tap.action_id))
    .build()?;
```

`liveactivity::connect_native(sink)` is the same in one call. Keep the
`LiveActivity` in application state; it is cheap to clone and safe to use from
any thread.

| Call | Does |
| --- | --- |
| `start(StartInput::progress(..))`, `StartInput::timer(..)` | starts a progress or countdown activity |
| `update(UpdateInput::progress(..))` | sends the whole content again under a higher revision |
| `end(EndInput::new(id))`, `EndInput::immediate(id)` | ends the activity |
| `list()` | the activities the backend knows |
| `start_layout`, `update_layout`, `end_layout`, `list_layout` | the same for an activity with a layout of its own |
| `capabilities()`, `permissions()`, `request_permissions(kind)` | what the device and the person allow |
| `write_image(name, png)` | stores a PNG that a layout's `image` node shows |
| `sync_events()` | delivers the taps Android queued; call it when the application resumes |

The rules are the same on every backend:

- **Key.** `activity_key` is the application's own name of the activity. A
  repeated start with the same key and request answers the running activity; a
  different request under that key is `CONFLICT`.
- **Handle.** Updates and ends take the `id` of the record, new for every start.
- **Revision.** A lower revision is `STALE_REVISION`. The same revision with
  other content is `CONFLICT`; with the same content it changes nothing.
- **End.** Ending twice is fine. An ended activity takes no update (`NOT_FOUND`).
- **Errors.** Every failure is an `Error` with a `code` out of `ERROR_CODES`
  and a message.

Requests are checked against the contract before a backend sees them, so an
invalid one is `INVALID_INPUT` everywhere.

Calls are futures for any executor; `liveactivity::block_on` drives one where
there is none. On iOS the answer comes from the main thread, so do not block
the main thread on a call. On Android a call runs on the thread that polls it.

[`examples/usage.rs`](crates/liveactivity/examples/usage.rs) runs anywhere:

```sh
cargo run -p liveactivity --example usage --features testkit
```

## Supporting it

### A framework plugin

A plugin carries the contract over its framework and adds nothing to it.

1. **Types.** Re-export `liveactivity::contract::*`: the requests and records
   are serde types with camelCase JSON, ready for the framework's IPC.
2. **Commands.** Each command parses its input and awaits the call of the same
   name: `live.start(parse_request("start", json)?).await`.
3. **Taps.** Give the builder a sink that forwards a tap to the framework's own
   event API. On iOS a tap keeps the app awake until the sink returns.
4. **Backend.** Name one only when the framework has a native side of its own:

```rust
// The framework reaches Android through its own call: only the transport differs.
LiveActivity::builder()
    .backend(LiveUpdateBackend::over_bridge(my_bridge)) // my_bridge: liveupdate::Bridge
    .event_sink(my_sink)
    .build()?;

// The framework draws more than the default: its backend answers that part and hands the rest on.
struct MyBackend {
    templates: LiveUpdateBackend<JsonDriver<MyBridge>>,
    // the framework's native handle
}

impl LiveActivityBackend for MyBackend {
    fn attach(&self, events: BackendContext) -> Result<()> {
        self.templates.attach(events)
    }
    fn start(&self, input: StartInput) -> BackendFuture<'_, ActivityRecord> {
        self.templates.start(input)
    }
    fn start_layout(&self, input: LayoutStartInput) -> BackendFuture<'_, LayoutActivityRecord> {
        Box::pin(async move { /* the framework's own native side */ })
    }
    // ...
}
```

`liveupdate::Bridge` is one call that carries a command name and a JSON payload
to the Android side.

### A platform

Implement `LiveActivityBackend`, one method per call of the table above. Only
the template calls and the two queries are required; layouts, permission
requests, images and queued events default to `UNSUPPORTED_FEATURE`, and
`capabilities` says what the backend has. `attach` hands the backend the sink
once, before any command; the backend sends taps with `events.emit(..)`.

The `testkit` feature checks that a backend keeps the rules above, which is what
makes it exchangeable with the default ones:

```rust
let live = LiveActivity::builder().backend(MyBackend::new()).build()?;
liveactivity::testkit::conformance(&live).await.assert_passed();
```

`testkit::MemoryBackend` is a backend without a platform, for an application's
own tests and as the smallest example of an implementation.

## Platform setup

**iOS.** activitykit builds its Swift bridge into the Rust library, which needs
Xcode. The app needs `NSSupportsLiveActivities` and a widget extension that
links activitykit's `ActivityKitWidget` product, and an App Group for images:
see activitykit's
[iOS setup](https://github.com/1eedaegon/activitykit-rs/blob/main/docs/ios-setup.md).
Live Activities need iOS 16.2, or iPadOS 17 on an iPad.

**Android.** Add the AAR from liveupdate's
[release](https://github.com/1eedaegon/liveupdate-rs/releases) and the Kotlin
standard library to the app, as its
[README](https://github.com/1eedaegon/liveupdate-rs#install) shows. The host
must initialize `ndk-context`, which common Rust Android runtimes do, and
request the notification permission on Android 13 and later. The default
Android backend draws progress and timer activities; layouts, images and the
permission dialog need a native side of the host's own (see "Supporting it").

A library linked with an NDK before r28 is aligned to 4 KB pages, which Android
warns about on devices with 16 KB pages. Use NDK r28 or later, or pass the
linker `-Wl,-z,max-page-size=16384` from the application's build script.

## Develop

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo llvm-cov --workspace --all-features --fail-under-lines 80
bash tools/check-requirements.sh
```

Requirements and their test evidence are in
[docs/requirements.md](docs/requirements.md), the structure in
[docs/design.md](docs/design.md).
