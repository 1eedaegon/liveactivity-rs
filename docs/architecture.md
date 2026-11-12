# Architecture

## Context

The existing Tauri plugin already separates Android transport through a
liveupdate Bridge implemented with Tauri PluginHandle. This spike promotes the
same dependency inversion to the whole Live Activity surface.

## Components

    Framework application
            |
            v
    Framework plugin commands
            |
            v
    LiveActivity service
            |
            v
    LiveActivityBackend trait
            |
       +----+----------------+
       |                     |
    ActivityKit backend   LiveUpdate backend
       |                     |
    Swift bridge          Tauri/Wry/JNI transport

Native events travel in the reverse direction:

    Native callback -> BackendContext -> EventSink -> framework event API

## Ownership

- liveactivity owns domain models, errors, backend interface, event
  interface and orchestration.
- activitykit and liveupdate remain platform engines.
- A framework plugin owns framework handles, lifecycle hooks, IPC commands and
  event emission.
- A setup tool or adapter owns Xcode, Gradle, manifest and entitlement changes.

## Injection

Cargo features make implementations available. They do not choose an exclusive
backend. LiveActivity::connect receives a factory closure, creates an immutable
BackendContext around the framework sink and stores the returned trait object.

This permits more than one backend implementation in a dependency graph and
avoids Cargo feature-unification conflicts.

## Error boundary

The core exposes a non-exhaustive machine-readable ErrorCode and a message.
Adapters may add internal detail to their own logs but must not expose native
secrets, paths or unstable platform error strings.

## Security boundary

- All serialized input is validated before native invocation in production.
- Framework permissions remain the framework plugin's responsibility.
- Event sinks receive owned values and no native pointers.
- The core contains no unsafe code or FFI.

## Prior-art consequence

Istmo demonstrates that a typed Rust Live Activity contract can generate
Kotlin and Swift dispatchers. This project deliberately stays smaller: it wraps
the existing activitykit and liveupdate engines and lets framework adapters
implement the public SPI without adopting a new mobile runtime.

See prior-art.md and ADR-0001.
