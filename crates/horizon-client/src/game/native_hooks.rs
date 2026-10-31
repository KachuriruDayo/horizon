//! Hook points for native mods (docs/native-mods.md, "Mods on the hook points"): empty
//! functions the game calls at fixed places, for horizon-mixins bundles to
//! hook with HEAD. Hooks that answer take an out parameter (`claimed`,
//! `zoom`…) so several mods can hook the same point. A native mod keeps its
//! own state in a module of its bundle; the helpers below read and draw what
//! Horizon's own HUD extras do, and keep a mod's settings where the
//! WebAssembly mod of the same id kept them.
//!
//! API 9 adds sliders on a mod's settings page (`native_settings_sliders`,
//! `native_settings_slider`: module `mod-settings`) and the picture's exposure for the shader
//! packs (`native_set_exposure`: module `frame-exposure`).
//!
//! API 8 adds the fog the camera sees (`native_fog`, module `fog`) and the light sources the
//! dynamic lights gather (`native_light_sources`, module `lights`).
//!
//! API 7 adds what a mod that changes how blocks are mined needs: the dig's speed, where it
//! resumes and where it stops (`native_dig_speed`, `native_dig_resume`, `native_dig_stop`), the
//! moments a player's break takes a block out (`native_block_breaking`, `native_block_broken`) and
//! the helpers to break more as the player and wear the tool (`native_break_block`,
//! `native_pause_tool_wear`, `native_wear_held`, `native_sneaking`, `native_creative`,
//! `native_game_time`): module `mining`; and blocks drawn thin and stripped
//! (`native_set_thin_blocks`): module `block-views`.
//!
//! API 6 adds what an effects mod on the water needs: the player jumping into water
//! (`native_splash`, `player-events` 2), the surface and the water's flow (`native_top_y`,
//! `native_chunk_loaded`, `native_falling_water_at`, `world-read` 3), the effect particles
//! (`native_particle`, `particles` 1) and the mobs and boats that move (`native_movers`,
//! `entities-read` 1).
//!
//! API 5 adds what a sound mod that listens to the surroundings needs: block states and their
//! sound walls (`native_state_at`, `native_state_info`, `native_solid_at`, `native_water_at`),
//! light, biome and the day's weather and time (`native_light_at`, `native_biome_at`,
//! `native_environment`), the player's eye height and whether a world is open (`world-read` 2),
//! and the mixer's own sounds: layers, the room's reverb and the filters of sounds in the world
//! (`audio-layers` 1).
//!
//! API 4 adds the player's steps, jumps and landings (`native_step`, `native_jump`,
//! `native_land`: module `player-events`), the block at a place (`native_block`,
//! `native_block_in_tag`: `world-read`) and the game's sounds (`native_has_sound`,
//! `native_play_sound`: `sound`), and the versions of such parts (`NATIVE_MODULES`).
//!
//! API 3 adds the world lifecycle hook (`native_world_lifecycle`).
//!
//! API 2 adds the item panel beside container screens and its recipe pages
//! (`native_panel_items`, `native_panel_recipes`, `native_panel_event`),
//! and the game's items and recipes (`native_recipes`,
//! `native_known_recipes`, `native_place_recipe`, `native_give`): what a
//! recipe viewer needs.
//!
//! Names, parameters and call sites are kept from release to release, and
//! hook points are only ever added: each addition raises [`NATIVE_HOOKS_API`].
//! A bundle that hooks only these names the API it needs (`hooks`), and the
//! launcher builds it on any Horizon whose API is at least that one. A change
//! that would break such a bundle is not made.
//! Bodies have no `cfg` and no macros: horizon-mixins refuses a target with
//! either (`tests::hook_points_stay_hookable`).
use super::native_lifecycle::{NativeLifecycleContext, WorldPhase};
use super::{Game, Screen, execution_role::ExecutionRole, native_panel_recipes::RecipePage};
use horizon_client::{
    chat::ChatSource,
    extras::Corner,
    extras_hud::Corners,
    hud::{HudBatch, rgba},
    keybinds,
    mod_list::{self, Installed, NativePage},
    text::Text,
};
use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use winit::{event::MouseButton, keyboard::KeyCode};

/// The hook points' API version. The launcher reads this line from the
/// source it builds; keep its form.
#[allow(dead_code)]
pub(crate) const NATIVE_HOOKS_API: u32 = 25;

/// The native API's parts and their versions. Each part groups the hook points and helpers of
/// one area of the game and is versioned on its own: its version is raised when it gains a hook
/// point or a helper, and `native-hooks.json` lists what each version holds (`modules`; a test
/// keeps the two the same). A mod names the parts it needs, so one area of the game can change
/// without every mod having to follow. Hook points keep their own contract versions there too.
#[allow(dead_code)]
pub(crate) const NATIVE_MODULES: &[(&str, u32)] = &[
    ("player-events", 2),
    ("world-read", 7),
    ("sound", 1),
    ("audio-layers", 1),
    ("particles", 4),
    ("entities-read", 4),
    ("mining", 1),
    ("block-views", 1),
    ("fog", 1),
    ("lights", 1),
    ("mod-settings", 1),
    ("frame-exposure", 1),
    ("block-sounds", 1),
    ("animate-tick", 1),
    ("level-events", 1),
    ("station-events", 1),
    ("villager-events", 1),
    ("use-events", 1),
    ("particle-swap", 2),
    ("chest-lids", 1),
];

/// A falling leaf that reached the ground (`native_take_landed_leaves`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeLanded {
    pub at: [f64; 3],
    pub velocity: [f64; 3],
    pub rgb: [f32; 3],
    pub alpha: f32,
    pub size: f32,
    /// Its turn about the vertical, in radians.
    pub roll: f32,
    /// Its sprite, to wear on a `NativeFx`.
    pub sprite: u32,
}

/// A vanilla particle the game just threw (`native_peek_particles`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeMade {
    /// `smoke`, `large_smoke`, `splash`.
    pub kind: String,
    pub at: [f64; 3],
    pub velocity: [f64; 3],
    /// Its place in the queue, for `native_drop_particles`.
    pub index: usize,
}

/// What the player's right click did (`native_use`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeUse {
    /// The item in the hand that was used (`minecraft:shears`), or empty.
    pub item: String,
    pub main_hand: bool,
    /// The block aimed at, with the face of it, and its name before the click and after it
    /// (the world may answer a moment later on a server).
    pub pos: Option<[i32; 3]>,
    pub face: [i32; 3],
    /// Where on the block the click landed.
    pub point: [f64; 3],
    pub block: String,
    pub block_after: String,
    /// The block's properties (`bites`, `level`) at the click.
    pub props_before: std::collections::BTreeMap<String, String>,
    /// The entity aimed at.
    pub entity: Option<i32>,
}

/// An entity the player knows of (`native_entities`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeEntity {
    pub id: u32,
    /// `minecraft:creeper`, `minecraft:tnt`, `minecraft:end_crystal`, `minecraft:item`,
    /// `minecraft:falling_block`.
    pub kind: String,
    pub position: [f64; 3],
    pub previous: [f64; 3],
    pub velocity: [f64; 3],
    pub width: f32,
    pub height: f32,
    pub on_ground: bool,
    pub in_water: bool,
    pub in_lava: bool,
    pub on_fire: bool,
    pub fall_distance: f32,
    /// Asleep (villagers and foxes).
    pub sleeping: bool,
    pub health: f32,
    pub max_health: f32,
    /// Ticks of breath left (300 when full).
    pub air: i32,
    /// Ticks spent freezing in powder snow.
    pub frozen: i32,
    /// Where the head looks, in degrees (0 faces south), and how high the eyes are above the feet.
    pub head_yaw: f32,
    pub head_pitch: f32,
    pub eye_height: f32,
    pub baby: bool,
    /// A panda with the weak gene.
    pub weak: bool,
    /// Ticks since it was hurt (above 0 right after), and the ticks of its death.
    pub hurt_time: i32,
    pub death_time: i32,
    /// The item of a dropped item, the block of a falling block.
    pub item: String,
    /// What is particular to the kind: a creeper's swell (above 0 when it is lit), a primed
    /// TNT's fuse, a falling block's state.
    pub extra: i32,
}

/// The local player's body (`native_player_state`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativePlayerState {
    pub health: f32,
    pub max_health: f32,
    /// Hunger, 0 to 20.
    pub food: i32,
    pub saturation: f32,
    pub sprinting: bool,
    pub swimming: bool,
    pub sneaking: bool,
    pub on_ground: bool,
    pub on_fire: bool,
    pub flying: bool,
    pub fall_distance: f32,
    /// Ticks of breath left (300 when full).
    pub air: i32,
    /// Ticks spent freezing in powder snow.
    pub frozen: i32,
    /// In bed.
    pub sleeping: bool,
    /// Hurt a moment ago and not yet able to be hurt again.
    pub hurt: bool,
}

/// A particle a mod describes itself (`native_fx`): how long it lives, how it moves and how it
/// looks over its life. Its sprites are the resource pack's `particles/horizon_fx_<slot>.json`.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeFx {
    /// Ticks to live.
    pub life: i32,
    /// Blocks a tick of fall (negative rises) and the share of speed kept each tick.
    pub gravity: f32,
    pub friction: f32,
    /// Quad size at birth, and how much of it is added by the end of life.
    pub size: f32,
    pub grow: f32,
    /// Ticks to fade in and out, and the opacity at its most.
    pub fade_in: i32,
    pub fade_out: i32,
    pub alpha: f32,
    pub rgb: [f32; 3],
    /// Lies on a surface, turned `yaw` about the vertical.
    pub flat: bool,
    pub yaw: f32,
    /// Stops against blocks.
    pub collide: bool,
    /// Drawn at full brightness, whatever the light there.
    pub glow: bool,
    /// A fixed path instead of the velocity: it is born `v` from `at` and by the end of its
    /// life lies `w` from `at`, at `at + v * (1 - t) + w * t^4` for the share `t` of its life
    /// gone.
    pub path: Option<([f64; 3], [f64; 3])>,
    /// Drifts like a firefly, and goes out when it stops or lands.
    pub wander: bool,
    /// Four upright sprites around the point, a quarter of a block out, each drawn from both
    /// sides (the ring an eye of ender leaves).
    pub boxed: bool,
    /// Lies on a wall: the way its face points (a unit vector), turned `yaw` about it; drawn
    /// from both sides. Instead of `flat`.
    pub facing: Option<[f32; 3]>,
    /// How many times its sprites run through over its life.
    pub loops: u8,
    /// One sprite to wear (`NativeLanded::sprite`), instead of the slot's own.
    pub sprite: Option<u32>,
}
impl Default for NativeFx {
    fn default() -> Self {
        Self {
            life: 20,
            gravity: 0.0,
            friction: 0.98,
            size: 0.1,
            grow: 0.0,
            fade_in: 2,
            fade_out: 8,
            alpha: 1.0,
            rgb: [1.0; 3],
            flat: false,
            yaw: 0.0,
            collide: false,
            glow: false,
            path: None,
            wander: false,
            boxed: false,
            facing: None,
            loops: 1,
            sprite: None,
        }
    }
}

/// A step of the player (`native_step`): the block stepped on and where the feet are.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeStep {
    pub(crate) floor: [i32; 3],
    pub(crate) feet: [f64; 3],
    pub(crate) sprinting: bool,
}

/// A block as a native mod reads it (`native_block`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeBlock {
    /// `minecraft:stone`.
    pub(crate) name: String,
    pub(crate) properties: std::collections::BTreeMap<String, String>,
    /// The sound event of its vanilla step (`block.stone.step`).
    pub(crate) step_sound: String,
}

/// The zoom a native mod asks for (`native_zoom`): the factor (1 is none),
/// eased through the field of view like the spyglass when `smooth`, and the
/// scale of mouse turning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeZoom {
    pub(crate) factor: f32,
    pub(crate) smooth: bool,
    pub(crate) turn: f32,
}

/// The minimap a native mod shows (`native_minimap`), drawn by Horizon's
/// minimap: its corner, its side in GUI pixels, blocks per map pixel,
/// turning with the player or north up, round or square.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeMinimap {
    pub(crate) corner: Corner,
    pub(crate) size: f32,
    pub(crate) blocks_per_pixel: f32,
    pub(crate) rotate: bool,
    pub(crate) round: bool,
}

/// The player as a native mod reads it.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativePlayer {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
    /// Degrees; 0 faces south, 90 west (`Direction.fromYRot`).
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    /// `minecraft:overworld`, `minecraft:the_nether`, `minecraft:the_end`.
    pub(crate) dimension: &'static str,
    /// How high the eyes are above the feet, in blocks (API 5).
    pub(crate) eye_height: f32,
    /// Blocks a tick (API 6).
    pub(crate) velocity: [f64; 3],
    /// Whether the player rides a vehicle or a mount (API 6).
    pub(crate) riding: bool,
}

/// A slider of a mod's settings page (`native_settings_sliders`): its label, with the value in it,
/// and its position, 0..1.
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeSlider {
    pub(crate) label: String,
    pub(crate) value: f32,
}

/// The fog of this frame (`native_fog`): the engine's own values, which a mod may change, and what
/// it may want to know.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeFog {
    /// Whether the camera is under water.
    pub(crate) underwater: bool,
    /// The fog colour, red, green, blue and strength (1).
    pub(crate) color: [f32; 4],
    /// Where the fog starts and where it is full, in blocks, and the ends of the sky's and the
    /// clouds' (`sky_end`, `cloud_end`).
    pub(crate) start: f32,
    pub(crate) end: f32,
    pub(crate) sky_end: f32,
    pub(crate) cloud_end: f32,
    /// The biome's water colour, 0..1 each. Read only.
    pub(crate) water_color: [f32; 3],
    /// How clear the biome's water is: a multiplier of the distance seen, 1 for most. Read only.
    pub(crate) clarity: f32,
    /// How bright the sky is, 0..1. Read only.
    pub(crate) daylight: f32,
}

/// A source of light a dynamic light may be made of (`native_light_sources`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeLightSource {
    pub(crate) at: [f64; 3],
    /// Its light level, 0..15.
    pub(crate) level: u8,
    /// An open flame (a torch, a burning mob or item), which does not burn under water.
    pub(crate) open_flame: bool,
}

/// A block the mesher draws as a thin post (`native_set_thin_blocks`), the world keeping the whole
/// block: a log being chopped, stripped and thinning. Its neighbours see it open.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeThinBlock {
    pub(crate) pos: [i32; 3],
    /// The block drawn, `minecraft:stripped_oak_log`, with its properties (`axis`).
    pub(crate) name: String,
    pub(crate) properties: std::collections::BTreeMap<String, String>,
    /// How wide the post is, 0.25 to 1 of the block.
    pub(crate) width: f32,
}

/// What the engine keeps for the native API: the thin blocks the mesher was last given, and
/// whether the tool's wear is paused (a mod breaks many blocks as one job).
#[derive(Default)]
pub(super) struct NativeEngine {
    thin: std::collections::HashMap<[i32; 3], horizon_client::mesher::ChopView>,
    wear_paused: bool,
    /// The picture's exposure shift (gain minus one) the tick before and now: a mod's
    /// `native_set_exposure`, which the frame reads between the two.
    exposure: [f32; 2],
}

/// A mob or a boat as a native mod reads it (`native_movers`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeMover {
    /// A mob's id; a boat has none worth keeping (`u32::MAX`).
    pub(crate) id: u32,
    pub(crate) boat: bool,
    pub(crate) position: [f64; 3],
    /// Where it was a tick ago, so `position - previous` is how it moves.
    pub(crate) previous: [f64; 3],
    pub(crate) width: f32,
}

/// What a block state is made of, read once per state (`native_state_info`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeStateInfo {
    /// `minecraft:stone`.
    pub(crate) name: String,
    /// An opaque block: sound does not pass it.
    pub(crate) solid: bool,
    pub(crate) water: bool,
}

/// A biome and its temperature (`native_biome_at`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeBiome {
    /// `minecraft:plains`.
    pub(crate) name: String,
    pub(crate) temperature: f32,
}

/// The world's sky: the dimension, the time of day and the weather (`native_environment`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeEnvironment {
    pub(crate) dimension: &'static str,
    pub(crate) day_time: i64,
    pub(crate) rain: f32,
    pub(crate) thunder: f32,
}

/// A native mod as Options > Mods lists it (`native_mods`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeMod {
    /// Its id: its settings folder and its keys' lines.
    pub(crate) id: String,
    /// `Name 1.0`.
    pub(crate) title: String,
    pub(crate) description: String,
    /// Its keys: name, label and default key (`key.keyboard.z`). The page
    /// lists them first; `native_mod_key` reads the player's choice.
    pub(crate) keys: Vec<[String; 3]>,
}

/// An item the panel shows (`native_panel_items`): its id
/// (`minecraft:diamond`), a count (1 draws none) and its components beyond
/// the item's own, in vanilla's item syntax, or "".
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeStack {
    pub(crate) item: String,
    pub(crate) count: u32,
    pub(crate) components: String,
}

/// How the panel lays out a recipe of `native_panel_recipes`.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeRecipeLayout {
    /// A crafting grid of `width` x `height` slots, an arrow and the result.
    Grid,
    /// One input over a flame, an arrow and the result (furnaces, smokers,
    /// campfires).
    Cooking,
    /// One input, an arrow and the result (the stonecutter).
    Single,
    /// Template, base and addition, an arrow and the result.
    Smithing,
    /// A potion and an ingredient, an arrow and the result.
    Brewing,
}

/// A recipe the panel shows (`native_panel_recipes`). Each slot, and the
/// result, cycles through its stacks, one a second; an empty slot shows
/// nothing.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeRecipeView {
    pub(crate) layout: NativeRecipeLayout,
    /// The grid's size (`Grid` only), at most 3 x 3.
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) slots: Vec<Vec<NativeStack>>,
    pub(crate) results: Vec<NativeStack>,
    /// A short line under the recipe (a cooking time), or "".
    pub(crate) note: String,
    /// The action button beside it: none, enabled, or shown greyed.
    pub(crate) action: Option<bool>,
}

/// What the player did on a native mod's panel (`native_panel_event`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NativePanelEvent {
    /// A click on an item of the panel, by its index in
    /// `native_panel_items`' list.
    Clicked { index: u32, right: bool },
    /// The search box's text, each time the player changes it.
    Search(String),
    /// A key (`key.keyboard.r`; compare it with `native_mod_key`) went down
    /// while the pointer was on an item: a slot of the screen's, or the
    /// panel's.
    Key { key: String, stack: NativeStack },
    /// A click on a slot of a recipe the panel shows: the recipe's index in
    /// `native_panel_recipes`' list and the slot's in it (the results come
    /// after the slots), and the stack it showed then.
    SlotClicked {
        recipe: u32,
        slot: u32,
        stack: NativeStack,
        right: bool,
    },
    /// The action button of the recipe at this index.
    Action(u32),
    /// The player turned the recipes' page: the index of the first recipe
    /// now shown.
    Page(u32),
    /// The recipe page's back button.
    Back,
    /// The player picked the tab at this index.
    Tab(u32),
}

/// An item of [`NativeRecipes::items`], by its index there, with a count
/// and the components it has beyond the item's own, in vanilla's item
/// syntax (`[minecraft:potion_contents={potion:"minecraft:swiftness"}]`),
/// or "".
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeRecipeStack {
    pub(crate) item: u32,
    pub(crate) count: u32,
    pub(crate) components: String,
}

/// The station a recipe is made at.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeRecipeKind {
    CraftingShaped,
    CraftingShapeless,
    Smelting,
    Blasting,
    Smoking,
    Campfire,
    Stonecutting,
    Smithing,
    Brewing,
}

/// A recipe of [`NativeRecipes`].
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeRecipe {
    /// `minecraft:oak_planks`.
    pub(crate) id: String,
    pub(crate) kind: NativeRecipeKind,
    /// The recipe book's tab (`crafting_building_blocks`, `furnace_food`),
    /// or "".
    pub(crate) category: String,
    /// Recipes the book shows on one button share a group, or "".
    pub(crate) group: String,
    /// A shaped recipe's grid: `slots` holds its cells row by row. 0 for
    /// the other kinds.
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Each slot's stacks, any one of which fits; an empty list is an empty
    /// cell. Cooking and stonecutting: the input. Smithing: the template,
    /// the base, the addition. Brewing: the bottle, the reagent.
    pub(crate) slots: Vec<Vec<NativeRecipeStack>>,
    /// What it makes; several when that varies (a trim on any armour).
    pub(crate) results: Vec<NativeRecipeStack>,
    /// Cooking: game ticks and experience; 0 for the other kinds.
    pub(crate) cooking_time: u32,
    pub(crate) experience: f32,
    /// The recipe book's id for it (one per way the book shows it), as
    /// `native_known_recipes` lists them; -1 for the kinds the book doesn't
    /// show (campfire, stonecutting, smithing, brewing).
    pub(crate) book_id: i32,
}

/// An item of [`NativeRecipes::items`].
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeItem {
    /// `minecraft:diamond`, `<mod>:<path>`.
    pub(crate) id: String,
    /// In the player's language.
    pub(crate) name: String,
    /// Its item tags, without the `#`.
    pub(crate) tags: Vec<String>,
}

/// Every item and recipe the game knows (`native_recipes`): `items` in the
/// registry's order, which each [`NativeRecipeStack::item`] indexes; the
/// recipes are the book's in its order, then campfire, stonecutting,
/// smithing and brewing.
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeRecipes {
    pub(crate) items: Vec<NativeItem>,
    pub(crate) recipes: Vec<NativeRecipe>,
}

/// The mappings `native_mapping` answers for, with the key their default
/// binding is (none: a mouse button, attack or use).
#[allow(dead_code)]
const MAPPINGS: [(&str, Option<KeyCode>); 9] = [
    ("key.forward", Some(KeyCode::KeyW)),
    ("key.left", Some(KeyCode::KeyA)),
    ("key.back", Some(KeyCode::KeyS)),
    ("key.right", Some(KeyCode::KeyD)),
    ("key.jump", Some(KeyCode::Space)),
    ("key.sneak", Some(KeyCode::ShiftLeft)),
    ("key.sprint", Some(KeyCode::ControlLeft)),
    ("key.attack", None),
    ("key.use", None),
];

/// The hook points. Each does nothing in Horizon.
impl Game<'_> {
    /// Once a frame, in a world or not, after Horizon's extras.
    pub(super) fn native_frame(&mut self, elapsed: Duration) {
        let _ = elapsed;
    }
    /// Once a tick, in every `Game`: the window's (in single player, on LAN
    /// and on servers), and the server's own (`native_window` is false
    /// there: the single player server thread, a dedicated server).
    pub(super) fn native_tick(&mut self) {}
    /// The world's phases, read-only; `error_text` carries the engine's
    /// own open-failure text, empty otherwise. Server-side; the deprecated
    /// in-frame path delivers no lifecycle events. A server loop or final
    /// save failure fires `WorldCloseFailed` with the error text.
    pub(super) fn native_world_lifecycle(
        &mut self,
        context: &NativeLifecycleContext,
        phase: WorldPhase,
        error_text: &str,
    ) {
        let _ = (context, phase, error_text);
    }
    /// The destroy progress per tick of the block at `pos` is multiplied by `factor`: set it
    /// (1 is as it is). Called wherever the progress is counted, on the client and on the server.
    /// Module `mining` 1.
    pub(super) fn native_dig_speed(&self, pos: [i32; 3], factor: &mut f32) {
        let _ = (pos, factor);
    }
    /// A dig of the block at `pos` begins: set `progress` to where it starts from (0 is the
    /// start). Module `mining` 1.
    pub(super) fn native_dig_resume(&self, pos: [i32; 3], progress: &mut f32) {
        let _ = (pos, progress);
    }
    /// The player left a dig of the block at `pos` (or went on to the next block) at `progress`
    /// of its destroy progress, counting what `native_dig_resume` gave. Module `mining` 1.
    pub(super) fn native_dig_stop(&mut self, pos: [i32; 3], progress: f32) {
        let _ = (pos, progress);
    }
    /// The sliders of mod `mod_id`'s settings page, under its buttons: push them when it is
    /// yours. Asked again each time the page opens and after each change. Module
    /// `mod-settings` 1.
    pub(super) fn native_settings_sliders(
        &mut self,
        mod_id: &str,
        sliders: &mut Vec<NativeSlider>,
    ) {
        let _ = (mod_id, sliders);
    }
    /// Slider `index` of mod `mod_id`'s settings page moved to `value` (0..1); called while it is
    /// dragged, so keep the work small. Module `mod-settings` 1.
    pub(super) fn native_settings_slider(&mut self, mod_id: &str, index: usize, value: f32) {
        let _ = (mod_id, index, value);
    }
    /// The fog is set for the frame: change `fog` to change what the camera sees. Module `fog` 1.
    pub(super) fn native_fog(&self, fog: &mut NativeFog) {
        let _ = fog;
    }
    /// The facts of every block state are loaded and about to be used (no `Game` yet, so this
    /// takes no `self`): change a state's `sounds` (volume, pitch and the break, step, place,
    /// hit and fall events). Module `block-sounds` 1.
    pub(super) fn native_block_sounds(states: &mut [horizon_items::blocks::StateFacts]) {
        let _ = states;
    }
    /// The game has thrown its particles for this tick but not yet shown them: a mod may look at
    /// them (`native_peek_particles`), drop some (`native_drop_particles`) and play its own instead. Module `particle-swap` 1.
    pub(super) fn native_new_particles(&mut self) {}
    /// The player's right click was taken: used an item, or a block or an entity answered it.
    /// Module `use-events` 1.
    pub(super) fn native_use(&mut self, usage: &NativeUse) {
        let _ = usage;
    }
    /// The game's `animateTick` visited these block positions this frame (about 1300 around the
    /// player, picked at random, as vanilla does for the particles of torches and the like): a
    /// mod looks at the blocks there (`native_block`, `native_state_at`) and spawns its effects.
    /// Module `animate-tick` 1.
    pub(super) fn native_animate_tick(&mut self, visited: &[[i32; 3]]) {
        let _ = visited;
    }
    /// The world reports a level event at `pos` (what vanilla's `levelEvent` hears): 1029 an anvil
    /// breaks, 1030 one is used, 1042 a grindstone, 1044 a smithing table, 1503 an eye of ender is
    /// placed, 2000 a dispenser shoots, 2001 a block breaks. Module `level-events` 1.
    pub(super) fn native_level_event(&mut self, id: i32, pos: [i32; 3], data: i32) {
        let _ = (id, pos, data);
    }
    /// The player took the result of a station (`minecraft:stonecutter`) at `pos`; `item` is what
    /// they got (empty when it went into the inventory). Module `station-events` 1.
    pub(super) fn native_station_result(&mut self, station: &str, pos: [i32; 3], item: &str) {
        let _ = (station, pos, item);
    }
    /// A villager works at its job site `site` (`farmer`, `mason`...) of the given `level`.
    /// Module `villager-events` 1.
    pub(super) fn native_villager_work(&mut self, mob: u32, site: [i32; 3], profession: &str, level: i32) {
        let _ = (mob, site, profession, level);
    }
    /// The light sources the dynamic lights gather this frame (the player's, the mobs' and the
    /// dropped items'): remove or change them. Module `lights` 1.
    pub(super) fn native_light_sources(&self, sources: &mut Vec<NativeLightSource>) {
        let _ = sources;
    }
    /// A player's break is about to take the block at `pos`, which still stands. Module
    /// `mining` 1.
    pub(super) fn native_block_breaking(&mut self, pos: [i32; 3]) {
        let _ = pos;
    }
    /// A player's break took the block at `pos` out (after `native_block_breaking`). Module
    /// `mining` 1.
    pub(super) fn native_block_broken(&mut self, pos: [i32; 3]) {
        let _ = pos;
    }
    /// The player hit the water: a jump or a fall into it, at `feet`. Module `player-events` 2.
    pub(super) fn native_splash(&mut self, feet: [f64; 3]) {
        let _ = feet;
    }
    /// The player stepped on the block `step.floor` (one the game would play a step sound for, not
    /// muffled in water): set `handled` when the mod played its own, and the vanilla step stays
    /// quiet. Module `player-events` 1.
    pub(super) fn native_step(&mut self, step: &NativeStep, handled: &mut bool) {
        let _ = (step, handled);
    }
    /// The player jumped from `feet`. Module `player-events` 1.
    pub(super) fn native_jump(&mut self, feet: [f64; 3]) {
        let _ = feet;
    }
    /// The player landed from a fall at `feet`. Module `player-events` 1.
    pub(super) fn native_land(&mut self, feet: [f64; 3]) {
        let _ = feet;
    }
    /// The HUD (not drawn under F1), after the minimap: place a box with
    /// `corners.place` (boxes stack away from their edge) and draw it with
    /// `native_fill` and `native_text`. Skip a corner `native_corner_shown`
    /// says is taken.
    pub(super) fn native_hud(&mut self, batch: &mut HudBatch, corners: &mut Corners) {
        let _ = (batch, corners);
    }
    /// A key or mouse button went down or up, by its vanilla name
    /// (`key.keyboard.z`, `key.mouse.left`): keys on any screen (check
    /// `native_playing`), mouse buttons while playing. Repeats are left out.
    pub(super) fn native_key(&mut self, key: &str, pressed: bool) {
        let _ = (key, pressed);
    }
    /// Wheel steps while playing: set `claimed` to keep them from the hotbar.
    pub(super) fn native_wheel(&mut self, steps: f32, claimed: &mut bool) {
        let _ = (steps, claimed);
    }
    /// A command typed or clicked in chat, without its slash, before the
    /// server and Horizon's commands see it: set `ran` when the mod ran it.
    pub(super) fn native_command(&mut self, command: &str, ran: &mut bool) {
        let _ = (command, ran);
    }
    /// The zoom a native mod asks for, applied while playing (the spyglass's
    /// own view wins): set `zoom` when it is none yet.
    pub(super) fn native_zoom(&self, zoom: &mut Option<NativeZoom>) {
        let _ = zoom;
    }
    /// The minimap a native mod shows: set `minimap` when it is none yet.
    pub(super) fn native_minimap(&self, minimap: &mut Option<NativeMinimap>) {
        let _ = minimap;
    }
    /// Options > Mods lists the native mods: push yours.
    pub(super) fn native_mods(&mut self, mods: &mut Vec<NativeMod>) {
        let _ = mods;
    }
    /// The buttons of mod `mod_id`'s settings page, under its keys: push
    /// their labels when it is yours. Asked again each time the page opens
    /// and after each click.
    pub(super) fn native_settings(&mut self, mod_id: &str, buttons: &mut Vec<String>) {
        let _ = (mod_id, buttons);
    }
    /// Button `button` of mod `mod_id`'s settings page was clicked.
    pub(super) fn native_settings_click(&mut self, mod_id: &str, button: usize) {
        let _ = (mod_id, button);
    }
    /// The player did `event` on mod `mod_id`'s panel (`native_panel_items`):
    /// act on it when it is yours. API 2.
    pub(super) fn native_panel_event(&mut self, mod_id: &str, event: &NativePanelEvent) {
        let _ = (mod_id, event);
    }
}

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Horizon does.
#[allow(dead_code)]
impl Game<'_> {
    /// Whether the player left mod `mod_id` on in Options > Mods. A mod
    /// turned off does nothing (check this first in each hook) until the
    /// launcher's next build leaves it out.
    pub(super) fn native_on(&self, mod_id: &str) -> bool {
        !self.settings.mods.disabled.contains(mod_id)
    }
    /// Playing, with nothing over the world: keys and zoom apply.
    pub(super) fn native_playing(&self) -> bool {
        self.screen == Screen::Playing
            && self.title.is_none()
            && self.sign_edit.is_none()
            && self.book.is_none()
    }
    /// This `Game` is the window's, not a server's.
    pub(super) fn native_window(&self) -> bool {
        self.options.execution_role == ExecutionRole::Client
    }
    /// Whether a box in `corner` shows: F3's debug screen takes both top
    /// corners (its left and right columns), and a bottom corner while one
    /// of its charts is drawn there.
    pub(super) fn native_corner_shown(&self, corner: Corner) -> bool {
        let charts = self.debug_charts.corners(!self.has_singleplayer_server());
        !debug_takes(corner, self.debug, charts)
    }
    /// The width of `text` in GUI pixels.
    pub(super) fn native_text_width(&self, text: &str) -> f32 {
        HudBatch::text_width(&self.gui.atlas, text)
    }
    /// A rectangle in GUI pixels, `argb` (0x78000000 is the extras' box).
    pub(super) fn native_fill(&self, batch: &mut HudBatch, rect: [f32; 4], argb: u32) {
        let [x, y, w, h] = rect;
        let [a, r, g, b] = argb.to_be_bytes();
        batch.fill(&self.gui.atlas, x, y, w, h, rgba(r, g, b, a));
    }
    /// A line of text at `(x, y)` in GUI pixels, opaque `rgb`.
    pub(super) fn native_text(
        &self,
        batch: &mut HudBatch,
        text: &str,
        at: [f32; 2],
        rgb: u32,
        shadow: bool,
    ) {
        let [x, y] = at;
        let [_, r, g, b] = rgb.to_be_bytes();
        let color = rgba(r, g, b, 0xFF);
        batch.text_styled(&self.gui.atlas, &Text::literal(text), x, y, color, shadow);
    }
    /// Sets how much brighter (above 1) or darker (below 1) the picture should be than the light
    /// says, for the shader packs (`frame.eye_adaptation.x`); call it once a tick, the frame goes
    /// smoothly from one call's value to the next. 1 is none. Module `frame-exposure` 1.
    pub(super) fn native_set_exposure(&mut self, gain: f32) {
        let shift = if gain.is_finite() {
            gain.clamp(0.1, 4.0) - 1.0
        } else {
            0.0
        };
        let e = &mut self.native_engine.exposure;
        *e = [e[1], shift];
    }
    /// The exposure shift the frame uses, `partial` of the way from the last tick to this one.
    pub(super) fn native_exposure_shift(&self, partial: f32) -> f32 {
        let [before, now] = self.native_engine.exposure;
        before + (now - before) * partial
    }
    /// Whether the player sneaks. Module `mining` 1.
    pub(super) fn native_sneaking(&self) -> bool {
        self.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
    }
    /// Whether the player is in creative mode. Module `mining` 1.
    pub(super) fn native_creative(&self) -> bool {
        self.creative
    }
    /// The world's game time, in ticks. Module `mining` 1.
    pub(super) fn native_game_time(&self) -> i64 {
        self.world.game_time
    }
    /// Breaks the block at `pos` as the player breaks it: its drops, its events, the tool's wear
    /// (unless `native_pause_tool_wear`). False where it failed. Module `mining` 1.
    pub(super) fn native_break_block(&mut self, pos: [i32; 3]) -> bool {
        self.break_block_as_player(pos).is_ok()
    }
    /// While paused the held tool does not wear for the blocks the player breaks. Module
    /// `mining` 1.
    pub(super) fn native_pause_tool_wear(&mut self, paused: bool) {
        self.native_engine.wear_paused = paused;
    }
    /// Whether the tool's wear is paused (`mine_block_with_held`).
    pub(super) fn native_wear_paused(&self) -> bool {
        self.native_engine.wear_paused
    }
    /// `uses` more uses of the held tool: its wear per block, with Unbreaking. Module `mining` 1.
    pub(super) fn native_wear_held(&mut self, uses: usize) {
        for _ in 0..uses {
            let Some(stack) = self.inventory[self.selected].clone() else {
                return;
            };
            let Some((per_block, max)) = self
                .res
                .items()
                .get(&stack.item)
                .and_then(|i| Some((i.tool_damage_per_block?, i.max_damage)))
            else {
                return;
            };
            if per_block == 0 || max == 0 {
                return;
            }
            let loss = self.wear(&stack, per_block);
            if let Some(held) = self.inventory[self.selected].as_mut() {
                held.damage += loss;
                if held.damage >= max {
                    self.inventory[self.selected] = None;
                    return;
                }
            }
        }
    }
    /// The destroy progress factor mods ask for at `pos` (`native_dig_speed`).
    pub(super) fn dig_scale(&self, pos: [i32; 3]) -> f32 {
        let mut factor = 1.0;
        self.native_dig_speed(pos, &mut factor);
        if factor.is_finite() {
            factor.clamp(0.01, 100.0)
        } else {
            1.0
        }
    }
    /// Where a dig of the block at `pos` starts from (`native_dig_resume`).
    pub(super) fn dig_resume(&self, pos: [i32; 3]) -> f32 {
        let mut progress = 0.0;
        self.native_dig_resume(pos, &mut progress);
        if progress.is_finite() {
            progress.clamp(0.0, 0.99)
        } else {
            0.0
        }
    }
    /// Whether a mod draws the block at `pos` thin (no crack goes over it).
    pub(super) fn native_thin_at(&self, pos: [i32; 3]) -> bool {
        self.native_engine.thin.contains_key(&pos)
    }
    /// Sets the blocks the mesher draws thin: these, and no others. Their neighbours see them
    /// open. Call it when they change (it does nothing when they have not). Module
    /// `block-views` 1.
    pub(super) fn native_set_thin_blocks(&mut self, blocks: &[NativeThinBlock]) {
        use horizon_client::mesher::{ChopView, ChopViews};
        use horizon_world::chunk::BlockState;
        let registry = &self.pack.assets.registry;
        let hollow = registry
            .resolve(&BlockState {
                name: "minecraft:oak_fence".into(),
                properties: Default::default(),
            })
            .ok()
            .map(|id| id.0);
        let Some(hollow) = hollow else {
            return;
        };
        let wanted: std::collections::HashMap<[i32; 3], ChopView> = blocks
            .iter()
            .filter_map(|b| {
                let log = registry
                    .resolve(&BlockState {
                        name: b.name.clone(),
                        properties: b.properties.clone(),
                    })
                    .ok()?
                    .0;
                let axis = match b.properties.get("axis").map(String::as_str) {
                    Some("x") => 0,
                    Some("z") => 2,
                    _ => 1,
                };
                Some((
                    b.pos,
                    ChopView {
                        log,
                        hollow,
                        axis,
                        width: b.width,
                    },
                ))
            })
            .collect();
        if wanted == self.native_engine.thin {
            return;
        }
        // The sections that show a changed block, and the ones next to it, which see it open.
        let changed: Vec<[i32; 3]> = wanted
            .keys()
            .filter(|p| wanted.get(*p) != self.native_engine.thin.get(*p))
            .chain(
                self.native_engine
                    .thin
                    .keys()
                    .filter(|p| !wanted.contains_key(*p)),
            )
            .copied()
            .collect();
        for p in changed {
            for d in [-1, 1, 0] {
                for (dx, dy, dz) in [(d, 0, 0), (0, d, 0), (0, 0, d)] {
                    let sy = (p[1] + dy - horizon_client::chunk::MIN_Y) >> 4;
                    self.world
                        .dirty_sections
                        .insert(((p[0] + dx) >> 4, sy, (p[2] + dz) >> 4));
                }
            }
        }
        self.streaming.chop = if wanted.is_empty() {
            None
        } else {
            Some(ChopViews::new(wanted.clone()))
        };
        self.native_engine.thin = wanted;
    }
    /// How rare an item is: `common`, `uncommon`, `rare` or `epic`. API 19, `world-read` 6.
    pub(super) fn native_item_rarity(&self, item: &str) -> String {
        self.item_rarity(item)
    }
    /// Whether fire can catch on the block at `x`, `y`, `z` (`FireBlock.canBurn`). API 13,
    /// `world-read` 4.
    pub(super) fn native_burnable(&self, x: i32, y: i32, z: i32) -> bool {
        self.res.animate().is_burnable(self.world.block(x, y, z).0)
    }
    /// How far the lid of the chest at `pos` is open, 0 to 1. API 25, `chest-lids` 1.
    pub(super) fn native_chest_openness(&self, pos: [i32; 3]) -> f32 {
        self.chest_lid(pos, 1.0)
    }
    /// Opens (`true`) or closes the lid of the chest at `pos` on this client alone, as a block
    /// event for one opener or none. API 25, `chest-lids` 1.
    pub(super) fn native_chest_lid(&mut self, pos: [i32; 3], open: bool) {
        self.block_entity_event(pos, 1, i32::from(open));
    }
    /// How many things are cooking on the campfire at `pos`. API 22, `world-read` 7.
    pub(super) fn native_campfire_items(&self, pos: [i32; 3]) -> usize {
        match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(horizon_client::containers::BlockEntity::Campfire(c)) => c.items.iter().filter(|i| i.is_some()).count(),
            _ => 0,
        }
    }
    /// The block at `x`, `y`, `z`, or `None` where it is not loaded. Module `world-read` 1.
    pub(super) fn native_block(&self, x: i32, y: i32, z: i32) -> Option<NativeBlock> {
        let state = self.world.block(x, y, z);
        let info = self.pack.assets.registry.state(state)?;
        Some(NativeBlock {
            name: info.name.clone(),
            properties: info.properties.clone(),
            step_sound: self.res.facts().get(state).sounds.step.to_string(),
        })
    }
    /// Whether the block `name` is in block tag `tag` (`minecraft:logs`). Module `world-read` 1.
    pub(super) fn native_block_in_tag(&self, tag: &str, name: &str) -> bool {
        self.res.items().block_in_tag(tag, name)
    }
    /// Whether the game knows the sound event `event` (a resource pack's too). Module `sound` 1.
    pub(super) fn native_has_sound(&self, event: &str) -> bool {
        self.audio.has_event(event)
    }
    /// Plays the sound event `event` at `at` (the listener's own when it is `None`). Module
    /// `sound` 1.
    pub(super) fn native_play_sound(
        &mut self,
        event: &str,
        at: Option<[f64; 3]>,
        volume: f32,
        pitch: f32,
    ) {
        self.audio.play(event, at, volume, pitch);
    }
    /// Whether a step of the player on `floor` was played by a native mod.
    pub(super) fn native_stepped(
        &mut self,
        floor: [i32; 3],
        feet: [f64; 3],
        sprinting: bool,
    ) -> bool {
        let mut handled = false;
        let step = NativeStep {
            floor,
            feet,
            sprinting,
        };
        self.native_step(&step, &mut handled);
        handled
    }
    /// The player, in a world.
    pub(super) fn native_player(&self) -> Option<NativePlayer> {
        let p = self.player.as_ref()?;
        Some(NativePlayer {
            x: p.position.x,
            y: p.position.y,
            z: p.position.z,
            yaw: self.yaw,
            pitch: self.pitch,
            dimension: self.dim.id(),
            eye_height: p.dimensions.eye_height,
            velocity: [p.velocity.x, p.velocity.y, p.velocity.z],
            riding: self.riding.is_some(),
        })
    }
    /// Whether a world is open: a player, and not the main menu. API 5, `world-read` 2.
    pub(super) fn native_in_world(&self) -> bool {
        self.player.is_some() && self.title.is_none()
    }
    /// The block state at `x`, `y`, `z` (an id that stays the same for the run), for
    /// `native_state_info`. API 5, `world-read` 2.
    pub(super) fn native_state_at(&self, x: i32, y: i32, z: i32) -> u32 {
        self.world.block(x, y, z).0
    }
    /// What state `state` is made of. API 5, `world-read` 2.
    pub(super) fn native_state_info(&self, state: u32) -> NativeStateInfo {
        let facts = self
            .res
            .facts()
            .get(horizon_world::registry::BlockStateId(state));
        NativeStateInfo {
            name: facts.name.to_string(),
            solid: facts.solid_render,
            water: facts.fluid == horizon_client::blocks::Fluid::Water,
        }
    }
    /// Whether the block at `x`, `y`, `z` is opaque. API 5, `world-read` 2.
    pub(super) fn native_solid_at(&self, x: i32, y: i32, z: i32) -> bool {
        self.res.facts().get(self.world.block(x, y, z)).solid_render
    }
    /// Whether the block at `x`, `y`, `z` holds water. API 5, `world-read` 2.
    pub(super) fn native_water_at(&self, x: i32, y: i32, z: i32) -> bool {
        self.res.facts().get(self.world.block(x, y, z)).fluid
            == horizon_client::blocks::Fluid::Water
    }
    /// The light at `x`, `y`, `z`: sky light in the high four bits, block light in the low.
    /// API 5, `world-read` 2.
    pub(super) fn native_light_at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.world.light(x, y, z)
    }
    /// The biome at `x`, `y`, `z`, or `None` where it is not loaded. API 5, `world-read` 2.
    pub(super) fn native_biome_at(&self, x: i32, y: i32, z: i32) -> Option<NativeBiome> {
        let chunk = self.world.chunks.get(&(x >> 4, z >> 4))?;
        let biome = self.res.biomes().list.get(chunk.biome(x, y, z) as usize)?;
        Some(NativeBiome {
            name: biome.name.clone(),
            temperature: biome.temperature,
        })
    }
    /// The dimension, the time of day and the weather now. API 5, `world-read` 2.
    pub(super) fn native_environment(&self) -> NativeEnvironment {
        let (rain, thunder) = self.world.weather.levels(1.0);
        NativeEnvironment {
            dimension: self.dim.id(),
            day_time: self.world.day_time,
            rain,
            thunder,
        }
    }
    /// The first y over the highest block at `x`, `z` that stops rain (the ground, or water).
    /// API 6, `world-read` 3.
    pub(super) fn native_top_y(&self, x: i32, z: i32) -> i32 {
        self.rain_height(x, z)
    }
    /// Whether the chunk with the block column `x`, `z` is loaded. API 6, `world-read` 3.
    pub(super) fn native_chunk_loaded(&self, x: i32, z: i32) -> bool {
        self.world.chunks.contains_key(&(x >> 4, z >> 4))
    }
    /// Whether the block at `x`, `y`, `z` is water that falls. API 6, `world-read` 3.
    pub(super) fn native_falling_water_at(&self, x: i32, y: i32, z: i32) -> bool {
        let facts = self.res.facts().get(self.world.block(x, y, z));
        facts.fluid == horizon_client::blocks::Fluid::Water && facts.fluid_falling
    }
    /// Plays the effect particle `kind` (`horizon_wave`, `horizon_wake`, `horizon_ripple`,
    /// `horizon_splash`, `horizon_splash_foam`, `horizon_splash_ring`, `horizon_cascade`,
    /// `horizon_spray`: their sprites are the resource pack's, in `particles/<kind>.json`) at
    /// `at`, moving `velocity` blocks a tick. `scale` is its size; `params` are its own: a wave's
    /// heading about the vertical in radians and its life in ticks. `always` shows it from any
    /// distance. False for a kind it does not know. API 6, `particles` 1.
    pub(super) fn native_particle(
        &mut self,
        kind: &str,
        at: [f64; 3],
        velocity: [f64; 3],
        scale: f32,
        params: [f32; 4],
        always: bool,
    ) -> bool {
        let Some(ty) = horizon_client::particles::ParticleType::from_name(kind) else {
            return false;
        };
        let mut options = horizon_client::particles::ParticleOptions::simple(ty);
        options.scale = scale;
        options.color = params;
        self.particles
            .add_with(&options, at, velocity, always, always);
        true
    }
    /// Plays a particle of the mod's own: slot `slot` (0 to 63) names its sprite set, the
    /// resource pack's `particles/horizon_fx_<slot>.json`; `fx` says how it lives. It starts at
    /// `at` and moves `velocity` blocks a tick. `always` shows it from any distance. False for
    /// a slot it does not have. API 11, `particles` 2.
    pub(super) fn native_fx(
        &mut self,
        slot: u8,
        at: [f64; 3],
        velocity: [f64; 3],
        fx: &NativeFx,
        always: bool,
    ) -> bool {
        use horizon_client::particles::{FxSpec, ParticleOptions, ParticleType as T};
        let Some(ty) = T::fx(slot) else {
            return false;
        };
        let mut options = ParticleOptions::simple(ty);
        options.fx = Some(FxSpec {
            life: fx.life,
            gravity: fx.gravity,
            friction: fx.friction,
            size: fx.size,
            grow: fx.grow,
            fade_in: fx.fade_in,
            fade_out: fx.fade_out,
            alpha: fx.alpha,
            rgb: fx.rgb,
            flat: fx.flat,
            yaw: fx.yaw,
            collide: fx.collide,
            glow: fx.glow,
            path: fx.path,
            wander: fx.wander,
            boxed: fx.boxed,
            facing: fx.facing,
            loops: fx.loops,
            sprite: fx.sprite,
        });
        self.particles
            .add_with(&options, at, velocity, always, always);
        true
    }
    /// Plays the vanilla particle `kind` that wears a block's texture (`block`, `falling_dust`,
    /// `block_crumble`) of the block state `block` (`minecraft:oak_log[axis=y]`, or just a name for
    /// its default state), at `at`, moving `velocity`. False when there is no such block, particle
    /// or texture. API 15, `particles` 3.
    pub(super) fn native_particle_block(
        &mut self,
        kind: &str,
        block: &str,
        at: [f64; 3],
        velocity: [f64; 3],
    ) -> bool {
        let Some(ty) = horizon_client::particles::ParticleType::from_name(kind) else {
            return false;
        };
        let registry = self.pack.assets.registry;
        let name = block.split('[').next().unwrap_or(block);
        let Some(mut state) = registry.default_state(name) else {
            return false;
        };
        if let Some(props) = block.split_once('[').map(|(_, p)| p.trim_end_matches(']')) {
            let wanted: Vec<(&str, &str)> = props
                .split(',')
                .filter_map(|kv| kv.split_once('='))
                .collect();
            for id in 0..registry.len() as u32 {
                let Some(info) = registry.state(horizon_world::registry::BlockStateId(id)) else {
                    continue;
                };
                if info.name == name
                    && wanted
                        .iter()
                        .all(|(k, v)| info.properties.get(*k).is_some_and(|x| x == v))
                {
                    state = horizon_world::registry::BlockStateId(id);
                    break;
                }
            }
        }
        let Some(sprite) = Self::block_particle_sprite(&self.pack, state) else {
            return false;
        };
        let options = horizon_client::particles::ParticleOptions::block(ty, sprite, 0xFFFFFF);
        self.particles.add(&options, at, velocity);
        true
    }
    /// The crumbs of the block at `pos` breaking, as vanilla's `addDestroyBlockEffect` throws
    /// them, for the block `block` (`minecraft:pumpkin`) there. API 15, `particles` 3.
    pub(super) fn native_destroy_effect(&mut self, pos: [i32; 3], block: &str) -> bool {
        let Some(state) = self.pack.assets.registry.default_state(block) else {
            return false;
        };
        self.destroy_particles(pos, state);
        true
    }
    /// The vanilla particles of the named kinds that the game threw since the last tick and has
    /// not yet shown. API 18, `particle-swap` 1.
    pub(super) fn native_peek_particles(&self, kinds: &[&str]) -> Vec<NativeMade> {
        let types: Vec<_> = kinds
            .iter()
            .filter_map(|k| horizon_client::particles::ParticleType::from_name(k))
            .collect();
        self.particles
            .peek_new(&types)
            .into_iter()
            .map(|(index, ty, at, velocity)| NativeMade {
                kind: ty.name().to_owned(),
                at,
                velocity,
                index,
            })
            .collect()
    }
    /// Asks the game for the falling leaves that land (it hands them over, off the ground, with
    /// `native_take_landed_leaves`); say it every tick you want them. API 21, `particle-swap` 2.
    pub(super) fn native_watch_leaves(&mut self, on: bool) {
        self.particles.watch_leaves = on;
    }
    /// The falling leaves that landed since the last call. API 21, `particle-swap` 2.
    pub(super) fn native_take_landed_leaves(&mut self) -> Vec<NativeLanded> {
        self.particles
            .take_landed_leaves()
            .into_iter()
            .map(|l| NativeLanded {
                at: l.position,
                velocity: l.velocity,
                rgb: l.rgb,
                alpha: l.alpha,
                size: l.size,
                roll: l.roll,
                sprite: l.sprite,
            })
            .collect()
    }
    /// Takes these particles (`index` of `native_peek_particles`) out before they are shown.
    /// API 18, `particle-swap` 1.
    pub(super) fn native_drop_particles(&mut self, indices: Vec<usize>) {
        self.particles.drop_new(indices);
    }
    /// Plays the vanilla item particle (the crumbs of `item`, `minecraft:bone_meal`) at `at`,
    /// moving `velocity`. False for an item it does not know. API 15, `particles` 3.
    pub(super) fn native_particle_item(
        &mut self,
        item: &str,
        at: [f64; 3],
        velocity: [f64; 3],
    ) -> bool {
        let stack = horizon_client::items::ItemStack::new(item, 1);
        let options = self.item_particle(&stack);
        self.particles.add(&options, at, velocity);
        true
    }
    /// What a right click aims at and holds, to hand to `native_use` once the click is done.
    pub(super) fn native_use_before(&self) -> Option<NativeUse> {
        let held = self.held().map(|s| s.item.to_string()).unwrap_or_default();
        let aimed = self.pick();
        let found = aimed
            .as_ref()
            .and_then(|h| self.native_block(h.pos[0], h.pos[1], h.pos[2]));
        let props_before = found
            .as_ref()
            .map(|b| b.properties.clone())
            .unwrap_or_default();
        let name = found.map(|b| b.name).unwrap_or_default();
        Some(NativeUse {
            item: held,
            main_hand: true,
            pos: aimed.as_ref().map(|h| h.pos),
            face: aimed.as_ref().map_or([0; 3], |h| h.face),
            point: aimed.as_ref().map_or([0.0; 3], |h| h.point),
            block: name.clone(),
            block_after: name,
            props_before,
            entity: self.pick_entity_hit().flatten(),
        })
    }
    /// The mobs, primed TNT, end crystals, falling blocks and dropped items around, with what
    /// a mod needs to draw effects on them. API 14, `entities-read` 2.
    pub(super) fn native_entities(&self) -> Vec<NativeEntity> {
        let v = |p: &horizon_core::geometry::Vec3| [p.x, p.y, p.z];
        let mut out = Vec::with_capacity(self.mobs.len() + self.tnts.len() + self.crystals.len());
        for mob in &self.mobs {
            out.push(NativeEntity {
                id: mob.id,
                kind: mob.kind.id.clone(),
                position: v(&mob.position),
                previous: v(&mob.previous),
                velocity: v(&mob.velocity),
                width: mob.width(),
                height: mob.height(),
                on_ground: mob.on_ground,
                in_water: mob.fluids.in_water,
                in_lava: mob.fluids.in_lava,
                on_fire: mob.fire_ticks > 0,
                fall_distance: mob.fall_distance as f32,
                head_yaw: mob.head_yaw,
                head_pitch: mob.head_pitch,
                eye_height: mob.eye_height() as f32,
                baby: mob.is_baby(),
                weak: horizon_client::mobs::panda::is_weak(mob),
                hurt_time: mob.hurt_time,
                death_time: mob.death_time,
                item: String::new(),
                sleeping: horizon_client::mobs::bat::resting(mob)
                    || horizon_client::mobs::villager::sleeping(mob)
                    || (mob.species() == horizon_client::mobs::Species::Fox
                        && horizon_client::mobs::fox::pose(mob, 0.0).sleeping)
                    // A cat lying down (`Cat.isLying`).
                    || (mob.species() == horizon_client::mobs::Species::Cat && mob.extra.flag),
                health: mob.health,
                max_health: mob.kind.max_health,
                air: mob.air,
                frozen: mob.ticks_frozen,
                extra: mob.swell,
            });
        }
        for tnt in &self.tnts {
            out.push(NativeEntity {
                id: tnt.id,
                kind: "minecraft:tnt".to_owned(),
                position: v(&tnt.position),
                previous: v(&tnt.previous),
                velocity: v(&tnt.velocity),
                width: 0.98,
                height: 0.98,
                on_ground: tnt.on_ground,
                in_water: false,
                in_lava: false,
                on_fire: false,
                fall_distance: 0.0,
                head_yaw: 0.0,
                head_pitch: 0.0,
                eye_height: 0.0,
                baby: false,
                weak: false,
                hurt_time: 0,
                death_time: 0,
                item: String::new(),
                sleeping: false,
                health: 1.0,
                max_health: 1.0,
                air: 300,
                frozen: 0,
                extra: tnt.fuse,
            });
        }
        for crystal in &self.crystals {
            out.push(NativeEntity {
                id: crystal.id,
                kind: "minecraft:end_crystal".to_owned(),
                position: v(&crystal.position),
                previous: v(&crystal.position),
                velocity: [0.0; 3],
                width: 2.0,
                height: 2.0,
                on_ground: false,
                in_water: false,
                in_lava: false,
                on_fire: false,
                fall_distance: 0.0,
                head_yaw: 0.0,
                head_pitch: 0.0,
                eye_height: 0.0,
                baby: false,
                weak: false,
                hurt_time: 0,
                death_time: 0,
                item: String::new(),
                sleeping: false,
                health: 1.0,
                max_health: 1.0,
                air: 300,
                frozen: 0,
                extra: 0,
            });
        }
        let plain = |id: u32,
                     kind: &str,
                     p: &horizon_core::geometry::Vec3,
                     prev: &horizon_core::geometry::Vec3,
                     vel: &horizon_core::geometry::Vec3,
                     size: f32,
                     ground: bool,
                     item: String| NativeEntity {
            id,
            kind: kind.to_owned(),
            position: v(p),
            previous: v(prev),
            velocity: v(vel),
            width: size,
            height: size,
            on_ground: ground,
            in_water: false,
            in_lava: false,
            on_fire: false,
            fall_distance: 0.0,
            head_yaw: 0.0,
            head_pitch: 0.0,
            eye_height: 0.0,
            baby: false,
            weak: false,
            hurt_time: 0,
            death_time: 0,
            item,
            sleeping: false,
            health: 1.0,
            max_health: 1.0,
            air: 300,
            frozen: 0,
            extra: 0,
        };
        for drop in &self.dropped {
            out.push(plain(
                drop.id,
                "minecraft:item",
                &drop.position,
                &drop.previous,
                &drop.velocity,
                0.25,
                drop.on_ground,
                drop.stack.item.to_string(),
            ));
        }
        for p in &self.creatures.projectiles {
            out.push(plain(
                p.id,
                p.kind.id(),
                &p.position,
                &p.previous,
                &p.velocity,
                p.kind.size() as f32,
                false,
                String::new(),
            ));
        }
        for f in &self.falling {
            let name = self
                .pack
                .assets
                .registry
                .state(f.state)
                .map(|s| s.name.clone())
                .unwrap_or_default();
            let mut e = plain(
                f.id,
                "minecraft:falling_block",
                &f.position,
                &f.previous,
                &f.velocity,
                0.98,
                false,
                name,
            );
            e.fall_distance = f.fall_distance as f32;
            out.push(e);
        }
        out
    }
    /// The local player's body: health, hunger, sprinting, air. API 16, `world-read` 5.
    pub(super) fn native_player_state(&self) -> Option<NativePlayerState> {
        let p = self.player.as_ref()?;
        Some(NativePlayerState {
            health: p.health.health(),
            // The attribute needs a mutable player; the usual 20 hearts-worth stands in.
            max_health: 20.0,
            food: p.health.food.food,
            saturation: p.health.food.saturation,
            sprinting: p.sprinting,
            swimming: p.swimming,
            sneaking: self.native_sneaking(),
            on_ground: p.ground.on_ground,
            on_fire: p.fire_ticks > 0,
            flying: p.flying,
            fall_distance: p.fall_distance as f32,
            air: p.air,
            frozen: p.ticks_frozen,
            sleeping: self.sleeping.is_some(),
            hurt: p.health.hurt.cooldown > 0,
        })
    }
    /// The mobs and boats the player knows of: where they are and were. API 6, `entities-read` 1.
    pub(super) fn native_movers(&self) -> Vec<NativeMover> {
        let mut movers = Vec::with_capacity(self.mobs.len() + self.boats.len());
        for boat in &self.boats {
            movers.push(NativeMover {
                id: u32::MAX,
                boat: true,
                position: [boat.position.x, boat.position.y, boat.position.z],
                previous: [boat.previous.x, boat.previous.y, boat.previous.z],
                width: 1.4,
            });
        }
        for mob in &self.mobs {
            movers.push(NativeMover {
                id: mob.id,
                boat: false,
                position: [mob.position.x, mob.position.y, mob.position.z],
                previous: [mob.previous.x, mob.previous.y, mob.previous.z],
                width: mob.width(),
            });
        }
        movers
    }
    /// Starts a sound from `file` that plays at the listener until stopped (`looping` or to its
    /// end), from `offset` (0 to 1) of its length; its id, or `None` where there is no output.
    /// The sound follows the Ambient volume. API 5, `audio-layers` 1.
    pub(super) fn native_layer_start(
        &mut self,
        file: &std::path::Path,
        gain: f32,
        looping: bool,
        offset: f32,
    ) -> Option<u64> {
        self.audio.layer_start(file, gain, looping, offset)
    }
    /// Sets the gain of layer `id`. API 5, `audio-layers` 1.
    pub(super) fn native_layer_set(&mut self, id: u64, gain: f32) {
        self.audio.layer_set(id, gain);
    }
    /// Stops layer `id`. API 5, `audio-layers` 1.
    pub(super) fn native_layer_stop(&mut self, id: u64) {
        self.audio.layer_stop(id);
    }
    /// Whether layer `id` has played to its end. API 5, `audio-layers` 1.
    pub(super) fn native_layer_done(&self, id: u64) -> bool {
        self.audio.layer_done(id)
    }
    /// The volume of the layers (0 to 1 and over), on top of the master. API 5, `audio-layers` 1.
    pub(super) fn native_layer_volume(&mut self, volume: f32) {
        self.audio.ambient_volume = volume;
    }
    /// The room's reverb: its level, its decay and the share every new sound sends to it.
    /// API 5, `audio-layers` 1.
    pub(super) fn native_set_reverb(&mut self, level: f32, decay: f32, send: f32) {
        self.audio.set_reverb(level, decay, send);
    }
    /// The sounds playing in the world: their ids and where they are. API 5, `audio-layers` 1.
    pub(super) fn native_voices(&self) -> Vec<(u64, [f64; 3])> {
        self.audio.physics_voices()
    }
    /// Muffles sound `id`: the low-pass cutoff (0 to 1), its send to the reverb and how much
    /// walls take off its volume (1 is none). API 5, `audio-layers` 1.
    pub(super) fn native_voice_filter(&mut self, id: u64, cutoff: f32, send: f32, occlusion: f32) {
        self.audio.set_voice_filter(id, cutoff, send, occlusion);
    }
    /// The frame counter: frames in the last second, the slowest frame and
    /// the 1% low of the last 5 seconds, in frames per second.
    pub(super) fn native_frame_rate(&self) -> (u32, u32, u32) {
        self.frame_reading()
    }
    /// Attack and use clicks in the last second.
    pub(super) fn native_clicks(&mut self) -> (u32, u32) {
        self.mouse_reading().0
    }
    /// A vanilla mapping (`key.forward`, `key.jump`, `key.attack`…): whether
    /// it is held, and its key's label as Controls shows it.
    pub(super) fn native_mapping(&mut self, name: &str) -> Option<(bool, String)> {
        let &(_, key) = MAPPINGS.iter().find(|m| m.0 == name)?;
        let label = keybinds::display_name(self.settings.keys.get(name)).plain();
        let down = match key {
            Some(k) => self.keys.contains(&k),
            None => self.mouse_reading().1[usize::from(name == "key.use")],
        };
        Some((down, label))
    }
    /// The key the player chose for a mod's key `name`, else `default`
    /// (`key.keyboard.z`), as the WebAssembly mods' keys were kept
    /// (`key.<mod id>.<name>` in `horizon-mods.txt`).
    pub(super) fn native_mod_key(&self, mod_id: &str, name: &str, default: &str) -> String {
        let chosen = self
            .settings
            .mods
            .keys
            .get(mod_id)
            .and_then(|k| k.get(name));
        chosen.map_or(default, String::as_str).to_owned()
    }
    /// A line in chat from the client.
    pub(super) fn native_chat(&mut self, text: &str) {
        let focused = self.screen == Screen::Chat;
        let line = Text::literal(text);
        let atlas = &self.gui.atlas;
        self.ui
            .chat
            .add(atlas, line, None, ChatSource::Client, self.ticks, focused);
    }
    /// A native mod's own folder: `horizon-mods/.horizon-mod-data/<id>/`,
    /// where the WebAssembly mod of the same id kept its storage, so a
    /// player's settings carry over. Not created here.
    pub(super) fn native_data_dir(&self, mod_id: &str) -> PathBuf {
        let mods = Self::options_path().with_file_name(mod_list::DIR);
        mods.join(".horizon-mod-data").join(mod_id)
    }
    /// A mod's setting `key` (one file per key in `native_data_dir`).
    pub(super) fn native_setting(&self, mod_id: &str, key: &str) -> Option<String> {
        let path = self.native_data_dir(mod_id).join(key);
        fs::read_to_string(path).ok().map(|v| v.trim().to_owned())
    }
    /// Keeps a mod's setting `key`.
    pub(super) fn native_set_setting(&self, mod_id: &str, key: &str, value: &str) {
        let dir = self.native_data_dir(mod_id);
        let _ = fs::create_dir_all(&dir).and_then(|()| fs::write(dir.join(key), value));
    }
    /// Shows the panel beside container screens with these items (at most
    /// 8192), as the game's own slots in pages, with a search box; the
    /// player's clicks, searches and keys over items come back as
    /// `native_panel_event`. The panel is the last mod's to show one; empty
    /// items and no recipe page take yours away.
    pub(super) fn native_panel_items(&mut self, mod_id: &str, items: Vec<NativeStack>) {
        self.panel.show_items(mod_id, items);
    }
    /// Shows these recipes in the panel instead of its items, as many as
    /// fit a page, under `title` and a back button, with a row of `tabs`
    /// (each an icon; `tab` is the selected one) between them. At most 256
    /// recipes and 16 tabs; no recipes shows the items again. The same title
    /// and tab again keep the page the player turned to.
    pub(super) fn native_panel_recipes(
        &mut self,
        mod_id: &str,
        title: &str,
        tabs: Vec<NativeStack>,
        tab: u32,
        views: Vec<NativeRecipeView>,
    ) {
        let page = RecipePage::new(title, tabs, tab, views);
        self.panel.show_recipes(mod_id, page);
    }
    /// Every item and recipe the game knows, made once (the same for every
    /// call); none outside a world. They are the window's own: the world's
    /// in single player, vanilla's on a server, which sends a client no
    /// others.
    pub(super) fn native_recipes(&mut self) -> Option<Arc<NativeRecipes>> {
        self.player.as_ref()?;
        self.made_recipes()
    }
    /// The `book_id`s of the recipes the player's recipe book holds now
    /// (the server's book on a server); empty outside a world.
    pub(super) fn native_known_recipes(&mut self) -> Arc<[i32]> {
        if self.player.is_none() {
            return Arc::from([]);
        }
        self.known_displays()
    }
    /// Places a recipe in the open screen's grid as a click in the recipe
    /// book does (`ServerboundPlaceRecipePacket`): the inventory's grid or a
    /// crafting table's. `all` places as many as the items allow, as with
    /// Shift. Refused while no such screen is open and for a recipe the
    /// player's book doesn't hold; one missing items shows as a ghost.
    pub(super) fn native_place_recipe(&mut self, book_id: i32, all: bool) -> Result<(), String> {
        self.recipe_place(book_id, all)
    }
    /// Creative mode only: gives the player `count` of `item`
    /// (`minecraft:diamond`) as taking it from the creative inventory does
    /// (`ServerboundSetCreativeModeSlot`): the selected slot first, then the
    /// hotbar, then the inventory. The count is cut to one stack.
    pub(super) fn native_give(&mut self, item: &str, count: u32) -> Result<(), String> {
        self.recipe_give(item, count)
    }
}

/// Where the game calls the hook points.
impl Game<'_> {
    /// Options > Mods' rows for the native mods, after the others.
    pub(super) fn list_native_mods(&mut self) {
        let mut mods = Vec::new();
        self.native_mods(&mut mods);
        // A WebAssembly mod of the same id: the native row stands for it,
        // and alone toggles its `mod.<id>` line.
        self.settings
            .mods
            .installed
            .retain(|row| row.native.is_none() && !mods.iter().any(|m| m.id == row.id));
        for m in mods {
            let mut buttons = Vec::new();
            self.native_settings(&m.id, &mut buttons);
            let sliders = self.native_slider_rows(&m.id);
            let page = NativePage {
                keys: m.keys,
                buttons,
                sliders,
            };
            self.settings.mods.installed.push(Installed {
                id: m.id,
                title: m.title,
                description: m.description,
                problem: None,
                dev: false,
                native: Some(page),
            });
        }
    }
    /// A click on button `button` of native mod `i`'s page (its index in
    /// Options > Mods), and the page's labels after it.
    pub(super) fn native_settings_event(&mut self, i: usize, button: usize) {
        let Some(id) = self.settings.mods.installed.get(i).map(|m| m.id.clone()) else {
            return;
        };
        self.native_settings_click(&id, button);
        self.native_settings_refresh(i);
    }
    /// Native mod `i`'s page opened: its labels asked again, so that they
    /// follow the language (and the mod's state) since the list was made.
    pub(super) fn native_settings_refresh(&mut self, i: usize) {
        let Some(id) = self.settings.mods.installed.get(i).map(|m| m.id.clone()) else {
            return;
        };
        let mut buttons = Vec::new();
        self.native_settings(&id, &mut buttons);
        let sliders = self.native_slider_rows(&id);
        let row = self.settings.mods.installed.get_mut(i);
        if let Some(page) = row.and_then(|m| m.native.as_mut()) {
            page.buttons = buttons;
            page.sliders = sliders;
        }
    }
    /// The sliders of mod `id`'s page, as the page keeps them.
    fn native_slider_rows(&mut self, id: &str) -> Vec<mod_list::NativeSliderRow> {
        let mut sliders = Vec::new();
        self.native_settings_sliders(id, &mut sliders);
        sliders
            .into_iter()
            .map(|s| mod_list::NativeSliderRow {
                label: s.label,
                value: if s.value.is_finite() {
                    s.value.clamp(0.0, 1.0)
                } else {
                    0.0
                },
            })
            .collect()
    }
    /// Slider `k` of native mod `i`'s page moved: the mod is told (the labels stay while it is
    /// dragged, so the handle does not jump).
    pub(super) fn native_slider_event(&mut self, i: usize, k: usize) {
        let Some(row) = self.settings.mods.installed.get(i) else {
            return;
        };
        let value = row
            .native
            .as_ref()
            .and_then(|p| p.sliders.get(k))
            .map(|s| s.value);
        let id = row.id.clone();
        if let Some(value) = value {
            self.native_settings_slider(&id, k, value);
            // The label carries the value: asked again, the position kept as it is dragged.
            let mut sliders = Vec::new();
            self.native_settings_sliders(&id, &mut sliders);
            if let Some(page) = self
                .settings
                .mods
                .installed
                .get_mut(i)
                .and_then(|m| m.native.as_mut())
            {
                for (slot, fresh) in page.sliders.iter_mut().zip(sliders) {
                    slot.label = fresh.label;
                }
            }
        }
    }
    /// A key event for `native_key`.
    pub(super) fn native_key_event(&mut self, key: KeyCode, pressed: bool, repeat: bool) {
        if repeat {
            return;
        }
        if let Some(name) = keybinds::key_name(key) {
            if pressed {
                self.native_panel_key(name);
            }
            self.native_key(name, pressed);
        }
    }
    /// A mouse button while playing, for `native_key`.
    pub(super) fn native_mouse_event(&mut self, button: MouseButton, pressed: bool) {
        let name = keybinds::mouse_name(button);
        self.native_key(&name, pressed);
    }
    /// Wheel steps while playing: whether a native mod took them.
    pub(super) fn native_wheel_event(&mut self, steps: f32) -> bool {
        let mut claimed = false;
        self.native_wheel(steps, &mut claimed);
        claimed
    }
    /// A native mod's command.
    pub(super) fn client_mod_command(&mut self, command: &str) -> bool {
        let mut ran = false;
        self.native_command(command, &mut ran);
        ran
    }
    /// The zoom in effect while playing (a native mod's); the spyglass's own
    /// view wins.
    pub(super) fn native_zoom_now(&self) -> Option<NativeZoom> {
        if !self.native_playing() || self.scoping() {
            return None;
        }
        let mut zoom = None;
        self.native_zoom(&mut zoom);
        // As the WebAssembly runtime clamped a mod's zoom.
        let z = zoom?;
        let factor = finite(z.factor, 1.0, 10.0);
        (factor > 1.0).then_some(NativeZoom {
            factor,
            smooth: z.smooth,
            turn: if z.turn.is_finite() {
                z.turn.clamp(0.05, 1.0)
            } else {
                1.0
            },
        })
    }
    /// The minimap a native mod shows.
    pub(super) fn native_minimap_now(&self) -> Option<NativeMinimap> {
        let mut minimap = None;
        self.native_minimap(&mut minimap);
        minimap.map(|m| NativeMinimap {
            size: finite(m.size, 16.0, 256.0),
            blocks_per_pixel: finite(m.blocks_per_pixel, 0.5, 4.0),
            ..m
        })
    }
}

/// Whether F3's debug screen, open when `debug`, covers `corner`
/// (`DebugScreenOverlay.extractRenderState`): its left and right columns
/// the top corners, and its charts the bottom corners `charts` (bottom left,
/// bottom right) says they are drawn in.
fn debug_takes(corner: Corner, debug: bool, charts: [bool; 2]) -> bool {
    debug
        && match corner {
            Corner::TopLeft | Corner::TopRight => true,
            Corner::BottomLeft => charts[0],
            Corner::BottomRight => charts[1],
        }
}

/// `v` within `lo..=hi`; `lo` when it is not a number.
fn finite(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { lo }
}

#[cfg(test)]
mod tests {
    /// horizon-mixins refuses targets whose body has a `cfg` or a macro:
    /// the hook points and helpers must stay free of both. The launcher
    /// reads the API line as it is.
    #[test]
    fn hook_points_stay_hookable() {
        let source = include_str!("native_hooks.rs");
        let code = source.split("#[cfg(test)]").next().unwrap_or_default();
        assert!(code.contains("\npub(crate) const NATIVE_HOOKS_API: u32 = "));
        let lines = code.lines().filter(|l| !l.trim_start().starts_with("//"));
        for line in lines {
            assert!(!line.contains("cfg"), "a cfg in the hook points: {line}");
            for mac in ["!(", "![", "! {", "!{"] {
                assert!(!line.contains(mac), "a macro in the hook points: {line}");
            }
        }
    }

    #[test]
    fn lifecycle_source_stays_hookable() {
        let source = include_str!("native_lifecycle.rs");
        let code = source.split("#[cfg(test)]").next().unwrap_or_default();
        for line in code.lines().filter(|l| !l.trim_start().starts_with("//")) {
            assert!(!line.contains("cfg"), "a cfg in lifecycle source: {line}");
            for mac in ["!(", "![", "! {", "!{"] {
                assert!(!line.contains(mac), "a macro in lifecycle source: {line}");
            }
        }
    }

    #[test]
    fn catalog_matches_hook_points() {
        use std::collections::BTreeSet;

        let catalog: serde_json::Value =
            serde_json::from_str(include_str!("../../../../native-hooks.json")).unwrap();
        let hooks = catalog["hooks"].as_array().unwrap();
        let source = include_str!("native_hooks.rs");
        let section = source
            .split("/// The hook points. Each does nothing in Horizon.")
            .nth(1)
            .unwrap()
            .split("/// Helpers for native mods")
            .next()
            .unwrap();
        let declared: BTreeSet<_> = section
            .lines()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix("pub(super) fn native_")?
                    .split('(')
                    .next()
            })
            .collect();
        let mut names = BTreeSet::new();
        let mut contracts = BTreeSet::new();
        for hook in hooks {
            let name = hook["name"].as_str().unwrap();
            let version = hook["version"].as_u64().unwrap();
            assert!(
                version > 0 && contracts.insert((name, version)),
                "invalid or duplicate hook: {name} v{version}"
            );
            names.insert(name);
            assert_eq!(
                hook["target"].as_str().unwrap(),
                format!("horizon_client::game::native_hooks::Game<'_>::native_{name}")
            );
        }
        assert_eq!(names, declared);
    }

    /// The parts of the native API (`NATIVE_MODULES`) and the catalog's `modules` list the same
    /// names at the same versions, and the hooks and helpers they name exist.
    #[test]
    fn the_api_parts_match_the_catalog() {
        let catalog: serde_json::Value =
            serde_json::from_str(include_str!("../../../../native-hooks.json")).unwrap();
        let source = include_str!("native_hooks.rs");
        let listed: Vec<(&str, u64)> = catalog["modules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| (m["name"].as_str().unwrap(), m["version"].as_u64().unwrap()))
            .collect();
        let coded: Vec<(&str, u64)> = super::NATIVE_MODULES
            .iter()
            .map(|(name, version)| (*name, u64::from(*version)))
            .collect();
        assert_eq!(listed, coded);
        for module in catalog["modules"].as_array().unwrap() {
            for key in ["hooks", "helpers"] {
                for name in module[key].as_array().into_iter().flatten() {
                    let name = name.as_str().unwrap();
                    assert!(
                        source.contains(&format!("fn native_{name}(")),
                        "{key} {name} of {}",
                        module["name"]
                    );
                }
            }
        }
    }

    /// Vanilla's F3 draws a column in each top corner (`extractLines`, left
    /// and right aligned), and its charts along the bottom.
    #[test]
    fn the_debug_screen_takes_both_top_corners_and_its_charts_corners() {
        use super::{Corner, debug_takes};
        for corner in Corner::ALL {
            assert!(!debug_takes(corner, false, [true, true]), "{corner:?}");
        }
        assert!(debug_takes(Corner::TopLeft, true, [false, false]));
        assert!(debug_takes(Corner::TopRight, true, [false, false]));
        assert!(!debug_takes(Corner::BottomLeft, true, [false, false]));
        assert!(!debug_takes(Corner::BottomRight, true, [false, false]));
        assert!(debug_takes(Corner::BottomLeft, true, [true, false]));
        assert!(!debug_takes(Corner::BottomRight, true, [true, false]));
        assert!(debug_takes(Corner::BottomRight, true, [false, true]));
    }
}
