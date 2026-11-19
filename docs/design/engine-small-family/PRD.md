# Engine small family: product requirements

## Problem

activitykit 0.6 gives a layout a region for the small activity family, which iOS shows in the Smart Stack of a
paired Apple Watch and in CarPlay. It is a breaking release for a Rust caller: the layout type that this crate
re-exports has a new member, so the crate does not build on it as it is.

[The plan for 0.5](../engine-interaction/PRD.md) left the region out because the engine did not have it.

## Goal

The crate builds on activitykit 0.6.1. A layout says its small region through the contract as activitykit takes
it, a request that breaks its rules is refused on every platform, and Android names what it cannot show.

## Requirements of this change

Each requirement is a row of `docs/requirements.md` with the test that is its evidence.

| Id | Requirement |
| --- | --- |
| REQ-I-009 | A layout's `small` reaches ActivityKit as written. It takes at most 12 nodes and no buttons, toggles or links; a request that breaks that is `INVALID_INPUT` on every platform. A layout without it asks for none, and a template has none. |
| REQ-A-021 | On Android nothing of the region is drawn, it does not choose the notification's style, and the record names `layout.small`. |

## Decisions

- **The region is the layout's, as activitykit has it.** This crate's `Layout` is activitykit's type, so the
  member, its rules and its limits are the engine's, and the crate adds none.
- **Android names it and draws nothing.** A notification has no place for the region. (This said first that an
  Android watch and car show the notification by themselves. They do not, by Android's documentation: a phone's
  ongoing notification is not bridged to a paired watch, a Live Update only to some, and Android Auto shows the
  notifications of messaging applications built for it.) The projection names every part of a layout that a notification does not draw (`layout.island`,
  `layout.styles`), and the region is one of them.
- **Whether iOS draws the region is the widget extension's choice**, with a key of its Info.plist. activitykit
  reports `layout.small` among the degraded features when the extension does not draw it, and the record carries
  that as it carries the rest.

## Not in this change

- **A capability for the small family.** Whether the extension draws it is known per activity, from the record.
- **A watch or a car to look at.** activitykit proved what its widget draws for the family in a simulator; what a
  watch and a car show is not seen there, and not here.
