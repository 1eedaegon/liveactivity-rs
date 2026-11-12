# liveactivity-rs

A framework-neutral Live Activity API for Rust. Framework plugins expose one
interface to their applications, and a native
implementation only has to fit it: iOS ActivityKit and Android Live Updates
are the defaults, and any other backend connects in their place.

Status: pre-release. The contract is version 1 and may still change before
the first published version.

The workspace contains:

- `liveactivity`: the contract, the object-safe interfaces, and the default
  backends. `activitykit` (iOS) and `liveupdate` (Android) are dependencies
  behind default features; the crate only fits them to the interface.
- `mock-framework-plugin`: an external consumer that implements both bridge
  directions against the interface alone (`default-features = false`).
- requirements, architecture decisions and a traceable test plan under
  `docs/`.

## Use

```rust
// The default backend of the target platform.
let live = liveactivity::connect_native(sink)?;

// Or any other backend through the same interface, for example the Android
// engine over a framework's own bridge.
let live = liveactivity::LiveActivity::connect(sink, |context| {
    Ok(liveactivity::android::LiveUpdateBackend::over_bridge(context, bridge))
})?;

// Portable content runs on whichever backend is connected.
use liveactivity::{StartRequest, Template};
let activity = live
    .start(StartRequest::template(
        "order:42",
        Template::progress("Order 42", "Preparing", 0.1),
    ))
    .await?;
```

## Implementing or replacing a backend

Implement `LiveActivityBackend` and pass it to `LiveActivity::connect`. The
`testkit` feature gives the checks that make it exchangeable with the default
ones, and a reference backend without a platform:

```rust
let report = liveactivity::testkit::conformance(&live).await;
report.assert_passed();
```

## Intended integration

```text
Framework command -> LiveActivity -> LiveActivityBackend -> native platform
Framework event   <- EventSink    <- BackendContext      <- native platform
```

Cargo features decide which implementation is compiled. `LiveActivity::connect`
selects and injects the implementation. This avoids mutually exclusive backend
features and keeps every framework's types outside this crate.

## Verify

```sh
nix develop --command cargo test --workspace
nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings
nix develop --command cargo llvm-cov --workspace --all-features --fail-under-lines 80
bash tools/check-requirements.sh
bash tools/check-dependencies.sh
```
