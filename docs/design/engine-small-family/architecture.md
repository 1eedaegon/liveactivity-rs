# Engine small family: architecture

Nothing moves. The layout of a request is activitykit's type, and validation runs before any backend.

| Part | Who owns it | What changes |
| --- | --- | --- |
| The region `small` of a layout | activitykit, re-exported | Read by this crate's strict reader as activitykit's type, with activitykit's rules. |
| The start of a layout on iOS | this crate's iOS backend | Hands the region on with the rest of the layout. |
| The projection on Android | this crate's Android backend | Reads nothing of the region and names `layout.small`. |
