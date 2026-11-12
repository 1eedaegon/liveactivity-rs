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

## Phase 1: default backends and the Tauri adapter

- [x] Extend the error and state vocabulary to the engines' (ADR-0002).
- [x] One crate, `liveactivity`: the interface, with activitykit and
      liveupdate as default-feature dependencies (ADR-0003).
- [x] Android default backend over any liveupdate::Driver or Bridge.
- [x] iOS default backend over activitykit; content mapping tested on the host.
- [x] connect_native, and LiveActivity::sync_events for queued events.
- [x] Compile for `aarch64-linux-android` (JNI path) and `x86_64-apple-ios`
      (Swift bridge) with the local toolchains.
- [x] Trial in a clone of tauri-plugin-live-activity (branch
      `feat/liveactivity`): Android v1 commands through
      `LiveUpdateBackend::over_bridge` with its TauriBridge, iOS through
      `ActivityKitBackend` behind the plugin's `Kit` seam. Host tests, and
      builds for both mobile targets.
- [x] Portable template content, first revision and fallback in the contract;
      both default backends draw it (ADR-0004).
- [x] Permissions, images and tri-state capabilities in the interface.
- [x] Testkit: the conformance suite and a reference memory backend.
- [x] Exchange proven on the host: the default Android backend (over a driver
      and over a string bridge) and the memory backend pass the same suite and
      give one caller the same answers; the plugin trial runs its Android v1
      commands on either.
- [x] The testkit's conformance suite on the iOS default backend (2026-11-12,
      iPhone 17 Pro simulator, iOS 26.3, through `connect_native` in the
      plugin's demo): every lifecycle rule holds with portable templates. The
      image write answered `configurationRequired`, the demo's widget extension
      naming no App Group, which the suite accepts as a named setup gap.
- [x] Plugin trial on Android devices (2026-11-12, debug demo of branch
      `feat/liveactivity`): Galaxy S23 on API 37 and an API 36 emulator pass
      the demo's 19 lifecycle checks; API 37 and API 26 emulators pass a
      10-check v1 lifecycle called straight through the plugin's commands.
      Portable templates were promoted on API 37 and fell back to an ordinary
      notification on API 36 and 26.
- [x] Plugin trial on iOS (2026-11-12): `tests/ios/run.sh` on an iPhone 17 Pro
      simulator with iOS 26.3 passes its 7 UI tests through the iOS backend:
      Lock Screen layouts and templates, a button tap reaching Rust and
      JavaScript, the list after a relaunch, the lifecycle checks, and a tap
      that launches the app and waits for a slow handler.
- [ ] Android layouts, the permission dialog and images are still the Tauri
      plugin's own Kotlin: the interface carries them, the liveupdate backend
      reports them unsupported. Move that Kotlin to a neutral AAR.
- [ ] A typed portable layout contract. Layout content is still native to the
      iOS backend.
- [ ] Reuse existing JSON contract fixtures.
- [ ] Verify Android Activity recreation.

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
