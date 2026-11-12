# Engine expressiveness: system design

## Requests

```jsonc
// start_layout
{ "activityKey": "ride:7", "schemaVersion": 2, "fallback": "notification",
  "layout": { /* activitykit's regions, nodes */ }, "styles": { /* activitykit's styles */ },
  "android": { "visibility": "private" },            // options of the start
  "push": "token",                                   // iOS
  "snapshot": {
    "revision": 0, "values": { "eta": "4 min", "done": 0.4 },
    "staleAt": 1790000000000, "timestamp": 1789999990000,
    "relevance": 80,                                  // iOS
    "android": {                                      // Android: shown instead of the projection
      "title": "Driver on the way", "status": "4 min",
      "presentation": { /* liveupdate's AndroidPresentation */ } } } }

// update_layout
{ "id": "...", "snapshot": { /* as above */ },
  "alert": { "title": "Driver arrived", "body": "Meet at the entrance" } }
```

## Reading a request

1. Refuse JSON nested deeper than 32.
2. Read it in this crate's type of the command; refuse unless it encodes back to the same JSON.
3. Convert the part activitykit knows and let activitykit check it (layout, styles, values, weight).
4. Check the rest here: relevance, alert, Android content (by liveupdate's own check), the template marker.

The presentation of an Android content is liveupdate's, which leaves a member at its default out when it
encodes one. Step 2 therefore allows, inside the presentation only, a written `false` or empty array; a `null`
and an array in place of an object are refused there as everywhere.

## Android: content or projection

For each snapshot: with an `android` content, send it to liveupdate as given, with the snapshot's revision and
stale date; without one, project the layout as before. liveupdate keeps the style of the first snapshot. A
snapshot of another style is a valid request that Android cannot show yet: the backend answers
`UNSUPPORTED_FEATURE` before liveupdate is asked, and iOS, which does not read the content, takes it. The
actions an activity declares are those of its layout's buttons and of the contents it has shown, the last 32.

## Events

| Event | iOS source | Android source |
| --- | --- | --- |
| `Started` | activitykit `Started`, for a layout activity | none |
| `State` | activitykit `State` | liveupdate `Dismissed` |
| `Permissions` | activitykit `Authorization` | none |

## Records and capabilities

A record's snapshot is what the platform holds: iOS reports the relevance and no Android content, Android
reports the values and the Android content and no relevance. `Capabilities.androidPresentation` is liveupdate's style support, present
when the Android backend has the app's files.
