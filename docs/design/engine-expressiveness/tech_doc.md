# Engine expressiveness: technical notes

## Changes by module

- `layout`: `AndroidOptions` and `AndroidVisibility` are defined here; `AndroidContent`, `Alert`,
  `LayoutSnapshot.relevance` and `.android`, `LayoutUpdateInput.alert`; a strict reader of this crate's types.
- `event`: `Event::Started`, `Event::State`, `Event::Permissions` and their payloads.
- `ios`: the specs carry relevance and alert; the kit's `Started`, `State` and `Authorization` events are forwarded.
- `android`: the store sends a snapshot's content or its projection; its event drain reports dismissals and takes
  the actions of contents; capabilities carry the style support.
- `blocking`, `error`: `block_on`, `wait` and the single-connection error leave the `ios` module.

## Compatibility

- Public structs gain fields (`LayoutSnapshot`, `LayoutUpdateInput`, `Capabilities`) and `Event` gains variants.
  The crate is 0.x, so this is a minor release: the commit subject is `feat:` without `!`.
- JSON written by 0.1 reads unchanged: every new member is optional. The Android layout store written by 0.1
  loads in 0.2; one written by 0.2 with an Android content does not load in 0.1, which matters only for a
  downgrade.
- `Alert` and `AndroidOptions` are `#[non_exhaustive]` and built with `new`, because the plan adds members to both.
- On iOS the Android options no longer take part in comparing a repeated start (REQ-C-007).

## Risks

- **Drift from activitykit's strict reading.** The reader here repeats activitykit's three steps on this crate's
  types. A test runs the request fixtures of the activitykit version in `Cargo.lock`, read from that crate's own
  folder, against it; the one fixture this contract answers differently is named in the test.
- **A content that changes style.** The request is valid and iOS takes it; Android answers
  `UNSUPPORTED_FEATURE`, like a delayed end. This is the one rule here that differs by platform, and it goes when
  liveupdate lets a style change.
- **Engine releases that rename members.** The vocabularies are re-exported, so an engine's breaking release is
  one of this crate too. This is accepted: the alternative is a copy of each vocabulary.

## Test plan

- Unit: strict reading and its refusals; the rules beside the layout; specs that reach the kit; events forwarded
  in order; the store with a content, with a projection, with an alert, with a dismissal.
- Integration (`tests/exchange.rs`): a content through the service on the Android default backend, and the same
  request on a backend without a platform.
- Devices: the conformance suite and a content with a segmented progress on an Android phone; the conformance
  suite on an iPhone. An alert is not sent to a person's phone in a test run.
