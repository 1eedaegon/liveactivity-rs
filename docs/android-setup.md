# Android setup

On Android `liveactivity` runs on the [liveupdate](https://github.com/1eedaegon/liveupdate-rs)
crate (0.1.3). An activity is an ongoing notification, which Android 16 and later can promote to a
Live Update. liveupdate keeps the lifecycle, the revisions and the receipts in Rust; a small AAR
talks to Android's `NotificationManager`.

```text
Rust -> liveactivity -> liveupdate (Rust) -> a transport -> liveupdate's AAR -> NotificationManager
A tap -> the AAR's receiver stores it -> sync_events() -> the event sink
```

## Requirements

- Android 8 (API 26) or later: the AAR's minimum SDK.
- Android 13 (API 33) or later asks the person for the notification permission.
- Android 16 (API 36) or later can promote the notification to a Live Update, when the person's
  settings and the device's system UI allow it. A successful post does not guarantee promotion.

## 1. Add the AAR

Put `liveupdate-android-0.1.3.aar` from liveupdate's
[release](https://github.com/1eedaegon/liveupdate-rs/releases) into `app/libs/`. The AAR is written
in Kotlin, and a file dependency brings no dependencies of its own, so add the Kotlin standard
library too:

```kotlin
dependencies {
    implementation(files("libs/liveupdate-android-0.1.3.aar"))
    implementation("org.jetbrains.kotlin:kotlin-stdlib:2.2.10") // or newer
}
```

Use the AAR of the liveupdate version this crate depends on. `liveactivity::liveupdate` re-exports
that crate.

## 2. The manifest

There is nothing to write. The AAR merges these into the application's manifest:

```xml
<uses-permission android:name="android.permission.POST_NOTIFICATIONS" />
<uses-permission android:name="android.permission.POST_PROMOTED_NOTIFICATIONS" />

<application>
    <receiver android:name=".LiveUpdateActionReceiver" android:exported="false" />
    <receiver android:name=".LiveUpdateDismissReceiver" android:exported="false" />
</application>
```

The receivers belong to the AAR's namespace, `io.github.eedaegon.liveupdate`. One stores taps on a
notification's actions, the other dismissals. Neither is exported. The AAR installs no foreground
service and no background work.

## 3. Choose a transport

`android::LiveUpdateBackend<D>` runs on any transport to the Android side. There are three ways to
make one.

| Constructor | Transport | Use it when |
| --- | --- | --- |
| `LiveUpdateBackend::android()` | the JNI transport of liveupdate's AAR | the app has no native layer of its own |
| `LiveUpdateBackend::over_bridge(bridge)` | a `liveupdate::Bridge` of the host | a framework reaches Android through its own native call |
| `LiveUpdateBackend::new(driver)` | a `liveupdate::Driver` of the host | the host implements the Android side itself, or tests with a fake |

### android()

```rust
use liveactivity::android::LiveUpdateBackend;

#[cfg(target_os = "android")]
let backend = LiveUpdateBackend::android()?;
```

It exists only on `target_os = "android"`. The host must have initialized
[`ndk-context`](https://crates.io/crates/ndk-context) and must package the AAR. Common
NativeActivity and Rust window runtimes initialize `ndk-context` before application code runs.
Without the context or the AAR's bridge class, `android()` fails with `NATIVE_ERROR`. This holds
while the build unwinds on panic, which is Rust's default; with `panic = "abort"` the process
aborts.

A builder without a backend uses this one:

```rust
let live = liveactivity::LiveActivity::builder().event_sink(sink).build()?;
```

That default has no files, so it runs the progress and timer templates only (step 4).

### over_bridge(bridge)

`liveupdate::Bridge` is one call that carries a command name and a JSON payload to the Android side
and brings the JSON answer back:

```rust
use liveactivity::android::LiveUpdateBackend;
use liveactivity::liveupdate::{self, Bridge};

struct MyBridge { /* the framework's native handle */ }

impl Bridge for MyBridge {
    fn invoke(&self, command: &str, payload: &str) -> liveupdate::Result<String> {
        // Hand `command` and `payload` to liveupdate's Android adapter and return its answer.
        todo!()
    }
}

let backend = LiveUpdateBackend::over_bridge(MyBridge { /* ... */ });
```

### new(driver)

`LiveUpdateBackend::new(driver)` takes any `liveupdate::Driver`. `over_bridge(bridge)` is
`new(JsonDriver::new(bridge))`.

## 4. Give the backend the app's files

Without files the backend runs templates. With them it also runs layouts, stores images and reports
taps with the activity's key:

```rust
use liveactivity::android::{AndroidFiles, LiveUpdateBackend};
use liveactivity::{Event, LiveActivity, Result};
use std::path::PathBuf;

#[cfg(target_os = "android")]
fn connect(app_name: &str, files_dir: PathBuf) -> Result<LiveActivity> {
    let backend = LiveUpdateBackend::android()?
        .with_files(AndroidFiles::new(app_name, files_dir));
    LiveActivity::builder()
        .backend(backend)
        .event_sink(|event: Event| { /* forward it */ })
        .build()
}
```

`files_dir` is the folder of the app's private files, `Context.getFilesDir()`. The host supplies
it.

| `AndroidFiles` field | Meaning | `AndroidFiles::new` sets |
| --- | --- | --- |
| `app_name` | the title of a layout that shows no text; a notification needs a title | the first argument |
| `files` | the folder of the app's files | the second argument |
| `layout_store` | the file of the layout activities, relative to `files`. It holds each layout request and its latest values, which liveupdate does not keep | `liveactivity/layouts.json` |
| `image_folder` | the folder of `write_image`, relative to `files`. liveupdate's AAR reads pictures from it | `liveactivity/images` |
| `samsung_now_bar` | adds Samsung One UI's Now Bar text extras to layout notifications on Android 17 and later | `false` |

What changes with files:

| | Without `with_files` | With `with_files` |
| --- | --- | --- |
| `start`, `update`, `end`, `list` | yes | yes |
| `start_layout`, `update_layout`, `end_layout` | `UNSUPPORTED_FEATURE` | yes |
| `list_layout` | an empty list | the layout activities |
| `write_image` | `UNSUPPORTED_FEATURE` | yes |
| `capabilities().layout`, `.actions` | `unsupported` | `supported` |
| A tap's `activityKey` | absent | the key of the activity |

A notification cannot draw a layout. The backend projects the lock screen tree onto a notification
style and reports what it left out in `degradedFeatures`:
[What Android shows](contract.md#what-android-shows).

The layout store keeps values in plain text in the app's private files. Do not put secrets in an
activity.

### An Android content

A layout is drawn by iOS; on Android the backend projects it onto a notification. When the
projection is not what the application wants, a snapshot says what the notification shows, in
Android's own terms: a progress with segments and points, metrics, the text of the status chip,
icons of actions. It needs the same files as layouts.

```rust
use liveactivity::{AndroidContent, AndroidPresentation, PresentationStyle, ProgressSegment, ProgressStyle};

let mut snapshot = LayoutSnapshot::new(1, values);
snapshot.android = Some(AndroidContent {
    title: "Delivery 7".into(),
    status: "2 stops away".into(),
    presentation: AndroidPresentation {
        style: PresentationStyle::Progress(ProgressStyle {
            progress: 60,
            indeterminate: false,
            styled_by_progress: true,
            segments: vec![
                ProgressSegment { length: 60, id: None, color: Some(0xFF00_7F00), semantic_style: None },
                ProgressSegment { length: 40, id: None, color: None, semantic_style: None },
            ],
            points: Vec::new(),
            start_icon: None,
            end_icon: None,
            tracker_icon: None,
        }),
        short_critical_text: Some("8 min".into()),
        large_icon: None,
        actions: Vec::new(),
        samsung_now_bar: None,
    },
});
live.update_layout(LayoutUpdateInput::new(&record.id, snapshot)).await?;
```

Ask `live.capabilities().await?.android_presentation` which styles the device draws before
choosing one: the progress style needs Android 16, the metric style Android 17. Android keeps the
style of the first snapshot and answers `UNSUPPORTED_FEATURE` to a content of another style, so
give a content with every snapshot of an activity or with none.
The rules are in [the contract](contract.md#an-android-content).

## 5. Permissions

`live.permissions()` reports what Android allows:

```json
{ "notifications": "denied", "liveActivities": "notApplicable",
  "promotion": "granted", "frequentPushes": "notApplicable" }
```

The backend reads the settings and cannot tell a refusal from a prompt that was never shown, so a
disabled permission is `denied`.

### Notifications, Android 13 and later

The AAR declares `POST_NOTIFICATIONS`. The app still has to ask the person at run time. Android's
permission dialog needs an Activity, which the backend does not have, so
`request_permissions` on `LiveUpdateBackend` answers `UNSUPPORTED_FEATURE`. The host shows the
dialog.

A host with an Activity puts its dialog in a backend of its own around the default one.
[`examples/adapter.rs`](../crates/liveactivity/examples/adapter.rs) has the whole type,
`WithPermissionDialog`. Its one method of its own:

```rust
impl<B: LiveActivityBackend, D: Dialog> LiveActivityBackend for WithPermissionDialog<B, D> {
    fn request_permissions(&self, kind: PermissionKind) -> BackendFuture<'_, Permissions> {
        Box::pin(async move {
            self.dialog.ask(kind)?;          // the host shows the system's dialog and waits
            self.inner.permissions().await   // then the backend reads the new state
        })
    }

    // Every other method hands the call to `self.inner`.
}
```

`Dialog` stands for the host's own code. Ask after an action of the person, not at launch. With
notifications disabled for the app or its channel, a start answers `PERMISSION_DENIED`.

### Promotion, Android 16 and later

The AAR declares `POST_PROMOTED_NOTIFICATIONS`. Promotion is not a run-time dialog. It follows the
person's settings and the device's system UI. `permissions().promotion` is `granted` or `denied`,
and `notApplicable` where the device has no Live Updates. `capabilities().nativeLiveActivity` is
`supported` when a notification can be promoted now.

What a start does without promotion depends on `fallback`:

- `notification` posts an ordinary ongoing notification. The record has
  `promotionState: notPromoted` and lists `nativeLiveActivity` in `degradedFeatures` once Android
  reports it.
- `none` answers `UNSUPPORTED_FEATURE` and posts nothing.

The system can still decline or later remove promotion. The record reports what Android did in
`promotionRequested` and `promotionState`.

## 6. Deliver taps

Android stores taps. liveupdate's receiver writes each one to a durable queue, also while the app
does not run. The backend hands them to the event sink when it is asked:

```rust
live.sync_events().await?;
```

Call it:

- once at startup, after the builder connected the sink;
- every time the application resumes;
- when the Android side signals that an event was stored.

For the signal, the AAR calls one listener after each stored event:

```kotlin
import io.github.eedaegon.liveupdate.LiveUpdateEvents

LiveUpdateEvents.setListener {
    // Called on the receiver's thread. Keep it short: tell Rust to drain, from another thread.
}
```

`LiveUpdateBackend::pump_events()` is the same drain as a direct call. It answers how many taps it
handed over, oldest first. A host that keeps the backend in an `Arc` can call it from the thread
its listener wakes:

```rust
let backend = Arc::new(LiveUpdateBackend::android()?.with_files(files));
let live = LiveActivity::builder().backend(backend.clone()).event_sink(sink).build()?;

let delivered = backend.pump_events()?;
```

Rules of the drain:

- Before the builder connected the sink, `pump_events` hands over nothing and leaves the taps
  queued.
- A builder without an event sink connects one that drops events. A drain then removes the taps.
  Name the sink before the first drain.
- With files, a tap carries the activity's key, and a tap on an `actionId` that the activity's
  layout never declared is dropped.
- Dismissals need no event. `list()` and `list_layout()` reconcile with the notifications Android
  shows: a notification the person removed becomes `dismissed`.

A tap on a notification of a killed app starts the process and runs the receiver, which stores the
tap. Android starts no Activity for it. The tap reaches the sink at the next `sync_events`, for
example when the person opens the app.

## 7. Samsung Now Bar

```rust
let files = AndroidFiles {
    samsung_now_bar: true,
    ..AndroidFiles::new(app_name, files_dir)
};
```

Layout notifications then carry One UI's text extras on Android 17 and later: the title as the
primary text, and the status line as the secondary text, or the chip text when the layout has no
second text. Template activities never carry them.

One UI decides on its own whether a Live Update appears in the Now Bar. The option only supplies
the text. It does not place the app there.

## 8. 16 KB pages

A library linked with an NDK before r28 is aligned to 4 KB pages, which Android warns about on
devices with 16 KB pages. Use NDK r28 or later, or pass the linker flag from the application's
`build.rs`:

```rust
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-arg=-Wl,-z,max-page-size=16384");
    }
}
```

## Behavior to know

- **Threads.** A call runs synchronously on the thread that polls it: a storage write and a
  framework call. Keep it off the UI thread.
- **One backend per process.** Build one `LiveActivity` and keep it, so that operations share one
  serialization boundary.
- **End policies.** `immediate` and `default` both remove the notification. `after` answers
  `UNSUPPORTED_FEATURE`.
- **Limits.** liveupdate keeps at most 32 active records and 128 ended ones, templates and layouts
  together. One more active activity answers `SYSTEM_LIMIT`.
- **Pushes.** Android has none. A layout start's `push` is dropped, `push_tokens()` is empty and
  `capabilities().push` is `unsupported`.
- **Visibility.** Records keep `visibility: unknown`. Asking for private content on the lock screen
  does not prove what the system UI revealed.
- **Lock screen.** A layout start's `android.visibility` is `public`, `private` or `secret`.
  Templates are `private`.

## Troubleshooting

**`build()` or `LiveUpdateBackend::android()` answers `NATIVE_ERROR`.** The host has not initialized
`ndk-context`, or the app does not package liveupdate's AAR. Check both Gradle lines of step 1.

**The process aborts at startup.** The build uses `panic = "abort"` and `ndk-context` was not
initialized when `android()` ran.

**A start answers `PERMISSION_DENIED`.** Notifications are disabled for the app or its channel.
Show the permission dialog (step 5), or send the person to the app's notification settings.

**A start answers `UNSUPPORTED_FEATURE`.** The request has `fallback: none` and promotion is not
available. Use `notification`, or check `capabilities().nativeLiveActivity` first.

**`start_layout` or `write_image` answers `UNSUPPORTED_FEATURE`.** The backend has no files. Connect
it with `with_files` (step 4). A builder without a backend never has them.

**`request_permissions` answers `UNSUPPORTED_FEATURE`.** The default backend cannot show a dialog.
Wrap it (step 5).

**`end` answers `UNSUPPORTED_FEATURE`.** The policy is `after`, which Android does not support.

**The notification is not promoted.** `promotionState` is `notPromoted`. Promotion needs
Android 16 or later, enabled notifications, the person's promotion setting and the device's system
UI. The app cannot force it.

**Taps do not arrive.** Nothing called `sync_events`; or the builder had no event sink; or the
backend has files and the layout does not declare the tapped `actionId`.

**An image is not the notification's large icon.** The record lists `layout.image`. The file was
not written before the start or update, or it is larger than 1024 pixels a side or 1 MiB. Only the
first shown `image` with a `file` source is used.

**Parts of a layout are missing.** A notification shows a title, a status line, one progress or
timer, up to three actions and one icon. The record's `degradedFeatures` names the rest.

**A start answers `SYSTEM_LIMIT`.** 32 activities are active. End the ones that are done.

**Android warns about page size.** See step 8.
