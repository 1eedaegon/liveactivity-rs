# Engine interaction: product requirements

## Problem

activitykit 0.5 gives a layout a toggle and nodes that are drawn only while the content is stale or fresh, a
start a time to begin at and a transient presentation, and an alert a sound. It is a breaking release for a Rust
caller, and this crate re-exports its vocabulary, so it does not build on it as it is.

[The first plan](../engine-expressiveness/PRD.md) said what this crate does with each when the engine opens it.
One answer has changed since: Android now tells the sink when an activity became stale
([state changes](../state-changes/PRD.md)), so an application can do there itself what a layout does on iOS.

## Goal

The crate builds on activitykit 0.5. What it added can be said through the contract as activitykit takes it, a
request that breaks one of its rules is refused on every platform, and Android shows what a notification can
show of it and names the rest.

## Requirements of this change

Each requirement is a row of `docs/requirements.md` with the test that is its evidence.

| Id | Requirement |
| --- | --- |
| REQ-I-004 | A layout takes the `toggle` node as activitykit takes it: it stands where a button may, shows a boolean value, and counts with buttons and links. A tap on it reaches the sink as `Action` with `on`, the state the person asked for; a button's tap has none. A request that breaks a rule of the node is `INVALID_INPUT` on every platform. |
| REQ-I-005 | A node's `ifStale` reaches ActivityKit as written, on any node. |
| REQ-I-006 | A start takes `scheduled`, a time and the alert the system shows then, and the iOS option `transient`. A scheduled start answers a record that is `pending`. A time or an alert that is none of the contract is `INVALID_INPUT` on every platform. |
| REQ-I-007 | An alert takes `sound`, the file name of a sound of the application, for an update and for a scheduled start. A name that is no file name of the contract is `INVALID_INPUT` on every platform. |
| REQ-A-013 | On Android a toggle is an action of the notification: its title is the toggle's label, with a check mark before it while the value is on. Its tap reaches the sink with `on`, the other state than the value the activity shows. It takes a place among the notification's actions as a button does. |
| REQ-A-014 | Android draws a layout as it is while fresh and names `layout.ifStale` for one whose nodes follow staleness; the sink is told `stale` (REQ-E-005). A scheduled start is `UNSUPPORTED_FEATURE`. The `transient` option is iOS's and is no part of what Android keeps. An alert's sound is named as `alert.sound` where the update alerts. |

## Decisions

- **`scheduled` is a member of the start, not an iOS option.** A platform that cannot start later refuses the
  start; starting at once in its place would show an activity at the wrong time. `transient` is a presentation of
  iOS alone, and stands with the iOS options as `visibility` stands with Android's.
- **Android does not follow staleness by itself.** A notification is drawn when it is posted, and nothing redraws
  it when a time passes while the application does not run. The backend has no clock and posts no content the
  application did not send. It tells the sink that the activity is stale, and the application updates.
- **A toggle's state is in its title.** An action of a notification is a title and nothing else. The check mark
  is no word of any language.

## Not in this change

- **Capability members for a scheduled and a transient start.** activitykit reports whether the system runs
  buttons, and not which version it is. Where the system is older its start answers `UNSUPPORTED_FEATURE`.
- **The small region** (the Apple Watch's Smart Stack). activitykit 0.5 does not have it.
- **Sounds on Android.** liveupdate alerts on a channel whose sound is the person's choice.
