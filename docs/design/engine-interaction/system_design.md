# Engine interaction: system design

## The contract

```json
{
  "activityKey": "ride:7",
  "schemaVersion": 2,
  "fallback": "notification",
  "layout": {
    "lockScreen": {
      "type": "vstack",
      "children": [
        { "type": "text", "bind": "status", "ifStale": false },
        { "type": "text", "value": "No signal", "ifStale": true },
        { "type": "toggle", "actionId": "mute", "bind": "muted", "child": { "type": "text", "value": "Mute" } }
      ]
    }
  },
  "snapshot": { "revision": 0, "values": { "status": "On the way", "muted": false } },
  "scheduled": { "at": 1790000000000, "alert": { "title": "Your ride", "body": "It starts now", "sound": "chime.aiff" } },
  "ios": { "transient": true }
}
```

| Member | Rule | Checked |
| --- | --- | --- |
| `toggle` | activitykit's: where a button may stand, `bind` names a boolean value, `actionId` is an action id, at most 4 buttons, links and toggles in a region | everywhere |
| `ifStale` | a boolean, on any node | everywhere |
| `scheduled.at` | epoch milliseconds, a time of the contract | everywhere |
| `scheduled.alert` | an alert: a title and a body within their lengths, and a sound if it names one | everywhere |
| `alert.sound` | a file name of the contract: 1 to 64 of `A-Za-z0-9._-`, beginning with a letter or a digit | everywhere |
| `ios.transient` | a boolean | everywhere |

An update's `alert` takes `sound` the same way.

## The event

`Action { actionId, activityId, activityKey, on }`. `on` is absent for a button and for an action of an Android
content.

## What each platform does

| | iOS | Android |
| --- | --- | --- |
| `toggle` | a switch (iOS 17; its label alone before). The tap is `Action` with `on`, the state asked for. | an action titled by the toggle's label, `✓` before it while the value is on. The tap is `Action` with `on`, the other state than the value of the latest snapshot. Named `layout.toggle` when it has no title or no place among the actions. |
| `ifStale` | the system draws the node while the content is stale, or while it is not, without the application running | drawn as while fresh: a node for stale content is left out. The record names `layout.ifStale`. The sink is told `stale`. |
| `scheduled` | the activity is `pending` until its time (iOS 26); an update before that is `CONFLICT`; `UNSUPPORTED_FEATURE` before iOS 26 | `UNSUPPORTED_FEATURE`, and nothing is kept |
| `ios.transient` | a transient activity (iOS 18); `UNSUPPORTED_FEATURE` before | no part of the request: two starts that differ only in it are the same activity |
| `alert.sound` | the alert plays it | the update alerts as the device alerts, and the record names `alert.sound`; where the device does not alert again it names `alert` as before |

## The toggle's tap on Android

Android stores a tap with its action id. The backend knows the layout and the latest snapshot of the activity: an
id that is a toggle's gets `on`, the opposite of the bound value there. A tap that comes after an update has
changed the value reads the new value: the application is told which state to move to from what it showed last.
An action of an Android content has no `on`, whatever its id.
