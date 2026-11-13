# The contract

This is everything `liveactivity` accepts and answers: the commands, their JSON, the limits, the
lifecycle rules, the error codes, the events and the layout DSL. It is the same on every backend.

The requests and records are serde types with camelCase JSON. `liveactivity::contract` re-exports
them without the service. An application calls the Rust methods with the typed inputs; a framework
plugin that receives JSON turns it into the typed input with `parse_request(command, json)`, which
also checks it.

There are two request versions. Version 1 is the two templates, progress and timer
(`schemaVersion: 1`). Version 2 is a layout of the application's own (`schemaVersion: 2`). Each has
its own commands, and neither sees the other's activities.

## Commands

| Command | Rust call | Request | Answer |
| --- | --- | --- | --- |
| `capabilities` | `live.capabilities()` | none | `Capabilities` |
| `permissions` | `live.permissions()` | none | `Permissions` |
| `request_permissions` | `live.request_permissions(kind)` | a `PermissionKind` | `Permissions` |
| `start` | `live.start(StartInput)` | `StartInput` | `ActivityRecord` |
| `update` | `live.update(UpdateInput)` | `UpdateInput` | `ActivityRecord` |
| `end` | `live.end(EndInput)` | `EndInput` | `ActivityRecord` |
| `list` | `live.list()` | none | `Vec<ActivityRecord>` |
| `start_layout` | `live.start_layout(LayoutStartInput)` | `LayoutStartInput` | `LayoutActivityRecord` |
| `update_layout` | `live.update_layout(LayoutUpdateInput)` | `LayoutUpdateInput` | `LayoutActivityRecord` |
| `end_layout` | `live.end_layout(LayoutEndInput)` | `LayoutEndInput` | `LayoutActivityRecord` |
| `list_layout` | `live.list_layout()` | none | `Vec<LayoutActivityRecord>` |
| `write_image` | `live.write_image(name, png)` | `ImageInput` | nothing |
| `push_tokens` | `live.push_tokens()` | none | `PushTokens` |
| `sync_events` | `live.sync_events()` | none | nothing |

Every call is a future and answers `liveactivity::Result<T>`. `LiveActivity` checks `start`,
`update`, `end`, the three layout requests and `write_image` before the backend sees them, so an
invalid request is `INVALID_INPUT` or `PAYLOAD_TOO_LARGE` on every backend.

`parse_request` reads `start`, `update`, `end`, `start_layout`, `update_layout` and `end_layout`.
`validate_request(command, json)` checks one without calling a backend; any other command name is
`INVALID_INPUT`.

## Common rules

Input is strict. An unknown member, `null`, a value of the wrong type or a value out of range is
`INVALID_INPUT`. Nothing is corrected or converted. An optional member is left out, not set to
`null`. Ranges include both ends.

| Value | Rule |
| --- | --- |
| `activityKey` | 1 to 128 bytes. Starts with an ASCII letter or digit, then ASCII letters, digits, `.`, `_`, `:` and `-` |
| `id` | The handle a start answered. 1 to 256 code points, at least one of them not whitespace |
| `schemaVersion` | `1` for `start`, `2` for `start_layout` |
| `fallback` | `none` or `notification`; required on a start |
| `revision` | An integer from 0 to 9007199254740991 |
| Times | Integer epoch milliseconds from 0 to 8640000000000000 |
| `staleAt` | An optional time, in every snapshot |

`fallback` says what a start does when the platform cannot give the activity its native
presentation. With `notification` the activity is shown in the platform's ordinary presentation;
with `none` the start fails with `UNSUPPORTED_FEATURE` and shows nothing. On Android the native
presentation is a promoted Live Update.

## Templates (version 1)

### start

```rust
let record = live
    .start(StartInput::progress("order:42", "Order 42", "Preparing", 0.1))
    .await?;
```

`StartInput::progress` and `StartInput::timer` start at revision 0 with `fallback: notification`.
`.without_fallback()` sets `none`, and `.stale(at)` sets `staleAt`.

```json
{
  "activityKey": "order:42",
  "schemaVersion": 1,
  "template": "progress",
  "fallback": "notification",
  "snapshot": { "revision": 0, "title": "Order 42", "status": "Preparing", "progress": 0.1 }
}
```

The answer is an `ActivityRecord`. This one is from iOS:

```json
{
  "id": "3F2A9C1E-6B7D-4E21-9A55-0C1D2E3F4A5B",
  "activityKey": "order:42",
  "schemaVersion": 1,
  "template": "progress",
  "snapshot": { "revision": 0, "title": "Order 42", "status": "Preparing", "progress": 0.1 },
  "backend": "activityKit",
  "state": "active",
  "visibility": "unknown",
  "degradedFeatures": [],
  "promotionRequested": false,
  "promotionState": "notApplicable"
}
```

The `id` is opaque. Never read it as a path or a URL.

| Member | Rule |
| --- | --- |
| `template` | `progress` or `timer`. It must match the snapshot: `progress` with `progress`, `timer` with `timer` |
| `snapshot.revision` | See the common rules |
| `snapshot.title` | 1 to 120 code points, not whitespace only |
| `snapshot.status` | 1 to 240 code points, not whitespace only |
| `snapshot.progress` | A finite number from 0 to 1 |
| `snapshot.timer` | `{ "startedAt": time, "endsAt": time }` with `endsAt` after `startedAt` |
| `snapshot.staleAt` | Optional time |

A snapshot has exactly one of `progress` and `timer`.

### update

```rust
live.update(UpdateInput::progress(&record.id, 1, "Order 42", "On the way", 0.6)).await?;
```

```json
{
  "id": "3F2A9C1E-6B7D-4E21-9A55-0C1D2E3F4A5B",
  "snapshot": { "revision": 1, "title": "Order 42", "status": "On the way", "progress": 0.6 }
}
```

An update is the whole content again, not a patch. It cannot change the template. The answer is the
record with the new snapshot:

```json
{
  "id": "3F2A9C1E-6B7D-4E21-9A55-0C1D2E3F4A5B",
  "activityKey": "order:42",
  "schemaVersion": 1,
  "template": "progress",
  "snapshot": { "revision": 1, "title": "Order 42", "status": "On the way", "progress": 0.6 },
  "backend": "activityKit",
  "state": "active",
  "visibility": "unknown",
  "degradedFeatures": [],
  "promotionRequested": false,
  "promotionState": "notApplicable"
}
```

### end

```rust
live.end(EndInput::new(&record.id)).await?;        // the platform's default
live.end(EndInput::immediate(&record.id)).await?;  // off the screen at once
```

```json
{ "id": "3F2A9C1E-6B7D-4E21-9A55-0C1D2E3F4A5B", "policy": { "kind": "immediate" } }
```

The policy is `{ "kind": "immediate" }`, `{ "kind": "default" }` or
`{ "kind": "after", "at": time }`.

| Policy | iOS | Android |
| --- | --- | --- |
| `immediate` | removes the activity at once | removes the notification |
| `default` | keeps it on the Lock Screen for up to four hours | removes the notification |
| `after` | keeps it until `at` or four hours after the end, whichever comes first; a past time removes it at once | keeps the last content, no longer ongoing, until `at`, where the device offers `delayedEnd`; `UNSUPPORTED_FEATURE` elsewhere, and the activity stays |

The answer is the record with `"state": "ended"`. A template end takes no final content. On Android
an end without a time, sent again, removes a last content that is still shown.

### list

`live.list()` answers the template activities the backend knows, as an array of `ActivityRecord`.
Layout activities are not in it.

### The record

| Member | Values |
| --- | --- |
| `backend` | `activityKit`, `androidNotification`, `unsupported` |
| `state` | `pending`, `active`, `stale`, `ended`, `dismissed`, `unknown`. More may come: match with a `_` arm |
| `visibility` | `unknown`. A successful call does not prove that anything was shown |
| `degradedFeatures` | What the platform left out of the request, such as `nativeLiveActivity` or `layout.<type>` |
| `promotionRequested` | Whether promotion to a Live Update was requested (Android) |
| `promotionState` | `promoted`, `notPromoted`, `unknown`, `notApplicable` |

## Layouts (version 2)

### start_layout

```rust
let input: LayoutStartInput = parse_request("start_layout", json)?;
let record = live.start_layout(input).await?;
```

```json
{
  "activityKey": "delivery:7",
  "schemaVersion": 2,
  "fallback": "notification",
  "layout": {
    "lockScreen": {
      "type": "vstack",
      "alignment": "leading",
      "spacing": 8,
      "children": [
        { "type": "text", "bind": "title", "style": "title" },
        { "type": "text", "bind": "status" },
        { "type": "progress", "bind": "done" },
        { "type": "button", "actionId": "track", "child": { "type": "text", "value": "Track" } }
      ]
    },
    "island": {
      "compactLeading": { "type": "symbol", "name": "shippingbox" },
      "compactTrailing": { "type": "text", "bind": "status" }
    },
    "keyline": { "bind": "tone" }
  },
  "styles": { "title": { "font": "headline", "color": { "bind": "tone" } } },
  "snapshot": {
    "revision": 0,
    "values": { "title": "Delivery 7", "status": "On the way", "done": 0.4, "tone": "green" }
  },
  "android": { "visibility": "private" },
  "push": "token"
}
```

The answer is a `LayoutActivityRecord`. It has no `template` and does not repeat the layout:

```json
{
  "id": "9B1D4C70-2E5A-4F0B-8C3D-7A6E5F4D3C2B",
  "activityKey": "delivery:7",
  "schemaVersion": 2,
  "snapshot": {
    "revision": 0,
    "values": { "done": 0.4, "status": "On the way", "title": "Delivery 7", "tone": "green" }
  },
  "backend": "activityKit",
  "state": "active",
  "visibility": "unknown",
  "degradedFeatures": [],
  "promotionRequested": false,
  "promotionState": "notApplicable"
}
```

| Member | Rule |
| --- | --- |
| `layout` | Required. One tree per region, two optional tints and an optional URL: see [Layout](#layout) |
| `styles` | Optional. 1 to 24 styles by token: see [Styles](#styles) |
| `snapshot.values` | 0 to 32 values by key: see [Values](#values) |
| `snapshot.staleAt` | Optional time. Each snapshot has its own; one without it clears the date |
| `snapshot.timestamp` | Optional time: when the content was made. Without it iOS takes the time the content is sent. ActivityKit (iOS 17.2) drops content older than what it shows, while the request still succeeds |
| `snapshot.relevance` | Optional finite number. Which of the application's activities the Dynamic Island shows first: the highest wins. Without it the activity keeps its score, 0 at first. Android ignores it, and an Android record does not repeat it |
| `snapshot.android` | Optional. What Android shows for this snapshot instead of the projection of the layout: see [An Android content](#an-android-content). iOS ignores it, and an iOS record does not repeat it |
| `android` | Optional. What only Android reads of a start: see [Options of a start on Android](#options-of-a-start-on-android). iOS ignores it |
| `push` | Optional. `"token"` or `{ "channel": "<base64 id>" }` (iOS 18): what the activity asks ActivityKit for. Android ignores it. In Rust it is `LayoutPush::Token` or `LayoutPush::Channel(id)` |

The layout and the styles are fixed at start. An update sends values only.

#### Options of a start on Android

```json
"android": {
  "visibility": "private",
  "publicVersion": { "title": "Delivery", "status": "On the way" },
  "placement": "foreground"
}
```

| Member | Rule |
| --- | --- |
| `visibility` | Required in `android`. `public`, `private` or `secret`: what a locked screen shows of the notification. A start without `android` is `private` |
| `publicVersion` | Optional. `{ "title", "status" }`, 1 to 120 and 1 to 240 code points: what a secure lock screen shows in place of the content. Only with `visibility` `private`; otherwise `INVALID_INPUT`, on every platform. It goes with every snapshot that has no public version of its own. On a device that does not offer `publicVersion` the snapshot is shown without it and the record names `publicVersion` |
| `placement` | Optional. `tagged`, which it is when absent, or `foreground`: the notification is the one that a foreground service of the application holds, so that the service shows no second one. One running activity has it at a time; a second is `SYSTEM_LIMIT`. On a device that does not offer `foregroundPlacement` the start is `UNSUPPORTED_FEATURE`. The placement is part of the request: another one under a running key is `CONFLICT`. The service is the application's ([Android setup](android-setup.md#a-foreground-service)) |

In Rust: `AndroidOptions::new(visibility)`, then set `public_version` or `placement`.

### update_layout

```json
{ "id": "9B1D4C70-2E5A-4F0B-8C3D-7A6E5F4D3C2B",
  "snapshot": { "revision": 1, "values": { "title": "Delivery 7", "status": "Arriving", "done": 0.9, "tone": "blue" } } }
```

An update replaces the whole map. It must have exactly the keys of the start, and each key keeps
its JSON type (string, number or boolean); otherwise it is `INVALID_INPUT`. The values are checked
again against the layout and the styles of the start, bound style members included.
`LayoutStartInput::validate_update` and `validate_end` apply that check to a start and the request
after it.

An update may bring an alert:

```json
{ "id": "9B1D4C70-2E5A-4F0B-8C3D-7A6E5F4D3C2B",
  "snapshot": { "revision": 2, "values": { "title": "Delivery 7", "status": "At the door", "done": 1, "tone": "green" } },
  "alert": { "title": "Delivery arrived", "body": "Your parcel is at the door" } }
```

| Member | Rule |
| --- | --- |
| `alert.title` | 1 to 120 code points, at least one of them not whitespace |
| `alert.body` | 1 to 240 code points, at least one of them not whitespace |

On iOS the device lights up, the Dynamic Island expands (without one, the Lock Screen presentation
shows as a banner), and an Apple Watch shows the title and the body. On Android the notification
alerts again with that revision, with its own title and text, where the device offers `alert`;
elsewhere the update is silent and the record names `alert` in `degradedFeatures` until the next
snapshot. Without `alert` an update is silent everywhere. In Rust:
`LayoutUpdateInput::new(id, snapshot)`, then set `alert`.

### An Android content

A layout says what iOS draws. Android draws a notification, and by default the backend projects
the layout onto one ([What Android shows](#what-android-shows)). A snapshot may instead say what
the notification shows, in Android's own terms:

```json
"snapshot": {
  "revision": 1,
  "values": { "title": "Delivery 7", "status": "On the way", "done": 0.6, "tone": "green" },
  "android": {
    "title": "Delivery 7",
    "status": "2 stops away",
    "presentation": {
      "style": { "kind": "progress", "value": {
        "progress": 60, "styledByProgress": true,
        "segments": [ { "length": 60, "color": 4278222848 }, { "length": 40 } ],
        "points": [ { "position": 60 } ]
      } },
      "shortCriticalText": "8 min",
      "actions": [ { "id": "call", "title": "Call the courier" } ]
    }
  }
}
```

| Member | Rule |
| --- | --- |
| `title` | The notification's title, 1 to 120 code points |
| `status` | The notification's text, 1 to 240 code points |
| `presentation` | liveupdate's `AndroidPresentation`, re-exported with its types: a `style` (`standard`, `bigText`, `progress` with up to 32 segments and points and three icons, `timer`, `metric` with up to three metrics), a `shortCriticalText` of at most 7 code points for the status chip, a `largeIcon`, up to three `actions` with an optional icon, `samsungNowBar` texts, and the members of the next table |

What liveupdate 0.2 added to a presentation, each optional and each behind a capability of its own.
A content is shown as given or not at all: one that uses a member on a device that does not offer
its capability is `UNSUPPORTED_FEATURE` on Android, at a start and in an update. The alert is the
exception, since a revision can be shown without alerting: there it is left out and the record
names `alert`.

| Member | Value | Capability |
| --- | --- | --- |
| `openUrl` | The URL a tap on the notification opens in the application. Without it a tap opens the launch activity | `links` |
| `actions[].openUrl` | The URL the action opens in the application. Such an action sends no tap | `links` |
| `chronometer` | `{ "timer": { "startedAt", "endsAt" }, "countsDown" }`: a clock in the header, beside any style but `timer`, which is that clock alone | `chronometer` |
| `alert` | `true`: this revision alerts again. An [alert of the update](#update_layout) sets it for both platforms | `alert` |
| `publicVersion` | `{ "title", "status" }`: what a secure lock screen shows of a private activity, with the bounds of the content's own | `publicVersion` |
| `smallIcon` | `{ "kind": "resource", "value": name }`: a drawable of the application | `branding` |
| `accentColor` | An unsigned ARGB number: the color of the icon and the header | `branding` |
| `subText` | The third text of the header, at most 64 code points | `branding` |

A URL here is liveupdate's: 1 to 2048 printable ASCII characters with a lowercase scheme, and none
of the schemes `intent`, `content`, `file`, `javascript`, `data` and `android-app`. The application
declares an activity that opens it ([Android setup](android-setup.md)).

- The content is checked on every platform, by liveupdate's own rules, so a wrong one is
  `INVALID_INPUT` on iOS too.
- It is read strictly, like the rest of a request, with one allowance that liveupdate's own
  encoding makes: a member at its default, `false` or an empty array, may be written or left out.
- A `publicVersion` is of a private activity. A start that gives one with `android.visibility`
  `public` or `secret` is `INVALID_INPUT` on every platform. In an update or an end only Android
  knows how the activity started, and refuses there.
- A snapshot without a content is projected from the layout, in the style the layout projects to.
- A snapshot may have another style than the one the activity shows: a content of another style,
  or a projection after a content. A device that offers `styleChange` shows it. On one that does
  not it is a valid request that the device cannot show: `UNSUPPORTED_FEATURE`, on Android only,
  and the activity keeps what it shows.
- A tap on one of `actions` reaches the sink as `Event::Action`, like a tap on a button node.
- Android refuses a content it cannot post, with `INVALID_INPUT` and nothing shown: a URL that no
  activity of the application opens, or a small icon that is no resource of it. Only the device
  knows, so iOS takes the same request. A start that is refused this way leaves no activity, and
  its key is free. An update that is refused leaves the activity as it is shown, in its record
  too, and the corrected content goes out under the same revision.
- `capabilities().androidPresentation` says which styles the device draws and which of these
  members it offers.
- An icon of the `appFile` kind is a path under the app's files. `write_image(name, png)` writes
  to `<image_folder>/<name>`, which is `liveactivity/images/<name>` unless the host chose another
  folder.

### end_layout

```json
{ "id": "9B1D4C70-2E5A-4F0B-8C3D-7A6E5F4D3C2B",
  "policy": { "kind": "default" },
  "snapshot": { "revision": 2, "values": { "title": "Delivery 7", "status": "Delivered", "done": 1, "tone": "green" } } }
```

`policy` is the same as for `end`. `snapshot` is optional: the final content, checked as an update,
with its own `relevance` and Android content when given. Without it the activity ends with its last
accepted values. An activity that is over takes no content: an end sent again answers its record,
with or without a `snapshot`, which is neither checked against the start nor applied.

### list_layout

`live.list_layout()` answers the layout activities, as an array of `LayoutActivityRecord`. A backend
that draws no layouts answers an empty array.

### A reserved value key

A layout request may not use the value key `liveActivityTemplate`. On iOS that key marks the
template activities, which run as layout activities with a fixed tree. A start, an update or an end
whose values hold it is `INVALID_INPUT` on every platform. On iOS a template and a layout activity
cannot run under the same `activityKey`: the second start answers `CONFLICT`.

## write_image

```rust
live.write_image("badge.png", &png_bytes).await?;
```

As JSON the request is `{ "name": "badge.png", "png": [137, 80, 78, 71, ...] }`: `png` is an array
of bytes. The PNG is stored under `name` and replaces an earlier one. An `image` node shows it with
`{ "file": "badge.png" }`. `check_image(name, png)` checks one without writing.

| Rule | Error |
| --- | --- |
| `name` matches `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$` | `INVALID_INPUT` |
| the bytes are a whole PNG: the signature, an image header with a width and a height of at least 1, and the end chunk | `INVALID_INPUT` |
| at most 1048576 bytes (`MAX_IMAGE_BYTES`), at most 1224 pixels wide (`MAX_IMAGE_WIDTH`) and 480 high (`MAX_IMAGE_HEIGHT`) | `PAYLOAD_TOO_LARGE` |

The name is checked first. On iOS the file goes to the App Group container that the widget
extension shares; on Android to the app's private files.

## capabilities, permissions, request_permissions

`live.capabilities()` on iOS with Live Activities:

```json
{
  "platform": "ios", "backend": "activityKit",
  "start": "supported", "update": "supported", "end": "supported",
  "progress": "supported", "timer": "supported", "actions": "supported",
  "stateObservation": "supported", "silentUpdate": "supported",
  "nativeLiveActivity": "supported", "maxPayloadBytes": 4096,
  "layout": "supported", "push": "supported"
}
```

On Android, when the backend has the app's files, the capabilities also say which notification
styles the device draws and what else it offers, for [an Android content](#an-android-content)
and for the rules that follow a device:

```json
"androidPresentation": {
  "standard": true, "bigText": true, "timer": true, "progressStyle": true, "metricStyle": false,
  "actions": true, "images": true, "events": true, "samsungNowBar": false,
  "chronometer": true, "links": true, "styleChange": true, "delayedEnd": true,
  "foregroundPlacement": true, "alert": true, "publicVersion": true, "branding": true
}
```

`progressStyle` needs Android 16 and `metricStyle` Android 17. `styleChange` lets a snapshot have
another style than the activity shows, `delayedEnd` is the `after` policy, `alert` an update that
alerts, and `foregroundPlacement` the `placement` of a start. `links` and `chronometer` also decide
what [a layout shows](#what-android-shows). A member that the Android side does not report is
false. Elsewhere `androidPresentation` is absent.

A feature is `supported`, `unsupported` or `unknown`, and unknown never counts as supported. On iOS
`actions` follows the device's support for buttons. On Android `layout` and `actions` are supported
when the backend has the app's files, `nativeLiveActivity` when the device can promote a
notification, and `push` is unsupported. Where there is no platform every feature is `unsupported`
and `backend` is `unsupported`.

`live.permissions()`:

```json
{ "notifications": "notApplicable", "liveActivities": "granted",
  "promotion": "notApplicable", "frequentPushes": "granted" }
```

A state is `granted`, `denied`, `prompt`, `settingsRequired`, `notApplicable` or `unknown`.

| Member | iOS | Android |
| --- | --- | --- |
| `notifications` | `notApplicable` | `granted` or `denied` |
| `liveActivities` | `granted`, `denied`; `notApplicable` without Live Activities | `notApplicable` |
| `promotion` | `notApplicable` | `granted` or `denied`; `notApplicable` where Live Updates are not supported |
| `frequentPushes` | `granted` or `denied`; `notApplicable` without Live Activities | `notApplicable` |

The Android backend cannot tell a refusal from a prompt that was never shown, so a disabled
permission is `denied`.

`live.request_permissions(kind)` takes `PermissionKind::Notifications`, `LiveActivities` or
`Promotion` (`notifications`, `liveActivities`, `promotion` in JSON). On iOS Live Activities are a
switch in Settings and there is no prompt: asking for `liveActivities` answers the permissions with
`settingsRequired` in place of `denied`, and another kind is `UNSUPPORTED_FEATURE`. The default
Android backend cannot ask (`UNSUPPORTED_FEATURE`); a host that has an Activity shows the dialog
(see [Android setup](android-setup.md#5-permissions)).

## Lifecycle rules

- **Key.** `activityKey` is the application's own name of the activity. While an activity of a key
  runs, a start with the same key and the same request answers that activity, with the same `id`. A
  different request under that key is `CONFLICT`. After the activity ended, the key starts a new
  activity with a new `id`.
- **Handle.** Updates and ends take the `id` of the record. An `id` of one version is `NOT_FOUND`
  for the other version's commands.
- **Revision.** A higher revision replaces the content. A lower one is `STALE_REVISION`. The same
  revision with the same content changes nothing and answers the record; with other content it is
  `CONFLICT`.
- **End.** Ending an ended activity again answers its record; a final content that comes with it is
  not applied. An ended activity takes no update: `NOT_FOUND`. An unknown `id` is `NOT_FOUND`.
- **Order of checks.** The contract is checked first. A request outside it never reaches a backend.

`testkit::conformance` checks these rules on any backend.

## Errors

Every failure is an `Error { code, message }`, in JSON `{ "code": "...", "message": "..." }`. The
code is one of the 12 in `ERROR_CODES`. The message is for a developer.

| Code | When |
| --- | --- |
| `UNSUPPORTED_PLATFORM` | The platform has no Live Activities: not iOS or Android, or an iOS version below the minimum |
| `UNSUPPORTED_FEATURE` | The backend does not provide what the request asks for: a method it does not implement, `fallback: none` where the native presentation is not available, the `after` policy or another notification style on an Android device that does not offer it, layouts or images on an Android backend without the app's files, a permission kind the platform cannot ask for, or an iOS feature that needs a later iOS (the message names the feature and the version) |
| `PERMISSION_DENIED` | The person or the system does not allow it: Live Activities are off in Settings on iOS, notifications are not allowed on Android |
| `NOT_FOREGROUND` | ActivityKit refused a start because the app was not in the foreground |
| `CONFIGURATION_REQUIRED` | The app is not set up: see [iOS setup](ios-setup.md#troubleshooting) |
| `INVALID_INPUT` | The request does not match the contract, or an update changes the template, the keys or the value types |
| `PAYLOAD_TOO_LARGE` | The layout, styles and values exceed the weight limit, the encoded payload exceeds 4096 bytes, or an image is too large |
| `NOT_FOUND` | No activity of that version has this `id`, or it has ended and the request is an update |
| `STALE_REVISION` | The revision is below the accepted one |
| `CONFLICT` | The accepted revision with other content, or a start under a running key with another request |
| `SYSTEM_LIMIT` | The system's limit of activities is reached. On iOS the message says whether it is the app's or the device's |
| `NATIVE_ERROR` | The platform or its bridge failed, or a code of the engine that the contract does not know |

## Events

A backend hands events to the sink that the builder was given. `Event` is `#[non_exhaustive]`:
match with a `_` arm.

| Variant | Payload | From |
| --- | --- | --- |
| `Event::Action` | `ActionEvent` | iOS, Android: a tap on a `button` node |
| `Event::PushToken` | `PushTokenEvent` | iOS: a push token of an activity, new or changed |
| `Event::PushToStartToken` | `PushToStartTokenEvent` | iOS: the app's push-to-start token, new or changed |
| `Event::Updated` | `UpdatedEvent` | iOS: content that a push delivered to a layout activity |
| `Event::Started` | `StartedEvent` | iOS: a layout activity began, one the application started or one a push started |
| `Event::State` | `StateEvent` | A new lifecycle state of an activity. iOS: every change, while the application runs. Android: each state once, `active`, `stale`, `ended` and `dismissed`, when the backend is asked (see below) |
| `Event::Permissions` | `Permissions` | iOS: what the person allows, after a change in Settings |
| `Event::Opened` | `OpenedEvent` | Android: the person opened the application from a notification, as the host reported it |

```json
{ "actionId": "track", "activityId": "9B1D4C70-...", "activityKey": "delivery:7" }
```

```json
{ "activityId": "9B1D4C70-...", "activityKey": "delivery:7", "token": "0a1b2c..." }
```

```json
{ "token": "0a1b2c..." }
```

```json
{ "record": { "id": "9B1D4C70-...", "activityKey": "delivery:7", "schemaVersion": 2, "snapshot": { "revision": 5, "values": {} }, "backend": "activityKit", "state": "active", "visibility": "unknown", "degradedFeatures": [], "promotionRequested": false, "promotionState": "notApplicable" } }
```

`StartedEvent` has the same shape as `UpdatedEvent`, a `record`. A `StateEvent`:

```json
{ "activityId": "9B1D4C70-...", "activityKey": "delivery:7", "state": "dismissed" }
```

`activityKey` is absent when the backend no longer knows the activity. Tokens are lowercase hex.

`live.push_tokens()` answers what the backend holds since launch, for a sink that listened late:

```json
{ "pushToStart": "0a1b2c...",
  "activities": [ { "activityId": "9B1D4C70-...", "activityKey": "delivery:7", "token": "0a1b2c..." } ] }
```

`pushToStart` is absent without a token. `activities` holds the newest token of each of the last 32
activities that got one (`TOKENS_KEPT`), in the order their first tokens came. Android holds none.

iOS delivers events as they come. On Android nothing calls the application: it asks.
`live.sync_events()` hands the sink what Android stored, the taps and the openings, oldest first,
and then each state of an activity that the sink was not told yet. On iOS it does nothing.

An activity is over when its state is `ended` or `dismissed`, on either platform. On iOS ActivityKit
reports every state as it comes: `active`, `stale`, `ended` for every end (the application's, a
push's or the system's) and `dismissed` when the activity leaves the screen. On Android each state
is reported once, with the activity's id and key, from the account that liveupdate keeps with its
receipts, so none is lost with the process and none comes twice:

| State | When it is reported on Android |
| --- | --- |
| `active` | after a start, once Android shows the notification: at the call, or at the next ask when Android shows it a moment later. Again after an update of a stale activity |
| `stale` | at the first ask after the stale time has passed |
| `ended` | at the application's end, once. An end of an activity that the person had removed makes its record `ended`, which is reported as well |
| `dismissed` | at the first ask after the person removed the notification; the event Android stores for it prompts the host to ask |

An ask is `sync_events`, and each start, update and end of the application, which report the
states they made due. Android tells nobody that a stale time has come, and the backend runs no
timer. A host with a thread asks the default backend when to ask again
(`LiveUpdateBackend::next_check`: now, soon, at a time, or not until a call or an event); a host
without one learns of a stale activity at its next ask, at the latest when the application
resumes. A last content that [stays after an end](#end) leaves without an event when its time
comes.

The system gives a URL that a tap opens to the application, as any URL it is opened with:
`onOpenURL` or the scene's URL contexts on iOS, the intent of the activity that declares the URL on
Android. On iOS that is all. On Android the host can hand the opening intent to liveupdate
([Android setup](android-setup.md#links)), and the sink then receives `Event::Opened` on the next
`sync_events`:

```json
{ "activityId": "9B1D4C70-...", "activityKey": "delivery:7", "actionId": "link:1" }
```

`actionId` is absent when the person tapped the notification itself. It is the id of an action of
an Android content, or `link:<n>` for the nth `link` of the layout's lock screen tree.

## The layout DSL

### Layout

```jsonc
"layout": {
  "lockScreen": Node,              // required
  "island": {                      // optional; at least one member
    "expanded": {                  // optional; at least one region
      "leading": Node, "trailing": Node, "center": Node, "bottom": Node
    },
    "compactLeading": Node,
    "compactTrailing": Node,
    "minimal": Node,
    "margins": { ... }             // optional; see "Options of the Dynamic Island"
  },
  "background": Color | Binding,   // optional; tint of the activity background
  "keyline": Color | Binding,      // optional; tint of the Dynamic Island keyline
  "url": Url | Binding             // optional; what a tap on the activity opens the app with
}
```

Without `url` a tap opens the app without a URL. [Links and URLs](#links-and-urls) has the rule of
a URL.

On iOS an island part that the layout leaves out is drawn with a default: `expanded` is the lock
screen tree in the bottom region; `compactLeading` and `minimal` are the first `symbol` node of the
lock screen tree, or the SF Symbol `circle.fill`; `compactTrailing` is empty.

### Values

`values` is an object with 0 to 32 entries.

- Key: `^[A-Za-z][A-Za-z0-9_]{0,31}$`.
- Value: a string of 0 to 1000 code points, a finite number from -8640000000000000 to
  8640000000000000, or a boolean. `null`, arrays and objects are not values.

A value is truthy when it is `true`, a number other than 0, or a non-empty string.

### Nodes

Every node is an object with a `type`. Any node may also carry:

- `style`, a style token;
- one of `if` and `unless`, a value key: with `if` the node and its subtree are drawn only when the
  value is truthy, with `unless` only when it is not;
- `accessibility`: what assistive technologies read of the node, see [Accessibility](#accessibility);
- `transition`: how the node comes and goes when its `if` or `unless` shows or hides it: `opacity`,
  `slide`, `moveTop`, `moveBottom`, `moveLeading`, `moveTrailing`, `pushTop`, `pushBottom`,
  `pushLeading` or `pushTrailing`. Without it the system chooses. iOS runs these and no others;
- `privacy`: `sensitive` marks what the node and its subtree show as private. iOS hides it on a
  locked device and in the always-on display when the person has asked for that.

`?` marks an optional member.

| `type` | Members |
| --- | --- |
| `vstack` | `children`: 1 to 12 nodes; `spacing`?: 0 to 64; `alignment`?: `leading` `center` `trailing` |
| `hstack` | `children`: 1 to 12 nodes; `spacing`?: 0 to 64; `alignment`?: `top` `center` `bottom` `firstTextBaseline` |
| `zstack` | `children`: 1 to 12 nodes; `alignment`?: `center` `leading` `trailing` `top` `bottom` `topLeading` `topTrailing` `bottomLeading` `bottomTrailing` |
| `spacer` | `minLength`?: 0 to 256 |
| `divider` | none |
| `text` | exactly one of `value`, a string of 0 to 200 code points, and `bind`, a value key of any type |
| `symbol` | `name`: an SF Symbol name, `^[a-z0-9]+(\.[a-z0-9]+)*$`, at most 64 characters; `rendering`?: `monochrome` `hierarchical` `palette` `multicolor`; `variableBind`?: a value key of a number from 0 to 1 |
| `image` | `source`: `{ "asset": name }`, an image in the asset catalog of the widget extension, or `{ "file": name }`, a PNG stored with `write_image`; a name is `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`; `resizable`?: boolean; `contentMode`?: `fit` `fill` |
| `path` | `d`: path data of 1 to 512 bytes; `box`: `[width, height]`, each 1 to 1024; `stroke`?: 0.5 to 16, the line width; without it the path is filled |
| `progress` | `bind`: a value key of a number, shown clamped to 0..1; `kind`?: `linear` `circular` |
| `gauge` | `bind`: a value key of a number; `min`?, `max`?: numbers, 0 and 1 when left out, `min` below `max`; `kind`?: `linear` `circular` |
| `timerText` | `start`, `end`: value keys of times in epoch milliseconds, the `end` value after the `start` value; `countsDown`?: boolean, true when left out |
| `button` | `actionId`: `^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$`; `child`: Node |
| `link` | `url`: a URL or a binding, see [Links and URLs](#links-and-urls); `child`: Node |
| `custom` | `name`: `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`, a view the app registers on iOS; `props`?: 1 to 16 entries, each a string of 0 to 200 code points, a number, a boolean or a binding |

`d` is SVG path data with the commands `M L H V C S Q T Z` in upper or lower case, no arcs, and `M`
or `m` first.

Limits of a tree:

- Depth at most 8. The root of a region has depth 1; a node in `children` or the `child` of a
  button or a link is one deeper.
- Nodes per region, containers included: `lockScreen` 40, each expanded region 12,
  `compactLeading`, `compactTrailing` and `minimal` 3 each.
- Buttons and links only in `lockScreen` and the expanded regions, never inside a button or a link,
  at most 4 per region, buttons and links together.

References are checked at start, and again with the new values on every update:

- every `bind`, `if`, `unless`, `variableBind`, `start` and `end` names a key of `snapshot.values`;
- `progress.bind`, `gauge.bind`, `variableBind`, `timerText.start` and `timerText.end` name a number;
- every `style` names a token of `styles`;
- every bound `label`, `value` and `hint` of an `accessibility` names a key, of any type;
- a bound `url`, of the layout or of a link, names a string that is a URL.

### Accessibility

```jsonc
{ "type": "progress", "bind": "done",
  "accessibility": { "label": "Delivery progress", "value": { "bind": "percent" } } }
{ "type": "symbol", "name": "sparkles", "accessibility": { "hidden": true } }
```

`accessibility` tells assistive technologies such as VoiceOver what a node is. It has at least one
of these members:

| Member | Value |
| --- | --- |
| `label` | what the node is: text of 1 to 200 code points, or a binding |
| `value` | what it shows now, such as `40 percent`: text of 1 to 200 code points, or a binding |
| `hint` | what happens when the person acts on it: text of 1 to 200 code points, or a binding |
| `hidden` | boolean; `true` hides the node and its subtree, and then stands alone |

- A bound member is read as a `text` node shows that value. A value that is missing or empty leaves
  the member out.
- On a stack or a `custom` node a `label`, `value` or `hint` makes the node one element that stands
  for what is inside; a button or a link inside stays an action of it.
- Without the member the system reads its own: the text of a `text`, the title of a button, the
  name of an SF Symbol. A `path`, an `image` and a `custom` view have none: label them or hide them.

Android reads the title and the text of the notification and has no label per node. The projection
uses the words where the notification would otherwise show nothing of a node: a node it cannot draw
becomes a text, and a button or a link without a text gets its title
([What Android shows](#what-android-shows)).

### Links and URLs

```jsonc
"layout": {
  "lockScreen": { "type": "link", "url": "myapp://delivery/7",
                  "child": { "type": "text", "value": "Open" } },
  "url": { "bind": "target" } }
```

`layout.url` is what a tap on the activity opens the app with, on the Lock Screen and in every
presentation of the Dynamic Island. A `link` node does the same for a tap on its `child`, on the
Lock Screen and in the expanded island.

A URL is a string of 1 to 1024 bytes: a scheme (`^[A-Za-z][A-Za-z0-9+.-]*:`) and at least one more
character, all of them characters of RFC 3986, with a `%` only before two hex digits. A space, a
quote, `<`, `>`, `\`, `^`, a backtick, `{`, `|`, `}`, a control character or a character outside
ASCII is `INVALID_INPUT`: percent-encode it.

- Only the form is checked. iOS gives the URL to the application that owns the activity and opens
  nothing else, so the application decides what it follows. With pushes a server writes the URL:
  treat it as any URL the application is opened with.
- A binding names a string value that is a URL, at start and on every update.
- The application receives the URL as any other; the contract reports no event for it.
- On Android, where the device offers `links`, `layout.url` is what a tap on the notification
  opens and a `link` is an action that opens its URL. liveupdate takes a URL with a lowercase
  scheme, none of `intent`, `content`, `file`, `javascript`, `data` and `android-app`, and
  printable ASCII; another one is left out and named. The application declares an activity for
  the URL's scheme ([Android setup](android-setup.md#links)). When none opens a URL, which only
  the device knows, the request does not fail: the snapshot is shown without its links, and the
  record names `layout.url` and `layout.link`.

### Options of the Dynamic Island

```jsonc
"island": {
  "expanded": { "leading": Node, "trailing": Node, "bottom": Node,
                "priority": { "leading": 1 }, "belowIfTooWide": ["trailing"] },
  "minimal": Node,
  "margins": { "expanded": 8, "compactLeading": [0, 4, 0, 4] }
}
```

| Member | Value |
| --- | --- |
| `expanded.priority` | an object with at least one of `leading`, `trailing`, `center`, `bottom`: a number from 0 to 100 for a region the layout has. The region with the higher one gets the width first; a region without one has 0 |
| `expanded.belowIfTooWide` | 1 or 2 of `leading` and `trailing`, each once and each a region the layout has: the region goes below the camera when it is too wide to stand beside it |
| `margins` | an object with at least one of `expanded`, `compactLeading`, `compactTrailing`, `minimal`: the margins of that presentation as a style's `padding`. A presentation without them keeps the system's |

The options are no region and no part: `expanded` still needs a region, and `island` a part.

### Buttons and actions

A `button` wraps one child and carries an `actionId`. A tap reaches the event sink as
`Event::Action` with that `actionId`, the activity's `id` and its key. A tap does not change the
activity: update it from the application if the button should change.

### Styles

`styles` maps a token (`^[A-Za-z][A-Za-z0-9_]{0,31}$`) to a style. A style has at least one member.

| Member | Value |
| --- | --- |
| `font` | `largeTitle` `title` `title2` `title3` `headline` `subheadline` `body` `callout` `footnote` `caption` `caption2`; not together with `size` |
| `size` | 6 to 96, in points; or a binding |
| `weight` | `ultraLight` `thin` `light` `regular` `medium` `semibold` `bold` `heavy` `black`; or a binding |
| `design` | `default` `rounded` `serif` `monospaced`; not together with `fontFamily` |
| `fontFamily` | the name of a font that the widget extension bundles, `^[A-Za-z0-9][A-Za-z0-9 ._-]{0,63}$` |
| `monospacedDigit` | boolean |
| `color` | Color, the foreground; or a binding |
| `lineLimit` | integer 1 to 10 |
| `minimumScaleFactor` | 0.1 to 1 |
| `alignment` | `leading` `center` `trailing`, for text of several lines |
| `padding` | 0 to 64, or `[top, leading, bottom, trailing]`, four numbers 0 to 64 |
| `frame` | at least one of `width`, `height`, `minWidth`, `minHeight` (0 to 1024), `maxWidth`, `maxHeight` (0 to 1024 or `"infinity"`), `alignment` (the nine `zstack` values) |
| `background` | Color; or a binding |
| `cornerRadius` | 0 to 64; or a binding |
| `opacity` | 0 to 1; or a binding |
| `tint` | Color, for buttons, progress and gauges; or a binding |
| `buttonStyle` | `automatic` `bordered` `borderedProminent` `plain` |
| `contentTransition` | `numericText` `numericTextDown` `opacity` `interpolate` `identity`: how text and symbols with the style change when an update gives them another value. `numericText` rolls the digits upward, `numericTextDown` the other way |

### Colors

A Color is one of:

- a name: `primary` `secondary` `accent` `red` `orange` `yellow` `green` `mint` `teal` `cyan` `blue`
  `indigo` `purple` `pink` `brown` `white` `black` `gray` `clear`;
- a hex string, `#RRGGBB` or `#RRGGBBAA`, used in light and dark mode alike;
- `{ "light": hex, "dark": hex }`, with both members.

### Bound values

A Binding is `{ "bind": "<value key>" }` in place of a literal. The member reads that value each
time the activity is drawn, so an update changes it. These take a binding:

| Member | The value it names |
| --- | --- |
| `color`, `background`, `tint`, `layout.background`, `layout.keyline` | a string: a color name or a hex color |
| `size` | a number from 6 to 96 |
| `opacity` | a number from 0 to 1 |
| `cornerRadius` | a number from 0 to 64 |
| `weight` | a string: one of the nine weight names |
| `layout.url`, the `url` of a `link` | a string that is a URL |
| `label`, `value`, `hint` of an `accessibility` | a value of any type, read as text |

The value must be one the member takes as a literal, at start and on every update; otherwise the
request is `INVALID_INPUT`. A bound color cannot be a light and dark pair. A style that no node
uses is checked all the same. The other style members and the other members of nodes take no
binding: to change them, end the activity and start a new one.

### Images

An `image` with an `asset` source is drawn from the asset catalog of the iOS widget extension. One
with a `file` source is drawn from the PNG that `write_image` stored under that name; a missing
file draws nothing.

### Payload weight

ActivityKit limits an activity's payload to 4 KiB, which `maxPayloadBytes` reports. Before a backend
is asked, the contract applies one estimate that does not depend on an encoding:

```text
weight(string)  = UTF-8 byte length + 2
weight(number)  = 8
weight(boolean) = 5
weight(array)   = 2 + sum over the elements of (weight(element) + 1)
weight(object)  = 2 + sum over the members of (UTF-8 byte length of key + 3 + weight(value) + 1)
```

At start, `weight(layout) + weight(styles) + weight(snapshot.values)` is at most 3584; a request
without `styles` counts 2 for them. In an update `weight(values)` alone is at most 3584, and the sum
with the layout and the styles of the start is checked again. Over the limit the request is
`PAYLOAD_TOO_LARGE`. The weight is checked last: a request that also breaks another rule is
`INVALID_INPUT`. `liveactivity::weight` computes it.

The weight is an estimate. On iOS the encoded payload is checked as well, at 4096 bytes, so text
full of quotes, slashes or control characters can fail below the limit.

## What Android shows

A notification cannot draw a layout. For a snapshot without [an Android content](#an-android-content),
the Android backend projects the `lockScreen` tree onto one of Android's notification styles and
reports what it left out. It reads only the lock screen tree, in
document order, depth first, with the current values and the `if` and `unless` conditions applied.

The style of the projection is the layout's, the same with every snapshot. It is chosen from the
tree, conditions aside: a `progress` anywhere gives the progress style; else a `timerText` the
timer style; else two or more texts outside buttons and links the big text style; else the
standard style. A node that Android cannot draw and that has a label or a value counts as a text.

| Layout | Notification |
| --- | --- |
| first shown `text` | the title, cut to 120 code points; the app's name when the layout shows no text or a blank one |
| second shown `text` | the status line, cut to 240. Without it the status is the chip text, then the title |
| first shown `progress` | the progress bar, clamped to 0..1; 0 while the node is hidden |
| first shown `timerText` | the chronometer, counting down to `end`, or up from `start` with `countsDown: false`; the tree's first timer while none is shown. Beside a progress it is the clock in the header, where the device offers `chronometer` |
| `button`s and `link`s, three in all | actions, in the order of the tree. A button's tap reaches the sink; a link's action opens its URL, where the device offers `links`, and its id is `link:<n>`, `n` counting the links of the tree from 1, hidden ones too. The title is the first shown text inside, cut to 64; without a text, the label of the first node inside that has one; then the button's or the link's own label |
| a shown `symbol`, `path`, `gauge`, `custom`, `spacer`, `divider` or asset `image` with an `accessibility` label, a value or both | a text, in its place among the texts: the label, the value, or `label, value`. A bound member reads as a text shows its value, and says nothing while the value is empty. A second progress or timer is named, whatever it says |
| a node of those kinds with `accessibility.hidden` | nothing: it is decoration, and it is not named |
| first shown `image` with a `file` source | the large icon, when the stored PNG is at most 1024 pixels a side and 1 MiB |
| `layout.url` | the URL a tap on the notification opens, where the device offers `links` |
| `android.visibility` | the lock screen visibility: `public`, `private` (also when absent) or `secret` |
| `android.publicVersion` | what a secure lock screen shows in place of the content, where the device offers `publicVersion` |

A text that is cut ends with `…`. The status chip text is the percentage, such as `65%`, when a
progress is shown and no timer. A bound text shows its value as text.

`degradedFeatures` of the record lists, in this order:

1. what the engine reports of the notification itself, such as `nativeLiveActivity` when Android
   did not promote it;
2. `layout.<type>` for every shown node that was left out, once per type: a third text
   (`layout.text`), a second progress or timer, a timer under the progress style on a device
   without `chronometer` (`layout.timerText`), a button or a link without a title, beyond the
   three actions or with an id that is taken (`layout.button`, `layout.link`), a link on a device
   without `links` or with a URL that liveupdate does not take or nothing opens (`layout.link`),
   every node inside a button or a link other than the one that titles it, an asset image, a second
   file image or a file that is missing or too large (`layout.image`), and every `symbol`,
   `spacer`, `divider`, `path`, `gauge` and `custom` node that says nothing in words. Among them,
   in the order of the nodes: `layout.transition` for a shown node with a `transition`, and
   `layout.privacy` for a shown node with `privacy` when `android.visibility` is `public`. A
   notification that a locked screen hides shows nothing private there, so the mark leaves nothing
   out;
3. `layout.island`, `layout.styles`, `layout.background` and `layout.keyline`, when the request has
   them, and `layout.url` when the notification does not open it. A notification draws no styles,
   so a bound color changes nothing on Android;
4. `alert` when an update asked for one on a device that does not offer it, and `publicVersion`
   when the start gave one on a device that shows none.

A node hidden by its condition is not listed. The list is computed from the current values, so it
can change with an update. The record carries the layout snapshot the application sent, not the
projection.

A tap on an action reaches the sink with the activity's key. A tap on an `actionId` that neither
the activity's layout nor an Android content it has shown declared is dropped. An activity keeps
the action ids of its last contents, up to 32, because a stored tap may come after a later content
replaced its action.

With an Android content nothing is projected, so the record lists no `layout.*` feature.

The template activities of version 1 need no projection: progress is a progress notification and
timer a countdown. They have no buttons.
