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
this crate's contract takes a member that activitykit's does not. Each valid one also reads by serde alone into
this crate's typed requests, passes their check, and weighs written back what the fixture states; an end is
written as it was read.

A name that is none of the six commands takes no request, not that of the command it resembles, and a start of
contract v1 is no start of a layout, nor the other way round.

Evidence: `layout::tests::req_c_005_a_request_is_read_strictly_in_this_crates_types`,
`engine_fixtures::req_c_005_the_engines_fixtures_get_the_same_verdict_here`,
`engine_fixtures::req_c_005_the_valid_fixtures_read_into_the_typed_requests_and_weigh_the_same_written_back` and
`requests::req_c_005_a_request_is_read_under_its_own_command_and_under_no_other_name`.

## REQ-C-006 The same check on every platform

A relevance is finite, an alert has a title and a body within their lengths, and an Android content is one
liveupdate takes: a timer style, for one, ends after its start and has its members, of their types, and no
other. The check runs before any backend, whatever the platform.

Evidence: `layout::tests::req_c_006_what_a_request_says_beside_the_layout_is_checked`,
`contract::req_c_006_a_wrong_member_of_another_platform_reaches_no_backend` and
`requests::req_c_006_a_timer_style_of_an_android_content_reads_from_json_and_is_checked_in_full`.

## REQ-C-007 Platform members stay with their platform

The Android options and the Android content of a request never reach the iOS engine: two starts that differ only
in them are the same activity on iOS. The relevance and the push request are no part of what Android keeps and
compares.

Evidence: `ios::tests::req_c_007_android_members_never_reach_the_ios_engine` and
`exchange::req_a_001_an_android_content_is_shown_as_given_in_place_of_the_projection`.

## REQ-C-008 The limits of a template's request

A `start`, an `update` and an `end` of contract v1 that arrive as JSON are checked at each limit: the last value
within it is taken, and the first one past it is `INVALID_INPUT`.

- A key has up to 128 bytes of ASCII letters, digits, `.`, `_`, `:` and `-`, and begins with a letter or a digit.
- A title has up to 120 code points and a status up to 240, whatever their bytes. Neither is blank as
  ECMAScript's `\S` reads it: a byte order mark alone is blank, and a next-line character is not.
- A progress is a number from 0 to 1, a revision a whole number up to 2^53 - 1, and a stale time a whole number
  up to 8,640,000,000,000,000.
- A snapshot has one presentation, a progress or a timer, and a start the one of its template. A timer ends
  after its start, by the largest time.
- An id has up to 256 code points. An end names a time with `after`, up to the largest one, and with no other
  policy.
- No member is `null`, and none is one that the contract does not name.

The requests of contract v1 that activitykit ships as fixtures, since its 0.6.3, get the same verdict here: each
valid one is taken, and each other is `INVALID_INPUT`.

Evidence: `requests::req_c_008_the_fixtures_of_the_templates_get_their_verdict`, with the requests in
`crates/liveactivity/tests/fixtures/requests.json`,
`requests::req_c_008_a_template_request_is_taken_at_each_limit_and_refused_past_it`,
`requests::req_c_008_a_template_request_of_a_wrong_shape_is_refused`,
`requests::req_c_008_a_snapshot_has_the_one_presentation_of_its_template`,
`requests::req_c_008_a_timer_ends_after_its_start_and_an_end_says_when_only_with_after` and
`engine_fixtures::req_c_008_the_engines_template_fixtures_get_the_same_verdict_here`.

## REQ-C-009 A number keeps its value

A request arrives as JSON text, and each number in it reaches the check as the double that was written. A number
just off what its member takes, past the end of a range or beside a whole number, is refused: no rounding on the
way brings it back.

Evidence: `requests::req_c_009_a_number_keeps_its_value_when_read_from_json_text`.

## REQ-C-010 A refusal is one of two errors

Whatever is wrong with a request, the error is `Error::invalid()` or `Error::payload_too_large()`, the whole
error with its message: it names no member and repeats no value of the request.

Evidence: `requests::req_c_010_a_refusal_repeats_nothing_of_the_request` and
`engine_fixtures::req_c_010_every_refused_fixture_gets_one_of_the_two_errors`.

## REQ-C-011 What a layout request says beside its layout, as JSON

The options of a start (`ios`, `scheduled` and `android`) and the alert of an update belong to their command, and
are an unknown member of any other. Each has its members and no other, each member has its JSON type, and none is
`null`.

- A scheduled time is a whole number up to the largest time, and comes with its alert.
- The sound of an alert is the name of a file: up to 64 ASCII letters, digits, `.`, `_` and `-`, of which the
  first is a letter or a digit.
- A placement is `tagged` or `foreground`.
- A public version has a title and a status within their lengths, and is of a private start only: in the options
  of the start and in the Android content of its snapshot alike. An update does not say how its activity started.
- A typed start writes these members as it read them: none that the request left out, and one that says its
  default.

Evidence: `requests::req_c_011_the_sound_of_an_alert_is_the_name_of_a_file`,
`requests::req_c_011_the_options_of_a_start_have_their_members_and_their_json_types`,
`requests::req_c_011_a_public_version_in_a_content_is_of_a_private_start`,
`requests::req_c_011_each_member_belongs_to_its_command` and
`requests::req_c_011_a_typed_start_writes_its_members_as_it_read_them`.

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

## REQ-I-004 A toggle

A layout takes the `toggle` node as activitykit takes it: it stands where a button may, shows a boolean value
under an action id, and counts with buttons and links toward the four of a region. A tap on it reaches the sink
as `Action` with `on`, the state the person asked for; a button's tap has none. A request that breaks a rule of
the node is `INVALID_INPUT` on every platform.

On iOS a button's tap has none since activitykit 0.6.4, which this crate builds on. The system hands the
application a value that an intent never set as `false`, and activitykit's intent carried the state as one
optional value: a button's tap reached the sink with `on: false`, as a toggle's that asks to switch off. The
intent now says beside the state whether the tap is a toggle's. This crate hands on what activitykit reports
and cannot tell the two apart itself.

Evidence: `ios::tests::req_i_004_a_toggle_reaches_the_kit_and_its_tap_says_what_was_asked`,
`contract::req_i_004_a_wrong_toggle_reaches_no_backend` and
`engine_fixtures::req_i_004_the_engines_intent_says_whether_a_tap_is_a_toggles`. The tap itself is run by
activitykit's tests on the simulator.

## REQ-I-005 Nodes that follow staleness

A node's `ifStale` reaches ActivityKit as written, on any node: the system draws the node only while the content
is stale, or only while it is not.

Evidence: `ios::tests::req_i_005_a_node_that_follows_staleness_reaches_the_kit`.

## REQ-I-006 A scheduled and a transient start

A start takes `scheduled`, a time and the alert the system shows then, and the iOS option `transient`. Both reach
ActivityKit, and a scheduled start answers a record that is `pending`. A time or an alert that is none of the
contract is `INVALID_INPUT` on every platform.

Evidence: `ios::tests::req_i_006_a_scheduled_and_a_transient_start_reach_the_kit` and
`contract::req_i_006_a_wrong_schedule_reaches_no_backend`.

## REQ-I-007 The sound of an alert

An alert takes `sound`, the file name of a sound of the application, with an update and with a scheduled start,
and it reaches ActivityKit. A name that is no file name of the contract is `INVALID_INPUT` on every platform.

Evidence: `ios::tests::req_i_007_the_sound_of_an_alert_reaches_the_kit` and
`contract::req_i_007_a_wrong_sound_reaches_no_backend`.

## REQ-I-008 The answer of an end

An end answers the record as `ended`, a template's and a layout's, also when ActivityKit has already reported the
activity as `dismissed`. After an end that removes the activity at once, that report can reach the handle before
the answer is read: an iPhone 16 with iOS 27.2 did not answer `ended` for such an end in half of twelve runs. The
list and the state events go on saying what ActivityKit says.

Evidence: `ios::tests::req_i_008_an_end_answers_ended_also_once_the_system_dismissed_the_activity`.

## REQ-I-009 The small region

A layout's `small` reaches ActivityKit as written: what the activity shows where the system asks for the small
family, in the Smart Stack of a paired Apple Watch and in CarPlay. It is a tree of at most 12 nodes without buttons,
toggles or links; a request that breaks that is `INVALID_INPUT` on every platform. A layout without it asks for none,
and a template has none.

Evidence: `ios::tests::req_i_009_the_small_region_reaches_the_kit`.

## REQ-E-002 Started, state and permission events

The sink receives a layout activity that began, a new lifecycle state of any activity, and the permissions after
the person changed them. On Android a dismissal is a state event.

Evidence: `ios::backend::tests::req_e_002_starts_states_and_permissions_reach_the_sink` and
`exchange::req_a_002_taps_of_a_content_and_dismissals_reach_the_sink`.

## REQ-E-003 An end is reported on Android

On Android the sink receives `State` with `ended`, the activity's id and its key, when an end of the application
ends an activity: a template or a layout, at once or with a last content that stays. An end sent again and an end
that fails report nothing. The test kit's backend without a platform does the same. The report comes from
liveupdate's account of states (REQ-E-005), at the call: so an end of an activity whose `dismissed` was
reported, which makes its record `ended`, is reported as that state too.

Evidence: `exchange::req_e_003_an_end_of_the_application_is_reported_on_android` and
`testkit::memory::tests::req_e_003_the_backend_without_a_platform_reports_its_ends`.

## REQ-E-005 Each state once on Android

On Android the sink receives `State` with the state an activity has when the backend asks liveupdate, if it is
another than the one the sink was told last, once: `active` when Android shows it, `stale` when its stale time
has passed, `active` again after an update that makes it fresh, `ended`, `dismissed`. A state that came and went
between two asks is not reported. It names the id and the key, of a template and of a layout, with or without
the app's files. The backend asks after each of its own calls that looks at what Android shows (a start, an
update, an end and a list, of templates and of layouts), when a state is due and a sink is connected, and on
`sync_events`; the stored event of a dismissal is no second report. A call without a state due asks Android
nothing more for it. A drain whose question for the states fails hands over the stored events it took and
fails; the states stay due.

Evidence: `exchange::req_e_005_each_state_of_an_activity_reaches_the_sink_once`.

## REQ-E-006 When to ask again

The default Android backend says when it is worth asking again: now, soon, at the earliest stale time ahead, or
not until an operation or an event.

Evidence: `exchange::req_e_006_the_backend_says_when_to_ask_again`.

## REQ-E-007 One event at a time, in order, on Android

On Android the sink is called for one event at a time, in the order the backend took the events from liveupdate,
also when several threads ask at once: a state never arrives before the one that preceded it. No lock is held
while the sink runs, so a sink may call the backend, `sync_events` included. A sink that panics loses the event
it was handed and no other: what was queued behind it reaches the sink with the next ask.

Evidence: `exchange::req_e_007_events_reach_the_sink_one_at_a_time_in_the_order_they_were_taken`.

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

## REQ-A-013 A toggle on Android

On Android a toggle is an action of the notification. Its title is the toggle's label, found as a button's is,
with a check mark before it while the value is on. Its tap reaches the sink with `on`, the other state than the
value in the activity's latest snapshot; a tap on a button or on an action of an Android content has none. An
action id is a toggle's when a toggle is the control the notification shows under it: the first shown one with
that id that has a title. A toggle without a title or without a place among the actions is left out and named
`layout.toggle`.

Evidence: `exchange::req_a_013_a_toggle_is_an_action_on_android_and_its_tap_says_what_was_asked` and
`android::layout::tests::req_a_013_a_toggle_is_titled_by_its_label_and_its_value`.

## REQ-A-014 Staleness, start options and sounds on Android

Android draws a layout as it is while fresh: a node for stale content is left out, one for fresh content is
drawn, and the record names `layout.ifStale`. The sink is told `stale` (REQ-E-005), so the application can
update. A scheduled start is `UNSUPPORTED_FEATURE` and leaves nothing where the device offers none (REQ-A-015).
The iOS options are no part of what Android keeps and compares. The sound of an alert is named `alert.sound` where
the update alerts.

Evidence: `exchange::req_a_014_android_names_what_it_leaves_of_staleness_and_sounds_and_refuses_a_schedule` and
`android::layout::tests::req_a_014_a_layout_is_drawn_as_while_fresh_and_its_staleness_is_named`.

## REQ-A-015 A scheduled start on Android

A start with `scheduled` is taken on Android where liveupdate offers it: the time reaches liveupdate, whose
Android library begins the activity then, also while the application does not run, and the record is `pending`.
Where it is not offered, as with an older Android library in the application, the start is
`UNSUPPORTED_FEATURE` and nothing is kept. A time that has passed begins the activity at once: its record is
never `pending`.

Evidence: `exchange::req_a_015_a_scheduled_start_waits_on_android_and_begins_at_its_time`.

## REQ-A-016 The rules of a pending activity on Android

A pending activity follows the rules of the contract. The same start again answers it, whatever its time and
also without one, and a scheduling that failed is made again by it: until then that activity has no alarm, and
nothing begins it. Another request under its key is `CONFLICT`. An update is `CONFLICT`. An end ends it, with any
policy, and nothing is ever shown: a final content that comes with the end is checked and not applied. Its key is
free then.

Evidence: `exchange::req_a_016_a_pending_activity_keeps_its_key_takes_no_update_and_can_be_ended`.

## REQ-A-017 The alert of a scheduled start on Android

The activity alerts when it begins, where the device offers an alert: the contract's scheduled start always
carries one. A device that offers none begins it silently, and the record names `alert`. A sound the alert names
is named `alert.sound` where it alerts. The alert's title and body are iOS's: a notification alerts with its own
content.

Evidence: `exchange::req_a_017_a_scheduled_start_alerts_when_it_begins_where_the_device_alerts`.

## REQ-A-018 The states of a scheduled start on Android

The sink is told `pending` once, and `active` once when the activity has begun, also when it began while the
application did not run. An activity that the person removed before the application ran again, or that Android
could not post at its time, is told `dismissed` without an `active`. One whose alarm was lost begins at the
application's next call. The time the backend should next be asked at is the earliest start time, with the stale
times.

Evidence: `exchange::req_a_018_the_sink_hears_pending_and_what_became_of_the_start`.

## REQ-A-019 Capabilities of a scheduled start

The capabilities of the Android presentation say whether a scheduled start is offered (`scheduledStart`) and
whether its alarm is exact (`exactStart`), as liveupdate reports both.

Evidence: `exchange::req_a_019_the_capabilities_say_whether_a_start_can_be_scheduled_and_how_exactly`.

## REQ-A-020 A scheduled start and the foreground placement

A scheduled start with the foreground placement is `UNSUPPORTED_FEATURE`, and nothing is kept: a service holds a
notification that is there.

Evidence: `exchange::req_a_020_a_scheduled_start_takes_no_foreground_placement`.

## REQ-A-021 The small region on Android

A notification has no place for the small region: a watch and a car show the notification by themselves. Nothing
of the region is drawn, it does not choose the notification's style, and the record names `layout.small`.

Evidence: `android::layout::tests::req_a_021_the_small_region_is_named_and_never_drawn`.

## REQ-A-022 A native call that panics

A call of the Android side that panics, as a framework's native call may, ends the one call of the application
that made it: the panic reaches the caller, and the Android backend catches none. No later call fails for it:
each lock of the Android backend is taken again after a thread panicked under it, the layouts' among them. What
the call had begun is left as by a native call that failed at that point, which the next layout call reconciles.
One thing differs: the events that a drain had taken before the panic are not handed over by that call, as they
are when the read of the states fails. They wait in the queue and reach the sink with the next ask.

Evidence: `exchange::req_a_022_a_native_call_that_panics_fails_no_later_call`.

## REQ-A-023 A layout's bar shows its value

The projection asks Android to fill a layout's bar up to the layout's progress and no further. It is one segment
of the whole scale with the progress in it, and it asks Android to draw what is ahead of the progress differently
from what is behind it (`styledByProgress`). Android 16's progress style gives every segment the filled look when
that is off, and leaves showing the progress to a tracker icon or to the segments' colors, of which the
projection has none: the bar was full whatever its value. A content that the application gives says for itself
how its bar is styled, and the projection changes nothing of it.

On an emulator of Android 16 the bar of a layout at 0.15, 0.65 and 1 was thick up to its value and a thin line
ahead of it, and at 0 the thin line alone; before, it was one thick bar at 0.65. No test here draws a bar: the
tests read what the projection asks for.

liveupdate keeps the projection of a start and compares a repeated start with it. An activity that an earlier
version started, without the flag, and that Android has not shown yet therefore answers `CONFLICT` to a repeat
of its start. One that Android shows answers the repeat as that activity, and an update replaces its projection.

Evidence: `android::layout::tests::req_a_023_a_layouts_bar_is_filled_up_to_its_value`, and
`exchange::req_a_001_an_android_content_is_shown_as_given_in_place_of_the_projection`, which reads the flag in
what the driver was handed, for a projection and for a content that leaves it out;
`exchange::req_a_023_a_start_projected_before_the_flag_is_compared_as_liveupdate_keeps_it` for a start of an
earlier version.

## REQ-N-001 No framework in the crate

The crate depends on its two engines, serialization and errors only.

Evidence: `contract::req_n_001_the_crate_declares_only_its_engines_and_serde`.

## REQ-N-002 No unwrap outside tests

The crate's own code and its examples never call `unwrap` or `expect`: a value that is not there is an error to
answer, and no reason to end the host's process. Clippy denies both at the root of the library, everywhere but in
its tests, and in each example, which is an application's code. A test may use them: a test that panics has failed.

Evidence: `contract::req_n_002_clippy_denies_unwrap_and_expect_outside_tests`, and the clippy step of CI, which
takes a warning for an error and runs over every target.
