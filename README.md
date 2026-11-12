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

## Install

```toml
[dependencies]
liveactivity = "0.1"
```

The crate uses [activitykit](https://github.com/1eedaegon/activitykit-rs) for
iOS and [liveupdate](https://github.com/1eedaegon/liveupdate-rs) for Android as
they are. It compiles on every target; where there is no platform, the default
is `UnsupportedBackend`: the queries answer that nothing is supported and every
other call answers `UNSUPPORTED_PLATFORM`.

| Document | For |
| --- | --- |
| [docs/contract.md](docs/contract.md) | every command, its JSON, the limits, the errors, the events and the layout tree |
| [docs/ios-setup.md](docs/ios-setup.md) | the widget extension, the App Group and pushes of an iOS app |
| [docs/android-setup.md](docs/android-setup.md) | the AAR, the permissions and the files of an Android app |
| [docs/framework-integration.md](docs/framework-integration.md) | writing a framework plugin: commands, events, lifecycle, threads |

## The two sides

Everything goes through one builder. An application says where events go; a
framework plugin or a platform says which backend runs the commands. Whatever
is not said is the default.

```rust
let live = LiveActivity::builder()
    .backend(backend)      // the supporting side; without it, the platform's default
    .event_sink(sink)      // the using side; without it, events are dropped
    .build()?;
```

| You are | You write |
| --- | --- |
| an application | `.event_sink(..)`, then the calls below |
| a framework plugin | an `EventSink` that forwards events to the framework's own, commands that call `LiveActivity`, and a backend only when the framework has a native side of its own |
| a platform | a `LiveActivityBackend` |

## Using it

```rust
use liveactivity::{EndInput, Event, LiveActivity, StartInput, UpdateInput};

let live = LiveActivity::builder()
    .event_sink(|event: Event| match event {
        Event::Action(tap) => println!("{} tapped {}", tap.activity_id, tap.action_id),
        Event::PushToken(token) => println!("send {} to the server", token.token),
        _ => {}
    })
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
| `push_tokens()` | the push tokens iOS has given since launch, for a sink that listened late |
| `sync_events()` | delivers the taps Android queued; call it when the application starts and resumes |

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

Calls are futures for any executor. Where there is none, `liveactivity::wait`
blocks for the answer. On iOS the answer comes from the main thread, so a
blocking call there would never end: `wait` answers `NATIVE_ERROR` at once on
the main thread instead of hanging. On Android a call runs on the thread that
polls it.

[`examples/usage.rs`](crates/liveactivity/examples/usage.rs) runs anywhere:

```sh
cargo run -p liveactivity --example usage --features testkit
```

### Events

| Event | When |
| --- | --- |
| `Event::Action` | a button of an activity was tapped |
| `Event::PushToken` | iOS gave an activity a push token, or changed it |
| `Event::PushToStartToken` | iOS gave the app its push-to-start token, or changed it |
| `Event::Updated` | a push changed the content of a layout activity |

The enum may grow: match with a `_` arm. On iOS a tap keeps the app awake
until the sink returns, for at most 4 seconds.

## Supporting it

### A framework plugin

A plugin carries the contract over its framework and adds nothing to it.
[`examples/adapter.rs`](crates/liveactivity/examples/adapter.rs) is a whole one
that compiles and runs, and
[docs/framework-integration.md](docs/framework-integration.md) walks through
it.

```sh
cargo run -p liveactivity --example adapter --features testkit
```

1. **Types.** Re-export `liveactivity::contract::*`: the requests and records
   are serde types with camelCase JSON, ready for the framework's IPC.
2. **Commands.** Each command parses its input and awaits the call of the same
   name:

   ```rust
   "start" => answer(live.start(parse_request("start", input)?).await?),
   ```

3. **Events.** Give the builder a sink that forwards each event to the
   framework's own event API:

   ```rust
   LiveActivity::builder()
       .event_sink(move |event: Event| match event {
           Event::Action(tap) => framework.emit("live-activity://action", &tap),
           // the push events the same way
           _ => {}
       })
       .build()?;
   ```

4. **Lifecycle.** Build once at setup and keep the `LiveActivity` in the
   framework's state. Call `sync_events()` at startup and on every resume.
5. **Backend.** Name one only when the framework has a native side of its own.
   Wrap a default backend, answer what the framework does itself, and hand the
   rest on:

   ```rust
   // The framework reaches Android through its own call: only the transport differs.
   let engine = LiveUpdateBackend::over_bridge(my_bridge) // my_bridge: liveupdate::Bridge
       .with_files(AndroidFiles::new(app_name, files_dir));

   // The framework shows Android's permission dialog, which needs an Activity.
   impl LiveActivityBackend for MyBackend {
       fn request_permissions(&self, kind: PermissionKind) -> BackendFuture<'_, Permissions> {
           Box::pin(async move {
               self.dialog.ask(kind)?;
               self.engine.permissions().await
           })
       }
       fn start(&self, input: StartInput) -> BackendFuture<'_, ActivityRecord> {
           self.engine.start(input)
       }
       // the other calls the same way
   }
   ```

`liveupdate::Bridge` is one call that carries a command name and a JSON payload
to the Android side.

### A platform

Implement `LiveActivityBackend`, one method per call of the table above. Only
the template calls and the two queries are required; layouts, permission
requests and images default to `UNSUPPORTED_FEATURE`, a backend without pushes
holds no token, and `capabilities` says what the backend has. `attach` hands
the backend the sink once, before any command; the backend sends each event
with `events.emit(..)`.

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
Xcode. The app needs `NSSupportsLiveActivities`, a widget extension that links
activitykit's `ActivityKitWidget` product, and an App Group for images. Live
Activities need iOS 16.2, or iPadOS 17 on an iPad. The steps are in
[docs/ios-setup.md](docs/ios-setup.md).

**Android.** The app packages liveupdate's AAR and the Kotlin standard library,
and the host initializes `ndk-context`, which common Rust Android runtimes do.
The default backend draws progress and timer activities as it is. Given the
folder of the app's files, it also draws layouts, stores images and reports
taps on buttons:

```rust
let backend = LiveUpdateBackend::android()?
    .with_files(AndroidFiles::new("My App", files_dir)); // Context.getFilesDir()
let live = LiveActivity::builder().backend(backend).build()?;
```

Only the system's permission dialog needs code of the host, because it needs an
Activity. The steps, and the 16 KB page-size note, are in
[docs/android-setup.md](docs/android-setup.md).

## Develop

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo llvm-cov --workspace --all-features --fail-under-lines 80
bash tools/check-requirements.sh
bash tests/release/run.sh
```

Requirements and their test evidence are in
[docs/requirements.md](docs/requirements.md), the structure in
[docs/design.md](docs/design.md).

## Releases

Releases follow the subjects of the commits on `main`, which are
[conventional commits](https://www.conventionalcommits.org/). After CI passes
on `main`, the release workflow bumps the version, writes the changelog, tags
the commit and publishes the crate:

| Subject | Release |
| --- | --- |
| `feat:` | minor |
| `fix:`, `perf:`, `refactor:`, `docs:`, `revert:` | patch |
| `test:`, `ci:`, `chore:`, `build:`, `style:` | none |
| `!` or a `BREAKING CHANGE:` footer | major, only by running the "Auto release" workflow by hand with `major` checked |

## License

MIT or Apache-2.0, at your option.
