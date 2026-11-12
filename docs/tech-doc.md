# Technical design

## Canonical contract

The public API of liveactivity is canonical because every intended
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
- Content is a portable template or the selected backend's own JSON, marked
  native (ADR-0004).
- Every top-level message carries contractVersion.
- Enums use camelCase JSON names and are non-exhaustive in Rust where backend
  evolution is expected.

A typed portable layout contract is still open: layout content is native to
the iOS backend.

## Feature policy

Features are additive. A feature may expose an implementation but must not
disable or silently replace another backend. Runtime construction selects the
backend.

## Test strategy

- Integration tests compile an independent mock-framework-plugin package.
- Command tests observe each typed call through a channel.
- Event tests simulate a backend-originated event.
- Serialization tests pin the public JSON shape.
- CI checks the dependencies of the interface against an allowlist.
- Coverage must be at least 80 percent.

## Production migration

1. Done: default backends over activitykit and liveupdate in this crate.
2. Open: Android layouts, the permission dialog and images in a
   framework-neutral AAR; the liveupdate backend reports them unsupported.
3. Open: a typed portable layout contract.
4. Then stabilize the contract and publish.
