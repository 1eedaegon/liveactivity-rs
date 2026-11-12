# Prior art

Reviewed on 2026-11-12.

## Istmo

Repository: https://github.com/SergioRibera/istmo

Crate: istmo-live-activity 0.1.1-alpha.5

Istmo is the closest existing implementation. It defines a typed async Rust
LiveActivity trait, generates Kotlin and Swift dispatchers, has Android and iOS
handlers, a versioned binary wire protocol and end-to-end mock-runtime tests.

It is not a drop-in answer for this project because consumers adopt the Istmo
runtime and generated transport. It does not wrap the existing activitykit and
liveupdate crates or preserve the current Tauri plugin interface.

Decision: reuse its typed-contract and mock-runtime lessons, not its runtime.

## Fandhe multi-platform

Repository: https://github.com/Fandhe-AI/fandhe-multi-platform

Path: crates/fandhe-cap-live-activity

Fandhe provides a renderer-independent LiveActivity trait with iOS ActivityKit,
Android ongoing notifications, native assets and explicit lifecycle behavior.
The contract is intentionally narrow and tied to the Fandhe runtime and effect
driver rather than being an external framework-plugin SPI.

Decision: reuse its explicit outcome, idempotency and lifecycle requirements
when the production contract is expanded.

## AstroBox Tauri plugin

Repository:
https://github.com/AstralSightStudios/AstroBox-NG-Plugin-LiveActivity

This is a Tauri plugin with Android, iOS and desktop implementations. It is
AstroBox-specific, depends on its corelib and uses AGPL-3.0 with additional
attribution terms. It does not expose a neutral Rust backend interface.

Decision: no code reuse; retain only as evidence that the Tauri plugin shape is
not unique.

## Existing 1eedaegon crates

- https://github.com/1eedaegon/activitykit-rs
- https://github.com/1eedaegon/liveupdate-rs
- https://github.com/1eedaegon/tauri-plugin-live-activity

These are the intended production engines and first framework adapter. The new
repository exists to invert their framework dependency, not replace their
platform logic.

## Registry result

At review time crates.io contained activitykit, istmo-live-activity and
tauri-plugin-live-activity. The names liveactivity, liveactivity and
liveactivity-testkit returned no crate. GitHub public repository search returned
no repository named liveactivity-rs.
