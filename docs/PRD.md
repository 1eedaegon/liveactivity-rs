# Product requirements

Status: feasibility spike, extended by ADR-0002 to ADR-0004 (the "out of scope"
list below describes the first spike only)

Date: 2026-11-12

## Problem

The current Tauri plugin owns framework commands, platform selection and native
bridges. Reusing the same ActivityKit and Android Live Update behavior from a
second Rust UI framework would copy that orchestration and allow the contracts
to drift.

## Goal

Prove that a framework-neutral Rust crate can own the observable Live Activity
contract while independently compiled framework plugins provide command and
event bridges.

## Users

- The Tauri plugin maintainer, who must preserve the existing JavaScript API.
- Rust framework adapter authors, who need a small stable interface.
- ActivityKit and LiveUpdate backend maintainers, who must not depend on a UI
  framework.

## In scope

- Framework-neutral request, response, capability, error and event models.
- An object-safe LiveActivityBackend interface implemented by external crates.
- An EventSink and BackendContext for native-to-framework events.
- Backend injection at construction time.
- Conformance tests using a separate mock framework-plugin crate.
- A documented path to wrap activitykit and liveupdate.

## Out of scope for this spike

- Calling real ActivityKit, Android Live Updates, JNI or Swift.
- Tauri JavaScript API compatibility tests.
- Widget Extension, Gradle or Android Manifest generation.
- Runtime-loaded Rust dynamic libraries or a stable C ABI.
- Publishing crates.

## Success criteria

1. A crate outside liveactivity implements both bridge directions using
   only public types.
2. Start, update, end, list and capability calls cross the injected backend.
3. A simulated native event reaches the framework event sink.
4. Two backend implementations can be swapped without changing the caller API.
5. The core dependency graph contains no Tauri, Wry, Dioxus or Slint package.
6. CI enforces formatting, Clippy, tests, Rust 1.90 compatibility and at least
   80 percent line coverage.

## Decision gate

Proceed to production adapters only after every requirement in requirements.md
has passing evidence. The first production proof should wrap the current Tauri
Android bridge without changing its public JavaScript contract.
