# Technical design

## Canonical contract

The public API of liveactivity-core is canonical because every intended
consumer is a Rust crate compiled in the same Cargo graph. A later Kotlin,
Swift or dynamic-library boundary will require a separate versioned wire
schema; handwritten duplicate structs are not acceptable.

## Async and trait objects

Framework plugins need runtime backend substitution, so LiveActivity stores
Arc dyn LiveActivityBackend. Trait methods return BackendFuture, a boxed Send
future borrowing the backend. This stays dyn-compatible on the MSRV without
requiring a procedural macro.

## Event interface

EventSink is a small trait rather than a framework channel type. BackendContext
owns Arc dyn EventSink and is Clone, allowing native callback registrations to
hold a context without mutable global state.

## Data model

- Activity identifiers and keys remain strings.
- Revisions are unsigned 64-bit integers.
- Platform-specific content is represented by serde_json Value during this
  interface spike.
- Every top-level message carries contractVersion.
- Enums use camelCase JSON names and are non-exhaustive in Rust where backend
  evolution is expected.

The production contract should replace unstructured content with the existing
template and layout types before release.

## Feature policy

Features are additive. A feature may expose an implementation but must not
disable or silently replace another backend. Runtime construction selects the
backend.

## Test strategy

- Integration tests compile an independent mock-framework-plugin package.
- Command tests observe each typed call through a channel.
- Event tests simulate a backend-originated event.
- Serialization tests pin the public JSON shape.
- CI inspects the core dependency tree.
- Coverage must be at least 80 percent.

## Production migration

1. Replace the mock backend with an adapter over activitykit.
2. Extract the current TauriBridge over liveupdate into a Tauri adapter.
3. Run the existing tauri-plugin-live-activity contract fixtures against the
   new service.
4. Add a second non-Tauri adapter proof.
5. Only then stabilize and publish the facade crate.
