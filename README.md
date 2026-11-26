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
liveactivity = "0.14"
```

A server that sends pushes takes the same version with `default-features = false`
and `features = ["server"]`: the sender, without the application's side
([Pushes from a server](#pushes-from-a-server)).

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
| [docs/server-push.md](docs/server-push.md) | a server's pushes: the key, the targets, the messages and what came back |
| [docs/design/engine-compatibility](docs/design/engine-compatibility/PRD.md) | a watch's Ongoing Activity, an event of an unknown kind, constructors and enums that may grow, on liveupdate 0.7 |
| [docs/design/engine-small-family](docs/design/engine-small-family/PRD.md) | the small region of a layout, for a paired Apple Watch and CarPlay on iOS, on activitykit 0.6 |
| [docs/design/engine-interaction](docs/design/engine-interaction/PRD.md) | toggles, staleness, scheduled and transient starts and alert sounds, on activitykit 0.5 |
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
| [Buttons and taps](#buttons-and-taps) | a `button` node, `Event::Action` | ✓ from iOS 17.2; the label alone before | ✓ up to three |
| [Toggles](#toggles-and-staleness) | a `toggle` node, `ActionEvent.on` | ✓ a switch, from iOS 17.2 | ✓ an action, with a check mark while on |
| [Staleness](#toggles-and-staleness) | `when_stale()` and `unless_stale()` on a node | ✓ drawn by the system when the time has passed | drawn as while fresh, reported as `layout.ifStale`; the sink is told `stale` |
| [StandBy](#standby) | `when_fullscreen()` and `unless_fullscreen()` on a node | ✓ iOS 17, while the Lock Screen fills the screen of a charging iPhone on its side | never full screen: a node of StandBy is left out, its counterpart shown |
| [A tap that runs your own code](#android-options) | `AndroidOptions::with_target`, `NotificationAction.target`, `Event::Handled` | ignored | ✓ a service or a receiver of the application, where the device offers `action_targets` |
| [A watch](docs/contract.md#a-watchs-ongoing-activity) | `Layout.small`; `AndroidOptions::with_wear_ongoing_activity`, `AndroidPresentation.wear_ongoing_activity`, `AndroidOptions::with_local_only` | ✓ a paired Apple Watch, in its Smart Stack from iOS 18: `small` where the widget extension sets `ActivityKitSmallFamily`, else the compact parts, reported as `layout.small`. The Ongoing Activity is ignored | ✓ an application on the watch, Wear OS 3 to 6, with `androidx.wear:wear-ongoing`: the activity's icon on the watch face, its Ongoing Activity, which one activity can do without. A phone's ongoing notification is not bridged to a paired watch, and a Live Update only to some; `with_local_only(true)` keeps one off it. `small` is not drawn, reported as `layout.small` |
| [A car](docs/contract.md#layout) | `Layout.small` | ✓ CarPlay, from iOS 26: `small` as on a watch, else the compact parts | none: Android Auto shows the notifications of messaging applications built for it, and Android's documents name no place there for a Live Update. `small` is reported as `layout.small` |
| [Links](#links-accessibility-and-motion) | a `link` node, `Layout.url` | ✓ | ✓ the notification and its actions open them, where the device opens URLs |
| [Accessibility, motion, privacy](#links-accessibility-and-motion) | members of a node and of a style | ✓ | a label is the text of what a notification cannot draw; motion and privacy are named as left out |
| [Alerts](#alerts-and-relevance) | `LayoutUpdateInput.alert` | ✓ | ✓ where the device alerts again; else silent, reported as `alert` |
| [A scheduled start](#start-options-and-sounds) | `LayoutStartInput.scheduled` | ✓ from iOS 26 | ✓ with liveupdate's library 0.4; on the minute with the exact-alarm permission |
| [A transient activity, an alert's sound](#start-options-and-sounds) | `LayoutStartInput.ios`, `Alert::with_sound` | ✓ | not kept; a sound is reported as `alert.sound` |
| [Relevance](#alerts-and-relevance) | `LayoutSnapshot.relevance` | ✓ | ignored |
| [Android content](#android-content) | `LayoutSnapshot.android` | ignored | ✓ |
| [A last content that stays](#ending) | `EndPolicy::After` | ✓ up to four hours | ✓ where the device offers it |
| [A lock screen version, a foreground service](#android-options) | `LayoutStartInput.android` | ignored | ✓ where the device offers them |
| [Images](#images) | `write_image`, an `image` node | ✓ needs an App Group | ✓ as the large icon |
| [Pushes](#pushes) | `LayoutStartInput.push`, `push_tokens` | ✓ | none |
| [Pushes from a server](#pushes-from-a-server) | `push::Message`, `push::Target`, `push::Sender` (feature `server`) | ✓ APNs | liveupdate's own payload with the feature `android-push`, reported as `Event::Pushed`; a layout activity not yet |
| [Events](#events) | the event sink, `sync_events` | ✓ as they come | ✓ on `sync_events` |
| [Capabilities and permissions](#capabilities-and-permissions) | `capabilities`, `permissions`, `request_permissions`, `open_promotion_settings` | ✓; no settings of promotion | ✓ the dialog is the host's; the settings of Live Updates open from a tap |
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

`live.list()` answers the activities the backend knows. Every request has a
constructor that takes what is required (`StartInput::new(key, Snapshot::progress(..))`,
`UpdateInput::new`, `EndInput::after(id, at)`, `ImageInput::new`, `AndroidContent::new`),
and the enums may grow: build on the constructors and match with a `_` arm, and
the code compiles against the next version. So do activitykit's layout types:
`Layout::new(lock_screen)` with `.island(..)`, `.background(..)`, `.url(..)` and
`.small(..)`, `Island::new()`, `Style::new()`, `Color::adaptive(light, dark)`.

### Layout activities

A tree of nodes, fixed at start, and values that updates replace. iOS draws
the tree on the Lock Screen and in the Dynamic Island; Android projects it
onto a notification and names what it left out in the record's
`degraded_features`. The nodes and styles are in
[docs/contract.md](docs/contract.md#the-layout-dsl).

```rust
use liveactivity::activitykit::{layout::{button, progress, text, vstack}, values};

let layout = Layout::new(vstack([
    text(Bound::key("title")),
    progress("done"),
    button("track", text("Track")),
]));
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

On the Lock Screen the card has no margin and shows a narrower layout in its
middle: give the root a padding, and a node that should reach an edge a frame
of infinite width
([placement](docs/contract.md#placement-on-the-lock-screen)).

### Buttons and taps

A `button(action_id, label)` node is a button on iOS and a notification action
on Android. Its tap reaches the sink as `Event::Action`, with the `action_id`,
the activity's `id` and its key. The tap does not change the activity: send an
update if the button should change something.

### Toggles and staleness

```rust
use liveactivity::activitykit::layout::{text, toggle, vstack};

let layout = Layout::new(vstack([
    // Drawn while the content is fresh, and the other in its place once its stale date has passed.
    text(Bound::key("eta")).unless_stale(),
    text("No signal").when_stale(),
    // A switch that shows `muted`. A tap asks for the other state and changes nothing itself.
    toggle("mute", "muted", text("Mute")),
]));
let values = values! { "eta" => "6 min", "muted" => false }.into_inner();
let mut snapshot = LayoutSnapshot::new(0, values);
snapshot.stale_at = Some(stale_at);
let record = live.start_layout(LayoutStartInput::new("ride:7", layout, snapshot)).await?;
```

A tap on the toggle reaches the sink as `Event::Action` with `on`, the state
the person asked for. The application sends an update with that value, or does
not when it refuses the change:

```rust
match event {
    Event::Action(tap) if tap.action_id == "mute" => tap.on,
    _ => None,
}
```

- **iOS** draws a switch (iOS 17.2; the label alone before). Once the stale date
  has passed, the system draws the stale nodes by itself, without an update
  and without the application running.
- **Android** shows the toggle as one of the notification's three actions,
  titled by its label, with `✓` before it while the value is on. It draws the
  content as it is while fresh and names `layout.ifStale`: nothing redraws a
  notification when a time passes. The sink is told `stale`
  ([Events](#events)), and an application that wants the notification to say
  so sends an update then.

### StandBy

An iPhone that charges on its side shows the activity full screen, in StandBy
(iOS 17), with room for more. A node can be drawn only there, or only outside
it:

```rust
let layout = Layout::new(vstack([
    text(Bound::key("eta")).unless_fullscreen(),
    text(Bound::key("route")).when_fullscreen(),
]));
```

A notification is never full screen: Android leaves the StandBy node out.

### Links, accessibility and motion

Members of the layout, which iOS draws: a URL that a tap opens the application
with, what VoiceOver reads of a node, how a node comes and goes, and what a
locked device hides.

```rust
use liveactivity::activitykit::layout::{link, progress, text, vstack};

let layout = Layout::new(vstack([
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
]))
// A tap anywhere else on the activity.
.url("myapp://delivery/9");
let record = live.start_layout(start).await?;
// On Android the links open too, and the record names layout.transition.
println!("left out: {:?}", record.degraded_features);
```

The application receives a URL as any URL it is opened with: `onOpenURL` on
iOS, the intent of the activity that declares the URL's scheme on Android. A
style's `content_transition` says how its text changes with an update, and
`IslandExpanded::priority`, `IslandExpanded::below_if_too_wide` and
`Island::margins` are the Dynamic Island's own options.

On Android the same layout gives the notification what it can show of this:

- `Layout.url` is what a tap on the notification opens, and a `link` is an
  action that opens its URL. When nothing in the application opens a URL the
  request does not fail: the links are left out and named in
  `degraded_features`.
- A timer beside a progress is the clock in the notification's header.
- A node that a notification cannot draw, such as a symbol or a gauge, is shown
  as the text of its label and value. A button whose label is a symbol is
  titled by the symbol's label; without one it is no action.
- A node with `Accessibility::new().hidden()` is decoration and is left out
  without being named.

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

### Start options and sounds

```rust
// A sound of the application's bundle. Without one the system plays its own.
let alert = Alert::new("Your ride", "It starts now").with_sound("chime.aiff");
let mut later = start.clone();
later.scheduled = Some(ScheduledStart::new(at, alert));
let record = live.start_layout(later).await?;
// Until its time the activity is pending: it holds its key, takes no update, and can be ended.
assert_eq!(record.state, ActivityState::Pending);
live.end_layout(LayoutEndInput::immediate(&record.id)).await?;

// An activity that the system ends when the person locks the device or leaves the application (iOS 18).
let mut transient = start;
transient.ios = Some(IosOptions::new(true));
live.start_layout(transient).await?;
```

- **A scheduled start** begins at `at`, in epoch milliseconds, with the alert
  the system shows then, also while the application does not run: iOS 26, and
  Android where `android_presentation.scheduled_start` says so (liveupdate's
  library 0.4 or later). The sink receives `State` with `active` when it
  begins. Android begins it at its time with the permission for exact alarms
  (`exact_start`), and up to ten minutes late without, an hour while the
  device dozes; see
  [Android setup](docs/android-setup.md#a-scheduled-start). Elsewhere the
  start is `UNSUPPORTED_FEATURE`.
- **`ios`** holds what only iOS reads, as `android` does for Android. It is no
  part of what makes two starts the same activity.
- **A sound** is the name of a file in the application's bundle or in its
  `Library/Sounds`, for the alert of an update as well. Android alerts with
  the sound of its channel and names `alert.sound`.

### Android content

What the notification shows, in Android's own terms, instead of the projection
of the layout: a progress with segments and points, metrics, the text of the
status chip, icons of actions. The types are liveupdate's, re-exported. Each is
built on its constructor, which takes what is required: a member that a later
version adds is then left at its default, and the code still compiles.

```rust
let segment = |length, color| ProgressSegment { color, ..ProgressSegment::new(length) };

// Which notification styles the device draws.
let styles = live.capabilities().await?.android_presentation;
let segmented = styles.is_some_and(|styles| styles.progress_style);

let mut snapshot = LayoutSnapshot::new(3, values);
let segments = if segmented {
    vec![segment(60, Some(0xFF00_7F00)), segment(40, None)]
} else {
    vec![segment(100, None)]
};
snapshot.android = Some(AndroidContent::new(
    "Delivery 7",
    "2 stops away",
    AndroidPresentation {
        // The text of the status chip, at most 7 characters.
        short_critical_text: Some("8 min".into()),
        // Built on its constructor, a track shows what is ahead of the progress fainter.
        ..AndroidPresentation::new(PresentationStyle::Progress(ProgressStyle::new(60, segments)))
    },
));
live.update_layout(LayoutUpdateInput::new(id, snapshot)).await?;
```

A content may also name a URL that the notification opens, an action that
opens one, a clock in the header, what a secure lock screen shows and the
application's own look. The device says which of them it offers, and a content
that uses one it does not offer is `UNSUPPORTED_FEATURE`:

```rust
let capabilities = live.capabilities().await?;
let offered = capabilities.android_presentation.unwrap_or_default();

snapshot.android = Some(AndroidContent::new(
    "Delivery 7",
    "At the door in 4 minutes",
    AndroidPresentation {
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
        chronometer: offered
            .chronometer
            .then(|| TimerStyle::new(Timer::new(ends_at - 240_000, ends_at), true)),
        // What a secure lock screen shows of an activity that started as private.
        public_version: offered
            .public_version
            .then(|| PublicVersion::new("Delivery", "On the way")),
        sub_text: offered.branding.then(|| "Mochi Express".into()),
        // In a watch application: this snapshot stays in the tray, off the watch face.
        wear_ongoing_activity: false,
        ..AndroidPresentation::new(PresentationStyle::Standard)
    },
));
```

A snapshot without a content is projected from the layout, in the layout's
own style. A snapshot may have another style than the activity shows where the
device offers `style_change`; elsewhere that is `UNSUPPORTED_FEATURE`, and the
activity keeps what it shows. The backend needs the app's files for a content:
see [Platform setup](#platform-setup). `Timer` is `liveactivity::liveupdate::Timer`.

### Android options

What only Android reads of a start. iOS ignores it.

```rust
let mut options = AndroidOptions::new(AndroidVisibility::Private);
// What a secure lock screen shows in place of the content of a private activity.
options.public_version = Some(PublicVersion::new("Delivery", "On the way"));
// The notification is the one a foreground service of the application holds.
if offered.foreground_placement {
    options.placement = Some(AndroidPlacement::Foreground);
}
start.android = Some(options);
let record = live.start_layout(start).await?;
if record.degraded_features.iter().any(|left| left == "publicVersion") {
    println!("this device shows no public version");
}
```

The public version goes with every snapshot that has none of its own. One
running activity has the foreground placement at a time, and a device that
does not offer it answers `UNSUPPORTED_FEATURE`. The service is the
application's: [docs/android-setup.md](docs/android-setup.md#a-foreground-service).

In a watch application an activity is on the watch face too, as its Ongoing
Activity, where the device says `android_presentation.wear_ongoing_activity`
([docs/android-setup.md](docs/android-setup.md#a-watch)). One activity can
stay in the tray, in all of its snapshots:

```rust
start.android = Some(
    AndroidOptions::new(AndroidVisibility::Private).with_wear_ongoing_activity(false),
);
```

A phone's activity can stay off a paired watch, where the device says
`android_presentation.local_only`:

```rust
start.android = Some(AndroidOptions::new(AndroidVisibility::Private).with_local_only(true));
```

A button can run the application's own service or receiver at once, with no
screen, as a Live Activity runs an App Intent: name its target by the action
id. The component takes the tap with `LiveUpdateActions.take`, and the sink
gets `Event::Handled`
([docs/android-setup.md](docs/android-setup.md#a-tap-that-runs-your-own-code)):

```rust
let record = ActionTarget::new(TargetKind::ForegroundService, "com.example.RecorderService");
start.android = Some(AndroidOptions::new(AndroidVisibility::Public).with_target("record", record));
```

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

The server builds its pushes with [`liveactivity::push`](#pushes-from-a-server).
Content that a push delivered comes back as `Event::Updated`, and an activity
that a push started as `Event::Started`. The steps are in
[docs/ios-setup.md](docs/ios-setup.md#7-pushes).

### Pushes from a server

The application sends its server what the device reported, as a `Target`:

```json
{ "activityKey": "delivery:7", "target": { "type": "apnsActivity", "token": "0a1b…" } }
```

The server makes one `Message` from the contract's own types and sends it with
the feature `server`. The key comes in as text; the crate reads no environment.

```rust
use liveactivity::push::{Apns, ApnsKey, Message, SendError, Sender};

let key = ApnsKey::from_pem(std::fs::read(path)?, "ABC123DEFG", "TEAM456789")?;
let sender = Sender::builder()
    .apns(Apns::new(key, "dev.example.app").sandbox())
    .build()?;

let message = Message::update("delivery:7", LayoutSnapshot::new(revision, values))
    .alert(Alert::new("Delivery", "2 minutes"));
match sender.send(&message, &target).await {
    Ok(delivery) => println!("sent {:?}", delivery.id),
    Err(SendError::Unregistered) => { /* forget the target */ }
    Err(error) => eprintln!("{error}"),
}
```

`Message::start` goes to the push-to-start token, `update` and `end` to an
activity's token or a channel. Without `server`, `message.request(&target,
&application)` gives the path, the headers and the body for another HTTP
client. On Android a layout activity is not pushed yet: liveupdate 0.10 runs its own payload
([docs/android-setup.md](docs/android-setup.md#pushes-from-the-server)). The steps, from the key to what
came back, are in [docs/server-push.md](docs/server-push.md); the example
`push_server` is a server with axum.

### Events

| Event | When | iOS | Android |
| --- | --- | --- | --- |
| `Event::Action` | a button, a toggle or an action was tapped; `tap` says which tap and when | ✓ also a tap the app took late | ✓ |
| `Event::State` | an activity has a new lifecycle state | every change while the app runs | the state it has when the backend is asked, once (`active`, `stale`, `ended`, `dismissed`) |
| `Event::Started` | a layout activity began, also one a push started | ✓ | |
| `Event::Updated` | a push changed the content of a layout activity | ✓ | |
| `Event::PushToken` | an activity got a push token, or a new one | ✓ | |
| `Event::PushToStartToken` | the app's push-to-start token, at launch and when it changes | ✓ | |
| `Event::Pushed` | a push from the server was taken or refused | | ✓ with the feature `android-push` |
| `Event::Permissions` | the person changed what the app may do | ✓ | |
| `Event::Opened` | the person opened the application from a notification | the application gets the URL itself | ✓ when the host reports the opening intent |
| `Event::Unknown` | something this version has no event for | | ✓ when the AAR is of a later liveupdate than the crate |
| `Event::Handled` | a tap that the application's own component took | | ✓ with an action's target |

A tap that the app did not take at once, because the tap launched it or its
process ended, comes to a later connection with `tap.late` (iOS); act on each
`tap.id` once:

```rust
// `handled` is the application's own set of tap ids, kept across launches.
if let Event::Action(action) = &event {
    if let Some(id) = &action.tap.id {
        if !handled.insert(id.clone()) {
            return; // this tap was handled already
        }
    }
}
```

The enum may grow: match with a `_` arm. An activity is over when a `State`
event says `ended` or `dismissed`, on either platform:

```rust
/// The id of the activity that `event` says is over.
fn over(event: &Event) -> Option<&str> {
    match event {
        Event::State(change)
            if matches!(change.state, ActivityState::Ended | ActivityState::Dismissed) =>
        {
            Some(&change.activity_id)
        }
        _ => None,
    }
}
```

iOS delivers events as they come, and a tap keeps the app awake until the sink
returns, for at most 4 seconds. On Android nothing calls the application: it
asks. A start, an update, an end and a list report the states that are due
with them, and `sync_events` hands over what Android stored, the taps and the
openings, and the state of every activity that the sink was not told yet, such
as a stale time that has passed or a notification the person removed. The sink
is told where an activity stands, once, one event at a time and in order; a
state that came and went between two asks is not reported:

```rust
// When the application starts and resumes. On iOS it does nothing.
live.sync_events().await?;
```

Android tells nobody that a stale time has come. A host that has a thread and
the default Android backend asks it when to ask again:

```rust
match backend.next_check()? {
    // A state is due: hand it over.
    NextCheck::Now => {
        backend.pump_events()?;
    }
    // Android has not shown a post yet: shortly, a little later each time, a few times.
    NextCheck::Soon => {}
    // The earliest stale time ahead, in epoch milliseconds. One that has passed means now.
    NextCheck::At(_time) => {}
    // Nothing until the next call, signal or resume.
    NextCheck::Never => {}
    // An answer that a later liveupdate adds: shortly, as for `Soon`.
    _ => {}
}
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
supported. On iOS `capabilities.ios` says what the device runs of the later
features (`push_to_start`, `push_channels`, `transient`, `scheduled_start`,
`small_family`), so a request need not be tried to learn it. On Android
`capabilities.android_presentation` also says which
notification styles the device draws and which extensions it offers: links, a
header clock, a style that changes, a last content that stays, an update that
alerts, a public version and branding. On iOS Live Activities are a switch in
Settings and there is no dialog. On Android the system's dialog needs an Activity, which the host
shows: see [Supporting it](#supporting-it).

When Android says `promotion: denied`, a person's tap can open the settings of
the application's Live Updates. The two channels take the names that the
application gives the strings `liveupdate_channel_name` and
`liveupdate_alert_channel_name` (and their `_description`)
([docs/android-setup.md](docs/android-setup.md#the-names-of-the-channels)):

```rust
if permissions.promotion == PermissionState::Denied {
    live.open_promotion_settings().await?; // Ok: Android was asked
}
```

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
  and a message. `SYSTEM_LIMIT` is the platform's limit: 32 live activities on
  Android, or no room left in liveupdate's store, and ActivityKit's on iOS.
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
Activities need iOS 16.1, or iPadOS 17 on an iPad. The steps are in
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
