# Engine small family: system design

## The contract

```json
{
  "activityKey": "ride:7",
  "schemaVersion": 2,
  "fallback": "notification",
  "layout": {
    "lockScreen": { "type": "text", "bind": "status" },
    "small": {
      "type": "hstack",
      "children": [
        { "type": "symbol", "name": "car.fill" },
        { "type": "text", "bind": "status" }
      ]
    }
  },
  "snapshot": { "revision": 0, "values": { "status": "On the way" } }
}
```

## What each platform does

| | iOS | Android |
| --- | --- | --- |
| A layout with `small` | The region reaches ActivityKit. The widget extension draws it for the small family when its Info.plist sets `ActivityKitSmallFamily`; otherwise iOS shows the compact parts and the record names `layout.small`. | The notification is the Lock Screen tree. The record names `layout.small`. |
| A layout without it | iOS shows the compact parts, or the extension draws them in a row when it draws the family. | Nothing changes. |
| A `small` that breaks the rules | `INVALID_INPUT`, before any backend. | The same. |
