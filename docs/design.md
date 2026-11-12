# Design

## Structure

```text
application or framework plugin
        |  commands                      ^  taps
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
  with fixed JSON. Layout nodes and styles are activitykit's, re-exported.
- **Interface** (`backend`, `event`, `service`): `LiveActivityBackend` has one
  method per command of the contract. `LiveActivity` validates a request and
  hands it to the backend it was connected to. `EventSink` takes taps.
- **Default backends**: `ios` runs the commands on activitykit, with templates
  as layout activities of a fixed tree; `android` runs templates on the
  liveupdate engine over a driver, a string bridge or the engine's own JNI.
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
   would refuse is refused before any backend, with `INVALID_INPUT`.
4. **A backend says what it lacks.** Layouts, images and permission requests
   default to `UNSUPPORTED_FEATURE`, and capabilities report them, so a host
   composes a default backend with its own native side instead of forking it.
5. **Exchangeable means conformant.** A backend that passes the conformance
   suite can replace another without a change in the caller.

## Open

- Android layouts, permission dialogs and images have no default backend:
  liveupdate's AAR draws system templates only.
- A wire schema for adapters that are not written in Rust.
- Publication on crates.io and a semver policy.
