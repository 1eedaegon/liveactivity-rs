# Changelog

Releases are prepared by `tools/release.sh` from the subjects of the commits since the last tag.

## Unreleased

Built on liveupdate 0.7.1, with its AAR `liveupdate-android-0.7.1.aar`. Three corrections of liveupdate's, two
of them in the AAR; nothing changes in this crate's own Rust or in the contract's JSON.

- `ProgressStyle::new`, which this crate re-exports, builds a style that is styled by its progress: what is
  ahead of the progress is shown fainter, as Android's own default is. With 0.7.0 it built one that Android 16
  shows with every segment in full, the same at 0 and at 100 unless it has a tracker icon. A style read from
  JSON is as it was: not styled when the member is left out.
- On a watch the icon of an Ongoing Activity goes with the update that turns it off, and with the end. A watch
  shows it until the notification is cancelled, so the AAR now cancels before it posts a snapshot that carries
  none. A notification that a foreground service holds keeps its icon until the activity ends.
- Below Android 17 a metric's decimal has at most two fraction digits, as Android 17's own style writes it; the
  AAR of 0.7.0 wrote every digit.
- The contract says what liveupdate saw of One UI 9 on a Galaxy S23: with `samsungNowBar` the Now Bar and its
  card show the two lines alone, a point at the full length of a track is not drawn, the status chip shows the
  first metric, and a long metric value is cut.

When updating: take `liveupdate-android-0.7.1.aar`. Code that built a progress style on `ProgressStyle::new`
and wants every segment in full says `styled_by_progress: false`.

## 0.8.0 - 2026-11-19

- feat: build on liveupdate 0.7: a watch's Ongoing Activity, an unknown event, enums and structs that may grow (#26) (`47d5667`)

Built on liveupdate 0.7.0, from 0.4.1, with its AAR `liveupdate-android-0.7.0.aar`. Breaking for a Rust caller:
the vocabulary of a notification that this crate re-exports is liveupdate's, whose enums are all
`#[non_exhaustive]` now and whose `AndroidPresentation` and `PresentationCapabilities` have a new member; this
crate's `Template` and `EndPolicy` are `#[non_exhaustive]` too, and `Event` has a new variant. JSON written for
0.7 reads unchanged.

- **A watch's Ongoing Activity.** In a watch application whose Gradle file has `androidx.wear:wear-ongoing`,
  liveupdate's library puts the activity on the watch face of a watch below Wear OS 7, as an icon that opens
  the application, and among the recent applications. The capabilities say `wearOngoingActivity` where the
  device does it. One activity can do without it: a layout with the Android option `wearOngoingActivity: false`
  of its start (`AndroidOptions::with_wear_ongoing_activity`), for all of its snapshots, and an Android content
  with the same member in its presentation, for its snapshot. A template has no such choice and always
  carries it. A phone carries none whatever a request says.
  This crate ran on no watch: liveupdate ran its library on a Galaxy Watch4 with Wear OS 5.
- **An event of a kind that is not known** reaches the sink as `Event::Unknown` with the activity's id and key.
  liveupdate reads such an event of a later AAR for what it knows since 0.7.0; before, the events taken with
  it were lost, and this crate dropped a kind it had no event for.
- **Code that compiles against the next version.** Every public enum is `#[non_exhaustive]`: match with a `_`
  arm, also liveupdate's `NextCheck`, which the backend's `next_check` answers. Every struct that a caller
  builds has a constructor that takes what is required: new are `Timer::new`, `Snapshot::progress` and
  `Snapshot::timer`, `StartInput::new`, `UpdateInput::new`, `EndInput::after`, `AndroidContent::new` and
  `ImageInput::new`, and liveupdate brings `BigTextStyle::new`, `ProgressStyle::new`, `ProgressSegment::new`,
  `ProgressPoint::new`, `TimerStyle::new`, `Metric::new`, `MetricStyle::new`, `PublicVersion::new` and
  `SamsungNowBar::default()`, and for a caller of liveupdate itself, through `liveactivity::liveupdate`, its
  `Snapshot::new`, `StartRequest::new`, `Timer::new`, `Observation::new`, `StartReport::new` and
  `ActivityEvent::new`.
- **The room of liveupdate's store.** A start and an update store liveupdate's receipts in at most 1,000,000
  bytes. Ended activities go for the room, the oldest first, where the store used to fill and fail every later
  start with `NATIVE_ERROR`. A start or an update for which the live activities alone leave no room is
  `SYSTEM_LIMIT`: some thirty live activities with the longest texts an Android content has.
- **What an earlier Android shows**, by liveupdate's account, in the contract: below Android 17 a metric style
  shows its numbers and texts as lines and its first stopwatch or countdown as the header clock, where it
  showed the title and the status alone; and below Android 12, which has no unlock before a tap, the tap of an
  action with `authenticationRequired` is dropped while the device is locked, where it was taken at once.
- An Android content may write `wearOngoingActivity: true`, the default, as it may write any other member at
  its default.
- The setup document has the AAR of 0.7.0, what a watch application adds, what a crate and an AAR of two
  versions do together, and that Android 12 needs `SCHEDULE_EXACT_ALARM` up to API 32 beside `USE_EXACT_ALARM`.

When updating:

- Take `liveupdate-android-0.7.0.aar`. An AAR up to 0.6.0 refuses a request with a member it does not know,
  and one before 0.6.0 refuses `wearOngoingActivity`.
- A `match` on `Template`, `EndPolicy` or one of liveupdate's enums needs a `_` arm, and so does one on
  `NextCheck`.
- A struct literal of `AndroidOptions` cannot be written outside the crate, as before: build on
  `AndroidOptions::new`.

## 0.7.6 - 2026-11-18

- fix: on iOS a color pair behind a card is of the appearance of its content, with activitykit 0.6.5 (#25) (`4e80fdd`)

Built on activitykit 0.6.5. The correction is in activitykit's widget, which an application's widget extension
is built from, and nothing changes in Rust or in the contract's shape.

- On iOS a layout's `background` that is a light and dark pair is of the appearance its content is drawn in. A
  pair was the system's tint of the activity, and a card came with the pair's light color behind content of the
  dark appearance, white text on a light card. It was seen on simulators and on no phone or watch: on the Lock
  Screen of iOS 26.3 at some starts, until the next update, and in the Smart Stack of a paired Apple Watch.
  activitykit's widget now draws a pair itself, behind the content, on the Lock Screen and in the small family.
  A color name and a hex color are the system's tint as before.
- What changes with a pair: the system's own parts of a card, such as its question whether the application may
  go on showing Live Activities, have the system's colors and no longer the pair's.
- An application whose own widget draws layouts, in place of `ActivityKitLayoutWidget`, does with
  `LayoutDocument.drawnBackground(values:)` and `backgroundTint(values:)` what that widget does.

## 0.7.5 - 2026-11-18

- fix: on iOS a button's tap reaches the sink without `on`, with activitykit 0.6.4 (#24) (`225db98`)

Built on activitykit 0.6.4. The correction is in activitykit's Swift sources, which an application builds with
this crate, and nothing changes in Rust.

- On iOS a button's tap reaches the sink without `on` again: `Event::Action` with `on: None`. Since 0.5.0 it
  came with `on: Some(false)`, on a phone and on the simulator, as a toggle's tap that asks to switch off, so
  an application had to tell the two apart by the action id. The system hands the application a value that an
  intent never set as `false`, and activitykit's intent carried the state as one optional value; it now says
  beside the state whether the tap is a toggle's. A toggle's tap is as before, and so is Android.
- An application that builds `ActivityKitActionIntent` itself changes nothing: its initializer is the same.
  Its property `isOn` is a `Bool` now, no longer optional, beside the new `isToggle`.

## 0.7.4 - 2026-11-18

- chore: build on activitykit 0.6.3, whose template fixtures get the same verdict here (#22) (`9804b0b`)
- fix: a layout's bar shows its value on Android 16 (#23) (`7170d14`)

- On Android 16 and later the bar of a layout shows its value. It was drawn full whatever the progress was: the
  projection is one segment, and Android's progress style fills every segment unless it is asked to style them
  by the progress, which the projection now does (`styledByProgress`). Nothing changes below Android 16, for
  the progress template, or for a content that the application gives: such a content says `styledByProgress`
  for itself, and one that leaves it out is drawn full as before.
- One case to know when an application is updated while such an activity runs. An activity of a layout with a
  progress that an earlier version started and that Android has not shown yet answers `CONFLICT` to a repeat of
  its start: liveupdate keeps the projection of that start and compares it, and the projection of the same
  request now has the flag. End the activity and start it again. An activity that Android shows answers the
  repeat as before, and updates are not affected.

## 0.7.3 - 2026-11-16

- docs: what becomes of events a drain took before a native call panicked (#21) (`960f2c0`)

## 0.7.2 - 2026-11-16

- chore: clippy denies unwrap and expect outside tests (#19) (`600209d`)
- fix: a native call that panics on Android fails no later call (#20) (`9352340`)

- On Android a native call that panics, as a framework's call may, no longer leaves every later layout call
  and every drain of the events as `NATIVE_ERROR`: the backend takes the layouts' lock again after a thread
  panicked under it, as it does its other locks. The panic still reaches the caller, and what the call had
  begun is left as by a native call that failed at that point. Catch a panic in `Bridge::invoke` where the
  framework's call can panic, so that a thread kept for the drain lives on
  ([Android setup](docs/android-setup.md#over_bridgebridge)).

## 0.7.1 - 2026-11-15

- fix: a scheduled start keeps an alarm when exact alarms are taken back, with liveupdate 0.4.1 (#18) (`fc27c36`)

Built on liveupdate 0.4.1: an application takes `liveupdate-android-0.4.1.aar`. The correction is in the AAR,
and nothing changes in Rust.

- A scheduled start still begins when the person takes the permission for exact alarms back while it waits:
  the AAR sets an inexact alarm beside the exact one, which Android keeps. With 0.4.0 Android removed the one
  alarm there was and stopped the application, and the activity began at the application's next call.
- The documents say how late a start without exact alarms can be, by liveupdate's measurements: up to three
  quarters of the wait, ten minutes at most while the device is awake and an hour while it dozes.

## 0.7.0 - 2026-11-15

- test: requests as JSON at the limits of the contract (#16) (`0faded1`)
- test: the typed read of the fixtures, the names of the commands and a timer style (#17) (`a9be684`)
- feat: build on activitykit 0.6: the small region for a paired Apple Watch and CarPlay (#14) (`c6843dd`)

Built on activitykit 0.6.1, whose vocabulary this crate re-exports. Breaking for a Rust caller: `Layout`, which
is activitykit's `LayoutRegions`, has the new public member `small`. JSON written for 0.6 reads unchanged.

- A layout takes `small`: what the activity shows in the Smart Stack of a paired Apple Watch (iOS 18) and in
  CarPlay (iOS 26), a tree of at most 12 nodes without buttons, toggles or links. iOS draws it when the widget
  extension's Info.plist sets `ActivityKitSmallFamily` to true, and names `layout.small` otherwise.
- Android draws nothing of the region, which a notification has no place for, and names `layout.small`.

## 0.6.0 - 2026-11-14

- feat: a scheduled start on Android, with liveupdate 0.4 (#15) (`899c088`)

Built on liveupdate 0.4.0: an application takes `liveupdate-android-0.4.0.aar`. Breaking for a host that writes
liveupdate's `StartRequest` as a struct literal, through the re-export: it has the new member `start_at`.

- A scheduled start is taken on Android. liveupdate's Android library sets an alarm and posts the notification at
  the time, also while the application does not run. Until then the record is `pending`, with the rules the
  contract has for it: the same start again answers it, whatever its time; an update is `CONFLICT`; an end ends it,
  with any policy, and nothing is shown. A start with the foreground placement, and one on a device whose library
  is older than 0.4, is `UNSUPPORTED_FEATURE`, as every scheduled start on Android was.
- The notification alerts when it begins, where the device offers an alert; elsewhere the record names `alert`. A
  sound of the alert is named `alert.sound`.
- The sink is told `pending` once and `active` once when the activity has begun, also when it began while the
  application did not run. One that the person removed meanwhile, or that Android could not post, is told
  `dismissed` with no `active`. One whose alarm was lost begins at the application's next call. `next_check` names
  the start time.
- The capabilities of the Android presentation have `scheduledStart` and `exactStart`: whether a start can be
  scheduled, and whether it begins at its time or when Android chooses. The time is exact with the permission for
  exact alarms, which the application declares and the person grants; the Android setup says how, and how a
  start survives a restart of the device.
- On a Galaxy S23 with Android 17, with the application's process ended: a start 30 s ahead was shown 25 s after
  its time without the permission for exact alarms and at its time with it, one that was ended before its time
  showed nothing, and one whose notifications were turned off was `dismissed` at the next launch.
- liveupdate 0.3.3 and 0.4.0 also bring: from Android 15 a post whose header clock is more than 14 days old is
  refused with `INVALID_INPUT` in place of being dropped by Android without a word, and `LiveUpdateForeground.attach`
  works again for an activity that Rust started.

## 0.5.2 - 2026-11-14

- fix: an end answers ended on iOS, also when the system has reported dismissed (#13) (`46570ab`)

- On iOS an end answers the record as `ended`, as the contract says, also when ActivityKit has already reported
  the activity as `dismissed`. That report follows an end that removes the activity at once, and the answer was
  read from the handle after it. On an iPhone 16 with iOS 27.2 such an end did not answer `ended` in half of
  twelve runs: five times a template's, which answered `dismissed` where the state was looked at, and once that
  of a layout that waited for its scheduled start. The list and the state events are unchanged: they say what
  ActivityKit says.

## 0.5.1 - 2026-11-13

- fix: a progress of a layout is posted on Android 16, with liveupdate 0.3.2 (#12) (`554421a`)

Built on liveupdate 0.3.2.

- A layout with a `progress` and an Android content of the progress style are posted on Android 16. liveupdate's
  AAR before 0.3.2 called a method there that only Android 17 has, and the call answered `NATIVE_ERROR`. The
  correction is in the AAR: take `liveupdate-android-0.3.2.aar`.

## 0.5.0 - 2026-11-13

- feat: build on activitykit 0.5: toggles, staleness, scheduled and transient starts, alert sounds (#11) (`a2c8977`)

Built on activitykit 0.5, whose vocabulary this crate re-exports, and on liveupdate 0.3.1. Breaking for a Rust
caller: `ActionEvent`, `Alert` and `LayoutStartInput` have new public members, the re-exported `Element` has the
variant `Toggle` and `Node` the member `if_stale`. JSON written for 0.4 reads unchanged.

- A layout takes the `toggle` node: a switch that shows a boolean value. Its tap reaches the sink as `Action`
  with `on`, the state the person asked for; `ActionEvent.on` is absent for a button. On Android a toggle is an
  action of the notification, titled by its label with `✓` before it while the value is on, and its tap has `on`
  as well, read from the activity's latest snapshot.
- A node takes `ifStale` (`when_stale()`, `unless_stale()`): iOS draws it only while the content is stale, or only
  while it is not, without the application running. Android draws a layout as it is while fresh and names
  `layout.ifStale`; the sink is told `stale` there since 0.4.
- A start takes `scheduled`, a time and the alert the system shows then (`ScheduledStart`, iOS 26). Its record is
  `pending` until the activity begins, and an update before that is `CONFLICT`. An older iOS and Android answer
  `UNSUPPORTED_FEATURE`.
- A start takes `ios`, the options that only iOS reads (`IosOptions`): `transient` asks for a transient activity
  (iOS 18). Android keeps nothing of them.
- An alert takes `sound`, the file name of a sound of the application (`Alert::with_sound`), with an update and
  with a scheduled start. Android alerts with the sound of its channel and names `alert.sound`.
- liveupdate 0.3.1 writes the stopwatch and the countdown of a metric as its Android side reads them
  (`startedAt`, `endsAt`): an Android content with one could not be posted before. Take its AAR of 0.3.1.
- Known: on Android 16, liveupdate's AAR up to 0.3.1 cannot post a progress of a layout or of an Android content
  (`NATIVE_ERROR`); the templates and other Android versions are not touched. The correction is liveupdate's.

## 0.4.0 - 2026-11-13

- feat: build on liveupdate 0.3, and tell the sink each state of an activity once on Android (#10) (`e3bf99f`)

Built on liveupdate 0.3.

- On Android the sink is told where an activity stands, as on iOS: `active` once Android shows it, `stale` after
  its stale time, `active` again after an update that makes it fresh, `ended` and `dismissed`, each once. The
  states come from the account liveupdate keeps with its receipts, with a start, an update, an end and a list
  and on `sync_events`. A state that came and went between two asks is not reported, and a state is handed over
  at most once. A state event now names the key also for a template and on a backend without the app's files.
  Until now Android reported `ended` and `dismissed` only; a sink that expected nothing after a start reads
  `active` there.
- On Android the sink is called for one event at a time, in the order the events were taken, also when several
  threads ask at once, and with no lock held: a sink may call the service, `sync_events` included. A call can
  return before its state arrived, when another thread is handing events over.
- `LiveUpdateBackend::next_check` says when it is worth asking again: now, soon, at the earliest stale time
  ahead, or not until a call or an event. The crate runs no timer.
- `LiveUpdateBackend::pump_events` answers how many events it took for the sink, and fails when the states
  cannot be read after the stored events were taken; those reach the sink all the same.
- A stored dismissal of an activity that liveupdate does not know is no longer reported. An end of an activity
  whose `dismissed` was reported reports `ended` as well.
- `testkit::MemoryBackend` reports `active` at a start.
- liveupdate 0.3 took its Android package back to `io.github.eedaegon.liveupdate`: take its AAR of 0.3, and name
  that package where Kotlin or a manifest names one of its classes. It reads what 0.1 stored and not what 0.2
  stored.

## 0.3.0 - 2026-11-13

- ci: tag the release commit in the run that prepared it (#8) (`febb247`)
- feat: build on activitykit 0.4 and liveupdate 0.2, and show more of a layout on Android (#9) (`f7d186a`)

Built on activitykit 0.4 and liveupdate 0.2 (0.2.1 or later), whose vocabularies this crate re-exports. Breaking
for Rust callers, as before 1.0; JSON written for 0.2 reads unchanged.

- A layout takes what activitykit 0.4 added: `accessibility`, `transition` and `privacy` on a node, the `link`
  node, `Layout.url`, a style's `content_transition` and the Dynamic Island's `priority`, `below_if_too_wide` and
  `margins`. `Node`, `Layout`, `Style`, `Island` and `IslandExpanded` have new public members and `Element` the
  variant `Link`: a struct literal names them and a match over `Element` has an arm for it. On Android the
  projection names `layout.link`, `layout.url`, `layout.transition` and, for a public notification,
  `layout.privacy`.
- An Android content takes what liveupdate 0.2 added: `open_url` of the presentation and of an action, a
  `chronometer`, `alert`, `public_version`, `small_icon`, `accent_color` and `sub_text`. Build a presentation with
  `AndroidPresentation::new(style)` and an action with `NotificationAction::new(id, title)`. A member that the
  device does not offer is `UNSUPPORTED_FEATURE`.
- Android follows the device where it used to refuse or stay silent: a snapshot of another notification style
  (`styleChange`), `EndPolicy::After` (`delayedEnd`) and the alert of an update (`alert`).
  `Capabilities.android_presentation` reports each. A layout is now projected in its own style with every
  snapshot, also after a content of another style.
- An end sent again for an activity that is over answers its record on Android as on iOS, also with a final
  content, which is not applied.
- On Android the sink receives `Event::State` with `ended` when an end of the application ends a running
  activity, as it does on iOS; until now Android reported only a dismissal. A sink that took every state event
  on Android for a dismissal reads `state` now. `testkit::MemoryBackend` reports its ends the same way.
- A layout shows more on Android, where the device offers it: `Layout.url` and `link` nodes open their URLs from
  the notification and its actions, a timer beside a progress is the clock in the header, and a node that a
  notification cannot draw is shown as the text of its accessibility label and value. A button or a link without
  a text is titled by a label. A URL that nothing in the application opens does not fail a layout: its links
  are left out and named. A node hidden from assistive technologies is no longer named in `degradedFeatures`.
- `AndroidOptions` has `public_version`, what a secure lock screen shows of a private activity, and `placement`,
  with `AndroidPlacement::Foreground` for a notification that a foreground service of the application holds.
  The capabilities report `foregroundPlacement` as the device offers it.
- `Event::Opened`: the person opened the application from a notification (Android), when the host reports the
  opening intent to liveupdate.
- liveupdate 0.2 renamed its Android package to `io.github.gon.liveupdate`: take its AAR of 0.2, and rename the
  package where Kotlin or a manifest names one of its classes. It keeps its receipts under the new name and does
  not read those of 0.1, so an activity that runs while the application is updated to this release is no longer
  listed afterwards; Android removes its notification with the update.

## 0.2.0 - 2026-11-12

- ci: release from conventional commits and publish to crates.io (#6) (`93bbe7a`)
- feat: relevance, alerts, Android content and lifecycle events through the contract (#7) (`7025315`)

## 0.1.0 - 2026-11-13

- The first release: the typed contract, `LiveActivityBackend` and `EventSink` behind one builder, the default
  backends on activitykit 0.3 and liveupdate 0.1.3, push events, Android layouts, and the `testkit` feature.
