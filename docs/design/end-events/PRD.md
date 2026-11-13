# End events: product requirements

## Problem

An application learns that an activity is over from one event. On iOS that works: ActivityKit reports every
lifecycle change, so the sink gets `State` with `ended` when an activity ends, whoever ended it, and `dismissed`
when it leaves the screen. On Android the sink gets only the dismissal by the person. liveupdate stores what the
person did to a notification (a tap, a dismissal, an opening), and an end is a call of the application, so
nothing is stored for it. Code that cleans up when an activity ends runs on iOS and never on Android.

## Goal

The same event tells an end on both default backends, without a change in either engine.

## Requirement of this change

| Id | Requirement |
| --- | --- |
| REQ-E-003 | On Android the sink receives `State` with `ended`, the activity's id and its key, when an end of the application ends a running activity: a template or a layout, at once or with a last content that stays. An end of an activity that is already over and an end that fails report nothing. The backend without a platform of the test kit does the same. |

## Not in this change

- **A `Started` event and the `active` state on Android.** The same reasoning would have the backend report its own
  starts. It is a separate decision.
- **The `stale` state on Android.** liveupdate reads a stale date when it is asked; no moment reports it.
- **The removal of a last content when its time comes.** liveupdate stores no event for it, by design: the
  notification has no delete intent.
- **An event of its own for an end, with the record.** `State` names the activity; the record is the answer of
  the end, and `list` has it.
