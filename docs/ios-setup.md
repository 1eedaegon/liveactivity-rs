# iOS setup

On iOS `liveactivity` runs on the [activitykit](https://github.com/1eedaegon/activitykit-rs) crate
(0.3). The Rust side needs no code beyond the builder:

```rust
let live = liveactivity::LiveActivity::builder()
    .event_sink(|event: liveactivity::Event| { /* forward it */ })
    .build()?;
```

The rest is the Xcode project. An app has three parts:

1. the Rust library, which holds activitykit's Swift bridge;
2. the app target, which links the library;
3. a widget extension, a Swift target, that draws the activities.

```text
Rust -> liveactivity -> activitykit (Rust) -> ActivityKitBridge (Swift, in the Rust library) -> ActivityKit
Lock Screen, Dynamic Island <- the widget extension: ActivityKitLayoutWidget (product ActivityKitWidget)
A tap -> ActivityKitActionIntent in the app (product ActivityKitIntent) -> activitykit -> the event sink
```

## Requirements

- Xcode. activitykit's build script runs SwiftPM for an iOS target and links the Swift bridge into
  the Rust library.
- iOS 16.2 on an iPhone, iPadOS 17 on an iPad. Below that every command answers
  `UNSUPPORTED_PLATFORM` and `capabilities()` reports every feature as `unsupported`.
- iOS 17 for buttons. Below it the widget shows a button's label without the button, and
  `capabilities().actions` is `unsupported`.
- iOS 17.2 for push-to-start, iOS 18 for broadcast channels.

## 1. Build the Rust library

For an iOS target, activitykit's `build.rs` builds the `ActivityKitBridge` product of its Swift
package and links it into the Rust library. On every other target it does nothing.

| Rust target | SDK |
| --- | --- |
| `aarch64-apple-ios` | iphoneos |
| `aarch64-apple-ios-sim` | iphonesimulator |
| `x86_64-apple-ios` | iphonesimulator |

- Set `IPHONEOS_DEPLOYMENT_TARGET` to the app's deployment target while cargo builds. The Swift
  bridge is built for that version, at least 15.0, so the linker does not warn about newer objects.
- `ACTIVITYKIT_SKIP_SWIFT=1` skips the Swift build, for `cargo check` or `clippy` of the iOS code
  on a machine without Xcode. Nothing links then.
- Use one version of activitykit in the app. `liveactivity::activitykit` re-exports the one this
  crate uses.

## 2. The app target

Most Rust iOS apps build Rust as a static library (`crate-type = ["staticlib"]`) and link it from
an Xcode app target. The app target needs:

- **The library and its frameworks.** Link the `.a` by its path. activitykit's Swift objects bring
  ActivityKit, Foundation, UIKit and CryptoKit through Swift's autolinking.
- **The Swift runtime's search paths**, when the app target compiles no Swift itself:

  ```yaml
  LIBRARY_SEARCH_PATHS: $(inherited) $(SDKROOT)/usr/lib/swift $(TOOLCHAIN_DIR)/usr/lib/swift/$(PLATFORM_NAME)
  ```

  Xcode adds these paths by itself to a target that compiles Swift.
- **`NSSupportsLiveActivities`** set to `YES` (Boolean) in the app's Info.plist.
- **The `ActivityKitIntent` product**, for buttons. The system runs a button's intent in the app
  process, so the app needs the intent's code and its App Intents metadata. Xcode extracts that
  metadata only for a target that compiles Swift: give the app target at least one Swift file. A
  file whose only code is `import ActivityKitIntent` is enough.
- **The widget extension** as a dependency, embedded in the app.

Never link `ActivityKitBridge` in Xcode. The Rust library holds it already, and a second copy would
duplicate its symbols. Do not define an `ActivityKitActionIntent` of your own: Xcode refuses an
intent that conflicts with one of a linked package.

## 3. Add activitykit's Swift package

The Swift package is the `swift` folder of activitykit. Cargo already has it, because activitykit
is a dependency of `liveactivity`. Find it:

```sh
cargo metadata --format-version 1 \
  | jq -r '.packages[] | select(.name == "activitykit") | .manifest_path'
```

The `swift` folder next to that `Cargo.toml` is the package. In Xcode, add it with File > Add
Package Dependencies > Add Local. Its products:

| Product | Link it to |
| --- | --- |
| `ActivityKitWidget` | the widget extension |
| `ActivityKitIntent` | the app target, for buttons |
| `ActivityKitViews` | both, only for custom views |
| `ActivityKitBridge` | nothing; `build.rs` builds it |

The path under Cargo's folder changes with the activitykit version. Update the package reference
when the version changes, so the widget and the Rust library come from the same version.

## 4. The widget extension

Add a Widget Extension target with a deployment target of iOS 16.2 or later. Link the
`ActivityKitWidget` product and replace the target's Swift files with this one:

```swift
import ActivityKitWidget
import SwiftUI
import WidgetKit

@main
struct Widgets: WidgetBundle {
    var body: some Widget { ActivityKitLayoutWidget() }
}
```

In the extension's Info.plist, set `ActivityKitLayout` to the integer `2`:

```xml
<key>NSExtension</key>
<dict>
  <key>NSExtensionPointIdentifier</key>
  <string>com.apple.widgetkit-extension</string>
</dict>
<key>ActivityKitLayout</key>
<integer>2</integer>
```

Before a start, the app checks that an embedded extension carries this key and answers
`CONFIGURATION_REQUIRED` otherwise. Give the extension a bundle identifier under the app's and the
same signing team. A widget of your own can sit in the same bundle.

`ActivityKitLayoutWidget` draws both kinds of activity: layouts, and the progress and timer
templates, which this crate starts as layout activities with a fixed tree.

## 5. Info.plist keys

| Key | Target | Value |
| --- | --- | --- |
| `NSSupportsLiveActivities` | app | `YES`. Without it a start answers `CONFIGURATION_REQUIRED` |
| `NSSupportsLiveActivitiesFrequentUpdates` | app | `YES`, only for frequent pushes |
| `ActivityKitLayout` | widget extension | the integer `2` |
| `ActivityKitAppGroup` | widget extension | the App Group for images, only with `write_image` |
| `UIAppFonts` | widget extension | the fonts the extension bundles, for the style member `fontFamily` |

## 6. An App Group, for images

`live.write_image(name, png)` writes a PNG that an `image` node shows with `{ "file": name }`. The
app writes the file and the widget extension reads it, so both need one shared container:

1. Add the App Groups capability to the app target and to the widget extension, with the same
   group, such as `group.com.example.app`. Both then have it in
   `com.apple.security.application-groups`.
2. Name the group in the extension's Info.plist:

   ```xml
   <key>ActivityKitAppGroup</key>
   <string>group.com.example.app</string>
   ```

The PNG goes to `activitykit/images` in the group's container, and the widget reads it each time it
draws. Without the group `write_image` answers `CONFIGURATION_REQUIRED`. A simulator needs nothing
more. For a device the group has to exist in the developer account, which automatic signing
registers once.

An app that writes no images needs no App Group.

## 7. Pushes

A server can update an activity, end it or start one through APNs. This crate asks ActivityKit for
what the server needs and reports it. It holds no key and sends nothing.

### In Xcode

- **Push Notifications** capability on the app target (`aps-environment` in the entitlements).
  Without it, and in the simulator, a start that asks for a token answers
  `CONFIGURATION_REQUIRED`.
- **`NSSupportsLiveActivitiesFrequentUpdates`** set to `YES`, for frequent pushes. The person can
  turn them off in Settings; `permissions().frequentPushes` reports `granted` or `denied`.
- **Broadcast capability** under Push Notifications in the developer account, for channels
  (iOS 18). Xcode cannot set it.

### In the request

A layout start asks with `push`:

```json
{ "push": "token" }
```

```json
{ "push": { "channel": "dHN0LXNyY2gtY2hubA==" } }
```

`"token"` asks for a push token of the activity's own. `{ "channel": id }` joins a broadcast
channel that the server made at APNs. In Rust they are `LayoutPush::Token` and
`LayoutPush::Channel(id)`. The progress and timer templates ask for no push.

### In the app

The token is not in the start's answer. ActivityKit makes it later and may change it. Each one
comes to the event sink:

```rust
use liveactivity::{Event, LiveActivity};

let live = LiveActivity::builder()
    .event_sink(|event: Event| match event {
        Event::PushToken(token) => { /* send token.activity_id and token.token to the server */ }
        Event::PushToStartToken(token) => { /* send token.token to the server */ }
        Event::Updated(updated) => { /* a push changed the content: updated.record has it */ }
        _ => {}
    })
    .build()?;
```

A token that came before the application listened is kept. `live.push_tokens().await?` answers the
newest token of each of the last 32 activities that got one, and the push-to-start token.

Push-to-start (iOS 17.2) is off by default. Turn it on when the kit is built (below). An activity
that a push starts is in `list_layout()`.

### On the server

The server sends the activity's token to APNs, with a payload for an update, an end or a start.
activitykit builds the payloads (`activitykit::push::Payload`) and has the header values
(`PUSH_TYPE`, `topic(bundle_id)`, `PRIORITY_NOW`); its
[usage guide](https://github.com/1eedaegon/activitykit-rs/blob/main/docs/usage.md#pushes) shows
them. Two rules matter for the application:

- A push carries a revision. Content a push delivers is reported as `Event::Updated` when its
  revision is above every revision the app sent or accepted for the activity. Number the server's
  pushes above the app's revisions.
- The app's own updates do not come back as `Event::Updated`.

## Relevance and alerts

Two members of a layout request reach ActivityKit beside the content.

```rust
let mut snapshot = LayoutSnapshot::new(2, values);
snapshot.relevance = Some(90.0);
let mut update = LayoutUpdateInput::new(&record.id, snapshot);
update.alert = Some(Alert::new("Delivery arrived", "Your parcel is at the door"));
live.update_layout(update).await?;
```

- `relevance` says which of the application's activities the Dynamic Island shows first: the
  highest score wins. A start, an update and the final content of an end take one. A snapshot
  without it keeps the activity's score, which is 0 at first, and the record reports a score other
  than 0.
- `alert` comes with an update. The device lights up, the Dynamic Island expands (without one, the
  Lock Screen presentation shows as a banner), and an Apple Watch shows the title and the body.
  The sound is the system's default.

## 8. Settings of the kit

`ActivityKitBackend::new()` uses activitykit's default settings, and so does a builder without a
backend. For other settings, build the kit and hand it over:

```rust
use liveactivity::activitykit::{ActivityKit, Receipts};
use liveactivity::ios::ActivityKitBackend;
use liveactivity::{Event, LiveActivity, Result};

fn connect() -> Result<LiveActivity> {
    let kit = ActivityKit::builder()
        .receipts(Receipts::Redacted)
        .push_to_start(true)
        .build()?;
    LiveActivity::builder()
        .backend(ActivityKitBackend::with_kit(kit))
        .event_sink(|event: Event| { /* forward it */ })
        .build()
}
```

| Setting | Does |
| --- | --- |
| `receipts(Receipts::Full)` | the default: the Swift layer keeps the accepted values of each activity, in plain text, in a file of the app's Application Support |
| `receipts(Receipts::Redacted)` | keeps a SHA-256 digest of the accepted content instead of the values |
| `push_to_start(true)` | watches the app's push-to-start token and reports it as `Event::PushToStartToken` |

The settings hold for the whole process, and the first ones win. A second `build()` with other
settings answers `CONFLICT`; the same settings again are fine. Build the kit before any other use of
activitykit in the process.

Connect early at launch, before the app waits for anything. A tap, a push-to-start or a push that
wakes the app in the background reaches it only once the kit exists.

## Behavior to know

- **Foreground.** Start activities while the app is in the foreground. ActivityKit refuses a start
  from the background: `NOT_FOREGROUND`.
- **Threads.** The Swift side runs each request on the main thread and completes the future from
  there. Never block the main thread on a call: that deadlocks. See
  [framework integration](framework-integration.md#ios-never-block-the-main-thread).
- **Taps.** A tap runs the button's intent in the app process, also when the app was not running:
  the system launches it in the background. The tap is held until the event sink returns, for at
  most 4 seconds. A tap does not change the activity.
- **Permissions.** Live Activities are a switch in Settings. There is no prompt:
  `request_permissions(PermissionKind::LiveActivities)` answers `settingsRequired` when they are
  off. No notification permission is needed.
- **Ending again.** The backend keeps the handles of the last 64 activities it ended in this launch,
  so ending one again still answers its record after the system stops listing it.
- **Templates and layouts share keys.** A template and a layout activity cannot run under the same
  `activityKey`: the second start answers `CONFLICT`.

## Troubleshooting

**A start answers `CONFIGURATION_REQUIRED`.**

- `NSSupportsLiveActivities` is missing from the app's Info.plist.
- No embedded widget extension carries `ActivityKitLayout` = `2`. Check that the extension is
  embedded in the app and that the value is an integer, not a string.
- The start asks for a push token and the app has no Push Notifications capability, or runs in the
  simulator.

**`write_image` answers `CONFIGURATION_REQUIRED`.** The widget extension names no App Group under
`ActivityKitAppGroup`, or the app target does not have that group.

**A start answers `PERMISSION_DENIED`.** Live Activities are turned off for the app in Settings.
`permissions().liveActivities` is `denied`.

**A start answers `NOT_FOREGROUND`.** The app was not in the foreground.

**A start answers `SYSTEM_LIMIT`.** The app's or the device's limit of Live Activities is reached.
The message says which. End activities that are no longer needed.

**Every call answers `UNSUPPORTED_PLATFORM`.** The device is below iOS 16.2, an iPad below
iPadOS 17, or the target is not iOS.

**A start succeeds but nothing shows.** A successful call does not prove that anything was shown;
records keep `visibility: unknown`. Check that the extension links `ActivityKitWidget`, that its
bundle is `ActivityKitLayoutWidget()`, and that its deployment target is iOS 16.2 or later.

**A button does nothing.** The app target does not link `ActivityKitIntent`, or it compiles no
Swift file, so Xcode extracted no App Intents metadata. Below iOS 17 buttons are not drawn as
buttons.

**An image draws nothing.** The file was not written, or the `asset` name is not in the extension's
asset catalog. Neither is reported.

**Text is drawn in the system font.** The `fontFamily` is not a font that the extension bundles and
lists under `UIAppFonts`. This is not reported.

**The activity keeps showing older content after an update.** The update's `timestamp` is older
than that of the content shown. From iOS 17.2 ActivityKit drops such content, and the request still
succeeds.

**The linker reports duplicate symbols.** `ActivityKitBridge` is linked in Xcode. Remove it.

**The linker cannot find `swiftCompatibility56`.** The app target compiles no Swift and lacks the
Swift runtime's search paths (step 2).

**The app hangs on a call.** The main thread is blocked on it. Run the call on another thread.

**The builder answers `CONFLICT`.** The kit was built twice with different settings, or activitykit
was used before the kit was built with non-default settings.
