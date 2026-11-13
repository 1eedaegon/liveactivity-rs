# State changes on Android: product requirements

## Problem

On iOS the sink receives every lifecycle state of an activity while the application runs: ActivityKit reports
`active`, `stale`, `ended` and `dismissed`. On Android it receives two of them, from two places: `ended`, which
the backend reports for its own end, and `dismissed`, from the event that liveupdate's Android side stores.
That an activity became `stale`, or `active` once Android showed it, is read from a record only.

liveupdate 0.3 answers each state of an activity once, durably, and says when it is worth asking again. It has
no timer: the layer that has a thread asks.

## Goal

On Android the sink receives each state of an activity once, from one account of states, and a host can know
when to ask for the next one.

## Requirements of this change

| Id | Requirement |
| --- | --- |
| REQ-E-005 | On Android the sink receives `State` for each state of an activity as liveupdate answers it, once: `active` when Android shows it, `stale` when its stale time has passed, `active` again after an update, `ended`, `dismissed`. It names the id and the key, of a template and of a layout, with or without the app's files. The states are reported after the backend's own calls and on `sync_events`; the stored event of a dismissal is no second report. |
| REQ-E-006 | The default Android backend says when it is worth asking again: now, soon (a post that Android has not shown yet), at a time (the earliest stale time ahead), or not until an operation or an event. |
| REQ-E-003 (changed) | The end of the application's own call is still reported at the call, now from the same account as the other states. |

## Also in this change

liveupdate 0.3 takes its Android package back to `io.github.eedaegon.liveupdate` and reads again what 0.1 stored.
The documents that named the package of 0.2 name this one.

## Not in this change

- **A timer.** The crate starts no thread on Android. A host that has one waits as the backend says and asks
  then; a host without one learns of a stale activity the next time it asks, at the latest when the application
  resumes.
- **`Started` on Android.** The start is the application's own call; `State` with `active` tells that Android
  shows the activity.
- **A state while the application does not run.** As on iOS.
