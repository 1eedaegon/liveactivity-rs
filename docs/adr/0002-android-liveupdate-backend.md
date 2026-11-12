# ADR-0002: Android backend over the liveupdate engine

Status: accepted

Date: 2026-11-12

## Context

Phase 1 moves the Android path of tauri-plugin-live-activity behind
LiveActivityBackend. The plugin's Android v1 commands already run on
`liveupdate::LiveUpdate<JsonDriver<TauriBridge>>`, where TauriBridge is a
string transport over `PluginHandle::run_mobile_plugin`. The core of the spike
had five error codes and four states, fewer than either engine reports.

## Decision

1. `liveactivity::android` provides `LiveUpdateBackend<D: liveupdate::Driver>`
   behind the default feature `liveupdate`. It depends on no framework.
   A framework supplies a `Driver`, or a `Bridge` through `over_bridge`;
   without one, `android` uses the JNI transport of liveupdate's own AAR.
2. Neutral content is liveupdate's own JSON contract with the members the
   neutral message already carries removed. The crate declares no model of its
   own and parses with liveupdate's types.
3. ErrorCode gains unsupportedPlatform, notForeground, configurationRequired,
   payloadTooLarge, notFound, staleRevision, conflict and systemLimit.
   ActivityState gains pending and unknown. Both enums are non-exhaustive, so
   this is additive within contract version 1.
4. Android-only detail (promotion capabilities) stays on the engine, reached
   with `LiveUpdateBackend::engine` from an `Arc` the adapter keeps.
5. Android queues events instead of calling back. `pump_events` drains them
   into the EventSink; the adapter decides when to call it, through the
   backend or through `LiveActivity::sync_events`.

## Mapping

| Neutral | liveupdate |
| --- | --- |
| StartRequest.content | StartRequest without activityKey, schemaVersion |
| UpdateRequest.content, revision | Snapshot; a repeated revision must be equal |
| EndRequest.dismissal | EndPolicy; `after.at` is milliseconds in both |
| EndRequest.content | none: `unsupported` when present |
| EndRequest.revision | none: not compared |
| ActivityRecord.content | ActivityRecord without id, activityKey, state |
| action event | EventKind::Action |
| stateChanged event | EventKind::Dismissed plus the reconciled record |

## Consequences

- The Tauri plugin keeps only TauriBridge and its JavaScript model mapping.
- Calls block the polling thread, as the plugin's Android commands do today.
- The engine's event `sequence` and `occurredAt` do not cross the core, and a
  drained event that the sink rejects is not retried. A dismissal whose record
  cannot be read is reported in the pump's result and recovered with list.
- The end revision is carried by the neutral contract but has no Android
  meaning. Whether the contract should make it optional is open for phase 3.
