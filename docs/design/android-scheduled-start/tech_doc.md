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

## What stays the application's

- `SCHEDULE_EXACT_ALARM` in its manifest, and the person's grant, for a start on the minute.
- `RECEIVE_BOOT_COMPLETED` and liveupdate's receiver declared for it, for a start that survives a restart of the
  device.
