# liveactivity-rs

Live Activities for Rust applications, behind one interface: iOS ActivityKit and
Android Live Updates by default, and any other backend in their place.

```rust
use liveactivity::{EndInput, StartInput, UpdateInput};

let live = liveactivity::connect_native(|tap: liveactivity::ActionEvent| {
    println!("{} tapped {}", tap.activity_id, tap.action_id);
})?;

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

## Use

`connect_native` connects the backend of the target and takes a sink for taps
on an activity's buttons: any closure, or a type that implements `EventSink`.
Keep the `LiveActivity` in application state; it is cheap to clone and safe to
use from any thread.

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
invalid one is `INVALID_INPUT` everywhere. The requests and records are plain
serde types, with camelCase JSON, for a framework that carries them over IPC.

Calls are futures for any executor; `liveactivity::block_on` drives one where
there is none. On iOS the answer comes from the main thread, so do not block
the main thread on a call. On Android a call runs on the thread that polls it.

[`examples/usage.rs`](crates/liveactivity/examples/usage.rs) runs anywhere:

```sh
cargo run -p liveactivity --example usage --features testkit
```

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
permission dialog need a native side of the host's own (see below).

A library linked with an NDK before r28 is aligned to 4 KB pages, which Android
warns about on devices with 16 KB pages. Use NDK r28 or later, or pass the
linker `-Wl,-z,max-page-size=16384` from the application's build script.

## Bringing a backend

A platform implements `LiveActivityBackend`, one method per call above, and is
connected in place of the default:

```rust
let live = LiveActivity::connect(sink, |context| Ok(MyBackend::new(context)))?;
```

Only the template calls and the two queries are required; the rest default to
`UNSUPPORTED_FEATURE`. A backend hands taps to `context.emit(..)`.

A host with a native side of its own, such as a framework plugin, usually
composes a default backend instead of replacing it:

- **Android transport only.** Implement `liveupdate::Bridge`, one call that
  carries a command name and a JSON payload to the Android side, and connect
  `android::LiveUpdateBackend::over_bridge(context, bridge)`.
- **More than the default draws.** Implement `LiveActivityBackend`, hand the
  template calls to a `LiveUpdateBackend` field, and answer layouts, images and
  permission requests from the host's own native code.

The `testkit` feature checks that a backend keeps the rules above, which is what
makes it exchangeable with the default ones:

```rust
liveactivity::testkit::conformance(&live).await.assert_passed();
```

`testkit::MemoryBackend` is a backend without a platform, for an application's
own tests and as the smallest example of an implementation.

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
