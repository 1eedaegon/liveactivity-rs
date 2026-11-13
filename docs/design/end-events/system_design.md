# End events: system design

## When the event is sent

For `end` and `end_layout` on the default Android backend:

1. Read liveupdate's list: is the activity of this id running (not `ended`, not `dismissed`)?
2. End it, as before.
3. When the end succeeded and the activity was running, hand `State { activityId, activityKey, state: "ended" }`
   to the sink.

| Call | Event |
| --- | --- |
| an end of a running activity, any policy | `State` with `ended` |
| a delayed end | `State` with `ended` at the call; nothing when the last content goes |
| an end again, or of an activity the person dismissed | none: `dismissed` was reported when it happened |
| an end that fails (`NOT_FOUND`, `UNSUPPORTED_FEATURE`, a final content that is refused) | none |

Two ends of one activity at the same time report once: the backend runs the three steps one end at a time.

## Threads and order

The sink is called on the thread that runs the end, after the end is done and outside the backend's locks, so a
sink may call the backend. Events that liveupdate stored reach the sink on `sync_events`; between those and an
end's event the order is the order of the calls.

## Other backends

- iOS: unchanged. ActivityKit reports `ended` for every end while the application runs.
- The test kit's backend without a platform reports its ends the same way.
- A backend of a host decides for itself; the rule is in the contract.
