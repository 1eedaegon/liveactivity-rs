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
liveactivity = "0.2"
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
| [docs/design/engine-extensions](docs/design/engine-extensions/PRD.md) | what the engines opened in activitykit 0.4 and liveupdate 0.2, what this crate does with it, and what is still planned |

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
| an application | `.event_sink(..)`, then the calls of [Features](#features) |
| a framework plugin | an `EventSink` that forwards events to the framework's own, commands that call `LiveActivity`, and a backend only when the framework has a native side of its own |
| a platform | a `LiveActivityBackend` |

## Features

Every snippet below is a function of
[`examples/features.rs`](crates/liveactivity/examples/features.rs), which
compiles and runs:

```sh
cargo run -p liveactivity --example features --features testkit
```

| Feature | Call | iOS | Android |
| --- | --- | --- | --- |
| [Connect](#connect) | `LiveActivity::builder()` | ✓ | ✓ |
| [Progress and timer](#progress-and-timer-activities) | `start`, `update`, `end`, `list` | ✓ | ✓ |
| [Layouts](#layout-activities) | `start_layout`, `update_layout`, `end_layout`, `list_layout` | ✓ | ✓ projected onto a notification |
| [Buttons and taps](#buttons-and-taps) | a `button` node, `Event::Action` | ✓ | ✓ up to three |
| [Links](#links-accessibility-and-motion) | a `link` node, `Layout.url` | ✓ | through an [Android content](#android-content); named as left out of a layout |
| [Accessibility, motion, privacy](#links-accessibility-and-motion) | members of a node and of a style | ✓ | a notification has none; motion and privacy are named as left out |
| [Alerts](#alerts-and-relevance) | `LayoutUpdateInput.alert` | ✓ | ✓ where the device alerts again; else silent, reported as `alert` |
| [Relevance](#alerts-and-relevance) | `LayoutSnapshot.relevance` | ✓ | ignored |
| [Android content](#android-content) | `LayoutSnapshot.android` | ignored | ✓ |
| [A last content that stays](#ending) | `EndPolicy::After` | ✓ up to four hours | ✓ where the device offers it |
| [Images](#images) | `write_image`, an `image` node | ✓ needs an App Group | ✓ as the large icon |
| [Pushes](#pushes) | `LayoutStartInput.push`, `push_tokens` | ✓ | none |
| [Events](#events) | the event sink, `sync_events` | ✓ as they come | ✓ on `sync_events` |
| [Capabilities and permissions](#capabilities-and-permissions) | `capabilities`, `permissions`, `request_permissions` | ✓ | ✓ the dialog is the host's |
| [Blocking calls](#blocking-calls) | `wait` | ✓ off the main thread | ✓ |

### Connect

```rust
let live = LiveActivity::builder()
    .event_sink(|event: Event| match event {
        Event::Action(tap) => println!("tapped {} on {}", tap.action_id, tap.activity_id),
        Event::State(change) => println!("{} is {:?}", change.activity_id, change.state),
        Event::PushToken(token) => println!("send {} to the server", token.token),
        _ => {}
    })
    .build()?;
```

Without `.backend(..)` the backend is the platform's own. Keep the
`LiveActivity` in application state: it is cheap to clone and safe to use from
any thread. `liveactivity::connect_native(sink)` is the same in one call.

### Progress and timer activities

A title, a status, and a progress from 0 to 1 or a countdown. They need no
layout and look native on both platforms.

```rust
let order = live
    .start(StartInput::progress("order:42", "Order 42", "Preparing", 0.1))
    .await?;
// An update sends the whole content again, under a higher revision.
live.update(UpdateInput::progress(&order.id, 1, "Order 42", "On the way", 0.6))
    .await?;
live.end(EndInput::new(&order.id)).await?;

// A countdown between two times, in epoch milliseconds.
let tea = live
    .start(StartInput::timer("tea:1", "Tea", "Steeping", started_at, ends_at))
    .await?;
live.end(EndInput::immediate(&tea.id)).await?;
```

`live.list()` answers the activities the backend knows.

### Layout activities

A tree of nodes, fixed at start, and values that updates replace. iOS draws
the tree on the Lock Screen and in the Dynamic Island; Android projects it
onto a notification and names what it left out in the record's
`degraded_features`. The nodes and styles are in
[docs/contract.md](docs/contract.md#the-layout-dsl).

```rust
use liveactivity::activitykit::{layout::{button, progress, text, vstack}, values};

let layout = Layout {
    lock_screen: vstack([
        text(Bound::key("title")),
        progress("done"),
        button("track", text("Track")),
    ]),
    island: None,
    background: None,
    keyline: None,
    url: None,
};
let values = values! { "title" => "Delivery 7", "done" => 0.4 }.into_inner();
let start = LayoutStartInput::new("delivery:7", layout, LayoutSnapshot::new(0, values));
let record = live.start_layout(start).await?;

// Every value again, under a higher revision.
let values = values! { "title" => "Delivery 7", "done" => 0.9 }.into_inner();
live.update_layout(LayoutUpdateInput::new(&record.id, LayoutSnapshot::new(1, values)))
    .await?;

live.end_layout(LayoutEndInput::new(&record.id)).await?;
```

A request that comes as JSON is read with
`parse_request("start_layout", json)?`, which checks it against the contract.

### Buttons and taps

A `button(action_id, label)` node is a button on iOS and a notification action
on Android. Its tap reaches the sink as `Event::Action`, with the `action_id`,
the activity's `id` and its key. The tap does not change the activity: send an
update if the button should change something.

### Links, accessibility and motion

Members of the layout, which iOS draws: a URL that a tap opens the application
with, what VoiceOver reads of a node, how a node comes and goes, and what a
locked device hides.

```rust
use liveactivity::activitykit::layout::{link, progress, text, vstack};

let layout = Layout {
    lock_screen: vstack([
        // What VoiceOver reads of a node that has no text of its own.
        progress("done").accessibility(
            Accessibility::new()
                .label("Delivery progress")
                .value(Bound::key("percent")),
        ),
        // Hidden on a locked device, when the person asked for that.
        text(Bound::key("address")).sensitive(),
        // Shown while `arrived` is true, and how it comes and goes.
        text("At the door")
            .when("arrived")
            .transition(Transition::MoveTop),
        // A tap on the label opens the application with this URL.
        link("myapp://delivery/9/map", text("Map")),
    ]),
    island: None,
    background: None,
    keyline: None,
    // A tap anywhere else on the activity.
    url: Some("myapp://delivery/9".into()),
};
let record = live.start_layout(start).await?;
// On Android: layout.transition, layout.link and layout.url.
println!("left out: {:?}", record.degraded_features);
```

The application receives a URL as any URL it is opened with (`onOpenURL` on
iOS); no event reports it. A style's `content_transition` says how its text
changes with an update, and `IslandExpanded::priority`,
`IslandExpanded::below_if_too_wide` and `Island::margins` are the Dynamic
Island's own options. On Android a notification opens the application, so the
projection shows no link: say where the notification leads with
[an Android content](#android-content).

### Alerts and relevance

```rust
let mut snapshot = LayoutSnapshot::new(2, values);
// Which of the application's activities the Dynamic Island shows first.
snapshot.relevance = Some(90.0);
let mut update = LayoutUpdateInput::new(id, snapshot);
update.alert = Some(Alert::new("Delivery arrived", "Your parcel is at the door"));
let record = live.update_layout(update).await?;
```

On iOS the device lights up, the Dynamic Island expands and an Apple Watch
shows the title and the body. On Android the notification alerts again with
that update where the device offers it (`android_presentation.alert`);
elsewhere the update is silent and `record.degraded_features` names `alert`.
An update without an alert is silent everywhere.

### Android content

What the notification shows, in Android's own terms, instead of the projection
of the layout: a progress with segments and points, metrics, the text of the
status chip, icons of actions. The types are liveupdate's, re-exported.

```rust
let segment = |length, color| ProgressSegment { length, id: None, color, semantic_style: None };

// Which notification styles the device draws.
let styles = live.capabilities().await?.android_presentation;
let segmented = styles.is_some_and(|styles| styles.progress_style);

let mut snapshot = LayoutSnapshot::new(3, values);
snapshot.android = Some(AndroidContent {
    title: "Delivery 7".into(),
    status: "2 stops away".into(),
    presentation: AndroidPresentation {
        // The text of the status chip, at most 7 characters.
        short_critical_text: Some("8 min".into()),
        ..AndroidPresentation::new(PresentationStyle::Progress(ProgressStyle {
            progress: 60,
            indeterminate: false,
            styled_by_progress: true,
            segments: if segmented {
                vec![segment(60, Some(0xFF00_7F00)), segment(40, None)]
            } else {
                vec![segment(100, None)]
            },
            points: Vec::new(),
            start_icon: None,
            end_icon: None,
            tracker_icon: None,
        }))
    },
});
live.update_layout(LayoutUpdateInput::new(id, snapshot)).await?;
```

A content may also name a URL that the notification opens, an action that
opens one, a clock in the header, what a secure lock screen shows and the
application's own look. The device says which of them it offers, and a content
that uses one it does not offer is `UNSUPPORTED_FEATURE`:

```rust
let capabilities = live.capabilities().await?;
let offered = capabilities.android_presentation.unwrap_or_default();

snapshot.android = Some(AndroidContent {
    title: "Delivery 7".into(),
    status: "At the door in 4 minutes".into(),
    presentation: AndroidPresentation {
        // A tap on the notification opens this URL in the application.
        open_url: offered.links.then(|| "myapp://delivery/7".into()),
        // An action that opens a URL sends no tap.
        actions: if offered.links {
            vec![NotificationAction {
                open_url: Some("myapp://delivery/7/map".into()),
                ..NotificationAction::new("map", "Map")
            }]
        } else {
            Vec::new()
        },
        // A countdown in the header, beside the style.
        chronometer: offered.chronometer.then(|| TimerStyle {
            timer: Timer { started_at: ends_at - 240_000, ends_at },
            counts_down: true,
        }),
        // What a secure lock screen shows of an activity that started as private.
        public_version: offered.public_version.then(|| PublicVersion {
            title: "Delivery".into(),
            status: "On the way".into(),
        }),
        sub_text: offered.branding.then(|| "Mochi Express".into()),
        ..AndroidPresentation::new(PresentationStyle::Standard)
    },
});
```

A snapshot without a content is projected from the layout, in the layout's
own style. A snapshot may have another style than the activity shows where the
device offers `style_change`; elsewhere that is `UNSUPPORTED_FEATURE`, and the
activity keeps what it shows. The backend needs the app's files for a content:
see [Platform setup](#platform-setup). `Timer` is `liveactivity::liveupdate::Timer`.

### Ending

```rust
// The platform's default: iOS keeps the activity on the Lock Screen for a while, Android removes it.
live.end_layout(LayoutEndInput::new(id)).await?;

// Or keep the last content on screen until a time, with a final snapshot.
let mut end = LayoutEndInput::new(id);
end.policy = EndPolicy::After { at: until };
end.snapshot = Some(LayoutSnapshot::new(5, values));
live.end_layout(end).await?;
```

`LayoutEndInput::immediate(id)` removes the activity at once. With
`EndPolicy::After` iOS keeps the ended activity for at most four hours. Android
keeps the last content, no longer ongoing, where the device offers
`delayed_end`, and answers `UNSUPPORTED_FEATURE` elsewhere, leaving the
activity as it was. Template activities end the same way, with `EndInput`.

### Images

```rust
live.write_image("badge.png", &png).await?;
```

Write the PNG first, then show it with an `image` node whose source is
`{ "file": "badge.png" }`. On iOS it goes to the App Group that the widget
extension reads. On Android the first such image of a layout becomes the
notification's large icon.

### Pushes

iOS only. A start asks for a token, the sink gets it as `Event::PushToken`,
and a server updates the activity through APNs.

```rust
start.push = Some(LayoutPush::Token);
live.start_layout(start).await?;

// What came before the sink listened.
for held in live.push_tokens().await?.activities {
    println!("{} has the token {}", held.activity_id, held.token);
}
```

The server builds its payloads with `activitykit::push::Payload`. Content that
a push delivered comes back as `Event::Updated`, and an activity that a push
started as `Event::Started`. The steps are in
[docs/ios-setup.md](docs/ios-setup.md#7-pushes).

### Events

| Event | When | iOS | Android |
| --- | --- | --- | --- |
| `Event::Action` | a button or an action was tapped | ✓ | ✓ |
| `Event::State` | an activity has a new lifecycle state | every change while the app runs | the person dismissed it |
| `Event::Started` | a layout activity began, also one a push started | ✓ | |
| `Event::Updated` | a push changed the content of a layout activity | ✓ | |
| `Event::PushToken` | an activity got a push token, or a new one | ✓ | |
| `Event::PushToStartToken` | the app got its push-to-start token | ✓ | |
| `Event::Permissions` | the person changed what the app may do | ✓ | |

The enum may grow: match with a `_` arm. iOS delivers events as they come, and
a tap keeps the app awake until the sink returns, for at most 4 seconds.
Android stores taps and dismissals, also while the application does not run:

```rust
// When the application starts and resumes. On iOS it does nothing.
live.sync_events().await?;
```

### Capabilities and permissions

```rust
let capabilities = live.capabilities().await?;
if capabilities.layout != FeatureSupport::Supported {
    println!("{}: progress and timer activities only", capabilities.platform);
}
let permissions = live.permissions().await?;
if permissions.notifications == PermissionState::Prompt {
    live.request_permissions(PermissionKind::Notifications).await?;
}
```

A feature is supported, unsupported or unknown, and unknown never counts as
supported. On Android `capabilities.android_presentation` also says which
notification styles the device draws and which extensions it offers: links, a
header clock, a style that changes, a last content that stays, an update that
alerts, a public version and branding. On iOS Live Activities are a switch in
Settings and there is no dialog. On Android the system's dialog needs an Activity, which the host
shows: see [Supporting it](#supporting-it).

### Blocking calls

Calls are futures for any executor. Where there is none:

```rust
let record = liveactivity::wait(live.start(input))?;
```

On iOS the answer comes from the main thread, so a blocking call there would
never end: `wait` answers `NATIVE_ERROR` at once on the main thread instead of
hanging. On Android a call runs on the thread that polls it.

### Rules of every backend

- **Key.** `activity_key` is the application's own name of the activity. A
  repeated start with the same key and request answers the running activity; a
  different request under that key is `CONFLICT`.
- **Handle.** Updates and ends take the `id` of the record, new for every start.
- **Revision.** A lower revision is `STALE_REVISION`. The same revision with
  other content is `CONFLICT`; with the same content it changes nothing.
- **End.** Ending twice is fine. An ended activity takes no update (`NOT_FOUND`).
- **Errors.** Every failure is an `Error` with a `code` out of `ERROR_CODES`
  and a message.
- **Validation.** Requests are checked against the contract before a backend
  sees them, so an invalid one is `INVALID_INPUT` on every platform.
- **What a platform leaves out** is named in the record's `degraded_features`.

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

Implement `LiveActivityBackend`, one method per call of the contract. Only
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
on `main`, the release workflow bumps the version, writes the changelog, waits
for CI on that commit, tags it and publishes the crate:

| Subject | Release |
| --- | --- |
| `feat:` | minor |
| `fix:`, `perf:`, `refactor:`, `docs:`, `revert:` | patch |
| `test:`, `ci:`, `chore:`, `build:`, `style:` | none |
| `!` or a `BREAKING CHANGE:` footer | major, only by running the "Auto release" workflow by hand with `major` checked |

## License

MIT or Apache-2.0, at your option.
