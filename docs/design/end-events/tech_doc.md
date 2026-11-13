# End events: technical notes

## Changes by module

- `android`: `end` and `end_layout` report the end; a lock keeps the question and the end together.
- `testkit::memory`: `end` reports the end of a running activity.
- `event`: the documentation of `Event::State` and of the sink's threads.

## Compatibility

- No type changes. An application on Android now receives `State` events with `ended`, where it received only
  `dismissed`: a sink that took every `State` event on Android for a dismissal reads the `state` member now.
- On iOS nothing changes.

## Risks

- **One more read for an end.** The backend asks liveupdate for its list before the end, which reads the receipts
  and observes the notifications once more.
- **An end the application did not make.** Nothing but the application's call ends an activity on Android, so the
  event is complete as far as `ended` goes. A dismissal by the person or by the system is `dismissed`.

## Test plan

- Integration (`tests/exchange.rs`): a template and a layout end over a driver and over a string bridge; an end
  again, a dismissed activity, a refused end and a delayed end.
- Unit: the test kit's backend.
- Device: an end through a host application on an Android phone, with a listener on the state events.
