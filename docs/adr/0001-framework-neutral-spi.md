# ADR-0001: Framework-neutral backend SPI

Status: accepted

Date: 2026-11-12

## Context

A framework plugin needs ActivityKit and Android Live Update behavior, while other
Rust frameworks need different lifecycle and native-call transports. Cargo
features are unified across a dependency graph and are a poor exclusive
backend selector.

## Decision

liveactivity defines LiveActivityBackend, EventSink and BackendContext.
Framework plugins implement or construct a backend and inject it through
LiveActivity::connect. Cargo features only make implementations available.

## Positive consequences

- Core models and behavior are shared.
- Framework crates own their handles and lifecycle.
- More than one implementation may compile together.
- Mock implementations need no mobile SDK.

## Negative consequences

- Calls use dynamic dispatch and boxed futures.
- Native build integration remains adapter-specific.
- A Rust trait is not a stable dynamic-library ABI.

## Alternatives

- Exclusive Cargo features: rejected because feature unification can enable
  multiple providers.
- One universal string command bridge: rejected because it loses typed
  compile-time checks at the main Rust boundary.
- Adopt Istmo wholesale: deferred because it replaces the transport runtime
  rather than wrapping the existing platform crates.
