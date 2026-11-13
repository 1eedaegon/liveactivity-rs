# A layout on Android: product requirements

## Problem

A layout is written once. iOS draws all of it; Android shows what a notification can show and names the rest.
Since activitykit 0.4 and liveupdate 0.2 a layout can say more than the projection uses, and liveupdate can show
more than the projection asks for:

- A layout names the URL a tap opens and has `link` nodes. The notification opens the application's launch
  screen, and the record names `layout.url` and `layout.link`.
- A layout with a progress and a timer loses its timer, though liveupdate has a clock beside any style.
- A node Android cannot draw is left out, also when the layout says in words what it is. A button whose label is
  a symbol becomes no action at all.
- A private notification shows nothing on a secure lock screen. liveupdate has a public version for it, which
  only an explicit Android content can give.
- liveupdate lets a foreground service of the application hold the notification. The contract has no member
  that asks for it, so the capability is reported as false.
- liveupdate reports that the person opened the application from a notification. The backend drops the event.

## Goal

One layout request gives the best notification a device offers, and nothing a device lacks makes it fail.

## Requirements of this change

| Id | Requirement |
| --- | --- |
| REQ-A-008 | Where the device offers `links`, the projection gives the notification the layout's `url` and makes each shown `link` an action that opens its URL, titled by the link's label and named `link:<n>` after its place among the links. A URL that liveupdate does not take, and a link beyond the three actions, is named as before. When the device refuses the links at the post, because nothing in the application opens a URL, the snapshot is shown without them and the record names `layout.url` and `layout.link`. |
| REQ-E-004 | The sink receives `Opened`, with the activity's id and key and the id of an opening action, when the host reported that the person opened the application from a notification (Android). |
| REQ-A-009 | Where the device offers `chronometer`, a layout with a progress and a timer shows the timer as the clock in the header, and `layout.timerText` is not named. |
| REQ-A-010 | A shown node that Android cannot draw and that has an accessibility label, a value or both is projected as that text. A button or a link without a text is titled by a label inside it or its own. A node hidden from assistive technologies is decoration: it is left out without being named. |
| REQ-A-011 | A start may give a public version among its Android options, for a private activity only. Where the device offers `publicVersion`, every snapshot that has none of its own shows it on a secure lock screen; elsewhere the record names `publicVersion`. |
| REQ-A-012 | A start may ask for the foreground placement among its Android options. The capabilities report whether the device offers it; where it does not, the start is `UNSUPPORTED_FEATURE`, and a second activity with the placement is `SYSTEM_LIMIT`. |

## Not in this change

- A foreground service. The service, its type and its permission are the application's; liveupdate's Android
  side attaches it to the notification.
- An event for a URL that iOS opened. The application receives the URL itself.
- `Started` and `active` on Android. The application's own start reports nothing; `State` tells an end.
- What waits for a later activitykit: a toggle, transient and scheduled starts, alert sounds, the small region.
