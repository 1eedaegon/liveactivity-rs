# State changes on Android: system design

## When states are reported

| Moment | What the backend does |
| --- | --- |
| after its own `start`, `update`, `end` (templates and layouts) | reads `next_check`; when a state is due now, takes `state_changes` and hands each to the sink |
| `sync_events` and `pump_events` | drains the stored events (taps and openings to the sink; a dismissal is only the reason to ask), then takes `state_changes` and hands each to the sink |

The first costs one read of the receipts for a call that changed no state. The second reconciles with what
Android shows, which is where a dismissal and a stale time are noticed.

## The event

`Event::State { activityId, activityKey, state }`, as on iOS. The key is the record's, so it is present for a
template and without the app's files. An activity that liveupdate does not know reports nothing.

| Sequence | Events |
| --- | --- |
| a start that Android shows at once | `active` |
| a start that Android shows a moment later | nothing at the call; `active` at the next ask (`next_check` says soon) |
| the stale time passes | `stale` at the next ask (`next_check` says at that time) |
| an update of a stale activity | `active` |
| an update of an active activity | nothing |
| an end, any policy | `ended`; an end again, nothing |
| the person removes the notification | `dismissed` at the next ask, which the stored event prompts |

## Order and threads

The sink is called on the thread of the call that asked, after the call's own work, and outside the layouts'
lock. In one drain the stored events come first, then the states. Two threads that ask at once get each state
once between them; their order is not defined.

## `next_check`

`LiveUpdateBackend::next_check() -> Result<NextCheck>`, liveupdate's type: `Now`, `Soon`, `At(unix_ms)`,
`Never`. A host with a thread calls `pump_events` then: at once, shortly and a little later each time for a
few times, at the time, or after the next signal. Other backends have no such question: their events come by
themselves.

## The test kit's backend

It reports `active` at a start and `ended` at an end, as a platform does.
