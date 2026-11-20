# Live Activities from iOS 16.1, activities of a later version, layouts on constructors: product requirements

## Problem

activitykit 0.8.0 and 0.8.1 change what a caller of this crate builds, where it runs, and what an earlier widget
draws.

| activitykit 0.8 | What it offers | Where it is here |
| --- | --- | --- |
| Every public enum, every variant with named members and every open struct is `#[non_exhaustive]` (0.8.0); the contract's requests and records have constructors and builders again (0.8.1) | A variant or a member that activitykit adds later breaks no code. A layout is built with `Layout::new(lock_screen)` and `.island`, `.background`, `.keyline`, `.url`, `.small`; `Island::new`, `IslandExpanded::new`, `Style::new`, `Color::adaptive(light, dark)` and the node functions as before | `Layout`, re-exported from activitykit, is built on its constructor; a `match` on the re-exported vocabulary has a `_` arm (REQ-C-012) |
| Live Activities from iOS 16.1, ActivityKit's first release, where it answered `Unsupported` below 16.2 | An iPhone with iOS 16.1 runs activities. On it ActivityKit takes the content state alone: no stale date, no relevance score, no frequent-pushes setting. The widget extension can be built for 16.1 | `UNSUPPORTED_PLATFORM` below iOS 16.1; the documents say what 16.1 lacks (REQ-I-016) |
| A widget draws an activity of a later crate or server: members it does not know are passed over, and a node of a type it does not know is drawn as nothing. The app's requests stay strict | A push of a later server shows what this version knows instead of an empty activity | Requests stay `INVALID_INPUT` here; the contract says what the widget draws (REQ-I-017) |
| A receipt that cannot be read costs that receipt, not the list; a load keeps the newest 256 | `list()` no longer answers `NATIVE_ERROR` for one bad receipt | The setup says it (REQ-I-017) |

## Goal

The crate builds on activitykit 0.8.1, and each part of the list is in the contract, the Rust API and the documents.
The README and the setup show a layout built on its constructor, and say iOS 16.1.

## Requirements of this change

| Id | Requirement |
| --- | --- |
| REQ-C-012 | activitykit's vocabulary, re-exported, follows the crate's rule: `Layout::new` and its builders build what the literal was, and its enums are matched with a `_` arm. |
| REQ-I-016 | Live Activities from iOS 16.1 on an iPhone, iPadOS 17 on an iPad; below, `UNSUPPORTED_PLATFORM`. On iOS 16.1 a snapshot's `staleAt` and `relevance` do not reach the system, a record has no relevance, and `frequentPushes` is `denied`. |
| REQ-I-017 | A request with a member or a node type that this version does not know is `INVALID_INPUT`, while activitykit's widget draws such an activity, from a push of a later server, without them. |

## Decisions

- **Breaking once, with activitykit.** A caller that built `Layout { .. }` as a literal or matched activitykit's
  enums without a last arm does not compile against this version. The change is a minor version of this crate,
  as each before it that broke a caller's code, and the changelog says what to write instead.
- **`frequentPushes` on iOS 16.1 is `denied`.** activitykit reports the setting as off where the system has none,
  and this crate passes on what it reports. `notApplicable` stays what a platform without Live Activities answers.
- **A record keeps the `staleAt` it was given on iOS 16.1.** It is what the application asked for; the documents say
  that the system does not draw the stale nodes there and that no `stale` state comes.

## Not in this change

- **Refusing `staleAt` or `relevance` on iOS 16.1.** activitykit takes them and the system ignores them; refusing
  them would make the same request fail on one iOS only.
- **Reading requests leniently.** A request that this version cannot read in full is refused, as before.
