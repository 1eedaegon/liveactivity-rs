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

The alert of an update reaches ActivityKit. Android alerts again with that revision where the device offers
`alert`; elsewhere it updates silently and names `alert` in the record's `degradedFeatures`.

Evidence: `ios::tests::req_i_002_an_alert_reaches_the_kit_with_its_update` and
`exchange::req_i_002_an_alert_alerts_on_android_where_the_device_offers_it`.

## REQ-I-003 The iOS surface of a layout

What activitykit 0.4 added to a layout reaches ActivityKit as written: a node's `accessibility`, `transition` and
`privacy`, the `link` node, the layout's `url`, a style's `contentTransition`, and the island's `priority`,
`belowIfTooWide` and `margins`. A request that breaks one of activitykit's rules for them is `INVALID_INPUT` on
every platform.

Evidence: `ios::tests::req_i_003_the_ios_surface_of_a_layout_reaches_the_kit` and
`contract::req_i_003_a_wrong_member_of_the_ios_surface_reaches_no_backend`.

## REQ-E-002 Started, state and permission events

The sink receives a layout activity that began, a new lifecycle state of any activity, and the permissions after
the person changed them. On Android a dismissal is a state event.

Evidence: `ios::backend::tests::req_e_002_starts_states_and_permissions_reach_the_sink` and
`exchange::req_a_002_taps_of_a_content_and_dismissals_reach_the_sink`.

## REQ-E-003 An end is reported on Android

On Android the sink receives `State` with `ended`, the activity's id and its key, when an end of the application
ends a running activity: a template or a layout, at once or with a last content that stays. An end of an activity
that is already over and an end that fails report nothing. The test kit's backend without a platform does the
same.

Evidence: `exchange::req_e_003_an_end_of_the_application_is_reported_on_android` and
`testkit::memory::tests::req_e_003_the_backend_without_a_platform_reports_its_ends`.

## REQ-A-001 Android content of a snapshot

A snapshot's Android content is shown as given, in place of the projection of the layout. A snapshot without one
is projected as before.

Evidence: `exchange::req_a_001_an_android_content_is_shown_as_given_in_place_of_the_projection`.

## REQ-A-002 Actions of an Android content

A tap on an action of an Android content reaches the sink with the key of its activity; a tap on an action that
neither the layout nor a content declared is dropped.

Evidence: `exchange::req_a_002_taps_of_a_content_and_dismissals_reach_the_sink`.

## REQ-A-003 Style support in the capabilities

Capabilities report which notification styles the device draws and which extensions it offers, when the Android
backend runs layouts.

Evidence: `exchange::req_a_003_capabilities_report_the_styles_the_device_draws`.

## REQ-A-004 What Android leaves out of the iOS surface

The projection names what it does not show: `layout.link` for a shown link and `layout.url` for the layout's URL
where the device offers no links (REQ-A-008 has the rest), `layout.transition` for a shown node with a
transition, and `layout.privacy` for a shown node marked sensitive when a locked screen shows the notification's
content. Accessibility members are named nowhere.

Evidence: `android::layout::tests::req_a_004_the_projection_names_what_android_leaves_out_of_the_ios_surface`
and `exchange::req_a_004_a_public_activity_names_the_privacy_a_notification_cannot_keep`.

## REQ-A-005 A style that changes between snapshots

A layout projects to its own style with every snapshot. A snapshot of another style than the one the activity
shows is shown where the device offers `styleChange`, and is `UNSUPPORTED_FEATURE` elsewhere.

Evidence: `exchange::req_a_005_a_snapshot_of_another_style_is_shown_where_the_device_changes_styles`.

## REQ-A-006 An end that keeps the last content

`EndPolicy::After` keeps the last content of a template or a layout activity on Android until its time, where the
device offers `delayedEnd`. Elsewhere it is `UNSUPPORTED_FEATURE` and the activity stays. An end sent again
answers the record, with or without a final content, which an activity that is over does not take; without a
time it removes a last content that is still shown.

Evidence: `exchange::req_a_006_a_delayed_end_keeps_the_last_content_where_the_device_offers_it`.

## REQ-A-007 Extensions of an Android content

An Android content takes what liveupdate 0.2 added to a presentation: the URL a tap opens, an action that opens a
URL, a clock in the header, a revision that alerts, a public version, a small icon, an accent color and a sub
text. A start that gives a public version to an activity that is not private is `INVALID_INPUT` on every
platform. A content that uses a member the device does not offer is `UNSUPPORTED_FEATURE` on Android, except the
alert, which is left out and named in `degradedFeatures`. A content that Android refuses when it posts it, such
as one whose URL nothing in the application opens, is `INVALID_INPUT`: a start that is refused leaves no activity
and its key free, and an update that is refused leaves the activity and its record as they were, so that the
corrected content can use the same revision.

Evidence: `layout::tests::req_a_007_a_content_takes_the_extensions_of_a_presentation`,
`exchange::req_a_007_the_extensions_of_a_content_reach_android` and
`exchange::req_a_007_a_content_that_android_refuses_at_the_post_leaves_nothing_behind`.

## REQ-A-008 Links of a layout on Android

Where the device offers `links`, the projection gives the notification the layout's `url` and makes each shown
`link` an action that opens its URL, titled by the link's label and named `link:<n>` after its place among the
links of the tree. A URL that liveupdate does not take, and a link beyond the three actions, is named as before.
When the device refuses the links at the post, because nothing in the application opens a URL, the snapshot is
shown without them and the record names `layout.url` and `layout.link`.

Evidence: `android::layout::tests::req_a_008_links_of_a_layout_become_what_the_notification_opens` and
`exchange::req_a_008_a_url_nothing_opens_is_left_out_and_named`.

## REQ-E-004 An opening is reported

The sink receives `Opened`, with the activity's id and key and the id of an opening action, when the host reported
that the person opened the application from a notification (Android).

Evidence: `exchange::req_e_004_an_opening_from_a_notification_is_reported`.

## REQ-A-009 A timer beside a progress

Where the device offers `chronometer`, a layout with a progress and a timer shows the timer as the clock in the
header, and `layout.timerText` is not named.

Evidence: `android::layout::tests::req_a_009_a_timer_beside_a_progress_is_the_header_clock`.

## REQ-A-010 Labels on Android

A shown node that Android cannot draw and that has an accessibility label, a value or both is projected as that
text. A button or a link without a text is titled by a label inside it or its own. A node hidden from assistive
technologies is decoration: it is left out without being named.

Evidence: `android::layout::tests::req_a_010_labels_are_the_text_of_what_android_cannot_draw`.

## REQ-A-011 A public version of the start

A start may give a public version among its Android options, for a private activity only. Where the device
offers `publicVersion`, every snapshot that has none of its own shows it on a secure lock screen; elsewhere the
record names `publicVersion`.

Evidence: `layout::tests::req_a_011_a_public_version_is_an_option_of_a_private_start` and
`exchange::req_a_011_the_public_version_of_a_start_goes_with_every_snapshot`.

## REQ-A-012 A foreground placement

A start may ask for the foreground placement among its Android options. The capabilities report whether the
device offers it; where it does not, the start is `UNSUPPORTED_FEATURE`, and a second activity with the placement
is `SYSTEM_LIMIT`.

Evidence: `exchange::req_a_012_a_start_may_ask_for_the_foreground_placement`.

## REQ-N-001 No framework in the crate

The crate depends on its two engines, serialization and errors only.

Evidence: `contract::req_n_001_the_crate_declares_only_its_engines_and_serde`.
