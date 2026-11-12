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
| `after` | keeps it until `at` or four hours after the end, whichever comes first; a past time removes it at once | `UNSUPPORTED_FEATURE` |

The answer is the record with `"state": "ended"`. A template end takes no final content.

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
| `layout` | Required. One tree per region and two optional tints: see [Layout](#layout) |
| `styles` | Optional. 1 to 24 styles by token: see [Styles](#styles) |
| `snapshot.values` | 0 to 32 values by key: see [Values](#values) |
| `snapshot.staleAt` | Optional time. Each snapshot has its own; one without it clears the date |
| `snapshot.timestamp` | Optional time: when the content was made. Without it iOS takes the time the content is sent. ActivityKit (iOS 17.2) drops content older than what it shows, while the request still succeeds |
| `android` | Optional. `{ "visibility": "public" \| "private" \| "secret" }`, the lock screen visibility of the notification; `private` when absent. iOS ignores it |
| `push` | Optional. `"token"` or `{ "channel": "<base64 id>" }` (iOS 18): what the activity asks ActivityKit for. Android ignores it. In Rust it is `LayoutPush::Token` or `LayoutPush::Channel(id)` |

The layout and the styles are fixed at start. An update sends values only.

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

### end_layout

```json
{ "id": "9B1D4C70-2E5A-4F0B-8C3D-7A6E5F4D3C2B",
  "policy": { "kind": "default" },
  "snapshot": { "revision": 2, "values": { "title": "Delivery 7", "status": "Delivered", "done": 1, "tone": "green" } } }
```

`policy` is the same as for `end`. `snapshot` is optional: the final content, checked as an update.
Without it the activity ends with its last accepted values.

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
- **End.** Ending an ended activity again answers its record. An ended activity takes no update:
  `NOT_FOUND`. An unknown `id` is `NOT_FOUND`.
- **Order of checks.** The contract is checked first. A request outside it never reaches a backend.

`testkit::conformance` checks these rules on any backend.

## Errors

Every failure is an `Error { code, message }`, in JSON `{ "code": "...", "message": "..." }`. The
code is one of the 12 in `ERROR_CODES`. The message is for a developer.

| Code | When |
| --- | --- |
| `UNSUPPORTED_PLATFORM` | The platform has no Live Activities: not iOS or Android, or an iOS version below the minimum |
| `UNSUPPORTED_FEATURE` | The backend does not provide what the request asks for: a method it does not implement, `fallback: none` where the native presentation is not available, the `after` policy on Android, layouts or images on an Android backend without the app's files, a permission kind the platform cannot ask for, or an iOS feature that needs a later iOS (the message names the feature and the version) |
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

`activityKey` is absent when the backend no longer knows the activity. Tokens are lowercase hex.

`live.push_tokens()` answers what the backend holds since launch, for a sink that listened late:

```json
{ "pushToStart": "0a1b2c...",
  "activities": [ { "activityId": "9B1D4C70-...", "activityKey": "delivery:7", "token": "0a1b2c..." } ] }
```

`pushToStart` is absent without a token. `activities` holds the newest token of each of the last 32
activities that got one (`TOKENS_KEPT`), in the order their first tokens came. Android holds none.

iOS delivers events as they come. Android stores taps; `live.sync_events()` hands the stored ones to
the sink, oldest first. On iOS it does nothing.

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
    "minimal": Node
  },
  "background": Color | Binding,   // optional; tint of the activity background
  "keyline": Color | Binding       // optional; tint of the Dynamic Island keyline
}
```

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

Every node is an object with a `type`. Any node may also carry `style`, a style token, and one of
`if` and `unless`, a value key: with `if` the node and its subtree are drawn only when the value is
truthy, with `unless` only when it is not. `?` marks an optional member.

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
| `custom` | `name`: `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`, a view the app registers on iOS; `props`?: 1 to 16 entries, each a string of 0 to 200 code points, a number, a boolean or a binding |

`d` is SVG path data with the commands `M L H V C S Q T Z` in upper or lower case, no arcs, and `M`
or `m` first.

Limits of a tree:

- Depth at most 8. The root of a region has depth 1; a node in `children` or the `child` of a
  button is one deeper.
- Nodes per region, containers included: `lockScreen` 40, each expanded region 12,
  `compactLeading`, `compactTrailing` and `minimal` 3 each.
- Buttons only in `lockScreen` and the expanded regions, never inside another button, at most 4 per
  region.

References are checked at start, and again with the new values on every update:

- every `bind`, `if`, `unless`, `variableBind`, `start` and `end` names a key of `snapshot.values`;
- `progress.bind`, `gauge.bind`, `variableBind`, `timerText.start` and `timerText.end` name a number;
- every `style` names a token of `styles`.

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

The value must be one the member takes as a literal, at start and on every update; otherwise the
request is `INVALID_INPUT`. A bound color cannot be a light and dark pair. A style that no node
uses is checked all the same. The other style members and the members of nodes take no binding: to
change them, end the activity and start a new one.

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

A notification cannot draw a layout. The Android backend projects the `lockScreen` tree onto one of
Android's notification styles and reports what it left out. It reads only the lock screen tree, in
document order, depth first, with the current values and the `if` and `unless` conditions applied.

The style of the notification is fixed when the activity starts. It is chosen from the tree,
conditions aside: a `progress` anywhere gives the progress style; else a `timerText` the timer
style; else two or more texts outside buttons the big text style; else the standard style.

| Layout | Notification |
| --- | --- |
| first shown `text` | the title, cut to 120 code points; the app's name when the layout shows no text or a blank one |
| second shown `text` | the status line, cut to 240. Without it the status is the chip text, then the title |
| first shown `progress` | the progress bar, clamped to 0..1; 0 while the node is hidden |
| first shown `timerText` | the chronometer, counting down to `end`, or up from `start` with `countsDown: false`; the tree's first timer while none is shown |
| up to three `button`s | actions, titled with the first shown text inside the button, cut to 64 |
| first shown `image` with a `file` source | the large icon, when the stored PNG is at most 1024 pixels a side and 1 MiB |
| `android.visibility` | the lock screen visibility: `public`, `private` (also when absent) or `secret` |

A text that is cut ends with `…`. The status chip text is the percentage, such as `65%`, when a
progress is shown and no timer. A bound text shows its value as text.

`degradedFeatures` of the record lists, in this order:

1. what the engine reports of the notification itself, such as `nativeLiveActivity` when Android
   did not promote it;
2. `layout.<type>` for every shown node that was left out, once per type: a third text
   (`layout.text`), a second progress or timer, a timer under the progress style
   (`layout.timerText`), a button without a shown text, a fourth button or a repeated `actionId`
   (`layout.button`), every node inside a button other than its first text, an asset image, a second
   file image or a file that is missing or too large (`layout.image`), and every `symbol`, `spacer`,
   `divider`, `path`, `gauge` and `custom` node;
3. `layout.island`, `layout.styles`, `layout.background` and `layout.keyline`, when the request has
   them. A notification draws no styles, so a bound color changes nothing on Android.

A node hidden by its condition is not listed. The list is computed from the current values, so it
can change with an update. The record carries the layout snapshot the application sent, not the
projection.

A tap on an action reaches the sink with the activity's key. A tap on an `actionId` that the
activity's layout never declared is dropped.

The template activities of version 1 need no projection: progress is a progress notification and
timer a countdown. They have no buttons.
