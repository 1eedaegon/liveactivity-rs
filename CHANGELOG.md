# Changelog

Releases are prepared by `tools/release.sh` from the subjects of the commits since the last tag.

## Unreleased

Built on activitykit 0.4 and liveupdate 0.2, whose vocabularies this crate re-exports. Breaking for Rust callers,
as before 1.0; JSON written for 0.2 reads unchanged.

- A layout takes what activitykit 0.4 added: `accessibility`, `transition` and `privacy` on a node, the `link`
  node, `Layout.url`, a style's `content_transition` and the Dynamic Island's `priority`, `below_if_too_wide` and
  `margins`. `Node`, `Layout`, `Style`, `Island` and `IslandExpanded` have new public members and `Element` the
  variant `Link`: a struct literal names them and a match over `Element` has an arm for it. On Android the
  projection names `layout.link`, `layout.url`, `layout.transition` and, for a public notification,
  `layout.privacy`.
- An Android content takes what liveupdate 0.2 added: `open_url` of the presentation and of an action, a
  `chronometer`, `alert`, `public_version`, `small_icon`, `accent_color` and `sub_text`. Build a presentation with
  `AndroidPresentation::new(style)` and an action with `NotificationAction::new(id, title)`. A member that the
  device does not offer is `UNSUPPORTED_FEATURE`.
- Android follows the device where it used to refuse or stay silent: a snapshot of another notification style
  (`styleChange`), `EndPolicy::After` (`delayedEnd`) and the alert of an update (`alert`).
  `Capabilities.android_presentation` reports each. A layout is now projected in its own style with every
  snapshot, also after a content of another style.
- An end sent again for an activity that is over answers its record on Android as on iOS, also with a final
  content, which is not applied. A start that Android refuses to post leaves no activity and its key free.
- liveupdate 0.2 renamed its Android package to `io.github.gon.liveupdate`: take its AAR of 0.2, and rename the
  package where Kotlin or a manifest names one of its classes. It keeps its receipts under the new name and does
  not read those of 0.1, so an activity that runs while the application is updated to this release is no longer
  listed afterwards; Android removes its notification with the update.

## 0.2.0 - 2026-11-12

- ci: release from conventional commits and publish to crates.io (#6) (`93bbe7a`)
- feat: relevance, alerts, Android content and lifecycle events through the contract (#7) (`7025315`)

## 0.1.0 - 2026-11-13

- The first release: the typed contract, `LiveActivityBackend` and `EventSink` behind one builder, the default
  backends on activitykit 0.3 and liveupdate 0.1.3, push events, Android layouts, and the `testkit` feature.
