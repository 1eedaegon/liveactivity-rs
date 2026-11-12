# Requirements

Each requirement names the test that is its evidence. `tools/check-requirements.sh`
fails when an identifier has no test.

## REQ-C-001 Typed contract with fixed JSON

Requests, records, capabilities, permissions and errors are serde types with
camelCase JSON and codes out of `ERROR_CODES`.

Evidence: `contract::req_c_001_the_contract_has_stable_json`.

## REQ-C-002 Validation before the backend

A request outside the contract is `INVALID_INPUT` and reaches no backend.

Evidence: `contract::req_c_002_invalid_requests_reach_no_backend`.

## REQ-C-003 One builder for both sides

`LiveActivity::builder()` takes an event sink from the using side and a backend
from the supporting side. Without a backend it connects the default one of the
platform; without a sink taps are dropped.

Evidence: `contract::req_c_003_whatever_the_builder_is_not_told_is_the_default`.

## REQ-X-001 Conformance of every backend

The default Android backend, over a driver and over a string bridge, and a
backend that is no default engine keep the same lifecycle rules.

Evidence: `exchange::req_x_001_every_backend_keeps_the_rules_of_the_interface`.

## REQ-X-002 One caller, exchanged backends

Caller code that names no backend gets the same answers from each.

Evidence: `exchange::req_x_002_one_caller_gets_the_same_answers_from_exchanged_backends`.

## REQ-X-003 Reported gaps

What a backend lacks shows in its capabilities and is refused with
`UNSUPPORTED_FEATURE`.

Evidence: `exchange::req_x_003_what_a_backend_lacks_is_reported_and_refused`.

## REQ-X-004 Taps

Taps of a backend that queues them and of one that calls back reach the same
sink, with the key of their activity when the backend knows it.

Evidence: `exchange::req_x_004_taps_of_any_backend_reach_the_same_sink`.

## REQ-X-005 Composed host backend

A host backend that hands templates to a default backend and answers the rest
itself stays conformant.

Evidence: `exchange::req_x_005_a_host_backend_composes_a_default_one_and_stays_exchangeable`.

## REQ-X-006 Foreign activities in the engine's store

An activity that an application started on the Android engine itself, outside
the template contract, is left out of `list` and does not fail it.

Evidence: `exchange::req_x_006_activities_outside_the_template_contract_do_not_break_the_list`.

## REQ-X-007 Layouts on the Android default

Given the folder of the app's files, the default Android backend runs layouts,
stores images and reports taps on declared buttons with the key of their
activity, and says so in its capabilities.

Evidence: `exchange::req_x_007_with_the_apps_files_the_android_default_runs_layouts_images_and_keyed_taps`.

## REQ-E-001 Events of taps and pushes

A tap keeps its intent until the sink has returned. A push token is in the
backend's record before its event goes out, and `push_tokens` answers the
newest of each activity.

Evidence: `ios::backend::tests::a_tap_keeps_its_intent_until_the_sink_returns_and_pushes_wait_for_nothing`
and `pushes::tests::the_book_keeps_the_newest_token_of_each_of_the_last_activities`.

## REQ-C-004 A platform without Live Activities

Elsewhere than on iOS and Android the default backend answers the two queries
and refuses every other call with `UNSUPPORTED_PLATFORM`. A blocking call on
the main thread of an Apple platform answers at once instead of hanging.

Evidence: `contract::req_c_003_whatever_the_builder_is_not_told_is_the_default`
and `blocking::tests::a_blocking_call_on_the_main_thread_answers_at_once`.

## REQ-C-005 Requests are read in this crate's types

A layout request is read strictly in this crate's types, and activitykit checks the part it knows. Members
activitykit does not know are taken; an unknown member, a `null`, a request that does not encode back to itself
and a request under another command's name are refused.

The request fixtures of the activitykit version this crate builds with get the same verdict here, except where
this crate's contract takes a member that activitykit's does not.

Evidence: `layout::tests::req_c_005_a_request_is_read_strictly_in_this_crates_types` and
`engine_fixtures::req_c_005_the_engines_fixtures_get_the_same_verdict_here`.

## REQ-C-006 The same check on every platform

A relevance is finite, an alert has a title and a body within their lengths, and an Android content is one
liveupdate takes. The check runs before any backend, whatever the platform.

Evidence: `layout::tests::req_c_006_what_a_request_says_beside_the_layout_is_checked` and
`contract::req_c_006_a_wrong_member_of_another_platform_reaches_no_backend`.

## REQ-C-007 Platform members stay with their platform

The Android options and the Android content of a request never reach the iOS engine: two starts that differ only
in them are the same activity on iOS. The relevance and the push request are no part of what Android keeps and
compares.

Evidence: `ios::tests::req_c_007_android_members_never_reach_the_ios_engine` and
`exchange::req_a_001_an_android_content_is_shown_as_given_in_place_of_the_projection`.

## REQ-I-001 Relevance

The relevance of a snapshot reaches ActivityKit with a start, an update and an end, and the record reports it.

Evidence: `ios::tests::req_i_001_a_relevance_reaches_the_kit_and_the_record_reports_it` and
`ios::kit::tests::specs_reach_activitykit_with_their_revision_times_and_push`.

## REQ-I-002 Alert of an update

The alert of an update reaches ActivityKit. Android updates silently and names `alert` in the record's
`degradedFeatures`.

Evidence: `ios::tests::req_i_002_an_alert_reaches_the_kit_with_its_update` and
`exchange::req_a_001_an_android_content_is_shown_as_given_in_place_of_the_projection`.

## REQ-E-002 Started, state and permission events

The sink receives a layout activity that began, a new lifecycle state of any activity, and the permissions after
the person changed them. On Android a dismissal is a state event.

Evidence: `ios::backend::tests::req_e_002_starts_states_and_permissions_reach_the_sink` and
`exchange::req_a_002_taps_of_a_content_and_dismissals_reach_the_sink`.

## REQ-A-001 Android content of a snapshot

A snapshot's Android content is shown as given, in place of the projection of the layout. A snapshot without one
is projected as before.

Evidence: `exchange::req_a_001_an_android_content_is_shown_as_given_in_place_of_the_projection`.

## REQ-A-002 Actions of an Android content

A tap on an action of an Android content reaches the sink with the key of its activity; a tap on an action that
neither the layout nor a content declared is dropped.

Evidence: `exchange::req_a_002_taps_of_a_content_and_dismissals_reach_the_sink`.

## REQ-A-003 Style support in the capabilities

Capabilities report which notification styles the device draws, when the Android backend runs layouts.

Evidence: `exchange::req_a_003_capabilities_report_the_styles_the_device_draws`.

## REQ-N-001 No framework in the crate

The crate depends on its two engines, serialization and errors only.

Evidence: `contract::req_n_001_the_crate_declares_only_its_engines_and_serde`.
