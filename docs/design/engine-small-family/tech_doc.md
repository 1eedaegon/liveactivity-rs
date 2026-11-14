# Engine small family: technical notes

## Changes by module

- `Cargo.toml`: activitykit 0.6.1.
- `ios`: the start of a layout hands `small` on; the layout of a template has none.
- `android::layout`: `layout.small` among what the projection names, after `layout.island`.
- Documents: the contract, the README, the setup notes and the example.

## Compatibility

- Breaking for a Rust caller, as a minor release before 1.0: `Layout` gains the member `small`.
- JSON written for 0.6 reads unchanged.
- An iOS application that wants the region drawn sets `ActivityKitSmallFamily` in its widget extension's
  Info.plist, with the widget of activitykit 0.6.
