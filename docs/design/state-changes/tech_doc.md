# State changes on Android: technical notes

## Changes by module

- `Cargo.toml`: liveupdate 0.3.
- `android`: `pump_events` and the own calls report from `state_changes`; `next_check`; the lock and the question
  of 0.3.0's own end report go.
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

## Test plan

- Integration (`tests/exchange.rs`): each row of the sequence table on the default backend, over a driver whose
  clock the test sets, for a template and a layout; a stored dismissal reports once; `next_check` in each of its
  four answers.
- Unit: the test kit's backend.
- Device: an Android phone: a start, a stale time a few seconds ahead, an update, an end, with a listener on the
  state events.
