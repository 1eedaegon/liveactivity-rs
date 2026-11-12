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

## REQ-N-001 No framework in the crate

The crate depends on its two engines, serialization and errors only.

Evidence: `contract::req_n_001_the_crate_declares_only_its_engines_and_serde`.
