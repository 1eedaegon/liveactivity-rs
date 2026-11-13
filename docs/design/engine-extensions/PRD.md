# Engine extensions: product requirements

## Problem

Both engines released what [the plan before](../engine-expressiveness/PRD.md) waited for. activitykit 0.4 gives a
layout accessibility members, links, a URL of its own, transitions, a privacy mark and the Dynamic Island's
options. liveupdate 0.2 gives a presentation links, a clock in the header, a revision that alerts, a public
version and branding, lets a style change between snapshots, keeps the last content of an ended activity, and
offers a foreground placement. Both are breaking releases for a Rust caller, and this crate re-exports both
vocabularies, so it does not build on them as it is.

Three answers of this crate were also written for what the engines could not do then: a style that changes is
refused, a delayed end is refused, and an alert on Android is reported as left out. With the new engines each of
them would be wrong.

## Goal

The crate builds on activitykit 0.4 and liveupdate 0.2. What an engine added to its own vocabulary can be said
through the contract as that engine takes it, and no answer of this crate says that a platform cannot do what its
engine now does.

## Requirements of this change

Each requirement is a row of `docs/requirements.md` with the test that is its evidence.

| Id | Requirement |
| --- | --- |
| REQ-I-003 | What activitykit 0.4 added to a layout reaches ActivityKit as written: a node's `accessibility`, `transition` and `privacy`, the `link` node, the layout's `url`, a style's `contentTransition`, and the island's `priority`, `belowIfTooWide` and `margins`. A request that breaks one of activitykit's rules for them is refused on every platform. |
| REQ-A-004 | The projection names what Android does not show of it: `layout.link`, `layout.url`, `layout.transition`, and `layout.privacy` when a locked screen shows the notification's content. Accessibility members are read by iOS only. |
| REQ-A-005 | A layout projects to its own style with every snapshot. A snapshot of another style than the one the activity shows is shown where the device offers `styleChange` and is `UNSUPPORTED_FEATURE` elsewhere. |
| REQ-A-006 | `EndPolicy::After` keeps the last content of a template or a layout activity on Android until its time, where the device offers `delayedEnd`; elsewhere it is `UNSUPPORTED_FEATURE` and the activity stays. An end sent again answers the record, with or without a final content, and without a time removes a last content that is still shown. |
| REQ-I-002 (changed) | The alert of an update reaches ActivityKit. Android alerts again with that revision where the device offers `alert`; elsewhere it updates silently and names `alert` in `degradedFeatures`. |
| REQ-A-007 | An Android content takes what liveupdate 0.2 added to a presentation: the URL a tap opens, an action that opens a URL, a clock in the header, a revision that alerts, a public version, a small icon, an accent color and a sub text. A start that gives a public version to an activity that is not private is refused on every platform. A content that uses a member the device does not offer is `UNSUPPORTED_FEATURE`, except the alert, which is left out and named. |
| REQ-A-003 (changed) | Capabilities report the extensions the device offers beside its styles. `foregroundPlacement` is reported as false: the contract has no member that asks for it. |

## Still planned

Not built here. Each needs a decision of this crate, not only a newer engine, and gets its own requirements.

| Feature | Engine | What this crate would do |
| --- | --- | --- |
| Links of a layout on Android | liveupdate 0.2 | The layout's `url` becomes the URL the notification opens and a `link` node an action that opens its URL, under one URL rule that both engines take. A new event reports an activity that was opened, which liveupdate queues as `opened` and this crate drops today. |
| A timer beside a progress | liveupdate 0.2 | The projection keeps the timer of a layout that also has a progress, as the clock in the header, and stops naming `layout.timerText`. |
| Accessibility labels on Android | activitykit 0.4 | The projection uses a label as the text of a node Android cannot draw. |
| A public version of a projected layout | liveupdate 0.2 | An Android option of the start, beside `visibility`. |
| A foreground placement | liveupdate 0.2 | An Android option of the start; the host runs the service that holds the notification. |
| Toggle, transient and scheduled starts, alert sounds, the small region | a later activitykit | As [planned before](../engine-expressiveness/PRD.md). |

## Non-goals

- A second vocabulary. The new members keep the names and the rules their engine gave them.
- A foreground service, a launch activity or a URL handler. They are the application's.
