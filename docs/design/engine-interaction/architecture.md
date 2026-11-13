# Engine interaction: architecture

Nothing moves. The crate owns the envelope of a request, each engine its vocabulary, and validation runs before
any backend.

| Part | Who owns it | What changes |
| --- | --- | --- |
| The `toggle` node, a node's `ifStale` | activitykit, re-exported | Read by this crate's strict reader as activitykit's types; the Android projection gets an arm for a toggle and a rule for `ifStale`. |
| `scheduled` and `ios.transient` of a start, `sound` of an alert | this crate's envelope | New members, checked with activitykit's rules before any backend. |
| `on` of an action event | this crate's event | From activitykit's event on iOS; on Android from the value the activity shows. |

The iOS backend hands activitykit the new members of a start and of an alert. The Android backend refuses a
scheduled start before it keeps anything, drops the iOS options from what it keeps and compares, and names the
sound of an alert that alerts.
