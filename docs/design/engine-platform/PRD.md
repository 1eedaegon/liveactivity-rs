# An activity off a paired watch, the settings of promotion, and channels in the application's words: product requirements

## Problem

liveupdate 0.9.0 adds three things to Android that a caller of this crate cannot reach yet.

| liveupdate 0.9.0 | What it offers | Where it is here |
| --- | --- | --- |
| `AndroidPresentation::local_only`, JSON `presentation.localOnly`, and the capability `PresentationCapabilities::local_only` | An activity that stays on the device it is posted on: a paired watch does not get a phone's notification. A snapshot that asks for it where the adapter does not offer it is refused with `UnsupportedFeature`, since an earlier adapter would send it to the watch | `AndroidOptions::local_only` of a layout's start, for every snapshot of the activity; `localOnly` of an Android content for its own snapshot; `capabilities().android_presentation.local_only` (REQ-A-032) |
| `LiveUpdate::open_promotion_settings()` (the bridge's `settings`) | The system's screen where a person allows or stops the application's Live Updates (Android 16 and later), or its notification settings before that; Android starts it only while the application is in the foreground, and `Ok` says that it was asked | `LiveActivity::open_promotion_settings()`, one backend method with a default that answers `UNSUPPORTED_FEATURE` (REQ-A-033) |
| The AAR names its two channels with the string resources `liveupdate_channel_name`, `liveupdate_channel_description`, `liveupdate_alert_channel_name` and `liveupdate_alert_channel_description` | An application names the channels in its own words and languages, also for a channel that exists | The Android setup says how (REQ-A-034) |

## Goal

The crate builds on liveupdate 0.9.0, and each part of the list is in the contract, the Rust API and the documents.

## Requirements of this change

| Id | Requirement |
| --- | --- |
| REQ-A-032 | `AndroidOptions::local_only` (JSON `android.localOnly`, false when absent): every snapshot of the activity stays off a paired device, the projected ones and an Android content; an Android content's `localOnly` keeps its own snapshot off. `capabilities().android_presentation.local_only` says whether the device can; where it cannot, such a start or update is `UNSUPPORTED_FEATURE` and posts nothing. |
| REQ-A-033 | `LiveActivity::open_promotion_settings()` asks Android for the settings of the application's Live Updates. A backend without the command, iOS and the default of a platform without Live Activities answer `UNSUPPORTED_FEATURE`, or `UNSUPPORTED_PLATFORM` there. |
| REQ-A-034 | The Android setup names the AAR's four channel strings, which the application overrides in its own `res/values*/strings.xml`. |

## Decisions

- **`local_only` on the start, as `wear_ongoing_activity` is.** A watch's Ongoing Activity and a notification kept
  off the watch are both the start's word for every snapshot. Unlike the Ongoing Activity, whose default is on,
  an activity is kept local only where something says so: the start's `true` holds for every snapshot, and a
  content's `true` holds for its own. Nothing turns the start's word off.
- **One method per command.** `open_promotion_settings` is a backend method of its own, not a kind of
  `request_permissions`: it answers nothing about the permissions, and Android may show nothing.
- **A default that answers `UNSUPPORTED_FEATURE`.** A backend of an application's own, written before, compiles
  and says that it cannot. A backend that wraps another forwards the method.

## Not in this change

- **A template's `local_only`.** A template's start has no Android options; an application that needs it starts a
  layout.
- **iOS's settings.** ActivityKit has no screen of its own to open; iOS answers `UNSUPPORTED_FEATURE`.
