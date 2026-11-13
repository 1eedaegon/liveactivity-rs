# State changes on Android: system design

## When states are reported

| Moment | What the backend does |
| --- | --- |
| after its own `start`, `update`, `end` and `list` (templates and layouts), when a sink is connected | reads `next_check`; when a state is due now, takes `state_changes` and queues each for the sink |
| `sync_events` and `pump_events` | drains the stored events (taps and openings for the sink; a dismissal is only the reason to ask), then takes `state_changes`, and queues all of it for the sink |

The first costs one read of the receipts for a call that changed no state. The second reconciles with what
Android shows, which is where a dismissal and a stale time are noticed. A list reconciles as well, so what it
finds is reported with it: the records it answers and the events agree.

liveupdate answers the state an activity has, when it is another than the one it answered last. So the sink is
told where an activity stands, not every step it took: a state that came and went between two asks is not
reported.

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
| the person removes it, and the application ends it before anyone asked | `ended` alone |
| the person removes it, an ask, then the application ends it | `dismissed`, then `ended`: the end makes the record `ended` |
| the stale time passes, and an update makes it fresh before anyone asked | nothing |

## Order and threads

What the backend takes from liveupdate it puts into a queue, under one lock that it holds from the question to
the queue: the queue has the order of liveupdate's answers. One thread at a time hands the queue to the sink,
event by event, with no lock held. A thread that finds another one handing over leaves its events in the queue,
and that thread hands them over too.

- The sink is never called by two threads at once, and no state arrives before the one that preceded it.
- A sink may call the backend, `sync_events` included: what such a call takes is queued behind the event in hand.
- A call can return before its state reached the sink, when another thread is handing over at that moment.
- In one drain the stored events come first, then the states.
- A state is handed over at most once. liveupdate marks it answered before the backend has it; one that was
  answered as the process died, or that a sink panicked on, is not answered again.
- A sink that panics ends that hand-over; the next ask goes on with the rest of the queue.

## `next_check`

`LiveUpdateBackend::next_check() -> Result<NextCheck>`, liveupdate's type: `Now`, `Soon`, `At(unix_ms)`,
`Never`. A host with a thread calls `pump_events` then: at once, shortly and a little later each time for a
few times, at the time (one that has passed means at once), or after the next signal. It reads the receipts and
asks Android nothing, so it does not know of a removal: the signal of the stored event is what prompts that
ask. Other backends have no such question: their events come by themselves.

## The test kit's backend

It reports `active` at a start and `ended` at an end, as a platform does.
