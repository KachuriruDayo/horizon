# Native mod API

Native mods are Rust bundles that `horizon-mixins` weaves into the client **at build time**
(launcher / `install.py`), never at game start. A mod sees the engine only through the API in
`crates/horizon-client/src/game/native_hooks.rs`, versioned per part.

## Pieces

- **Hooks** – empty `native_*` functions the engine calls at fixed points (`native_step`,
  `native_fog`, `native_settings_sliders`, ...). A mod attaches a `HEAD` injection to a hook by
  name (`hook = "fog"`, `hook_version = 1`); the name is resolved through `native-hooks.json`.
- **Helpers** – `native_*` methods a mod calls (`native_block`, `native_particle`,
  `native_set_exposure`, ...). They are the read/act surface of the engine.
- **Modules** – the API is split by engine subsystem (`player-events`, `world-read`, `sound`,
  `audio-layers`, `particles`, `entities-read`, `mining`, `block-views`, `fog`, `lights`,
  `mod-settings`, `frame-exposure`). Each has its own version (`NATIVE_MODULES`, mirrored in
  `native-hooks.json`); `NATIVE_HOOKS_API` is the sum-up number. A part's version rises only when
  its own contract changes, so unrelated mods keep building.
- **Catalog** – `native-hooks.json` lists the hooks (name, version, target function) and the
  modules. Tests `catalog_matches_hook_points`, `the_api_parts_match_the_catalog` and
  `hook_points_stay_hookable` keep it in step with the code.

## A mod bundle

```
my-mod/
  horizon-mixin.toml   [mod] id/version, [[module]] payload, [[injection]] list
  runtime.rs           the mod's code, injected as `crate::game::<name>` into game.rs
  tick.rs ...          one tiny helper fn per injection (named on_*, never like a parameter)
```

Each injection binds the hook's arguments with typed bindings, e.g.
`{ name = "game", ty = "&mut crate::game::Game<'_>", value = "&mut *self" }`. Because the module
lives inside `game.rs` it can use `Game` privately; keep per-side state through `native_window()`.
A mod's settings: files under `horizon-mods/.horizon-mod-data/<id>/` (`native_setting`,
`native_set_setting`); sliders on its settings page via `mod-settings`.

## Installing on a clean build

A clean upstream tree has no API: `native-hooks.json` is missing, `horizon-mixins` would fail with
`E_HOOK_UNKNOWN`/`E_HOOK_VERSION`. So:

1. Take the Horizon source that carries the API (this fork's branch) – the API is part of the
   engine, the mods are not.
2. `python native-mods/install.py --workspace <source> [--mixins <horizon-mixins>] [mod-id ...]`
   - checks every hook a mod pins against `native-hooks.json`, says what is missing;
   - runs `horizon-mixins apply` into a fresh stage dir (the source stays untouched);
   - builds `horizon-client` there. Copy `horizon-client.exe`/`horizon-ctl.exe` into the game folder.
3. A mod built for hook versions the engine has is guaranteed to apply; a newer engine part with a
   higher version asks for a mod update, an older one asks for a newer engine.

## What stays in the engine

Leaf fluff, dynamic-light gathering, shader/shadow API, LOD cache, upload budget/EcoQoS, audio
mixer, datafix, thin-block mesher support: they sit in hot paths or need engine internals; mods
reach them only through the parts above.
