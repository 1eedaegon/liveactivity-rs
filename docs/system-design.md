# System design

## Construction

1. The framework plugin creates its EventSink implementation.
2. The plugin calls LiveActivity::connect with the sink and a factory closure.
3. The core wraps the sink in BackendContext.
4. The factory builds a platform backend using its framework or native handle.
5. The core stores Arc dyn LiveActivityBackend.

Construction is the only backend-selection point for a LiveActivity instance.

## Command flow

1. The framework adapter parses and validates its boundary payload.
2. It creates a typed request from liveactivity.
3. LiveActivity forwards the request to the backend.
4. The backend calls activitykit, liveupdate or another provider.
5. The typed result or Error crosses the same path back.

The spike tests exactly-once forwarding. Production retry policy belongs to a
backend and must preserve revision semantics.

## Event flow

1. A native platform reports an action or state change.
2. The backend converts it into LiveActivityEvent.
3. BackendContext sends the owned event to EventSink.
4. The framework adapter emits its native framework event or channel message.

EventSink is synchronous and non-blocking by contract. A framework requiring
asynchronous delivery must queue the event in its implementation.

## Lifecycle

The core has no Activity, AppHandle, WebView or UIApplication concept.
Framework adapters construct and drop the service with their lifecycle and may
reconstruct backends after an Activity recreation.

## Backend availability

The `liveactivity` crate holds both default backends behind additive default
features (ADR-0003):

- `activitykit`: `ios::ActivityKitBackend`
- `liveupdate`: `android::LiveUpdateBackend`

`connect_native` selects the one of the target. A framework's own transport,
such as a plugin's bridge to its Kotlin side, stays in that framework's
plugin and is handed to the backend at construction.

## Compatibility

CONTRACT_VERSION is carried in every top-level serialized message. It is 1
until the first release. Unsupported versions fail before native invocation.
