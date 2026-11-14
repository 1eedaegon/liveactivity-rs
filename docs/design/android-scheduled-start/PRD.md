# A scheduled start on Android: product requirements

## Problem

A start may name a time to begin at (`scheduled`). iOS 26 begins such an activity by itself, also while the
application does not run. On Android the crate answered `UNSUPPORTED_FEATURE`: a notification is posted when it
is asked for, and nothing in the crate runs when the time comes.

liveupdate now has the part that Android can start without the application: its Android library sets an alarm,
and a receiver posts the stored content at the time. A start takes `start_at`, the activity is `scheduled`
until then, and the lifecycle answers what happened meanwhile at the next call.

## Goal

The same request starts an activity later on both platforms. On Android the record is `pending` until the time,
with the rules iOS has for it, and the application learns what Android made of the time it asked for.

## Requirements of this change

Each requirement is a row of `docs/requirements.md` with the test that is its evidence.

| Id | Requirement |
| --- | --- |
| REQ-A-015 | A start with `scheduled` is taken on Android where liveupdate offers it: the time reaches liveupdate, and the record is `pending`. Where it does not offer it, as with an older Android library in the application, the start is `UNSUPPORTED_FEATURE` and nothing is kept, as before. A time that has passed begins the activity at once. |
| REQ-A-016 | A pending activity follows the rules of the contract: the same start again answers it, whatever its time, and makes a scheduling that failed again; another request under its key is `CONFLICT`; an update is `CONFLICT`; an end ends it, with any policy, and nothing is ever shown: a final content that comes with the end is not applied. |
| REQ-A-017 | The activity alerts when it begins, where the device offers an alert: the contract's scheduled start always carries one. A sound the alert names is reported as `alert.sound`. The alert's title and body are iOS's: a notification alerts with its own content. |
| REQ-A-018 | The sink is told `pending` once, and `active` once when the activity has begun; an activity that the person removed before the application ran again, or that Android could not post at its time, is told `dismissed` without an `active`. The time the backend should next be asked at is the earliest start time, with the stale times. |
| REQ-A-019 | The capabilities say whether a scheduled start is offered and whether its alarm is exact, as liveupdate reports both. |
| REQ-A-020 | A scheduled start with the foreground placement is `UNSUPPORTED_FEATURE`: a service holds a notification that is there. |

## Decisions

- **The alarm is liveupdate's.** Only code that Android can start without the application can post at a time.
  This crate has no process then, and a host that owns the timer would begin the activity only while it runs.
- **The contract does not change.** `scheduled` is `{ at, alert }` on both platforms, the state is `pending`, and
  the rules of a pending activity are those written for iOS. Android's differences are in what a device offers.
- **An inexact time is a time.** Without the permission for exact alarms Android chooses the moment, within
  minutes while the device is awake. The start is taken either way, and the capability says which it will be: an
  application that needs the minute asks the person for the permission.
- **Late is not refused.** After a restart of the device, or a forced stop, the alarm is gone, and the activity
  begins at the application's next call. The record says `active` then. An application that must not show a late
  activity compares the time with its clock and ends it.
- **`scheduled` is no part of what makes two starts the same.** As on iOS, the same request under a key answers
  the pending activity whatever its time. liveupdate itself would call another time a conflict, so the crate
  compares with the request it keeps, and then asks liveupdate with the request liveupdate keeps: liveupdate
  answers the activity, and hands it to its Android side again when that had failed.
- **An end of a pending activity needs no device that keeps a last content.** Nothing was shown, so the policy
  has nothing to say, and liveupdate is asked for an end without a time.

## Not in this change

- **Setting the alarm again after a restart of the device.** liveupdate's receiver can be declared for it by the
  application; the crate documents how, and declares nothing.
- **Asking for the permission for exact alarms.** It is the application's to declare and the person's to grant.
- **A scheduled template (contract v1).** The contract has `scheduled` for layouts alone.
