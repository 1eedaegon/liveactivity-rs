# Task list

## Phase 0: feasibility

- [x] Create a private liveactivity-rs repository.
- [x] Research existing Rust Live Activity plugins and adapters.
- [x] Define product, architecture, system and technical requirements.
- [x] Implement liveactivity public contract.
- [x] Implement an external mock framework plugin.
- [x] Add command, event, substitution, error and serialization tests.
- [x] Enforce 80 percent coverage and Rust 1.90 in CI.
- [x] Push the initial validated commit.

## Phase 1: default backends

- [x] Extend the error and state vocabulary to the engines' (ADR-0002).
- [x] One crate, `liveactivity`: the interface, with activitykit and
      liveupdate as default-feature dependencies (ADR-0003).
- [x] Android default backend over any liveupdate::Driver or Bridge.
- [x] iOS default backend over activitykit.
- [x] connect_native, and LiveActivity::sync_events for queued events.
- [x] Portable template content, first revision and fallback in the contract;
      both default backends draw it (ADR-0004).
- [x] Permissions, images and tri-state capabilities in the interface.
- [x] Testkit: the conformance suite and a reference memory backend.
- [x] Exchange proven on the host: the default Android backend (over a driver
      and over a string bridge) and the memory backend pass the same suite and
      give one caller the same answers.
- [x] Builds for `aarch64-linux-android` and `x86_64-apple-ios`.
- [ ] Android layouts, the permission dialog and images in a framework-neutral
      AAR. The interface carries them; the liveupdate backend reports them
      unsupported.
- [ ] A typed portable layout contract. Layout content is still native to the
      iOS backend.

## Phase 2: stabilization

- [ ] Define the native wire schema if non-Rust adapters are required.
- [ ] Add semver and migration policy.
- [ ] Decide crate publication and governance.
