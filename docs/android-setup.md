# Android setup

On Android `liveactivity` runs on the [liveupdate](https://github.com/1eedaegon/liveupdate-rs)
crate (0.7.1 or later). An activity is an ongoing notification, which Android 16 and later can promote to a
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

Put `liveupdate-android-0.7.1.aar` from liveupdate's
[release](https://github.com/1eedaegon/liveupdate-rs/releases) into `app/libs/`. The AAR is written
in Kotlin, and a file dependency brings no dependencies of its own, so add the Kotlin standard
library too, and in a watch application the library of [its Ongoing Activity](#a-watch):

```kotlin
dependencies {
    implementation(files("libs/liveupdate-android-0.7.1.aar"))
    implementation("org.jetbrains.kotlin:kotlin-stdlib:2.2.10") // or newer
    implementation("androidx.wear:wear-ongoing:1.1.0") // a watch application only
}
```

Use the AAR of the liveupdate version this crate depends on; [the two versions](#9-a-crate-and-an-aar-of-two-versions)
has what happens when they differ. `liveactivity::liveupdate` re-exports that crate. The AAR of 0.2 does not fit: it had another package name, which liveupdate 0.3 took
back, and the JNI transport looks for the name of the version it was built with.

## 2. The manifest

There is nothing to write. The AAR merges these into the application's manifest:

```xml
<uses-permission android:name="android.permission.POST_NOTIFICATIONS" />
<uses-permission android:name="android.permission.POST_PROMOTED_NOTIFICATIONS" />

<application>
    <receiver android:name=".LiveUpdateActionReceiver" android:exported="false" />
    <receiver android:name=".LiveUpdateDismissReceiver" android:exported="false" />
    <receiver android:name=".LiveUpdateStartReceiver" android:exported="false" />
</application>
```

The receivers belong to the AAR's namespace, `io.github.eedaegon.liveupdate`. One stores taps on a
notification's actions, another dismissals, and the third posts
[a scheduled start](#a-scheduled-start) when its alarm fires. None is exported. The AAR installs no
foreground service and no background work, and declares no permission for alarms.

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

Where the native call fails, `invoke` returns an error, which the application reads as
`NATIVE_ERROR`. A call that panics instead ends the call of the application that made it, and the
thread that polled it. The backend catches no panic and keeps working: what the call had begun is
left as by a native call that failed at that point, and the events that a drain had taken by then
reach the sink with the next call or drain. Where the framework's call can panic, catch it in
`invoke` (`std::panic::catch_unwind`) and return an error: a thread the host keeps for the drain
lives on, and the events a drain took are handed over by that drain.

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
        short_critical_text: Some("8 min".into()),
        // Every member that is not named stays unset.
        ..AndroidPresentation::new(PresentationStyle::Progress(ProgressStyle {
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
        }))
    },
});
live.update_layout(LayoutUpdateInput::new(&record.id, snapshot)).await?;
```

Ask `live.capabilities().await?.android_presentation` what the device draws and offers before
choosing: the progress style needs Android 16, the metric style Android 17. A snapshot may have
another style than the one the activity shows where the device offers `style_change`, which the
AAR of 0.2 does; elsewhere that is `UNSUPPORTED_FEATURE`.
The rules are in [the contract](contract.md#an-android-content).

### Links

A layout's `url` is what a tap on the notification opens, and each of its `link` nodes is an
action that opens its URL: nothing to write for Android, where the device offers `links`. A
content may name them itself, the URL of the notification and an action that opens one instead
of sending a tap:

```rust
snapshot.android = Some(AndroidContent {
    title: "Delivery 7".into(),
    status: "2 stops away".into(),
    presentation: AndroidPresentation {
        open_url: Some("myapp://delivery/7".into()),
        actions: vec![NotificationAction {
            open_url: Some("myapp://delivery/7/map".into()),
            ..NotificationAction::new("map", "Map")
        }],
        ..AndroidPresentation::new(PresentationStyle::Standard)
    },
});
```

liveupdate opens the URL with `ACTION_VIEW` inside the application's own package only, so one of
its activities, exported or not, needs an intent filter for it:

```xml
<activity android:name=".MainActivity" android:exported="true">
    <intent-filter>
        <action android:name="android.intent.action.VIEW" />
        <category android:name="android.intent.category.DEFAULT" />
        <data android:scheme="myapp" />
    </intent-filter>
</activity>
```

The activity reads the URL from its intent, as for any deep link. Without such an activity
liveupdate refuses to post a URL. For a content the request then fails with `INVALID_INPUT`, and
nothing is posted. For a layout it does not fail: the snapshot is shown without its links, the
notification opens the launch activity as before, and the record names `layout.url` and
`layout.link` in `degradedFeatures`. That costs a second post with every snapshot, so declare the
scheme.

To learn in Rust that the person opened the application from a notification, hand the opening
intent to liveupdate in that activity. The sink then receives `Event::Opened`, with the
activity's id and key and the action that opened it, on the next `sync_events`:

```kotlin
import io.github.eedaegon.liveupdate.LiveUpdateEvents

override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    // A new intent, not one the system restored.
    if (savedInstanceState == null) LiveUpdateEvents.opened(this, intent)
}

override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    LiveUpdateEvents.opened(this, intent)
}
```

### A last content that stays, a header clock, a lock screen version, branding

They are members of the content too, each behind a capability of its own:
[the table in the contract](contract.md#an-android-content). An end with the `after` policy keeps
the last content on screen, no longer ongoing and no longer promoted, until its time.

A layout gets two of them without a content. A timer beside a progress is the clock in the
header. And a start may say what a secure lock screen shows of a private activity:

```rust
let mut options = AndroidOptions::new(AndroidVisibility::Private);
options.public_version = Some(PublicVersion {
    title: "Delivery".into(),
    status: "On the way".into(),
});
start.android = Some(options);
```

### A foreground service

Android shows one notification for a foreground service. An application that updates an activity
from such a service, in the background, lets the service hold the activity's notification instead
of a second one. The start asks for it:

```rust
let mut options = AndroidOptions::new(AndroidVisibility::Private);
options.placement = Some(AndroidPlacement::Foreground);
start.android = Some(options);
let record = live.start_layout(start).await?;
```

The service, its type and its permission are the application's; liveupdate's AAR declares none.
Start the activity first and the service second, and attach the service in Kotlin:

```kotlin
import io.github.eedaegon.liveupdate.LiveUpdateForeground

class DeliveryService : Service() {
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // The activity's notification becomes this service's. False when no activity has the placement.
        if (!LiveUpdateForeground.attach(this, ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION)) stopSelf()
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null
}

// Once, for example in Application.onCreate: an end takes the service out of the foreground.
LiveUpdateForeground.setListener { stopService(Intent(this, DeliveryService::class.java)) }
```

One running activity has the placement at a time: a second start with it is `SYSTEM_LIMIT`. A
device that does not offer `foregroundPlacement` answers `UNSUPPORTED_FEATURE`. Without a service
the notification is an ordinary one.

### A scheduled start

A start with `scheduled` is stored now and begins at its time, also while the application does not
run: the AAR sets an alarm, and its receiver posts the notification from what it stored. No Rust is
loaded then. Until the time the record is `pending`, and what happened meanwhile is answered at
the application's next call ([the contract](contract.md#a-scheduled-start) has the rules).

```rust
let offered = live.capabilities().await?.android_presentation.unwrap_or_default();
if offered.scheduled_start {
    let mut later = start.clone();
    later.scheduled = Some(ScheduledStart::new(at, Alert::new("Your ride", "It starts now")));
    let record = live.start_layout(later).await?;
    assert_eq!(record.state, ActivityState::Pending);
}
```

How close to its time it begins is Android's to say:

| The application | Begins |
| --- | --- |
| may set exact alarms: `exact_start` is true | at its time, also while the device dozes |
| may not, as most applications | late by up to three quarters of the wait: ten minutes at most while the device is awake, an hour while it dozes |

The manifest is the application's for both of the following; the AAR declares neither.

- **On the minute.** Declare `SCHEDULE_EXACT_ALARM` and send the person to the "Alarms &
  reminders" setting: from Android 14 it is off until they allow it. `USE_EXACT_ALARM` is for alarm
  and calendar applications only, by Google Play's policy, and needs no setting. Android 12 does not
  know `USE_EXACT_ALARM`: an application that uses it also declares `SCHEDULE_EXACT_ALARM` with
  `android:maxSdkVersion="32"`, which Android 12 allows without asking.

  ```xml
  <uses-permission android:name="android.permission.SCHEDULE_EXACT_ALARM" />
  ```

- **After a restart of the device.** Android drops alarms when the device restarts and when the
  person force-stops the application. The activity then begins late, at the application's next
  call. When the person takes the permission for exact alarms back, Android drops the exact alarm
  and stops the application: the activity then begins by the inexact alarm that the AAR sets beside
  the exact one, from 0.4.1. To have the alarms set again after a restart, declare the AAR's
  receiver for it:

  ```xml
  <uses-permission android:name="android.permission.RECEIVE_BOOT_COMPLETED" />

  <receiver
      android:name="io.github.eedaegon.liveupdate.LiveUpdateStartReceiver"
      android:exported="false">
      <intent-filter>
          <action android:name="android.intent.action.BOOT_COMPLETED" />
      </intent-filter>
  </receiver>
  ```

An application that runs at the time and asks the backend when [`next_check`](#states) says has the
activity begin on the dot, whatever the alarm does. An activity that must not appear late is the
application's to end: compare the time with the clock at launch. Android asks that a Live Update
appears only when its activity is imminent, and mostly for activities the person started: schedule
the start of something they asked for, not a reminder.

liveupdate followed this on the emulators of Android 16 and 17, with a package that targets API 36
([its notes](https://github.com/1eedaegon/liveupdate-rs/blob/main/docs/testing.md)). With exact
alarms a start began within a second of its time, awake and in Doze. Without them, a start 45
seconds ahead began 32 seconds late, and one 20 minutes ahead 5 minutes late while the device was
awake and 15 minutes late in Doze. After a restart with the receiver declared, the alarms were set
again 84 seconds after the device was up. An update of the application kept them, and a force stop
removed them until the application ran again. An application that nobody had opened, on a device
that did not charge, had its inexact alarms put off: its start did not begin in the ten minutes it
was watched, while an exact alarm came on time. Not verified there: a real phone and the battery
savers of device makers.

### A watch

In a watch application the same calls put the activity on the watch face, as an icon that opens the
application, and among the recent applications: the Ongoing Activity of Wear OS, below Wear OS 7.

1. Add `androidx.wear:wear-ongoing` to the watch application, as in [Add the AAR](#1-add-the-aar). The
   AAR uses it when it is there and does not bring it: a phone application adds nothing.
2. Build the Rust library for `armeabi-v7a` too (`armv7-linux-androideabi`): a watch such as the Galaxy
   Watch4 runs 32-bit code only.
3. Start and update as on a phone. The icon, the title, the text and a header clock are the
   notification's, and a tap opens what a tap on the notification opens. An update may turn the Ongoing
   Activity off or on; a notification that a foreground service holds keeps its icon until the activity
   ends. The icon is all the watch shows of an activity: Wear OS does not list an ongoing notification
   among its notifications, and a revision that alerts does not pop up there.

`capabilities().android_presentation` says `wear_ongoing_activity` where the watch shows it; it is there
when the backend has [the app's files](#4-give-the-backend-the-apps-files). One activity can stay in the
tray: `AndroidOptions::new(visibility).with_wear_ongoing_activity(false)` for a layout,
and `wear_ongoing_activity: false` in the presentation of an Android content
([contract](contract.md#a-watchs-ongoing-activity)). A template's activity (`start`) has no such choice and
always carries it.

By liveupdate's account, which ran its library on a Galaxy Watch4 with Wear OS 5: it is for Wear OS 3 to
6, older watches ignore it, Wear OS 7 shows Live Updates itself, and on that watch a phone's activity stayed
on the phone. By Android's documentation an ongoing notification is not bridged to a paired watch, and a Live
Update is bridged to some watches, by the Wear OS version and the maker. This crate ran on no watch.

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

## 6. Deliver taps and states

On Android nothing calls the application: it asks. liveupdate's receiver writes each tap to a
durable queue, also while the app does not run, and liveupdate keeps with its receipts which
state of each activity the application was told last. The backend hands both to the event sink
when it is asked:

```rust
live.sync_events().await?;
```

Call it:

- once at startup, after the builder connected the sink;
- every time the application resumes;
- when the Android side signals that an event was stored;
- at the time the backend names, when the host has a thread for it (below).

For the signal, the AAR calls one listener after each stored event, a tap or a dismissal:

```kotlin
import io.github.eedaegon.liveupdate.LiveUpdateEvents

LiveUpdateEvents.setListener {
    // Called on the receiver's thread. Keep it short: tell Rust to drain, from another thread.
}
```

`LiveUpdateBackend::pump_events()` is the same drain as a direct call. It answers how many events
it took for the sink: the stored taps and openings, oldest first, then the states. A host that keeps the
backend in an `Arc` can call it from the thread its listener wakes:

```rust
let backend = Arc::new(LiveUpdateBackend::android()?.with_files(files));
let live = LiveActivity::builder().backend(backend.clone()).event_sink(sink).build()?;

let delivered = backend.pump_events()?;
```

### States

The sink is told the state an activity has when the backend asks, if it is another than the one it
was told last: `pending` while a scheduled start waits, `active` once Android shows it, `stale`
after its stale time, `active` again after an update that makes it fresh, `ended`, `dismissed`. A start, an update, an end and a list report
the states that are due with them; the others come with a drain. A state that came and went
between two asks is not reported.

Android tells nobody that a stale time has come, and the backend runs no timer, so a host whose
thread waits for the listener's signal also asks the backend how long to wait:

```rust
use liveactivity::liveupdate::NextCheck;

match backend.next_check()? {
    // A state is due.
    NextCheck::Now => { backend.pump_events()?; }
    // Android has not shown a post yet: ask shortly, a little later each time, and give up after a few.
    NextCheck::Soon => wait_a_moment(),
    // The earliest stale time ahead, in epoch milliseconds: sleep until then, or until the signal.
    // A time that has passed means now.
    NextCheck::At(time) => wait_until(time),
    // Nothing until the next call of the application, signal or resume.
    NextCheck::Never => wait_for_the_signal(),
    // An answer that a later liveupdate adds: shortly, as for `Soon`.
    _ => wait_a_moment(),
}
```

`next_check` reads what liveupdate stored and asks Android nothing. It does not know that the
person removed a notification: the listener's signal is what prompts that drain. A host without
such a thread reports a stale activity at its next drain, at the latest when the application
resumes.

Rules of the drain:

- Before the builder connected the sink, `pump_events` hands over nothing and leaves the taps
  queued and the states due.
- A builder without an event sink connects one that drops events. A drain then removes the taps.
  Name the sink before the first drain.
- With files, a tap carries the activity's key, and a tap on an `actionId` that the activity's
  layout never declared is dropped. A state carries the key with or without files.
- A dismissal is one of the states. The event Android stores for it only prompts the drain, which
  reports `dismissed` once, when the notification is gone.
- The sink is called for one event at a time, in the order the events were taken, with no lock
  held: it may call the service, a drain included. When two threads drain or call at once, one of
  them hands over what both took, and the other returns before its events arrived.
- A state is handed over at most once. When the read of the states fails after the stored events
  were taken, those reach the sink and the drain fails: drain again.
- A sink does not panic. One that does loses the event it was handed, and the call that was
  handing over panics with it; what was queued behind reaches the sink with the next call or
  drain.

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

## 9. A crate and an AAR of two versions

An application takes this crate, and liveupdate with it, from Cargo and the AAR as a file, so the two
are not always of one version. They are tested as a pair of one version. Apart, by liveupdate's account:

| liveupdate in the crate | AAR | Together |
| --- | --- | --- |
| Later | 0.7.0 or later | The activity is posted. A member that the AAR does not know is not shown, and its capability reads `false` |
| Later | Up to 0.6.0 | A request with a member that the AAR does not know is `INVALID_INPUT`: `wearOngoingActivity`, with an AAR before 0.6.0. One without it works |
| Earlier | Later | It works: liveupdate reads what it knows of a reply. An event of a kind it does not know reaches the sink as `Event::Unknown`; before liveupdate 0.7.0 the events taken with it were lost |

A member that an activity depends on, a start time or the foreground placement, is never left out
silently: where the AAR does not report its capability the request is `UNSUPPORTED_FEATURE`. A crate
that is earlier than the one that stored the receipts does not read a receipt with a member of the later
one, and answers `NATIVE_ERROR` until the application's data is cleared: do not go back to an earlier
crate over the data of a later one.

## Behavior to know

- **Threads.** A call runs synchronously on the thread that polls it: a storage write and a
  framework call. Keep it off the UI thread.
- **One backend per process.** Build one `LiveActivity` and keep it, so that operations share one
  serialization boundary.
- **End policies.** `immediate` and `default` both remove the notification. `after` keeps the last
  content until its time, as an ordinary notification that the person can remove; a second end
  without a time removes it early. A device that does not offer `delayedEnd` answers
  `UNSUPPORTED_FEATURE`.
- **Alerts.** An update with an `alert` is posted on a second channel of high importance, "Live
  update alerts", which liveupdate creates with the first one. A person who blocked that channel
  gets the update silently. The sound is that channel's, which the person chooses: a `sound` that
  the alert names is not played, and the record names `alert.sound`.
- **Toggles.** A `toggle` of a layout is an action of the notification, titled by its label, with
  `✓` before it while its value is on. A tap reaches the sink with `on`, the other state than the
  value of the activity's latest snapshot, and changes nothing: update the value to move it.
- **Staleness.** Nothing redraws a notification when a time passes. A layout whose nodes say
  `ifStale` is drawn as it is while fresh, and the record names `layout.ifStale`. The sink is told
  `stale` ([States](#states)): update the content then if the notification should say so.
- **The small region.** It is iOS's, for a paired Apple Watch and CarPlay, and a notification has
  no place for it: nothing of a layout's `small` is drawn, and the record names `layout.small`. On
  Android a phone's ongoing notification is not bridged to a paired watch, and a Live Update only to
  some; Android Auto shows the notifications of messaging applications built for it, and Android's
  documents name no place there for a Live Update. An application on a watch shows its own activity
  on the watch face: [A watch](#a-watch).
- **A scheduled start.** The AAR posts the notification at the time, through an alarm and a
  receiver of its own: see [A scheduled start](#a-scheduled-start). With an AAR older than 0.4 it
  is `UNSUPPORTED_FEATURE`.
- **Updates need the process.** Android has no push that updates a notification. In the
  background an application updates from a foreground service or from a data message that wakes
  it. A start with the foreground placement lets such a service hold the notification
  ([A foreground service](#a-foreground-service)).
- **Limits.** liveupdate keeps at most 32 active records and 128 ended ones, templates and layouts
  together. One more active activity answers `SYSTEM_LIMIT`. It stores them in at most 1,000,000 bytes
  at a start and at an update: ended ones go, the oldest first, sooner than the 128th when the contents
  are large, and a start or an update for which the live ones alone leave no room answers `SYSTEM_LIMIT`
  too. That takes some thirty live activities with the longest texts an Android content has; ending one
  makes room.
- **A time long past, on Android 16.** Android 16 shows nothing of a notification whose time is
  more than two weeks old, and tells the application nothing: the post is answered as shown. For a
  timer that is a start or an end that far before the post (measured on an Android 16 emulator:
  13 days shows, 15 days does not).
- **An update of the application.** Android removes an application's notifications when the
  application is updated, so an activity that ran across the update is `dismissed`. liveupdate 0.3
  reads what 0.1 stored and not what 0.2 stored: after an update from a release on liveupdate 0.2
  the activities that were running are gone from the lists and their ids are `NOT_FOUND`. Their
  keys start new activities.
- **Pushes and the iOS options.** Android has no pushes. A layout start's `push` and `ios` are
  dropped, `push_tokens()` is empty and `capabilities().push` is `unsupported`.
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

**A start with `scheduled` answers `UNSUPPORTED_FEATURE`.** The application packages an AAR of
liveupdate older than 0.4, or the host's own Android side does not report `scheduledStart`; or the
start asks for the foreground placement, which a scheduled start cannot have.

**A scheduled activity begins minutes late.** The application may not set exact alarms
(`exactStart` is false): see [A scheduled start](#a-scheduled-start).

**A scheduled activity is `pending` and never begins.** Its start answered an error: the alarm
could not be set, and the activity is kept without one. Send the same start again, or end it.

**`end` answers `UNSUPPORTED_FEATURE`.** The policy is `after` and the device does not offer
`delayedEnd`: the AAR is older than 0.2, or the host's own Android side does not report it.

**An update answers `UNSUPPORTED_FEATURE`.** Its snapshot has another notification style than the
activity shows, and the device does not offer `styleChange`.

**A layout with a `progress`, or a content of the progress style, answers `NATIVE_ERROR` on
Android 16.** The application packages an AAR of liveupdate older than 0.3.2, which calls, for
every segment and point of a progress, a method that only Android 17 has. Take the AAR this guide names:
the correction is in the AAR, so a newer crate alone does not bring it. The progress and timer
templates were not touched, nor were Android 15 and earlier or Android 17.

**A start or an update with a content answers `INVALID_INPUT` on Android only.** The content names
a URL that no activity of the application opens, a small icon that is no resource of it, or a
public version for an activity that did not start as private.

**The notification is not promoted.** `promotionState` is `notPromoted`. Promotion needs
Android 16 or later, enabled notifications, the person's promotion setting and the device's system
UI. The app cannot force it.

**Taps do not arrive.** Nothing called `sync_events`; or the builder had no event sink; or the
backend has files and the layout does not declare the tapped `actionId`.

**An image is not the notification's large icon.** The record lists `layout.image`. The file was
not written before the start or update, or it is larger than 1024 pixels a side or 1 MiB. Only the
first shown `image` with a `file` source is used.

**Parts of a layout are missing.** A notification shows a title, a status line, a progress, a
timer, up to three actions and one icon. The record's `degradedFeatures` names the rest. A picture
with an accessibility label is shown as that text, and a button whose label is a picture needs
one to become an action.

**A layout's links do not open.** The record names `layout.url` or `layout.link`: no activity of
the application declares the URL's scheme, or liveupdate does not take the URL (a scheme with a
capital letter, or one of `intent`, `content`, `file`, `javascript`, `data`, `android-app`).

**A start answers `SYSTEM_LIMIT`.** 32 activities are active. End the ones that are done.

**Android warns about page size.** See step 8.
