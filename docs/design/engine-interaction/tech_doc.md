# Engine interaction: technical notes

## Changes by module

- `Cargo.toml`: activitykit 0.5.
- `layout`: `Alert.sound`, `ScheduledStart`, `IosOptions`, the members `scheduled` and `ios` of a start, `on` of
  `ActionEvent`; re-exports of what activitykit added. The checks beside activitykit's own.
- `ios`: the start's schedule and transient style, the alert's sound, `on` of a tap.
- `android::layout`: an arm for a toggle, `ifStale` in what is shown, `layout.ifStale`.
- `android::layouts`: a scheduled start is refused, the iOS options are dropped, `alert.sound`, `on` of a tap.
- Documents: the contract, the README, the setup notes, the example.

## Compatibility

- Breaking for a Rust caller, as a minor release before 1.0: `ActionEvent`, `Alert` and `LayoutStartInput` gain
  members, and the re-exported `Element` a variant, `Node` a member.
- JSON written for 0.4 reads unchanged, and an event without `on` is written as before.
- An Android store written by 0.4 reads unchanged: the new members of a start are absent there.

## Risks

- **A tap that comes late.** Android stores taps while the application does not run. `on` is read from the
  latest snapshot, not from the one the person saw.
- **A check mark in a title.** A device's font may draw it as another glyph; the title is still the label.
- **A pending activity.** iOS takes no update before the activity began. The backend passes on activitykit's
  `CONFLICT`.

## Test plan

- Unit (`layout`): the strict reader takes the new members and refuses each broken rule.
- Unit (`ios`): the members reach the kit; a tap carries `on`; a scheduled start is pending.
- Unit (`android::layout`): a toggle's title with and without the mark, its place among the actions,
  `layout.toggle`, `layout.ifStale` and what is drawn.
- Integration (`tests/exchange.rs`, `tests/contract.rs`): the Android backend end to end, and a wrong member that
  reaches no backend.
- activitykit 0.5's request fixtures get the same verdict from this crate's reader.
- Device: an Android phone and an iPhone, one layout with a toggle and nodes that follow staleness.
