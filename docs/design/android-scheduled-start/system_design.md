# A scheduled start on Android: system design

## A start

1. The request is validated as on every platform: a time, and an alert with a title and a body.
2. The backend drops what Android has no use for (pushes, the iOS options) and reconciles with liveupdate.
3. A running or pending activity under the key is compared without `scheduled`: the same request answers that
   activity, another is `CONFLICT`.
4. Where liveupdate does not offer a scheduled start, or the start asks for the foreground placement, the answer
   is `UNSUPPORTED_FEATURE`, and nothing is kept.
5. The layout is projected as for a start now, with the alert of `scheduled` as the alert of an update is
   projected: `alert` in the presentation where the device offers it, `alert` named otherwise, `alert.sound`
   named for a sound.
6. liveupdate's start gets `start_at`. Its record is `scheduled`, which the backend answers as `pending`. A time
   that has passed is liveupdate's to begin at once.

## Until the time

| Call | Answer |
| --- | --- |
| The same start, whatever its time | the pending record |
| Another start under the key | `CONFLICT` |
| An update | `CONFLICT` |
| An end, with any policy | the record, `ended`; nothing is shown |
| A list | the record, `pending` |

## At the time

The application runs: the host asks the backend at the time `next_check` named, liveupdate posts what is due, and
the sink is told `active`. The alarm finds the activity begun.

The application does not run: Android starts liveupdate's receiver, which posts the stored content, alerting
where the start alerts. No Rust runs. At the application's next call, liveupdate answers what Android shows:

| What Android shows then | The record | The sink |
| --- | --- | --- |
| The notification | `active`, or `stale` past its stale time | `active` once |
| Nothing, the person removed it | `dismissed` | `dismissed`, with no `active` before it |
| Nothing, Android could not post it | `dismissed` | `dismissed`, with no `active` before it |
| Nothing yet, the alarm was lost | begins now: `active` | `active` once |

## Events

`pending` is told once, when the backend first reports the states after the start. The events go through the
queue of the backend as every state does, one at a time and in order.
