# Engine extensions: architecture

Nothing moves. The picture of [the change before](../engine-expressiveness/architecture.md) holds: the crate owns
the envelope, each engine its vocabulary, and validation runs before any backend.

What changes is which side answers three questions on Android.

| Question | Before | Now |
| --- | --- | --- |
| May a snapshot have another style than the activity shows? | This crate refused it. | The device's `styleChange` capability decides; this crate asks it only when the style differs. |
| May an ended activity keep its last content? | This crate refused it. | liveupdate decides by the device's `delayedEnd`. |
| Does an update alert? | Never; `alert` was named as left out. | Where the device offers `alert`, this crate marks the revision as alerting. |

- The iOS backend passes a layout through as before. The new members are inside the layout, so no call of the
  backend changes.
- The Android backend still sends a snapshot's content as given or else its projection. The projection names the
  new members it does not show. A layout now projects to its own style whatever the activity shows, so a
  projection after a content of another style is a style change like any other.
- Capabilities are liveupdate's, less what the contract cannot ask for.
