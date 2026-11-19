# Engine compatibility: system design

## A watch's Ongoing Activity

```text
start_layout { android: { wearOngoingActivity: false } }      snapshot.android.presentation.wearOngoingActivity
                     \                                                          /
                      carried = start is not false  AND  content is not false
                                              |
                       liveupdate::AndroidPresentation.wear_ongoing_activity
                                              |
               the AAR, on a watch below Wear OS 7 with androidx.wear:wear-ongoing
```

The start's option is kept with the activity, as its other options are, and read at every post. A start that
says `true` is stored as one that says nothing, so a repeat of either is the same activity. A projected
snapshot has liveupdate's default, `true`, before the option is applied; a content has what it says.

## An event of an unknown kind

liveupdate drains the adapter's queue and reads each event for what it knows. `EventKind::Unknown` and any
variant that a later liveupdate adds become `Event::Unknown` with the activity's id; the layouts' store adds
the key. `EventKind::Dismissed` stays the prompt to ask for the states, and is no event of its own.

## The store's room

Nothing here: liveupdate refuses the start or the update before it stores or posts. The backend already drops
its entry of a start that liveupdate refused, and keeps the shown content of an update that failed.
