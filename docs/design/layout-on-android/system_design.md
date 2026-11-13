# A layout on Android: system design

## Request

```jsonc
{ "activityKey": "ride:7", "schemaVersion": 2, "fallback": "notification",
  "layout": { "url": "myapp://rides/7", "lockScreen": { "type": "vstack", "children": [
      { "type": "text", "bind": "title" },
      { "type": "progress", "bind": "done" },
      { "type": "timerText", "start": "from", "end": "until" },
      { "type": "symbol", "name": "car.fill", "accessibility": { "label": "Driver", "value": { "bind": "driver" } } },
      { "type": "link", "url": "myapp://rides/7/map", "child": { "type": "text", "value": "Map" } },
      { "type": "button", "actionId": "call",
        "child": { "type": "symbol", "name": "phone.fill", "accessibility": { "label": "Call" } } } ] } },
  "android": { "visibility": "private",
               "publicVersion": { "title": "Ride", "status": "On the way" },   // new
               "placement": "foreground" },                                      // new; "tagged" when absent
  "snapshot": { "revision": 0, "values": { "title": "Ride 7", "done": 0.4, "from": 1790000000000,
                                           "until": 1790000240000, "driver": "Mina" } } }
```

## Projection

| In the layout | In the notification | Needs |
| --- | --- | --- |
| `layout.url`, a URL or bound to one | the URL a tap opens | `links`; a URL liveupdate takes |
| a shown `link` | an action that opens the URL: its title is the first shown text inside, else a label; its id is `link:<n>`, `n` counting the links of the tree from 1 | `links`; a URL liveupdate takes; room among the three actions |
| a timer in a layout whose style is the progress | the clock in the header | `chronometer` |
| a shown `symbol`, `image` that is not the large icon, `path`, `gauge` or `custom` with a label or a value | a text: the label, the value, or `label, value` | — |
| a `button` or a `link` without a shown text | titled by the first label inside it, else its own | — |
| a node with `accessibility.hidden` | nothing, and nothing is named | — |

A URL liveupdate takes has a lowercase scheme, none of `intent`, `content`, `file`, `javascript`, `data` and
`android-app`, and printable ASCII only. What the table's "needs" lack is named as before: `layout.url`,
`layout.link`, `layout.timerText`, `layout.<type>`.

## A URL that nothing opens

Only the device knows whether an activity of the application opens a URL, and liveupdate refuses a post whose
URL none opens. A layout that works on iOS must not fail for it:

1. Send the projection with its links.
2. On `INVALID_INPUT`, when the projection had links, send it again without them.
3. If that is taken, the record names `layout.url` and `layout.link` for what was left out. If it is refused
   too, the first answer stands.

An explicit Android content is never changed: it is shown as given or refused.

What a projection uses follows what the device offers, which may differ from one call to the next. The same
start again is therefore answered from what is kept: when an activity of the key runs with the same request and
Android shows it, that activity is the answer, and liveupdate is not asked to compare two projections.

## Options of the start

| Option | Rule | Where the device lacks it |
| --- | --- | --- |
| `android.publicVersion`: `{ title, status }` | 1 to 120 and 1 to 240 code points; `visibility` is `private` or absent, else `INVALID_INPUT` on every platform | shown without it; the record names `publicVersion` |
| `android.placement`: `"tagged"` or `"foreground"` | part of the request: another placement under a running key is `CONFLICT` | `UNSUPPORTED_FEATURE`, from liveupdate |

The public version is added to every snapshot that is sent without one, a projection or a content. A content's
own public version stays.

## Events

| liveupdate's event | The sink receives |
| --- | --- |
| `opened`, with or without an action id | `Opened { activityId, activityKey, actionId? }` |

The host hands the opening intent to liveupdate's `LiveUpdateEvents.opened`; without that nothing is stored.

## Capabilities

`androidPresentation.foregroundPlacement` is the device's, no longer false.
