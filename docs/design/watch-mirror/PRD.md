# A phone's activity on a paired watch, through a mirror: product requirements

## Problem

A watch's bridge leaves an ongoing notification out, and a Live Update is one: liveupdate measured a Galaxy Watch4
on Wear OS 6, whose Galaxy Wearable dropped a phone's activity. liveupdate 0.11.0 adds a mirror: a second
notification of the activity, not ongoing, which the bridge takes to the watch.

| liveupdate 0.11.0 | What it offers | Here |
| --- | --- | --- |
| `AndroidPresentation::watch_mirror`, JSON `watchMirror` | While it is set, the activity's own notification is local and a second one (tag `liveupdate-rs-watch:<id>`, channel `liveupdate-rs.watch.v1` at the lowest importance) carries the same content to a paired watch. A revision that alerts alerts through the mirror only. A person who removes the mirror stops it for that activity; the activity's end or dismissal removes it | Nothing: an Android content cannot ask for it, a layout's start has no option for it |
| `PresentationCapabilities::watch_mirror` | True on a phone with this AAR; false on a watch and with an earlier AAR, where asking for it is `UNSUPPORTED_FEATURE` | Not in the contract |
| `watch_mirror` beside `local_only` | `INVALID_INPUT`: one keeps the activity off the watch, the other sends it there | Nothing |
| The strings `liveupdate_watch_channel_name` and `liveupdate_watch_channel_description` | The mirror's channel in the application's words | The setup names four strings |

## Goal

The crate builds on liveupdate 0.11.0, and an application can send a layout activity to a paired watch as it can
keep one off it.

## Requirements of this change

| Id | Requirement |
| --- | --- |
| REQ-A-040 | `AndroidOptions::watch_mirror` (JSON `android.watchMirror`, false when absent): every snapshot of the activity is mirrored to a paired watch, the projected ones and an Android content; an Android content's `watchMirror` mirrors its own snapshot. `capabilities().android_presentation.watch_mirror` says whether the device can; where it cannot, such a start or update is `UNSUPPORTED_FEATURE` and posts nothing. A start with both `localOnly` and `watchMirror` true is `INVALID_INPUT` on every platform, and so is a snapshot whose merged presentation has both. |
| REQ-A-034 | The Android setup names the AAR's six channel strings. |

## Decisions

- **As `local_only` is.** The start's `true` holds for every snapshot and a content's `true` for its own, and
  nothing turns the start's word off. The two are each other's opposite, so a request that asks for both is
  refused before any device sees it.
- **No mirror on iOS.** A paired Apple Watch draws the `small` region (REQ-I-009). iOS ignores the option.

## Not in this change

- A template's mirror. A template's start has no Android options: an application that needs it starts a layout.
