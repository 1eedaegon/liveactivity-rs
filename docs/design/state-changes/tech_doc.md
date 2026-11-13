# State changes on Android: technical notes

## Changes by module

- `Cargo.toml`: liveupdate 0.3.
- `android`: `pump_events` and the own calls report from `state_changes`; `next_check`; the lock and the question
  of 0.3.0's own end report go. A queue for the sink, filled under one lock and handed over by one thread at a
  time, replaces the lock that was held for a whole drain.
- `android::layouts`: a stored dismissal is no event of its own.
- `testkit::memory`: `active` at a start.
- Documents: liveupdate's Android package, the AAR of 0.3, the events.

## Compatibility

- No type changes. On Android a sink now also receives `State` with `active` and `stale`, and with the key
  where it had none (templates, a backend without the app's files).
- A stored dismissal of an activity that liveupdate does not know is no longer reported.
- liveupdate 0.3 reads the receipts of 0.1 and not those of 0.2: an activity that runs while the application is
  updated from a release on liveupdate 0.2 is no longer listed. Android removes its notification with the update.
  The first ask after an update from 0.1 adopts the states it finds without reporting them.
- The Android package is `io.github.eedaegon.liveupdate` again. A host that named `io.github.gon.liveupdate`
  for liveupdate 0.2 renames it back.

## Risks

- **One more read per call.** Each own call reads the receipts once more to learn whether a state is due.
- **A start that Android shows late.** Its `active` waits for the next ask. A host without a timer reports it
  at the next `sync_events`.
- **Order against taps.** A dismissal used to stand among the taps in the order Android stored them. It now
  follows the stored events of the same drain.
- **At most once.** A state that liveupdate answered as the process died is lost. The record still says where
  the activity stands: an application that needs every state reads the list when it starts.
- **A call returns before its state arrived**, when another thread is handing events over. The sink still
  receives it, in order.
- **A failed read of the states after a drain.** The stored events of that drain reach the sink, and the call
  fails, so that the host asks again: the dismissal whose stored event prompted it is found by the next ask.

## Test plan

- Integration (`tests/exchange.rs`): each row of the sequence table on the default backend, over a driver whose
  clock the test sets, for a template and a layout; a stored dismissal reports once; a call without a state due
  asks Android nothing more; `next_check` in each of its four answers, and nothing consumed before a sink is
  connected; a sink that is busy while another thread ends an activity, a sink that calls the backend, and a
  sink that panics.
- Unit: the test kit's backend.
- Device: an Android phone: a start, a stale time a few seconds ahead, an update, an end, with a listener on the
  state events.
