# ADR-0004: Portable content and exchangeable backends

Status: accepted

Date: 2026-11-12

## Context

A backend could be replaced at construction, but a caller could not be: the
content of a request was the selected backend's own JSON, and permissions,
images and promotion were reached on the backend type. The trial in
tauri-plugin-live-activity kept an `Arc` of each backend for that.

## Decision

1. `Content` is `template` or `native`. A template (title, status, and a
   progress or a countdown) is portable: the Android backend maps it to
   liveupdate's progress and timer templates, the iOS backend to a layout
   activity with a fixed tree, taken from tauri-plugin-live-activity 0.4 with
   its marker, so activities of that plugin stay readable. Native content is
   the backend's own contract, as before.
2. A start carries a first revision and a fallback for portable content. A
   record carries the template it shows beside the backend's own record.
3. `LiveActivityBackend` gains `permissions`, `request_permission` and
   `write_image` with defaults that report unknown and unsupported.
   `Capabilities` is tri-state and names templates, layouts, images and the
   native presentation.
4. The `testkit` feature holds the conformance suite and `MemoryBackend`.
   A backend is exchangeable when it passes the suite.

5. The core validates portable requests against the strictest limits the two
   engines share, so a request one backend takes every backend takes.
6. What an end can do differs between platforms and is a capability:
   `finalContent` and `delayedDismissal`. Android ends at once without either.

## Known differences that remain

- After an end without final content, iOS reports the next revision and no
  stale date; Android and the memory backend keep the last ones. The suite
  requires only that the revision does not go back.
- A repeated start with another fallback is the same activity on iOS, which
  has no fallback, and a conflict elsewhere.
- Blank text is judged with Rust's whitespace, which differs from the engines'
  in U+FEFF and U+0085.

## Consequences

- The plugin trial names a backend in one place per platform and holds only
  `LiveActivity`.
- What a backend cannot do is visible before the call (capabilities) and as
  `unsupported` at the call. The liveupdate backend reports layouts, images
  and permission requests so: its AAR draws system templates and reads
  permissions, and the Tauri plugin's Kotlin still does the rest.
- Layout content is not portable yet. A typed layout contract is phase 3.
- JSON of contract version 1 changed shape (`content.kind`) before any
  release; the version number stays 1.
