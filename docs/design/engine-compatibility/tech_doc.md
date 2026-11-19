# Engine compatibility: technical notes

- **Breaking.** liveupdate 0.5 to 0.7 change types that this crate re-exports, and the crate's own `Template`
  and `EndPolicy` become `#[non_exhaustive]`: the next version is a minor one, 0.8.0.
- **The strict reader.** A layout request is taken only when it encodes back to itself. liveupdate leaves a
  member of a presentation out when it is at its default, so the reader lets a default be written: `false`, an
  empty array, and now `true` for `wearOngoingActivity`, the one member that is true by default.
- **`StartInput::new`.** It takes its template from its snapshot, as liveupdate's `StartRequest::new` does. A
  snapshot that is neither a progress nor a timer is refused by `validate`, whatever the template says.
- **What is not tested here.** The Ongoing Activity on a watch, the metrics below Android 17 and the dropped
  tap below Android 12 are the AAR's. The crate's tests read what liveupdate is handed, and for the last two the
  rows of liveupdate's specification.
- **The store's room is reachable.** Thirty live activities whose Android content has a text of 4,000
  characters of four bytes fill it: the test starts them.
