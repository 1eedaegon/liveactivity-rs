# A layout on Android: technical notes

## Changes by module

- `layout`: `AndroidOptions.public_version` and `.placement`, `AndroidPlacement`; the rule of the option.
- `event`: `Event::Opened` and `OpenedEvent`.
- `android::layout`: links, the header clock, labels, hidden nodes; what the projection may use comes with the
  environment.
- `android::layouts`: the device is asked for a layout that could use it; the retry without links; the public
  version of the start; the placement; the opened event.
- `android::convert`: the capabilities report the placement.

## Compatibility

- `AndroidOptions` gains two optional members; it is built with `new`, so no caller breaks. `Event` gains a
  variant; it is non-exhaustive.
- Records change for the same request: a layout with links, a timer beside a progress, or labels now shows more
  on a device that offers it, and its `degradedFeatures` names less. A node hidden from assistive technologies is
  no longer named.
- The layout store reads entries written before: the new options are absent there.

## Risks

- **A second post.** A layout whose URL nothing opens is posted twice with every snapshot: once refused, once
  without links. An application that declares the URL's scheme pays nothing. A projection with links that the
  device refuses for another reason is also sent a second time, and the second answer is the one returned.
- **A link's id.** `link:<n>` is a valid action id, so a button may have it. liveupdate takes each id once: the
  later of the two is left out and named, as two buttons with one id are.
- **A label as text.** A label is written for a screen reader. As a notification's title it may read oddly;
  it is used only for what would otherwise be left out.

## Test plan

- Unit: the projection with each row of the table, with and without what it needs.
- Integration (`tests/exchange.rs`): each requirement through the service on the default backend; the retry
  without links over a driver that refuses a URL; the opened event.
- Device: an Android phone, with an application that declares the URL's scheme and with a URL it does not open.
