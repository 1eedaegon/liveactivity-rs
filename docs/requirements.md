# Verifiable requirements

The public Rust types and traits of liveactivity are the authoritative
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

## REQ-F-009 Engine error and state vocabulary

ErrorCode and ActivityState shall name every category the activitykit and
liveupdate engines report, with stable camelCase JSON names, so an adapter can
restore its existing public codes after a call crosses the core.

Evidence:
serialization_contract::req_f_009_engine_error_codes_and_states_have_stable_names.

## REQ-A-001 Android lifecycle on the liveupdate engine

The Android backend shall run start, update, end and list on the liveupdate
engine through LiveActivity, keeping the engine's idempotency and revisions, and
return the engine's record as neutral content.

Evidence: android_backend::req_a_001_the_lifecycle_runs_on_the_liveupdate_engine.

## REQ-A-002 Android error categories

Every liveupdate error shall cross the core with its category and its
sanitized message. A request the Android engine cannot honor, such as delayed
dismissal or final content on end, shall fail instead of being ignored.

Evidence: android_backend::req_a_002_engine_errors_keep_their_category.

## REQ-A-003 Android content contract

Neutral content for this backend shall be liveupdate's own JSON contract. A
content that does not match it shall fail as invalidRequest before any driver
call, without repeating the content in the message.

Evidence:
android_backend::req_a_003_content_outside_the_engine_contract_stops_before_the_driver.

## REQ-A-004 Android events

Queued Android action and dismissal events shall reach the framework EventSink
exactly once and in order. A rejected or unresolvable event shall be counted
and shall not stop the remaining events.

Evidence: android_backend::req_a_004_android_events_reach_the_framework_sink.

## REQ-A-005 Framework transport injection

A framework shall back the Android backend with only a string
liveupdate::Bridge, the shape of a framework's own native call.

Evidence: android_backend::req_a_005_a_framework_string_bridge_backs_the_same_backend.

## REQ-A-006 Android detail outside the neutral contract

Capabilities shall be reported in neutral terms, and Android-only detail such
as promotion shall stay reachable on the engine without widening the core.

Evidence:
android_backend::req_a_006_capabilities_are_neutral_and_android_detail_stays_on_the_engine.

## REQ-F-010 Queued event synchronization

LiveActivity shall let a framework ask the backend to deliver the events its
platform queued. A backend whose platform calls back shall need no
implementation.

Evidence:
external_adapter::req_f_010_a_backend_that_is_called_back_has_no_events_to_sync
and android_backend::req_a_004_android_events_reach_the_framework_sink.

## REQ-F-011 Portable content

The contract shall carry portable template content (a title, a status and a
progress or a countdown), a first revision and a fallback, with stable JSON.
A request with it shall run unchanged on every backend that reports templates.
Content in a backend's own contract shall stay possible and be marked native.

Evidence:
serialization_contract::req_f_011_portable_content_and_the_added_models_have_stable_json.

## REQ-F-012 Permissions and images in the interface

LiveActivity shall read and request operating-system permissions and store
images through the backend. A backend that provides neither shall say so with
unknown permissions and unsupported, never by silence.

Evidence:
external_adapter::req_f_012_a_backend_without_permissions_or_images_says_so,
android_backend::req_a_006_capabilities_are_neutral_and_android_detail_stays_on_the_engine
and ios::backend::tests::req_k_004_off_ios_the_backend_answers_as_activitykit_does.

## REQ-F-013 Shared limits

The core shall refuse, before any backend, a portable request that one default
engine would refuse: an activity key outside the engines' alphabet, a title or
status beyond their lengths, a time or revision out of their range. It shall
refuse a first revision or fallback beside native content, which would be
dropped.

Evidence:
serialization_contract::req_f_013_the_core_keeps_the_limits_every_backend_shares.

## REQ-X-001 Conformance of every backend

The testkit shall check the rules a backend keeps to be exchangeable: repeated
and conflicting starts, revisions, the fixed template kind, idempotent ends, no
revival after an end, and a new handle after a restart. The default Android
backend, over a driver and over a string bridge, and a backend that is no
default engine shall pass it.

Evidence: exchange::req_x_001_every_backend_keeps_the_rules_of_the_interface.

## REQ-X-002 One caller, exchanged backends

Caller code that names no backend shall get the same neutral answers from each
backend connected in its place.

Evidence:
exchange::req_x_002_one_caller_gets_the_same_answers_from_exchanged_backends.

## REQ-X-003 Reported differences

Where backends differ (images, permissions, native content), the difference
shall show in capabilities or as a stable error, not as different behavior
under the same answer.

Evidence:
exchange::req_x_003_what_backends_differ_in_is_reported_and_never_guessed.

## REQ-X-004 Events of an exchanged backend

Events of a backend that is no default engine shall reach the same EventSink in
the same neutral form.

Evidence:
exchange::req_x_004_events_of_an_exchanged_backend_reach_the_same_sink.

## REQ-K-005 iOS templates

The iOS backend shall draw a portable template as a layout activity with a
fixed tree, read it back as the template, and refuse an update of another kind.

Evidence:
ios::content::tests::req_k_005_portable_templates_become_layout_activities_of_one_kind
and ios::template::tests::req_k_005_a_template_is_a_marked_layout_and_reads_back.

## REQ-K-001 iOS content contract

Neutral content for the iOS backend shall be activitykit's request contract v2.
A content that does not match it shall fail as invalidRequest before the kit is
called, without repeating the content in the message.

Evidence: ios::content::tests::req_k_001_start_content_is_the_layout_contract
and req_k_001_update_and_end_content_is_a_snapshot_at_the_request_revision.

## REQ-K-002 iOS error categories

Every activitykit error shall cross the interface with its category and its
message.

Evidence:
ios::content::tests::req_k_002_every_activitykit_error_keeps_its_category.

## REQ-K-003 iOS records

An activity shall be returned as a neutral record whose content is
activitykit's layout record.

Evidence:
ios::content::tests::req_k_003_a_record_carries_the_layout_record_as_content.

## REQ-K-004 iOS backend off iOS

Off iOS the backend shall compile, report no capability and answer
unsupportedPlatform, while still rejecting invalid content first.

Evidence:
ios::backend::tests::req_k_004_off_ios_the_backend_answers_as_activitykit_does.

## REQ-D-001 Default native backend

connect_native shall give every target a working service on the default
backend of its platform, using activitykit and liveupdate as dependencies.

Evidence: default_backend::req_d_001_every_target_has_a_default_backend.

## REQ-D-002 Replaceable default

Another backend shall connect through the same interface in place of the
default, and the interface shall build without the default engines.

Evidence:
default_backend::req_d_002_another_backend_replaces_the_default_through_the_same_interface
and the mock fixture, which depends on liveactivity without default features.

## REQ-N-001 Framework-neutral core

The interface of liveactivity shall depend on serialization and error crates
only: on no UI framework and none of its native handles. Framework-specific
types stay in adapter crates.

Evidence: CI dependency gate (tools/check-dependencies.sh) and
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
lifecycle hooks shall remain outside liveactivity.

Evidence: CI dependency-tree gate and architecture review.

## Compatibility rule

Contract changes are additive within a major version. Removing a field,
repurposing an enum variant or changing error semantics requires a new major
contract version and migration notes.
