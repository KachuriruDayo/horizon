# Horizon shader packs

Horizon has its own shader packs: WGSL written against stages of Horizon's renderer, checked before they run. This page
is the guide for pack authors. The design and the pack format are in [api/shaders.md](api/shaders.md); every stage's
signature, input, resource and limit is in the generated [api/shaders-reference.md](api/shaders-reference.md).

Shader packs in the Iris/OptiFine format are a separate feature with its own loader; this page is only about
Horizon's own format.

| Part | Where | State |
|---|---|---|
| Format, stages, check | `crates/horizon-shader-api` | API 0.1, experimental |
| Authoring tool | `horizon-shader` (same crate) | `new`, `check`, `pack` |
| Templates | `crates/horizon-shader-api/templates/` | post, sky, material |
| Example pack | `crates/horizon-shader-api/packs/golden_hour` | a warm sky and two post passes |
| In the game: every stage | Options > Horizon Settings > Shader Packs | sky, material, post passes, hot reload |
| The registry and the launcher | `shader` kind ([api/mod-index.md](api/mod-index.md)); the Mods page's Shader packs tab | checked, then installed into `shaderpacks/` |

## Quick start

```sh
cargo install --path crates/horizon-shader-api      # or: cargo run -p horizon-shader-api --bin horizon-shader -- <command>
horizon-shader new my_pack --template post          # also: --template sky, --template material; --name "My Pack"
horizon-shader check my_pack                        # what the game, the launcher and the registry will check
horizon-shader pack my_pack                         # check, then write my_pack-0.1.0.hshader beside the folder
```

`check` prints each stage with its cost, and the pack's SHA-256:

```
golden_hour 1.0.0 (shader API 0.1): ok
  sky       Warm horizon                     99 expressions, 0 texture reads
  post      Glow                            192 expressions, 3 texture reads
  post      Colour grade                    201 expressions, 2 texture reads
  sha256 b5e0d98c...
```

or every problem, at its file, line and column:

```
my_pack 0.1.0 (shader API 0.1): FAILED
  post.wgsl:9:1: expected `;`, found "}"
```

`--json` prints the same for tools (`ok`, `id`, `version`, `api`, `sha256`, `stages`, `problems`).

## Trying a pack in the game

Put the pack folder (or its `.hshader`) in `shaderpacks/` beside `options.txt` (Open Pack Folder opens it) and pick
it on Options > Horizon Settings > Shader Packs, which lists Horizon's packs, marked (Horizon), beside packs in the
`shaders/` format; one runs at a time, and clicking it again turns it off. `HORIZON_SHADER_PACK=<its folder or file
name>` runs a pack whatever the page says, for development. The game checks the pack first, and refuses one that
fails, with its problems in the chat. While it runs, the game looks at the pack twice a second: save a file
and the pack is checked and rebuilt in place ("Shader pack ... reloaded"); if the new version fails, the chat says
where and the last good version keeps drawing. All three stages run.

## A pack

A folder (or the same files zipped as a `.hshader`) with `horizon-shaders.toml` and WGSL files:

```toml
schema = 1
id = "my_pack"
name = "My Pack"
version = "0.1.0"
api = "0.1"

[[post]]
file = "post.wgsl"

[[options]]
id = "saturation"
name = "Saturation"
type = "float"
default = 1.15
min = 0.0
max = 2.0
```

Every key is in [api/shaders.md](api/shaders.md#the-pack-format-schema-1).

## Stages

A stage is one function your file defines. Horizon puts its prelude (the structs, `frame`, the stage's resources and
helpers) before your file and calls the function from its own pass, so the world is still culled, batched and meshed
by Horizon: a pack changes how things look, not how they are drawn.

| Stage | Your function | What it does |
| --- | --- | --- |
| `[sky]` | `fn sky(input: SkyInput) -> vec4<f32>` | the sky's colour along each view ray, opaque (alpha is ignored), drawn where vanilla draws its sky dome (the Overworld's sky; not the Nether's or the End's); the sun, moon, stars and clouds are drawn over it |
| `[material]` | `fn material(input: MaterialInput) -> Surface` | the lit colour of each point of a block's surface, before the alpha test and fog: every terrain section and distant tile, solid, cutout and translucent (not entities, block entities or particles) |
| `[[post]]` | `fn post(input: PostInput) -> vec4<f32>` | a full-screen pass over the world's picture (up to 8, in order), under the hand and HUD |

A pack has at least one stage; a stage it leaves out keeps vanilla's look. Each input says what vanilla would draw
(`input.color` in the sky and post stages; `albedo * tint * vec4(lightmap, 1.0)` for a material), so a pack can start
from vanilla and change only what it means to.

Every stage can read `frame`: the camera's matrices and position, the sun and moon directions, the time of day,
vanilla's sky and fog colours, rain and thunder, fog distances, the light at the camera, effects and the dimension.
Post passes also read `scene_color`, `scene_depth`, `previous` (the last pass's output), two samplers and the helpers
`depth_at(uv)` and `view_position(uv, depth)`.

Rules the check enforces: your files declare no bindings (`@group`/`@binding`), no entry points and no names starting
with `horizon_`. Code shared by several stages goes in files listed in `shared`, put before every stage.

## Sun shadows

A pack with a `[material]` can ask for the sun's shadow map (API 0.2, `api = "0.2"`):

```toml
[shadow]
resolution = 2048   # the map's side in pixels, 256 to 4096
distance = 96.0     # half the side of the area around the camera it covers, in blocks, 16 to 256
```

Before it draws the world, Horizon draws the terrain's depth as the sun sees it (at night, as the moon sees it,
unless it is new: `frame.shadow_direction` is the light the map was drawn for, `frame.moon_phase` how much of the moon is
lit) into a square map around the
camera (the same sections, culled for the sun, that a `shaders/` pack's shadow pass draws; cut-out leaves and plants
cast the shape of their texture). The material stage reads it with

```wgsl
fn shadow_visibility(position: vec3<f32>, normal: vec3<f32>) -> f32   // 1 in the sun, 0 in shadow
```

where `position` is `input.position` and `normal` the face's normal (it moves the lookup off the surface, so a face does
not shadow itself). It is soft at the edges, 1 outside the map, and 1 where the pack has no `[shadow]` or the sun is
down. Because the map is drawn from the world, not the screen, a mountain behind you shades the valley in front of you.
Only terrain casts a shadow: not entities, the hand, translucent blocks or the distant tiles beyond the render
distance. The post and sky stages do not read the map.

The map follows the camera in whole texels, so shadows do not swim as you move. It costs one extra pass over the
terrain around you: lower `resolution` or `distance` first if a pack is too slow.

## Options

Each `[[options]]` entry is a WGSL `override` of the same name, set when the game builds the pipeline: no text
substitution, no recompiling a different program per value.

```wgsl
override saturation: f32 = 1.15;   // type = "float"; bool → bool, int → i32, choice → u32 (the value's index)
```

The game shows the options on the pack's settings screen. Branches on an option are on a constant, so the GPU
skips the side that isn't taken.

## What the check does

The game runs the same check when it loads a pack (a pack that fails is refused with the message, and the last good
one keeps drawing), the launcher before it installs one, and the registry's CI before a pack is listed:

1. The manifest and the package: known keys, valid ids, every file used, sizes.
2. Each stage built as the renderer builds it and validated by naga.
3. The stage function's signature, and the rules above.
4. Each `override` matches its option's type (or has a default); each option is used.
5. Budgets: 20 000 expressions and 64 texture reads per stage.
6. Every entry point translated for Vulkan (SPIR-V), Metal and DirectX 12 (HLSL), with the options at their
   defaults, minimums and maximums.

`[budget] gpu_ms` is the GPU time your pack aims to add to a frame. To compare, open F3 in the game: while it is
open, each pass is timed on the GPU and the overlay shows the pack's total and its costliest passes, by their `name`
(the median of the last 15 timed frames). Timing waits for nothing, so it does not slow the game. In a timed frame the sky
stage draws in a pass of its own (it shares the world's pass otherwise), so its time includes clearing the picture. The
material stage runs inside terrain's own draws, so it has no time of its own: compare the frame's GPU time with and
without the pack.

## Next

The overlay, lighting and shadow stages, and option values other than the defaults.
