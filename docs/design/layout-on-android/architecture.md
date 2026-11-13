# A layout on Android: architecture

Nothing moves. The projection (`android::layout`) still turns one snapshot of a layout into one liveupdate
snapshot, and the store (`android::layouts`) still sends it. What changes is what the projection is told and
what it may use.

```text
layout + values ──▶ projection ──▶ liveupdate snapshot ──▶ post
                        ▲                                    │ refused for its links
        what the device offers (asked once,                  ▼
        and only for a layout that could use it)     the same snapshot without links
```

- The projection is given what the device offers of `links` and `chronometer`. A layout that has neither a URL,
  a link nor a timer beside a progress costs no question to the device.
- The options of the start (`android.publicVersion`, `android.placement`) are this crate's envelope. The store
  adds the public version to what is sent and hands the placement to liveupdate's start.
- The event of an opening comes from liveupdate's queue, like taps and dismissals, on `sync_events`.
