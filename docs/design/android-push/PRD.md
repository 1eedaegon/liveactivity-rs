# Layout activities from a push on Android: product requirements

## Problem

liveupdate 0.10.0 runs a data message from the application's server through its Rust lifecycle, also in a process
that the message started (`LiveUpdatePush.handle(context, message.data)`, the crate feature `push`, the payload
`liveupdate::push::Payload` of liveupdate's own types). What it offers, and where it is here:

| liveupdate 0.10.0 | What it offers | Here |
| --- | --- | --- |
| The feature `push` and `LiveUpdatePush.handle` | A messaging service hands a data message to the lifecycle in one line; the feature exports the native entry from the host's library | Nothing: no feature turns it on |
| `liveupdate::push::Payload::{start, update, end}`, `to_data`, `parse`, `DATA_KEY`, `MAX_DATA_BYTES` | A server builds the data member of liveupdate's start, update and end | `push::Target::Fcm` answers `UNSUPPORTED_FEATURE` |
| `LiveUpdate::push(&payload)` | A payload run from Rust | Nothing |
| `EventKind::Pushed { command, activity_key, revision, code }` | What a push did, drained with the other events; the id is empty when the push named no live activity | Not mapped |
| `PresentationCapabilities::push` | An AAR that has `LiveUpdatePush` | Not in the contract |
| One lock for every client of a process | A push and the application's own client take turns | Nothing needed |

A layout activity of this crate is more than liveupdate's activity. On Android the backend keeps, beside
liveupdate's receipt, the layout request and its latest snapshot (`android::layouts::Entry`). Each snapshot is
projected into a notification by what the device offers and by its image files. A liveupdate payload would bypass
both:

- **The server would have to project.** It does not know the device's capabilities or its files, and the
  projection of a later version could differ from the device's.
- **The store would go stale.** An update by push changes the notification and liveupdate's receipt, but not the
  entry, so `list()` and the state of a toggle would answer the older content. A start by push has no entry: this
  crate leaves such an activity out of `list` (REQ-X-006), and the application cannot end it by its id.

## Goal

A server sends one `push::Message` to an Android target as it does to an iOS one. On the device, this crate's
own layout path runs the message, with the device's projection and the backend's store, also in a process that
the message started. The application learns of it from the events, as on iOS.

## Proposal

1. **One generic hook in liveupdate's push entry.** `LiveUpdatePush.handle` also takes a member for a layer
   above liveupdate, for example `liveupdate.layer`. It hands that member to a second native entry
   (`nativeLayer(context, member)`), which a crate built into the host's library exports, and stores the outcome
   as a `pushed` event, as it does its own. liveupdate knows nothing of this crate. Without such a crate the
   entry is missing, and the push is logged and stores nothing, as a library without `push` is.
2. **This crate's native entry**, under a feature of this crate (`android-push`, which turns on liveupdate's
   `push`). It builds the Android backend from the process's `JavaVM` and the context it is given
   (`LiveUpdateBackend::android_from_raw`, REQ-A-030), reads the member as a `push::Message` of the contract,
   resolves the activity's id from its key in the store, and runs `start_layout`, `update_layout` or
   `end_layout` as the application would. The projection is the device's, and the store keeps the request and its
   snapshot.
3. **The server side.** `Message::request(&Target::Fcm { token }, &application)` makes FCM HTTP v1's request:
   `POST /v1/projects/<project>/messages:send` with
   `{"message":{"token":…,"android":{"priority":"high"|"normal"},"data":{"liveupdate.layer":"<json>"}}}`.
   `Application::fcm_project(id)` names the project. The data is checked against FCM's 4096 bytes.
4. **The sender.** `Sender::builder().fcm(Fcm::new(FcmAccount::from_json(service_account)?))`: an OAuth token
   for the service account (an RS256 JWT exchanged at its `token_uri`, kept until shortly before its hour), and
   FCM's answers in `SendError` (`UNREGISTERED` as `Unregistered`, `INVALID_ARGUMENT` as `Rejected`,
   `QUOTA_EXCEEDED` as `TooManyRequests`, `UNAVAILABLE` and `INTERNAL` as `Unavailable`).
5. **The events.** `Event::Pushed(PushedEvent { command, activity_key, revision, code })` on Android, from
   liveupdate's `pushed` events, so that the application learns that a push was taken or refused. An update
   that was applied is also `Event::Updated` with the snapshot the store now holds, and a start
   `Event::Started`, as on iOS.
6. **The capability.** `capabilities().android_presentation.push`, liveupdate's, in the contract: whether the
   AAR has the entry. Whether the host's library was built with the feature is the application's to know.

## Requirements

With liveupdate 0.10.0, now:

| Id | Requirement |
| --- | --- |
| REQ-A-035 | liveupdate 0.10.0's interfaces are reachable: the feature `android-push` turns on liveupdate's `push`; `liveactivity::liveupdate::push::Payload` comes with the crate; `android_presentation.push` is in the contract and the capabilities; the contract's feature `push` stays `unsupported` on Android. |
| REQ-A-038 | What a push did reaches the sink as `Event::Pushed`. |

With liveupdate's layer hook (0.12.0, agreed with liveupdate's session: the member `liveupdate.layer`, the native
method `LiveUpdatePush.nativeLayer(context, member): String`, the helper `liveupdate::push::outcome`, the capability
`presentation.pushLayer`):

| Id | Requirement |
| --- | --- |
| REQ-A-036 | A message to an FCM target is FCM HTTP v1's request with the data member the layer's entry reads, checked as the backend checks the call and against 4096 bytes. |
| REQ-A-037 | On the device, a message runs this crate's layout start, update or end by the activity's key, with the device's projection, and the store holds what it did. A key without a live activity is `NOT_FOUND`, a stale revision `STALE_REVISION`, a repeat changes nothing. |
| REQ-A-039 | A layout activity's push that was applied is also `Event::Updated` or `Event::Started` with the content the store holds, and the contract's feature `push` is `supported` where `pushLayer` is. |
| REQ-P-009 | The sender signs in to FCM with a service account, keeps the token until shortly before it runs out, and answers FCM's errors in `SendError`. |

## Decisions

- **The device projects, not the server.** Only the device knows what it offers and which files it has, and the
  projection of this crate's version on the device is the one that the rest of the activity was drawn with.
- **A hook in liveupdate, not an AAR of this crate's.** This crate ships no Android code. One line in the
  messaging service stays one line: liveupdate's entry hands the layer's member over.
- **The activity's key.** The server knows it; the store maps it to the id.

## Agreed with liveupdate's session

- The member `liveupdate.layer`; a map with it and `liveupdate` goes to the layer alone.
- `LiveUpdatePush.nativeLayer(context, member): String`, whose symbol
  `Java_io_github_eedaegon_liveupdate_LiveUpdatePush_nativeLayer` this crate's feature exports.
- The outcome in liveupdate's shape, by `liveupdate::push::outcome`, stored by liveupdate's event store as a
  `pushed` event with liveupdate's id, which is this crate's id of a layout activity on Android.
- `presentation.pushLayer`, checked before a server sends the layer's member.
- `watch_mirror` (liveupdate 0.11.0) is taken in its own change.

## Not in this change

- Template activities by push: the contract's templates have no `push::Message` yet.
- A Firebase dependency, a messaging service of this crate's, or keys in the repository.
