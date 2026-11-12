# Task list

## Phase 0: feasibility

- [x] Create a private liveactivity-rs repository.
- [x] Research existing Rust Live Activity plugins and adapters.
- [x] Define product, architecture, system and technical requirements.
- [x] Implement liveactivity-core public contract.
- [x] Implement an external mock framework plugin.
- [x] Add command, event, substitution, error and serialization tests.
- [x] Enforce 80 percent coverage and Rust 1.90 in CI.
- [x] Push the initial validated commit.

## Phase 1: current Tauri adapter

- [ ] Wrap activitykit without changing its behavior.
- [ ] Move the current liveupdate TauriBridge behind LiveActivityBackend.
- [ ] Reuse existing JSON contract fixtures.
- [ ] Preserve the current JavaScript API and permissions.
- [ ] Verify Android Activity recreation and iOS event acknowledgements.

## Phase 2: second framework proof

- [ ] Select one of Wry, Dioxus or Slint.
- [ ] Implement only the framework transport and event sink.
- [ ] Run the same conformance suite.
- [ ] Record missing lifecycle or build-system abstractions.

## Phase 3: stabilization

- [ ] Replace unstructured content with stable template and layout contracts.
- [ ] Define the native wire schema if non-Rust adapters are required.
- [ ] Add semver and migration policy.
- [ ] Decide crate publication names and governance.
