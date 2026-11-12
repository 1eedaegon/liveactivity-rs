# Verifiable requirements

The public Rust types and traits of liveactivity-core are the authoritative
contract for this spike. Prose explains intent but does not redefine field
shapes. Test names and comments carry the requirement identifiers below.

## REQ-F-001 External backend implementation

An independent Cargo package shall implement LiveActivityBackend without
private APIs, framework types or code generation.

Evidence: external_adapter::req_f_001_external_plugin_implements_the_backend.

## REQ-F-002 Command forwarding

LiveActivity shall forward capabilities, start, update, end and list exactly
once to the injected backend and return its typed response.

Evidence: external_adapter::req_f_002_commands_reach_the_injected_backend.

## REQ-F-003 Reverse event bridge

A backend shall publish a native-originated event through BackendContext, and
the framework-provided EventSink shall receive the same typed event exactly
once.

Evidence: external_adapter::req_f_003_native_events_reach_the_framework_sink.

## REQ-F-004 Backend substitution

The caller shall use the same LiveActivity type and methods with independently
implemented backends.

Evidence: external_adapter::req_f_004_backends_are_substitutable.

## REQ-F-005 Stable error propagation

Backend errors shall retain their machine-readable code and human-readable
message at the framework boundary.

Evidence: external_adapter::req_f_005_backend_errors_keep_their_contract.

## REQ-F-006 Capability negotiation

The backend shall report its identity and supported features before the caller
starts an activity.

Evidence: external_adapter::req_f_006_capabilities_are_backend_owned.

## REQ-F-007 Versioned serialization

Requests, responses, errors and events shall serialize with an explicit
contract version and stable camelCase field names.

Evidence: serialization_contract::req_f_007_public_models_have_stable_json.

## REQ-F-008 Boundary validation

The core shall reject an unsupported contract version or empty required
identifier before invoking a backend or framework event sink.

Evidence:
external_adapter::req_f_008_invalid_messages_stop_at_the_core_boundary.

## REQ-N-001 Framework-neutral core

liveactivity-core shall not depend on Tauri, Wry, Dioxus, Slint or their native
handles. Framework-specific types stay in adapter crates.

Evidence: CI dependency-tree gate and
external_adapter::req_n_001_public_contract_is_framework_neutral.

## REQ-N-002 Thread-safe boundaries

Backends, event sinks and LiveActivity shall be Send and Sync so callbacks may
arrive from native or framework-managed threads.

Evidence: external_adapter::req_n_002_boundaries_are_send_and_sync.

## REQ-N-003 Object-safe backend

LiveActivityBackend shall support Arc dyn LiveActivityBackend on Rust 1.90.
Async operations therefore return BackendFuture rather than exposing async fn
in the object-safe trait.

Evidence: external_adapter::req_n_003_backend_is_dyn_compatible.

## REQ-N-004 Coverage gate

Host-side line coverage shall remain at or above 80 percent.

Evidence: GitHub CI cargo llvm-cov gate.

## REQ-I-001 Build integration separation

Widget extensions, Kotlin libraries, manifests, entitlements and framework
lifecycle hooks shall remain outside liveactivity-core.

Evidence: CI dependency-tree gate and architecture review.

## Compatibility rule

Contract changes are additive within a major version. Removing a field,
repurposing an enum variant or changing error semantics requires a new major
contract version and migration notes.
