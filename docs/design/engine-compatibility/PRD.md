# Engine compatibility: product requirements

## Problem

liveupdate went from 0.4.1 to 0.7.0 in seven releases, and this crate still builds on 0.4.1. It re-exports
liveupdate's vocabulary of a notification (`AndroidPresentation` and its parts), so 0.5.0 to 0.7.0 are breaking
for a Rust caller of this crate, and the crate does not build on them as it is: a presentation and the
capabilities have a new member, and twelve more of liveupdate's enums are `#[non_exhaustive]`.

What those releases add has to be reachable through this crate, each part of it. The list below is that of
liveupdate's changelog, its specification and the differences of its public API between the two versions.

| liveupdate | What it offers | Where it is here |
| --- | --- | --- |
| 0.4.2 | Android 12 needs `SCHEDULE_EXACT_ALARM` beside `USE_EXACT_ALARM` (a document) | `docs/android-setup.md` |
| 0.4.3 | A start and an update store their receipts in at most 1,000,000 bytes: older ended activities go for the room, and `SystemLimit` when the live ones alone leave none | The error reaches the caller as `SYSTEM_LIMIT`; the contract says the second meaning of the code (REQ-A-026) |
| 0.4.4 | Below Android 12 the tap of an action that requires authentication is dropped while the device is locked | `NotificationAction.authentication_required` is re-exported; the contract says what each Android does (REQ-A-025) |
| 0.5.0 | On a watch below Wear OS 7 whose application has `androidx.wear:wear-ongoing`, a notification carries an Ongoing Activity; `PresentationCapabilities.wear_ongoing_activity` says so | `capabilities().android_presentation.wear_ongoing_activity`; the setup of a watch application in `docs/android-setup.md` (REQ-A-024) |
| 0.5.1 | The README says how to use a watch | `docs/android-setup.md` |
| 0.6.0 | `AndroidPresentation.wear_ongoing_activity`: one activity can do without the Ongoing Activity | An Android content has the member, since the type is liveupdate's; a layout says it with `AndroidOptions.wear_ongoing_activity` (REQ-A-024) |
| 0.7.0 | Constructors for every struct a caller builds, and `Default` for `SamsungNowBar` and `Capabilities` | The re-exported types bring theirs; this crate's own types that a caller builds have one each (REQ-C-012) |
| 0.7.0 | Every public enum is `#[non_exhaustive]` | So are the re-exported ones; this crate's `Template` and `EndPolicy` join its others (REQ-C-012) |
| 0.7.0 | `EventKind::Unknown`: an event of a kind that the crate does not know, from a later AAR | `Event::Unknown` (REQ-E-008) |
| 0.7.0 | The crate reads what it knows of a later AAR's reply; the AAR posts a record with members it does not know, without them; which crate works with which AAR | `docs/android-setup.md` names the AAR of 0.7.0 and has the table of versions (REQ-A-027) |
| 0.7.0 | Below Android 17 a metric style shows its metrics: numbers and texts as lines, the first stopwatch or countdown as the header clock | Nothing to say in a request; the contract says what each Android shows (REQ-A-025) |
| 0.7.0 | For a driver: `Observation::new`, `StartReport::new`, `ActivityEvent::new` | Through `liveactivity::liveupdate`, which the crate re-exports whole |

## Goal

The crate builds on liveupdate 0.7.0. Each part of the list is in the contract, in the Rust API or in the
documents, and code written against this version of the crate compiles against the next one where it builds its
values on the constructors and matches with a `_` arm.

## Requirements of this change

Each requirement is a row of `docs/requirements.md` with the test that is its evidence.

| Id | Requirement |
| --- | --- |
| REQ-A-024 | A watch's Ongoing Activity. An Android content says `wearOngoingActivity` in its presentation, as liveupdate takes it, and a layout says it with the Android option `wearOngoingActivity` of its start, which goes with every snapshot that the backend projects. Absent, the activity carries the Ongoing Activity where the device offers one. The capabilities say `androidPresentation.wearOngoingActivity`. A template has no such option and carries it where it is offered. iOS ignores the member. Anything but a boolean is `INVALID_INPUT` on every platform. |
| REQ-A-025 | What an earlier Android shows is said in the contract, by liveupdate's account: metrics as lines and a header clock below Android 17, with `metricStyle` in the record as before; an action that requires authentication asks from Android 12, and below it its tap is dropped while the device is locked. The crate adds nothing to either and tests neither. |
| REQ-A-026 | A start or an update for which the live activities leave no room in the store of liveupdate's receipts answers `SYSTEM_LIMIT`, and the activity it asked for is not kept by the backend either. |
| REQ-A-027 | The crate is built and tested with the AAR of liveupdate 0.7.0. The setup document names it, what a watch application adds, and what a crate and an AAR of two versions do together. |
| REQ-E-008 | An event of a kind that liveupdate does not know, from a later AAR, reaches the sink as `Event::Unknown` with the activity's id and its key when the backend knows it. Nothing else of it is known. A dismissal stays a state event. |
| REQ-C-012 | Every public enum of the crate is `#[non_exhaustive]`, and every struct of the contract that a caller builds has a constructor that takes what is required and sets nothing else: `Timer`, `Snapshot`, `StartInput`, `UpdateInput`, `EndInput` and `AndroidContent` get theirs beside those the layout's types have. What they build is written as the JSON it was. |

## Decisions

- **A layout's option, not a member of its snapshot.** liveupdate has the choice in the presentation, which is
  content: an update may change it. An Android content of this crate is such a presentation and keeps that. A
  layout's snapshot is values for a tree that both platforms draw, and has no place for what one platform's watch
  does. The Android options of the start are that place, as for the public version, which also goes with every
  snapshot.
- **An unknown event is an event.** Until now the backend dropped a kind it had no event for, with the
  dismissal, whose state event says it. A drain removes an event from the adapter, so one that is dropped is
  gone for good: an application that is told can at least say that its library is older than its AAR.
- **The crate's own types follow liveupdate's rule.** It has had constructors and `#[non_exhaustive]` for the
  layout's types since they came. The types of the two templates were written before, as literals: they get
  constructors now, and the two enums that lacked the attribute get it, in the release that is breaking anyway.
- **No lenient reading of a request.** liveupdate reads what an adapter says for what it knows, and a request of
  the application strictly. So does this crate: a member that is not known in a request is `INVALID_INPUT`.

## Not in this change

- **A member that says "this action requires authentication" for a layout's button.** A layout's tree is
  activitykit's, which has no such member; an Android content's action has it.
- **Live Updates on Wear OS 7, a phone's activity on a paired watch, Samsung's Now Bar on a watch.** liveupdate
  names each as not done.
- **A test on a watch.** The crate has no application: liveupdate ran its library on a Galaxy Watch4 with Wear
  OS 5. What the crate tests is that the member, the option and the capability reach liveupdate and come back.
