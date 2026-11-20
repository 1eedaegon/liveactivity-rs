# A tap that runs the application's own component: product requirements

## Problem

liveupdate 0.8.0 lets a notification's action run a service or a receiver of the application at once, with no
screen and no process of the application running: a recorder's Record, a player's Pause. It is what an iPhone's
Live Activity does with an App Intent. This crate builds on liveupdate 0.7.1 and offers none of it.

| liveupdate 0.8.0 | What it offers | Where it is here |
| --- | --- | --- |
| `NotificationAction.target: Option<ActionTarget>` (`ActionTarget { kind: TargetKind, component, action }`) | An action of an Android content runs a service, a foreground service or a receiver of the application, by an explicit, immutable PendingIntent | An Android content's action takes it as it is (REQ-A-029) |
| — | A layout's button or toggle is projected onto a notification action: liveupdate cannot name its target | `AndroidOptions.targets`, keyed by the action id (REQ-A-029) |
| `PresentationCapabilities.action_targets` | Whether the device's adapter delivers targets; a start with one where it is false is `UnsupportedFeature` | `capabilities().android_presentation.action_targets` (REQ-A-029) |
| `EventKind::Handled { action_id }` and `LiveUpdateActions.take(context, intent)` | The application's component takes the tap and records it as handled, so that the queue does not act on it a second time | `Event::Handled` (REQ-E-010) |
| `LiveUpdateEvents.addNativeListener` / `removeNativeListener` | Listeners that are told of every stored event and take nothing from the queue | Documented (REQ-A-031) |
| `LiveUpdateForeground.attachOr(service, type, fallbackId, fallback)` | A foreground service that is never without a notification: its own until an activity holds the placement | Documented (REQ-A-031) |
| `LiveUpdate::android_from_raw(vm, context)` | A client built from the process's `JavaVM` and a context, for a service's own Rust, without `ndk-context` | `LiveUpdateBackend::android_from_raw(vm, context)` (REQ-A-030) |
| `liveupdate-android-0.8.0.aar` | The AAR of the version | The setup names it (REQ-A-027) |
| Resource shrinking removes an icon that only a presentation names | `res/raw/keep.xml` | The setup says it (REQ-A-031) |

## Goal

The crate builds on liveupdate 0.8.0, and each part of the list is in the contract, the Rust API or the documents.

## Requirements of this change

| Id | Requirement |
| --- | --- |
| REQ-A-029 | An Android content's action takes `target`, as liveupdate takes it. A layout's start takes `android.targets`, an object of action ids to targets: each key is the `actionId` of a button or a toggle of the lock screen tree, each value a target that liveupdate takes, else `INVALID_INPUT` on every platform. The projection puts the target on that action. iOS ignores it. Where the device does not deliver targets (`actionTargets` false), a start or an update that would post one answers `UNSUPPORTED_FEATURE`. |
| REQ-E-010 | A tap that the application's component took with `LiveUpdateActions.take` reaches the sink as `Event::Handled` with the activity's id, its key when the backend knows it, the action id and what is known of the tap (its sequence and time). It is no `Event::Action`. |
| REQ-A-030 | `LiveUpdateBackend::android_from_raw(vm, context)` builds the backend on liveupdate's AAR from raw JNI pointers, for a service whose process has no `ndk-context`. |
| REQ-A-031 | The setup documents what the application's own code does: a target's component in the manifest, `LiveUpdateActions.take` in it, the native listeners, `attachOr`, and `res/raw/keep.xml`. |
| REQ-A-027 | The crate is built with the AAR of liveupdate 0.8.0, which the setup names. |

## Decisions

- **A layout's target is an option of its start, keyed by the action id.** The button is activitykit's node, which
  has no member for Android, and iOS refuses a member it does not know. The options of the start are the place
  of what one platform needs of a layout, as the public version and the watch's Ongoing Activity are.
- **A key must name a control of the lock screen tree.** That is what Android projects; a key that names nothing
  is a mistake that would never show, so it is refused rather than ignored.
- **A handled tap is an event of its own.** The application's component acted on it already, so it is no
  `Action`: an application that acts on every `Action` would act twice. It is reported so that the application
  can follow, as with a tap.

## Not in this change

- **A tap of a target as an `Action` too.** The component is the one place that acts on it.
- **A start from Kotlin.** liveupdate has none; a service starts through its own Rust, with the backend built from
  the raw pointers.
