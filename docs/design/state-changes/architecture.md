# State changes on Android: architecture

```text
            own call (start, update, end)        sync_events (startup, resume, a signal, a timer of the host)
                       │                                   │
                       ▼                                   ▼
          liveupdate: next_check is "now"?        liveupdate: events (taps, openings; dismissals only wake)
                       │ yes                               │
                       └──────────────┬────────────────────┘
                                      ▼
                       liveupdate: state_changes  ──▶  Event::State, once per state
```

- One account. liveupdate keeps, with each receipt, the state it last answered. The backend reports what
  `state_changes` answers and nothing else, so no state is reported twice and none is lost with the process.
- The backend's own report of an end, and the report from a stored dismissal, are gone: both states come from
  the account. A stored dismissal still wakes the host, which then asks.
- The host owns the timer. `LiveUpdateBackend::next_check` hands on liveupdate's answer.
