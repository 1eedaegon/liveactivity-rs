# ADR-0003: One crate with default backends

Status: accepted

Date: 2026-11-12

## Context

The intent is that Rust frameworks use one interface, a plugin hands a backend
to that interface, and a native implementation only has to fit it. The default
native implementations already exist as crates: activitykit and liveupdate.

## Decision

The workspace publishes one crate, `liveactivity`. It holds the contract, the
interfaces and two modules that fit the existing engines to the interface:
`ios::ActivityKitBackend` and `android::LiveUpdateBackend`. The engines are
ordinary Cargo dependencies behind the default features `activitykit` and
`liveupdate`; nothing of them is reimplemented. `connect_native` selects the
default of the target, and `LiveActivity::connect` takes any other backend.

## Consequences

- A framework plugin depends on one crate.
- An adapter that brings its own backend disables the default features and
  gets the interface without either engine.
- The features are additive, as ADR-0001 requires: both engines compile on
  every target and the choice is made at construction.
- Neutral content stays each engine's own JSON contract until the typed
  template and layout contract of phase 3 replaces it.
