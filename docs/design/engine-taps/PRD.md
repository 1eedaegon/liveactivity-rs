# A tap that says which it is, and what the device runs: product requirements

## Problem

activitykit 0.7.0 adds two things that a caller of this crate cannot reach yet, and changes one thing it draws.

| activitykit 0.7.0 | What it offers | Where it is here |
| --- | --- | --- |
| `Event::Action { tap: Tap { id, at, revision, late } }` | Which tap it is (the same id however often it is handed on), when it was made, the revision of the content shown, and whether it came late: a tap that the app did not take within the intent's first two posts (200 ms) is kept in the app's container, at most 32 and 12 hours, and handed to the first sink of a later kit, in this launch or the next | `ActionEvent.tap` (REQ-E-009) |
| A tap for an activity that ActivityKit no longer lists comes with the key the button was drawn with | The application learns which of its activities a late tap was for | `ActionEvent.activity_key`, as before, now present for such a tap (REQ-E-009) |
| `Authorization { push_to_start, push_channels, transient, scheduled_start, small_family }` | What the device runs of the features after the first Live Activities, without trying a request | `capabilities().ios` (REQ-I-014) |
| The expanded island keeps its leading and trailing regions 6 points from the top and the outer side | A short trailing value is no longer cut | Nothing to say in a request; the contract says it (REQ-I-015) |
| An update and a render do their fixed work once | Cheaper updates | Nothing to say |

## Goal

The crate builds on activitykit 0.7.0, and each part of the list is in the contract, the Rust API and the documents.
Android fills what it knows of a tap: the adapter's sequence as its id and the time it was stored.

## Requirements of this change

| Id | Requirement |
| --- | --- |
| REQ-E-009 | `ActionEvent.tap`: `id`, `at` (epoch milliseconds), `revision` and `late`, each left out when unknown. iOS gives what activitykit gives; Android the adapter's sequence and the time the tap was stored, no revision, never late. |
| REQ-I-014 | `capabilities().ios`: `push_to_start`, `push_channels`, `transient`, `scheduled_start`, `small_family`, as activitykit's authorization says; absent off iOS. |
| REQ-I-015 | The crate builds with the activitykit whose widget keeps the expanded island's leading and trailing content 6 points from the top and the outer side. |

## Decisions

- **One `tap` member, not four.** activitykit gives the four as one value, and an application that does not care
  reads nothing of it. It is left out of the JSON when nothing is known, so a request written before reads the
  same.
- **A late tap is a tap.** It reaches the sink as `Event::Action`, and the application decides by `tap.late` and
  `tap.id` whether to act on it: the activity may have ended since, and a tap handed on in an earlier launch has
  the same id.
- **`capabilities().ios` beside `android_presentation`.** The features of one platform are an object of their own,
  absent on the others, as Android's are.

## Not in this change

- **Refusing a request early by these members.** activitykit refuses a start that the device cannot run with
  `UnsupportedFeature` before ActivityKit is asked, and this crate passes that on as before.
