# A scheduled start on Android: architecture

Nothing moves. The crate owns the envelope of a start, liveupdate the lifecycle on Android and the alarm, and
validation runs before any backend.

| Part | Who owns it | What changes |
| --- | --- | --- |
| The alarm, the receiver, the post at the time | liveupdate's Android library | New in liveupdate; the application's manifest gets the receiver with the library. |
| `scheduled` until the time, what happened meanwhile | liveupdate's lifecycle | `start_at` of a start, the state `scheduled`, `state_changes()` and `next_check()` with start times. |
| `scheduled` of a start, the state `pending` | this crate's contract | Unchanged. |
| The Android backend | this crate | Hands liveupdate the time in place of refusing the start; maps `scheduled` to `pending`; compares starts without their `scheduled`. |
| The capabilities | this crate | Two members of the Android presentation, from liveupdate's. |

The Android backend stays a mapping. It sets no timer and keeps no clock: it asks liveupdate, which answers
from Android and from what its receiver stored.
