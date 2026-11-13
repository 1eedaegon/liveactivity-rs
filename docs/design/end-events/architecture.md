# End events: architecture

```text
application ── end ──▶ liveactivity (Android backend) ── end ──▶ liveupdate ── cancel ──▶ notification
     ▲                         │
     └── State { ended } ──────┘   the backend reports what its own call did
```

- liveupdate and its Android side do not change. The backend knows that an end succeeded, and whether the
  activity was running before it, so it is the one that reports.
- The event is the one iOS sends, `Event::State`, with the same payload. No type is added.
- What the person does still comes from liveupdate's queue, on `sync_events`.
