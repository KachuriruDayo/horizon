use super::*;
use crate::nbt::parse_snbt;

fn snbt(s: &str) -> Tag {
    parse_snbt(s).unwrap().0
}

#[test]
fn current_data_is_left_alone() {
    let mut tag = snbt("{DataVersion: 5023, Status: \"minecraft:full\"}");
    let before = tag.clone();
    update_chunk(&mut tag, "minecraft:overworld", Some("minecraft:noise"));
    assert_eq!(tag, before);
}

#[test]
fn item_stacks_of_1_20_take_components() {
    let mut player = snbt(
        "{DataVersion: 3465, Inventory: [{Slot: 0b, id: \"minecraft:diamond_sword\", Count: 1b, \
         tag: {Damage: 5, display: {Name: '{\"text\":\"Blade\"}'}}}]}",
    );
    update_player(&mut player);
    let stack = &dynamic::stream(player.get("Inventory"))[0];
    assert_eq!(
        dynamic::as_str(stack.get("id")).as_deref(),
        Some("minecraft:diamond_sword")
    );
    assert_eq!(stack.get("count"), Some(&Tag::Int(1)));
    let components = stack.get("components").expect("components");
    assert_eq!(components.get("minecraft:damage"), Some(&Tag::Int(5)));
    assert_eq!(
        components.get("minecraft:custom_name"),
        Some(&Tag::compound([("text", dynamic::string("Blade"))]))
    );
}

#[test]
fn level_data_of_1_20_moves_to_the_new_fields() {
    let mut level = snbt(
        "{DataVersion: 3465, Difficulty: 3b, DifficultyLocked: 1b, hardcore: 0b, SpawnX: 1, SpawnY: 64, SpawnZ: -3, \
         SpawnAngle: 90.0f, GameRules: {doFireTick: \"false\", keepInventory: \"true\", randomTickSpeed: \"5\"}}",
    );
    update(Ty::Level, &mut level, 3465, CURRENT);
    let settings = level
        .get("difficulty_settings")
        .expect("difficulty settings");
    assert_eq!(
        dynamic::as_str(settings.get("difficulty")).as_deref(),
        Some("hard")
    );
    assert_eq!(settings.get("locked"), Some(&Tag::Byte(1)));
    let rules = level.get("game_rules").expect("game rules");
    assert_eq!(rules.get("minecraft:keep_inventory"), Some(&Tag::Byte(1)));
    assert_eq!(rules.get("minecraft:random_tick_speed"), Some(&Tag::Int(5)));
    assert_eq!(
        rules.get("minecraft:fire_spread_radius_around_player"),
        Some(&Tag::Int(0))
    );
    assert_eq!(
        level.get("spawn").and_then(|s| s.get("pos")),
        Some(&Tag::IntArray(vec![1, 64, -3]))
    );
}

#[test]
fn block_states_take_their_new_field_names() {
    let mut chunk = snbt(
        "{DataVersion: 4903, Status: \"minecraft:full\", sections: [{Y: 0b, block_states: \
         {palette: [{Name: \"minecraft:chain\", Properties: {axis: \"y\"}}]}}]}",
    );
    update_chunk(&mut chunk, "minecraft:the_nether", Some("minecraft:noise"));
    let section = &dynamic::stream(chunk.get("sections"))[0];
    let state = &dynamic::stream(section.get("block_states").and_then(|b| b.get("palette")))[0];
    assert_eq!(
        dynamic::as_str(state.get("id")).as_deref(),
        Some("minecraft:chain")
    );
    assert!(state.get("properties").is_some());
    assert_eq!(chunk.get("DataVersion"), Some(&Tag::Int(CURRENT)));
}

#[test]
fn a_structure_template_of_1_20_is_upgraded_like_a_chunk() {
    let mut tag = snbt(
        "{DataVersion: 3465, size: [1, 1, 2], \
         palette: [{Name: \"minecraft:grass\"}, {Name: \"minecraft:chest\", Properties: {facing: \"north\"}}], \
         blocks: [{pos: [0, 0, 0], state: 0}, \
                  {pos: [0, 0, 1], state: 1, nbt: {id: \"minecraft:chest\", \
                     Items: [{Slot: 0b, id: \"minecraft:grass\", Count: 2b}]}}], \
         entities: [{pos: [0.5d, 0.0d, 0.5d], blockPos: [0, 0, 0], \
                     nbt: {id: \"minecraft:item_frame\", Item: {id: \"minecraft:grass\", Count: 1b}}}]}",
    );
    update_structure(&mut tag);
    assert_eq!(tag.get("DataVersion"), Some(&Tag::Int(CURRENT)));
    let palette = dynamic::stream(tag.get("palette"));
    // 1.20.3 renamed the grass plant to short grass: block names and item ids alike. Block
    // states take `id` and `properties` (as in chunks).
    assert_eq!(
        dynamic::as_str(palette[0].get("id")).as_deref(),
        Some("minecraft:short_grass")
    );
    assert!(palette[1].get("properties").is_some());
    let blocks = dynamic::stream(tag.get("blocks"));
    let items = dynamic::stream(blocks[1].get("nbt").and_then(|n| n.get("Items")));
    assert_eq!(
        dynamic::as_str(items[0].get("id")).as_deref(),
        Some("minecraft:short_grass")
    );
    // 1.20.5's item stacks: `count`, not `Count`.
    assert_eq!(items[0].get("count"), Some(&Tag::Int(2)));
    let entities = dynamic::stream(tag.get("entities"));
    let frame = entities[0].get("nbt").expect("the entity's data");
    let held = frame.get("item").or_else(|| frame.get("Item")).expect("the held item");
    assert_eq!(
        dynamic::as_str(held.get("id")).as_deref(),
        Some("minecraft:short_grass")
    );
}

#[test]
fn a_current_structure_template_is_left_alone() {
    let mut tag = snbt("{DataVersion: 5023, size: [1, 1, 1], palette: [{Name: \"minecraft:grass\"}], blocks: []}");
    let before = tag.clone();
    update_structure(&mut tag);
    assert_eq!(tag, before);
}
