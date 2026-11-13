# Engine extensions: technical notes

## Changes by module

- `Cargo.toml`: activitykit 0.4, liveupdate 0.2.1.
- `layout`: re-exports the new types of both engines (`Accessibility`, `Transition`, `Privacy`,
  `ContentTransition`, `ExpandedRegion`, `RegionPriorities`, `IslandMargins`, `PublicVersion`); the rule of a
  public version at start.
- `ios`: the layouts the backend builds itself name the new members as absent.
- `android::layout`: the projection names links, the layout URL, transitions and the privacy mark; it builds its
  presentation and its actions with liveupdate's constructors.
- `android::layouts`: a layout projects to its own style; a style change and an alert ask the capabilities; the
  delayed end is no longer refused here; an event kind without a contract event is dropped.
- `android::convert`: capabilities without the foreground placement; a tagged placement for every start.

## Compatibility

- The re-exported types gained public members and a variant, so a struct literal and a match over `Element`
  written against 0.2 need the new ones. The crate is 0.x: a minor release, `feat:` without `!`.
- JSON written for 0.2 reads unchanged. Every new member is optional.
- liveupdate 0.2 renamed its Android package to `io.github.gon.liveupdate`. A host that names the AAR's classes
  in Kotlin or in a manifest renames them when it takes the new AAR; the JNI transport of this crate follows by
  itself.
- With the package, liveupdate renamed where it keeps its receipts and stored events, and does not read those of
  0.1. Android removes an application's notifications when the application is updated, so an activity that ran
  across the update is gone from the screen under any version. After the update to this release it is also gone
  from the lists, where it would have read `dismissed`: its id is `NOT_FOUND`, its key starts a new activity, and
  a tap that 0.1 stored is not delivered. The Android layout store of 0.2 loads unchanged, and forgets the
  activities liveupdate no longer lists. Seen on a phone with Android 17, updated in place with two running
  activities.
- An activity that mixed a content with projected snapshots was drawn in the style of its first snapshot. It is
  now drawn in each snapshot's own style where the device changes styles, and refused elsewhere.

## Risks

- **A capability call in an update.** Only an update that changes the style or alerts asks for the capabilities,
  one more call of the driver.
- **An alert sent twice.** liveupdate may post a revision again after a failed post, which alerts again. This is
  the engine's documented behavior.
- **A URL nothing opens.** liveupdate refuses the post of a content whose URL no activity of the application
  handles (`INVALID_INPUT`). Only a device knows, so the same request is taken on iOS. liveupdate writes its
  receipt before it posts, for a post that failed to be tried again. In 0.2.0 it kept that receipt also when
  the Android side refused the content, so a corrected start of the key was a conflict and a corrected update
  needed a higher revision. 0.2.1 returns its receipts to what they were before a post that is refused as
  invalid, and this crate requires it: nothing of the kind is done here.
- **Published engines.** The change was developed against the engines' release commits, before liveupdate 0.2.0
  was on crates.io. The published sources are those commits', and the tests ran again on them, and then on
  liveupdate 0.2.1, whose Android side is that of 0.2.0.

## Test plan

- Unit: the projection's new names; the store with a style change with and without the capability, with an alert
  with and without it, and with a delayed end; the public version rule; the new members through the strict reader.
- Integration (`tests/exchange.rs`): each Android requirement through the service on the default backend, over a
  driver that offers the extensions and one that does not.
- Fixtures: activitykit 0.4's request fixtures against this crate's reader.
- Devices: the conformance suite, a style change, an alerting update and a delayed end on an Android phone; the
  conformance suite and a layout with a link and a label on an iPhone.
