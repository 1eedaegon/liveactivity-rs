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

## REQ-C-012 Code written against this version compiles against the next

Every public enum of the crate is `#[non_exhaustive]`: a `match` outside the crate has a `_` arm, and compiles
when a variant is added. `Template` and `EndPolicy` join the others. Every struct of the contract that a caller
builds has a constructor that takes what is required and sets nothing else, to build on with the struct update
syntax where the struct allows it or with the constructor alone where it is `#[non_exhaustive]`. New beside
those the layout's types have: `Timer::new`, `Snapshot::progress` and `Snapshot::timer`, `StartInput::new`,
which takes its template from its snapshot, `UpdateInput::new`, `EndInput::after`, `AndroidContent::new`,
`ImageInput::new`, and `AndroidOptions::with_wear_ongoing_activity` for the member that struct gained. What a
constructor builds is written as the JSON that the literal was.

The vocabulary that the crate re-exports follows the same rule in its own crates: liveupdate since 0.7.0, whose
enums are all `#[non_exhaustive]` and whose structs have constructors (`BigTextStyle::new`, `ProgressStyle::new`,
`ProgressSegment::new`, `ProgressPoint::new`, `TimerStyle::new`, `Metric::new`, `MetricStyle::new`,
`PublicVersion::new`, beside `AndroidPresentation::new` and `NotificationAction::new`; `SamsungNowBar::default()`).
The backend's `next_check` answers liveupdate's `NextCheck`, which may gain an answer: in a `_` arm, ask again
shortly.

activitykit follows it since 0.8.0: each of its public enums, each variant with named members and each struct whose
members are all public is `#[non_exhaustive]`, and since 0.8.1 the contract's types have constructors again. A
layout is built with `Layout::new(lock_screen)` and `.island`, `.background`, `.keyline`, `.url` and `.small`, with
`Island::new`, `IslandExpanded::new`, `Style::new`, `Color::adaptive(light, dark)` and the node functions; a
literal of them, or a `match` on `Element`, `Value` or `Bindable` without a `_` arm, does not compile. What the
constructors build is written as the JSON that the literal was.

Evidence: `api::req_c_012_every_public_enum_may_gain_a_variant`,
`api::req_c_012_a_constructor_builds_what_the_literal_was`,
`api::req_c_012_the_reexported_vocabulary_is_built_on_its_constructors` and
`api::req_c_012_a_layout_is_built_on_activitykits_constructors`.

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

On iOS a button's tap has none since activitykit 0.6.4, and this crate builds on no earlier one. The system hands
the application a value that an intent never set as `false`, and activitykit's intent carried the state as one
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

## REQ-I-010 A color pair behind a card

A layout's `background` that is a light and dark pair is of the appearance its content is drawn in, on the Lock
Screen and where the system asks for the small family. activitykit's widget draws a pair itself, behind the
content, since 0.6.5, which this crate builds on. Before, a pair was the system's tint of the activity, and a card
came with the pair's light color behind content of the dark appearance: seen on simulators, on the Lock Screen of
iOS 26.3 at some starts and in the Smart Stack of a paired Apple Watch, and on no phone or watch. A color name
and a hex color are the system's tint. With a pair the system's own parts of a card, such as its question whether
the application may go on showing Live Activities, have the system's colors and not the pair's. This crate hands
the layout on as written and draws nothing.

Evidence: `engine_fixtures::req_i_010_the_engines_widget_draws_a_color_pair_itself`. The drawing is run by
activitykit's tests on the simulator.

## REQ-I-011 Buttons and toggles from iOS 17.2

A `button` and a `toggle` run their tap from iOS 17.2, as activitykit has them since 0.6.6, which this crate builds
on. Below it activitykit's widget draws them as their labels, and `capabilities().actions` is `unsupported`, since
it follows activitykit's `buttons`. A request with buttons and toggles is taken on every iOS as before. With an
earlier activitykit a button was drawn on iOS 17.0 and its tap ran nothing (17.1 was not run): Xcode writes activitykit's
intent, newer than the iOS its module is built for, into the application's App Intents metadata as one that begins
with iOS 17.2 (seen by activitykit on the iOS 17.0 simulator).

Evidence: `engine_fixtures::req_i_011_the_engines_buttons_and_toggles_begin_with_ios_17_2` and
`ios::tests::capabilities_and_permissions_follow_activitykits_authorization`. The taps are run by activitykit's tests
on the simulator.

## REQ-I-012 A layout's place on the Lock Screen

On the Lock Screen activitykit's widget gives a layout the card's whole width since 0.6.6, and a layout that is
narrower stands in its middle as before. With an earlier activitykit iOS 17.2 and 17.4 did not find a button or a
toggle of a layout narrower than the card under a tap, and opened the application as for a tap on the activity
(activitykit's simulators). The contract says how a layout is placed in the card, by activitykit's measurements on
simulators of iOS 16.2 to 26.3: the card has no margin, a layout is as wide as its widest node and stands in the
middle, a style is applied in one order, and the card is as high as the layout, at most 160 points. This crate hands
the layout on as written.

Evidence: `engine_fixtures::req_i_012_the_engines_widget_gives_a_layout_the_cards_width`. The taps are run by
activitykit's tests on the simulator.

## REQ-I-013 A build with Xcode 27

The crate builds for iOS with Xcode 27. activitykit's build script names SwiftPM's own build system since 0.6.6
(`--build-system native`): Swift Build, the default of `swift build` from Xcode 27, does not build for iOS from the
flags the script passes, and the build of the bridge failed. That was reported to activitykit by an application
built with Xcode 27.2; activitykit runs no Xcode 27, and no test of this crate builds for iOS.

Evidence: `engine_fixtures::req_i_013_the_engines_build_names_swiftpms_own_build_system`.

## REQ-I-014 What the device runs

`capabilities().ios` says what an iPhone runs of the features after the first Live Activities, as activitykit's
authorization says since 0.7.0: `push_to_start` (iOS 17.2), `push_channels` and `transient` (iOS 18),
`scheduled_start` (iOS 26, in an application built with the iOS 26 SDK) and `small_family` (iOS 18, and the widget
extension draws the small family). An application need not try a request to learn it. It is absent off iOS. In JSON
it is `ios: { pushToStart, pushChannels, transient, scheduledStart, smallFamily }`.

Evidence: `ios::tests::req_i_014_the_capabilities_say_what_the_device_runs`.

## REQ-I-015 The expanded island's corners

activitykit's widget keeps the expanded Dynamic Island's leading and trailing content 6 points from the top and from
its outer side since 0.7.0, on top of the margins and the padding a layout states: before, the top outer corner cut
the last letter of a short trailing value (activitykit's iOS 26.3 simulator). This crate hands the layout on as
written.

Evidence: `engine_fixtures::req_i_015_the_engines_widget_keeps_the_expanded_islands_corners_clear`.

## REQ-I-016 Live Activities from iOS 16.1

An iPhone runs Live Activities from iOS 16.1, ActivityKit's first release, as activitykit does since 0.8.0, which
this crate builds on; an iPad from iPadOS 17. Below that every command answers `UNSUPPORTED_PLATFORM`, whose message
says the two floors. On iOS 16.1 ActivityKit takes the content alone: a snapshot's `staleAt` and `relevance` do not
reach the system, so the stale nodes are never drawn, no `stale` state comes, and a record has no relevance (it keeps
the `staleAt` it was given). The person's frequent-pushes setting begins with iOS 16.2: on 16.1
`permissions().frequentPushes` is `denied`. An alert comes with an update as on later versions. The widget
extension can be built for iOS 16.1, and the setup and the README name the floor that activitykit has.

Evidence: `engine_fixtures::req_i_016_the_engines_iphones_begin_with_ios_16_1` and
`ios::tests::req_i_016_a_record_without_the_systems_score_has_no_relevance`. activitykit runs iOS 16.1 on no
simulator of its CI; its tests of the floors are its own.

## REQ-I-017 Activities of a later version

A request with a member, a node type or a value that this version does not know is `INVALID_INPUT`, on every
platform, as before. activitykit's widget reads leniently since 0.8.0 what it did not get from this application's
requests: a content state or a layout that a later crate or a push of a later server wrote is drawn without the
members the widget does not know, and a node of a type it does not know is drawn as nothing, where the activity was
drawn empty. A value it does not know of a member it knows still refuses the layout. A receipt of the Swift layer
that cannot be read costs that receipt, not the list: `list()` answers `NATIVE_ERROR` only for a file that cannot be
opened or written, and a load keeps the newest 256.

Evidence: `contract::req_i_017_a_request_that_this_version_cannot_read_in_full_is_refused`. The widget's reading
and the receipts are run by activitykit's tests.

## REQ-I-018 A node drawn only in StandBy

Any node of a layout may carry `ifFullscreen`, a Boolean (`Node::when_fullscreen()`, `unless_fullscreen()`), as
activitykit has it since 0.9.0: with `true` iOS draws the node only while the Lock Screen presentation fills the
screen, in StandBy (iOS 17), with `false` only while it does not, beside the other conditions. Anything but a
Boolean is `INVALID_INPUT` on every platform. A notification is never full screen: the Android projection never
shows a node with `true`, always one with `false`, does not count the first for its style, and names nothing in
`degraded_features`, since nothing that Android has is left out.

Evidence: `contract::req_i_018_a_node_says_whether_it_is_drawn_in_standby` and
`android::layout::tests::req_i_018_a_notification_never_fills_the_screen_as_standby_does`. The drawing in StandBy
is run by activitykit's tests.

## REQ-I-019 The push-to-start token the app has

With `push_to_start(true)`, `Event::PushToStartToken` brings the push-to-start token that the app has when the
Swift layer begins to watch it, at each launch, and then each new one, never the same one twice; `push_tokens()`
holds it from then on. activitykit does so since 0.9.1, which this crate builds on. Before, only a change of the
token came: on an iPhone XS with iOS 18.7 the app's token did not change and no event came in five minutes, so a
server never learned it. This crate hands each token on as it comes.

Evidence: `engine_fixtures::req_i_019_the_engine_reports_the_push_to_start_token_the_app_has`, and
`ios::backend::tests::a_tap_keeps_its_intent_until_the_sink_returns_and_pushes_wait_for_nothing` for a token handed
on and held. The token on a phone is run by activitykit's tests; a simulator has no APNs.

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

## REQ-E-008 An event of a kind that is not known

An event that liveupdate reads as of a kind it does not know, which a later AAR stored, reaches the sink as
`Event::Unknown` with the activity's id, and with its key when the backend knows the activity. Nothing else of
it is known: liveupdate reads its sequence, its activity and its time. A drain removes an event from the
adapter, so one that is not passed on is gone. A dismissal is no such event: it stays a state event
(REQ-E-005).

Evidence: `exchange::req_e_008_an_event_of_an_unknown_kind_reaches_the_sink`.

## REQ-E-009 A tap says which tap it is

`ActionEvent.tap` says which tap it is and when it was made: `id`, the same however often the tap is handed on;
`at`, the time of the tap in epoch milliseconds; `revision`, the revision of the content the activity showed; and
`late`. Each is left out when it is not known, and so is `tap` when nothing is. On iOS they are activitykit's, since
0.7.0: a tap that the application did not take within the intent's first two posts (200 ms) is kept in the
application's container, at most 32 and 12 hours, and comes to the sink of a later kit, in this launch or the next,
with `late`; the activity may have ended since, and its key is then the one the button was drawn with. On Android
`id` is the adapter's sequence of the stored tap and `at` the time it was stored; there is no revision, and a tap is
never late.

Evidence: `ios::kit::tests::req_e_009_a_tap_of_activitykit_says_which_tap_it_is`,
`android::layouts::tests::req_e_009_a_stored_tap_says_its_sequence_and_time` and
`contract::req_e_009_a_tap_is_written_with_what_is_known_of_it`.

## REQ-E-010 A tap that the application's component took

A tap whose target took it with `LiveUpdateActions.take` is stored by liveupdate as handled since 0.8.0, and
reaches the sink as `Event::Handled`: the activity's id, its key when the backend knows it, the action id and what
is known of the tap (`tap`, with the adapter's sequence and the time it was stored). It is no `Event::Action`: the
component acted on it already. In JSON it is `HandledEvent { activityId, activityKey?, actionId, tap? }`.

Evidence: `exchange::req_e_010_a_handled_tap_is_reported_and_is_no_action`.

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

A notification has no place for the small region, which is iOS's, for a paired Apple Watch and CarPlay. Nothing
of the region is drawn, it does not choose the notification's style, and the record names `layout.small`. What an
Android watch and car show is not the region's: an application on a watch shows its activity as its Ongoing
Activity (REQ-A-024); a phone's ongoing notification is not bridged to a paired watch, and a Live Update only to
some, by Android's documentation; Android Auto shows the notifications of messaging applications built for it.

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

## REQ-A-024 A watch's Ongoing Activity

On a watch below Wear OS 7 whose application has `androidx.wear:wear-ongoing`, liveupdate's library puts an
Ongoing Activity into the notification of an activity: its icon on the watch face, and an entry among the recent
applications. The capabilities say whether the device does: `androidPresentation.wearOngoingActivity`.

One activity can do without it. An Android content says `wearOngoingActivity: false` in its presentation, as
liveupdate takes it. A layout says it with the Android option `wearOngoingActivity` of its start, which goes
with every snapshot of the activity: the Ongoing Activity is carried where neither the start nor the content of
the snapshot turns it off. Absent means carried, and a start that says `true` is the same activity as one that
says nothing. A template has no such option and always carries it where the device offers it: liveupdate has
the choice in a presentation, which a template's snapshot does not have. iOS ignores the member.
Anything but a boolean is `INVALID_INPUT` on every platform.

A watch shows the icon of an Ongoing Activity until its notification is cancelled. So from liveupdate 0.7.1 its
library takes the notification away before it posts a snapshot that carries none, where one that carried it is
shown: after an update that turns it off, and with the last content of an end. Before, the icon stayed. A
notification that a foreground service holds cannot be cancelled and keeps its icon until the activity ends.

No test here runs on a watch: the tests read what liveupdate is handed.

Evidence: `exchange::req_a_024_a_start_says_whether_its_activity_carries_a_watchs_ongoing_activity`,
`exchange::req_a_024_the_capabilities_say_whether_a_watch_shows_an_ongoing_activity` and
`contract::req_a_024_a_wrong_ongoing_activity_member_reaches_no_backend`.

## REQ-A-025 What an earlier Android shows

By liveupdate's account, which this crate hands on and does not test:

- Below Android 17 a metric style shows its numbers and texts as lines under the status, and its first
  stopwatch or countdown as the clock in the header. A decimal has at most two fraction digits there, as Android
  17's own style writes it (liveupdate 0.7.1; 0.7.0 wrote every digit). The capability `metricStyle` is false
  there as before, and an Android content of that style is taken.
- On One UI 9, as liveupdate saw it on a Galaxy S23: with `samsungNowBar` the Now Bar and its card show the two
  lines alone, with the application's icon and no progress bar; without it a promoted activity is in the Now Bar
  with its progress bar once it is opened. A point at the full length of a progress track is not drawn. The
  status chip shows the first metric, whatever `criticalMetric` names, and of three metrics a value longer than
  about seven characters is cut, a number too. A right-to-left layout is the system's: nothing of a content
  gives a direction.
- An action that requires authentication (`authenticationRequired`) asks for the unlock from Android 12. Below
  it Android has no such setting and would send the tap from a locked device at once: there liveupdate's library
  drops a tap while the device is locked, and the person taps again after unlocking.

Evidence: `engine_fixtures::req_a_025_the_engine_says_what_an_earlier_android_shows`.

## REQ-A-026 The room of liveupdate's store

liveupdate stores its receipts in at most 1,000,000 bytes at a start and at an update: ended activities go, the
oldest first, for the room that a new one needs, and a live one never does. A start or an update for which the
live activities alone leave no room answers `SYSTEM_LIMIT`, and the backend keeps nothing of what was asked: a
start leaves no activity, and an update leaves the content that was shown.

Evidence: `exchange::req_a_026_a_start_without_room_in_the_store_is_a_system_limit`.

## REQ-A-027 The AAR this crate is built with

The crate is built and tested with liveupdate 0.9.0, whose AAR is `liveupdate-android-0.9.0.aar`. The setup
document names it, what a watch application adds, and what a crate and an AAR of two versions do together: an
AAR from 0.7.0 posts a record with a member it does not know without it, and an earlier one refuses such a
request with `INVALID_INPUT`.

Evidence: `engine_fixtures::req_a_027_the_setup_names_the_aar_of_the_engine_in_the_lock`.

## REQ-A-028 A progress style built on its constructor shows its progress

`ProgressStyle::new`, liveupdate's, which this crate re-exports, builds a style that is styled by its progress,
as Android's own default is: what is ahead of the progress is shown fainter. Since liveupdate 0.7.1; 0.7.0 built
it with `styled_by_progress` false, and Android 16 then shows every segment in full, so that a bar without a
tracker icon looks the same at 0 and at 100. The JSON is what it was: `styledByProgress` is written when true and
reads as false when it is left out, so a request written for an earlier version reads as before. The projection
of a layout asks for it by itself (REQ-A-023).

Evidence: `api::req_a_028_a_progress_style_from_its_constructor_is_styled_by_its_progress`.

## REQ-A-029 A tap that runs the application's own component

An action can run a service, a foreground service or a receiver of the application at once, with no screen and no
process running, as liveupdate offers it since 0.8.0: its tap goes by an explicit, immutable PendingIntent to that
component of the application's own package. An Android content's action takes `target` as liveupdate takes it
(`{ kind: "service" | "foregroundService" | "broadcast", component, action? }`). A layout's start takes
`android.targets`, an object of action ids to targets, and the projection puts each target on the action of its
button or toggle. Each key is the `actionId` of a button or a toggle of the lock screen tree, and each target one
that liveupdate takes (a component of two or more Java identifiers, an intent action of printable ASCII), else the
start is `INVALID_INPUT` on every platform; a target beside a link is liveupdate's refusal too. iOS ignores the
option. `capabilities().android_presentation.action_targets` says whether the device's adapter delivers targets; a
start or an update that would post one where it does not answers `UNSUPPORTED_FEATURE` and posts nothing. A target
whose component Android does not resolve in the application is `INVALID_INPUT` from the adapter, also in an update
after an update of the application removed it (liveupdate 0.8.1): an update then names another target, or none.

Evidence: `contract::req_a_029_a_target_is_checked_on_every_platform` and
`exchange::req_a_029_a_layouts_button_and_a_contents_action_carry_their_targets`.

## REQ-A-030 A backend from the raw pointers of a service

`LiveUpdateBackend::android_from_raw(vm, context)` builds the backend on liveupdate's AAR from the process's
`JavaVM` and a context, raw JNI pointers valid on the calling thread, as liveupdate's `android_from_raw` does since
0.8.0: for a service of the application that runs its own Rust, where no activity initialized `ndk-context`. It is
unsafe for those pointers. The backend then runs as the one of `android()` does.

Evidence: CI's `cargo check --locked -p liveactivity --target aarch64-linux-android`.

## REQ-A-031 What the application's own code does

The setup documents what an application writes beside the crate for liveupdate 0.8.0: a target's component in its
manifest, not exported and in the application's process; `LiveUpdateActions.take(context, intent)` first in that
component, which answers the tap (or that a locked device below Android 12 withheld it) and records it as handled;
`LiveUpdateEvents.addNativeListener`, which is told of every stored event and takes nothing from the queue;
`LiveUpdateForeground.attachOr(service, type, fallbackId, fallback)`, which keeps a service in the foreground with
its own notification until an activity holds the placement; and `res/raw/keep.xml` for an icon that only a
request names. The AAR of the lock has each of those methods.

Evidence: `engine_fixtures::req_a_031_the_aar_has_what_the_setup_names`.

## REQ-A-032 An activity off a paired watch

A layout's start takes `android.localOnly` (`AndroidOptions::local_only`, `with_local_only`), false when absent, as
liveupdate offers it since 0.9.0: every snapshot of the activity, the projected ones and an Android content, stays on
the device it is posted on, so a paired watch does not get the phone's notification. An Android content's
`presentation.localOnly` keeps its own snapshot off; nothing turns the start's word off. Set by an update, it does not
take back a copy that a watch has. `capabilities().android_presentation.local_only` says whether the device's adapter
keeps a notification local; a start or an update that asks for it where it does not answers `UNSUPPORTED_FEATURE`
and posts nothing, since an earlier adapter would send the activity to the watch. iOS ignores the option, and a
template's start has none.

Evidence: `exchange::req_a_032_a_start_keeps_every_snapshot_of_its_activity_off_a_paired_watch`,
`exchange::req_a_032_an_activity_kept_off_the_watch_needs_the_devices_capability` and
`api::req_a_032_the_options_of_a_start_kept_off_the_watch`.

## REQ-A-033 The settings of promotion

`LiveActivity::open_promotion_settings()` asks the system for the screen where a person allows or stops the
application's Live Updates (Android 16 and later), or its notification settings before that, as liveupdate offers it
since 0.9.0: for an application whose `permissions().promotion` is `denied`. Android starts it only while the
application is in the foreground and drops it otherwise without a word: `Ok` says that it was asked, not that it
shows. A backend has a method of its own for it, whose default answers `UNSUPPORTED_FEATURE`, so that a backend of an
application's own compiles as before and a backend that wraps another forwards it; iOS answers
`UNSUPPORTED_FEATURE`, the default backend of a platform without Live Activities `UNSUPPORTED_PLATFORM`, and so do an
adapter of an earlier version and a device with no such screen.

Evidence: `exchange::req_a_033_the_settings_of_promotion_open_through_the_android_backend`,
`exchange::req_x_005_a_host_backend_composes_a_default_one_and_stays_exchangeable` and
`ios::backend::tests::req_a_033_ios_has_no_settings_of_promotion`.

## REQ-A-034 Channels in the application's words

The setup says that the AAR posts on two channels named by its string resources `liveupdate_channel_name`,
`liveupdate_channel_description`, `liveupdate_alert_channel_name` and `liveupdate_alert_channel_description`, as
liveupdate has them since 0.9.0, and that an application overrides them in its own `res/values*/strings.xml`, in each
language it has, also for a channel that exists. The AAR of the lock has those four strings.

Evidence: `engine_fixtures::req_a_034_the_setup_names_the_strings_of_the_aars_channels`.

## REQ-N-001 No framework in the crate

The crate depends on its two engines, serialization and errors only.

Evidence: `contract::req_n_001_the_crate_declares_only_its_engines_and_serde`.

## REQ-N-002 No unwrap outside tests

The crate's own code and its examples never call `unwrap` or `expect`: a value that is not there is an error to
answer, and no reason to end the host's process. Clippy denies both at the root of the library, everywhere but in
its tests, and in each example, which is an application's code. A test may use them: a test that panics has failed.

Evidence: `contract::req_n_002_clippy_denies_unwrap_and_expect_outside_tests`, and the clippy step of CI, which
takes a warning for an error and runs over every target.
