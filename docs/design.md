# Design

## Structure

```text
application or framework plugin
        |  commands                      ^  events
        v                                |
   LiveActivity  -- validates -->  LiveActivityBackend  -->  EventSink
                                        |
          +-----------------------------+---------------------------+
          |                             |                           |
  ios::ActivityKitBackend   android::LiveUpdateBackend     a backend of the host
          |                             |
     activitykit                    liveupdate
```

- **Contract** (`models`, `layout`, `error`, `validation`, `image`): the
  requests, records, capabilities, permissions and error codes, as serde types
  with fixed JSON. The envelope of a request is this crate's: identity,
  revisions, the options of each platform, the relevance, the alert of an
  update. The vocabulary of each presentation is its engine's, re-exported:
  layout nodes and styles are activitykit's, the notification styles of an
  Android content are liveupdate's.
- **Interface** (`backend`, `event`, `service`): `LiveActivityBackend` has one
  method per command of the contract. `LiveActivity` validates a request and
  hands it to the backend it was connected to. `EventSink` takes an `Event`:
  a tap, or what a push brings on iOS.
- **Default backends**: `ios` runs the commands on activitykit, with templates
  as layout activities of a fixed tree; `android` runs templates on the
  liveupdate engine over a driver, a string bridge or the engine's own JNI,
  and, given the folder of the app's files, projects layouts onto the
  engine's notification styles and keeps their records. Elsewhere the default
  is `UnsupportedBackend`.
- **Testkit**: the conformance suite and a backend without a platform.

## Decisions

1. **One interface, injected through one builder.** `LiveActivity::builder()`
   is where the two sides meet: the using side names its event sink, the
   supporting side its backend, and whatever is not named is the default. The
   backend is chosen there, not by Cargo features, so several backends can be
   compiled together and a caller never names one after the connection.
2. **The engines are dependencies.** activitykit and liveupdate keep their
   lifecycle, revisions and receipts. This crate maps the contract onto them
   and adds only what neither has: templates on iOS, the split between
   template and layout activities, and the handles of ended activities.
3. **The contract is typed and validated once.** A request that one backend
   would refuse is refused before any backend, with `INVALID_INPUT`. A layout
   request is read in this crate's types; activitykit checks the part it
   knows, and its own request fixtures are run against this reader.
4. **A backend says what it lacks.** Layouts, images and permission requests
   default to `UNSUPPORTED_FEATURE`, and capabilities report them, so a host
   composes a default backend with its own native side instead of forking it.
   Android's permission dialog is such a part: it needs an Activity, which
   only the host has.
5. **Exchangeable means conformant.** A backend that passes the conformance
   suite can replace another without a change in the caller.
6. **What one platform reads stays with it.** Android options and content
   never reach the iOS engine; a relevance and a push request are no part of
   what Android keeps. What a platform would show if it could, such as an
   alert on Android, is named in the record's `degradedFeatures`.

What the engines are about to open, and what this crate does with each, is in
[design/engine-expressiveness](design/engine-expressiveness/PRD.md).

## Open

- A signal from liveupdate's AAR to the JNI transport when a tap is stored, so
  that a host without a native side of its own need not call `sync_events`.
- A wire schema for adapters that are not written in Rust.
- A semver policy.
