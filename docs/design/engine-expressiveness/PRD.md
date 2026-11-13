# Engine expressiveness: product requirements

## Problem

The contract of this crate is narrower than both engines. activitykit takes a relevance score and an alert, and
reports when an activity starts, changes state or loses its permission; liveupdate draws segmented progress,
metrics, a status chip and action icons. None of it can be said through the contract, so an application that goes
through a framework plugin cannot reach it. Both engines are also about to open more (accessibility, deep links,
a timer beside any style, an end screen), and the contract needs a place for each.

## Goal

Whatever an engine can show or report can be asked for and received through the contract, without the contract
becoming the union of two platforms' vocabularies.

## Principles

1. **Each engine owns its vocabulary.** Layout nodes and styles are activitykit's, notification styles are
   liveupdate's. This crate re-exports both and defines neither.
2. **This crate owns the envelope.** Identity, lifecycle, revisions, the options of each platform, validation and
   the projection of a layout onto Android are this crate's.
3. **Valid everywhere or nowhere.** A request is checked the same way on every platform, before any backend.
4. **What a platform cannot do is reported.** A member a platform ignores by design is documented; one it would
   show if it could is named in the record's `degradedFeatures`, and capabilities say what a device draws.

## Requirements of this change

Engines as released: activitykit 0.3, liveupdate 0.1.3. Each requirement is a row of `docs/requirements.md` with
the test that is its evidence.

| Id | Requirement |
| --- | --- |
| REQ-C-005 | A layout request is read strictly in this crate's types, and activitykit checks the part it knows. Members activitykit does not know are taken; an unknown member, a `null` and a request that does not encode back to itself are refused, as is a request under another command's name. activitykit's own request fixtures get the same verdict here, except where this contract takes more. |
| REQ-C-006 | What a request says beside the layout is checked on every platform: a relevance is finite, an alert has a title and a body within their lengths, and an Android content is one liveupdate takes. |
| REQ-C-007 | What only one platform reads stays with it. The Android options and the Android content of a request never reach the iOS engine, so two starts that differ only in them are the same activity on iOS. The relevance and the push request are no part of what Android keeps and compares. |
| REQ-I-001 | The relevance of a snapshot reaches ActivityKit with a start, an update and an end, and the record reports it. |
| REQ-I-002 | The alert of an update reaches ActivityKit. Android updates silently and reports `alert` in `degradedFeatures`. |
| REQ-E-002 | The sink receives a layout activity that began (also one a push started), a new lifecycle state of any activity (on Android, a dismissal), and the permissions after the person changed them (iOS). |
| REQ-A-001 | A snapshot's Android content is shown as given, in place of the projection: the style with its segments, points or metrics, the chip text, the icons and the actions. A snapshot without one is projected as before. |
| REQ-A-002 | A tap on an action of an Android content reaches the sink with the key of its activity; a tap on an action no snapshot declared is dropped. |
| REQ-A-003 | Capabilities report which notification styles the device draws, so that an application chooses a content the device can show. |

## Planned, when an engine opens it

Not built in this change. Each row is added to `docs/requirements.md` with its test when the engine release it
depends on is out. activitykit 0.4 and liveupdate 0.2 are out: [engine extensions](../engine-extensions/PRD.md)
says which of these rows are built on them and which still wait.

| Feature | Engine release | What this crate does then |
| --- | --- | --- |
| Accessibility labels | activitykit | Re-exports the node members. The projection uses a label as the text of a node Android cannot draw. |
| Deep links | activitykit, liveupdate | One `url` member for a region and for a button. iOS draws it with the widget's link; Android opens it from the notification and its actions. A new event reports which activity was opened. |
| Toggle, transitions, island details, the small region, privacy | activitykit | Re-exports the members. The projection names each in `degradedFeatures`, except a toggle, which becomes an action whose title follows its value. |
| Transient and scheduled starts, alert sounds | activitykit | Options of a start and of an alert; `UNSUPPORTED_FEATURE` where the system is older. |
| A timer beside any style | liveupdate | The projection keeps the timer of a layout that also has a progress, and stops reporting `layout.timerText`. |
| A style that changes between updates | liveupdate | Stops answering `UNSUPPORTED_FEATURE` to a content of another style than the activity's first. |
| An end screen | liveupdate | `EndPolicy::After` stops being `UNSUPPORTED_FEATURE` on Android; the final snapshot stays until then. |
| Alerts of an update | liveupdate | `alert` leaves `degradedFeatures` on Android. |
| A redacted lock screen version | liveupdate | An Android option beside `visibility`. |

## Non-goals

- A full CSS model, arbitrary Android XML or `RemoteViews`, and free animation. Neither system honors them:
  a promoted Android notification may not carry a custom view, and ActivityKit animates content changes itself.
- A layout language for Android content. It is resolved for each snapshot; a framework computes it from the values.
- Typed bindings and a session that coalesces updates. Each gets requirements of its own.
- Sending pushes. Payloads are activitykit's; keys, tokens and transport are a server's.
