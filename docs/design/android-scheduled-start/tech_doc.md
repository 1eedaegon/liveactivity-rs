# A scheduled start on Android: technical notes

## What liveupdate gives

- `StartRequest::start_at: Option<u64>`, epoch milliseconds, with schema version 2 and the tagged placement.
- `ActivityState::Scheduled`, and `ActivityRecord::start_at` on every record of the activity.
- `presentation.scheduled_start` and `presentation.exact_start` among the capabilities.
- `state_changes()` answers `scheduled` once and `active` once; `next_check()` is `At(start time)` when that is
  the earliest.
- For a driver in tests: `schedule` and `starts`.

## In this crate

- `android::layouts::start`: the refusal of `scheduled` goes. The comparison of a start with the request an
  activity keeps leaves `scheduled` out on both sides. The projection takes the alert of `scheduled` where it took
  none; the request gets `start_at`.
- `android::convert`: `Scheduled` is `Pending`, in records and in states; the two capabilities.
- `android::layouts::update`: a pending activity answers `CONFLICT` before liveupdate is asked, whatever the
  revision: liveupdate would answer an update that repeats the content of the start.
- `android::layouts::end`: a pending activity ends without a final content and without a time.
- `layout::PresentationCapabilities` is liveupdate's type: `scheduled_start` and `exact_start` come with the
  dependency, absent from an older library as false.
- The memory driver of the tests keeps a clock already; it gets the schedule, an alarm the tests fire, and the
  reports of its starts.

## What is checked on a device

- A start a minute ahead with the application's process ended: the notification is there after the time, and the
  next launch answers `active`.
- The same with notifications turned off before the time: `dismissed`, with no `active`.
- An end before the time: nothing is shown at the time.
- The capability with and without the permission for exact alarms.

## What a phone showed

A Galaxy S23 with Android 17 ran an application built on this change, with liveupdate 0.4.0's Android library. Each
start was 30 s ahead, and the application's process was ended right after it.

| Run | What happened |
| --- | --- |
| Without the permission for exact alarms | `scheduledStart` true, `exactStart` false. The record was `pending` and the sink heard `pending`. The notification was there 55 s after the start was asked, 25 s after its time. The application that was opened then listed the activity as `active` |
| With the permission | `exactStart` true. The notification was there 34 s after the start was asked, looked for every 5 s |
| Ended before its time | Nothing was shown, 95 s after the start was asked |
| Notifications turned off before its time | Nothing was shown, and the application that was opened afterwards listed the activity as `dismissed` |

Android started a process of the application for the alarm each time. Not run: a restart of the device, a doze,
and a start whose alarm could not be set.

## What stays the application's

- `SCHEDULE_EXACT_ALARM` in its manifest, and the person's grant, for a start on the minute.
- `RECEIVE_BOOT_COMPLETED` and liveupdate's receiver declared for it, for a start that survives a restart of the
  device.
