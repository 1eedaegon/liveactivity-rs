# liveactivity-rs

A private feasibility workspace for a framework-neutral Live Activity API.
It answers one architectural question: can Tauri and other Rust framework
plugins implement the same command and event contracts without making the
core depend on a framework?

The current spike contains:

- `liveactivity-core`: canonical Rust models and object-safe interfaces.
- `mock-framework-plugin`: an external consumer that implements both bridge
  directions and is tested through the public API only.
- requirements, architecture decisions and a traceable test plan under
  `docs/`.

## Intended integration

```text
Framework command -> LiveActivity -> LiveActivityBackend -> native platform
Framework event   <- EventSink    <- BackendContext      <- native platform
```

Cargo features decide which implementation is compiled. `LiveActivity::connect`
selects and injects the implementation. This avoids mutually exclusive backend
features and keeps Tauri, Wry, Dioxus and Slint types outside the core crate.

## Verify

```sh
nix develop --command cargo test --workspace
nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings
nix develop --command cargo llvm-cov --workspace --all-features --fail-under-lines 80
bash tools/check-requirements.sh
```

This is a contract spike, not yet the production replacement for
`activitykit`, `liveupdate` or `tauri-plugin-live-activity`.
