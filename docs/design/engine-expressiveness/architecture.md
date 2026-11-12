# Engine expressiveness: architecture

```text
            application, or a framework plugin
                         │  typed requests, or the same as JSON
                         ▼
        ┌──────────────── liveactivity ────────────────┐
        │ envelope: key, revision, lifecycle, options  │
        │ validation: the same on every platform       │
        │ events: one enum for both platforms          │
        ├───────────────────────┬──────────────────────┤
        │ ios backend           │ android backend      │
        │ passes layout, values,│ shows the Android    │
        │ relevance, alert      │ content, or projects │
        │                       │ the layout           │
        └───────────┬───────────┴──────────┬───────────┘
                    ▼                      ▼
               activitykit             liveupdate
        nodes, styles, renderer   notification styles, receipts
```

- A request names its layout in activitykit's vocabulary and, optionally, its Android content in liveupdate's.
  The crate defines neither vocabulary; it defines where each sits in a request and who reads it.
- The iOS backend never sees Android members. The Android backend never sees push, relevance or alert, except to
  report an alert it could not give.
- Validation runs in the service, before the backend, and does not depend on the platform it runs on.
- The engines stay usable on their own. Nothing here requires a change in either.
