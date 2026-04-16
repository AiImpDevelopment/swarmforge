// SPDX-License-Identifier: Elastic-2.0
//! Static data -- Zones, Recipes, Action mapping, Evolution paths, Shop items.

use chrono::Utc;

use super::types::*;
use super::swarm_types::*;
use super::colony_types::*;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("forge_quest::static_data", "Static Data");

/// XP needed to reach a given level: 150 * level^1.6
pub(crate) fn xp_for_level(level: u32) -> u64 {
    (150.0 * (level as f64).powf(1.6)) as u64
}

pub(crate) fn map_action_to_rpg(action: &str) -> RpgReward {
    match action {
        "create_document" => RpgReward { xp: 25, gold: 10, material: Some("Parchment".to_string()), monster_fight: false },
        "run_workflow" => RpgReward { xp: 50, gold: 25, material: Some("Gear".to_string()), monster_fight: true },
        "ai_query" => RpgReward { xp: 15, gold: 5, material: Some("Crystal".to_string()), monster_fight: false },
        "send_email" => RpgReward { xp: 10, gold: 8, material: None, monster_fight: false },
        "create_spreadsheet" => RpgReward { xp: 30, gold: 15, material: Some("Iron Ore".to_string()), monster_fight: false },
        "social_post" => RpgReward { xp: 20, gold: 12, material: Some("Song Scroll".to_string()), monster_fight: false },
        "team_contribution" => RpgReward { xp: 35, gold: 20, material: Some("Banner".to_string()), monster_fight: false },
        "complete_quest" => RpgReward { xp: 100, gold: 50, material: Some("Quest Token".to_string()), monster_fight: true },
        "create_note" => RpgReward { xp: 20, gold: 8, material: Some("Parchment".to_string()), monster_fight: false },
        "create_slide" => RpgReward { xp: 20, gold: 10, material: Some("Canvas".to_string()), monster_fight: false },
        "import_file" => RpgReward { xp: 10, gold: 5, material: None, monster_fight: false },
        _ => RpgReward { xp: 5, gold: 2, material: None, monster_fight: false },
    }
}

pub(crate) fn all_zones() -> Vec<Zone> {
    vec![
        Zone {
            id: "beginners_meadow".into(), name: "Beginner's Meadow".into(),
            description: "A peaceful field where novice adventurers hone their skills.".into(),
            level_min: 1, level_max: 5,
            monsters: vec![
                monster("Slime", 1, 20, 3, 1, 8, 5, vec![("Slime Gel", 0.5)]),
                monster("Rat", 2, 25, 5, 2, 12, 6, vec![("Rat Tail", 0.4)]),
                monster("Goblin", 4, 40, 8, 3, 20, 10, vec![("Goblin Ear", 0.35), ("Rusty Dagger", 0.1)]),
            ],
            boss: Some(monster("Goblin Chief", 5, 80, 14, 6, 50, 30, vec![("Chief's Crown", 0.5), ("Goblin Blade", 0.25)])),
            unlock_condition: "Start here".into(),
        },
        Zone {
            id: "dark_forest".into(), name: "Dark Forest".into(),
            description: "Ancient trees block the sunlight. Beware the creatures within.".into(),
            level_min: 5, level_max: 10,
            monsters: vec![
                monster("Wolf", 5, 50, 12, 5, 25, 12, vec![("Wolf Pelt", 0.45)]),
                monster("Spider", 7, 45, 14, 4, 30, 15, vec![("Spider Silk", 0.5), ("Venom Sac", 0.15)]),
                monster("Bandit", 9, 65, 16, 8, 40, 20, vec![("Stolen Coin", 0.6), ("Bandit Mask", 0.1)]),
            ],
            boss: Some(monster("Forest Wraith", 10, 120, 22, 10, 80, 50, vec![("Wraith Essence", 0.5), ("Shadow Cloak", 0.2)])),
            unlock_condition: "Reach level 5".into(),
        },
        Zone {
            id: "crystal_cave".into(), name: "Crystal Cave".into(),
            description: "Glittering caves filled with magical crystals and their guardians.".into(),
            level_min: 10, level_max: 15,
            monsters: vec![
                monster("Cave Bat", 10, 55, 15, 6, 35, 18, vec![("Bat Wing", 0.5)]),
                monster("Stone Golem", 12, 100, 20, 18, 50, 25, vec![("Golem Core", 0.3), ("Stone Shard", 0.5)]),
                monster("Crystal Elemental", 14, 80, 22, 12, 55, 30, vec![("Pure Crystal", 0.35), ("Elemental Spark", 0.15)]),
            ],
            boss: Some(monster("Crystal Dragon", 15, 180, 30, 20, 120, 80, vec![("Dragon Scale", 0.5), ("Crystal Heart", 0.15)])),
            unlock_condition: "Reach level 10".into(),
        },
        Zone {
            id: "dragons_peak".into(), name: "Dragon's Peak".into(),
            description: "The mountain summit where drakes and wyverns nest.".into(),
            level_min: 15, level_max: 20,
            monsters: vec![
                monster("Drake", 15, 90, 25, 14, 60, 35, vec![("Drake Fang", 0.4)]),
                monster("Wyvern", 17, 110, 28, 16, 70, 40, vec![("Wyvern Wing", 0.35), ("Sky Gem", 0.1)]),
                monster("Fire Dragon", 19, 140, 32, 20, 85, 50, vec![("Dragon Flame", 0.3), ("Fire Ruby", 0.08)]),
            ],
            boss: Some(monster("Elder Dragon", 20, 250, 40, 28, 200, 120, vec![("Elder Scale", 0.5), ("Dragon Heart", 0.1)])),
            unlock_condition: "Reach level 15".into(),
        },
        Zone {
            id: "shadow_realm".into(), name: "Shadow Realm".into(),
            description: "A dimension of darkness where fallen warriors dwell.".into(),
            level_min: 20, level_max: 30,
            monsters: vec![
                monster("Shadow Knight", 22, 130, 35, 22, 90, 55, vec![("Shadow Steel", 0.35)]),
                monster("Lich", 25, 100, 40, 15, 110, 65, vec![("Phylactery Shard", 0.2), ("Death Rune", 0.3)]),
                monster("Demon", 28, 160, 45, 25, 130, 80, vec![("Demon Horn", 0.25), ("Infernal Gem", 0.08)]),
            ],
            boss: Some(monster("Shadow Lord", 30, 350, 55, 35, 300, 200, vec![("Shadow Crown", 0.4), ("Void Fragment", 0.1)])),
            unlock_condition: "Reach level 20".into(),
        },
        Zone {
            id: "forge_of_legends".into(), name: "Forge of Legends".into(),
            description: "An ancient workshop where legendary weapons were born.".into(),
            level_min: 30, level_max: 40,
            monsters: vec![
                monster("Forge Golem", 32, 200, 48, 35, 150, 90, vec![("Legendary Ingot", 0.25)]),
                monster("Flame Spirit", 35, 150, 55, 20, 170, 100, vec![("Eternal Flame", 0.2)]),
                monster("Iron Colossus", 38, 280, 52, 45, 190, 120, vec![("Colossus Plate", 0.15)]),
            ],
            boss: Some(monster("Ancient Forge Guardian", 40, 500, 65, 50, 400, 300, vec![("Guardian Hammer", 0.3), ("Forge Heart", 0.08)])),
            unlock_condition: "Reach level 30".into(),
        },
        Zone {
            id: "celestial_tower".into(), name: "Celestial Tower".into(),
            description: "A tower that pierces the heavens, guarded by celestial beings.".into(),
            level_min: 40, level_max: 50,
            monsters: vec![
                monster("Cloud Serpent", 42, 220, 58, 30, 200, 130, vec![("Cloud Pearl", 0.2)]),
                monster("Thunder Titan", 45, 300, 65, 40, 250, 160, vec![("Thunder Core", 0.15)]),
                monster("Star Warden", 48, 260, 70, 35, 280, 180, vec![("Star Fragment", 0.12)]),
            ],
            boss: Some(monster("Sky Emperor", 50, 700, 80, 55, 500, 400, vec![("Emperor's Crown", 0.25), ("Celestial Gem", 0.05)])),
            unlock_condition: "Reach level 40".into(),
        },
        Zone {
            id: "void_abyss".into(), name: "Void Abyss".into(),
            description: "The edge of existence. Reality itself frays here.".into(),
            level_min: 50, level_max: 75,
            monsters: vec![
                monster("Void Walker", 55, 350, 75, 45, 350, 220, vec![("Void Shard", 0.2)]),
                monster("Reality Breaker", 65, 450, 90, 55, 450, 300, vec![("Reality Tear", 0.1)]),
                monster("Entropy Beast", 72, 550, 100, 60, 550, 380, vec![("Entropy Core", 0.08)]),
            ],
            boss: Some(monster("Void Devourer", 75, 1200, 120, 75, 800, 600, vec![("Devourer Fang", 0.2), ("Void Orb", 0.03)])),
            unlock_condition: "Reach level 50".into(),
        },
        Zone {
            id: "eternal_citadel".into(), name: "Eternal Citadel".into(),
            description: "The fortress of the immortal king. Only the worthy may enter.".into(),
            level_min: 75, level_max: 99,
            monsters: vec![
                monster("Eternal Sentinel", 78, 600, 110, 70, 600, 400, vec![("Sentinel Core", 0.15)]),
                monster("Time Weaver", 85, 500, 130, 50, 700, 500, vec![("Time Crystal", 0.08)]),
                monster("Immortal Knight", 92, 800, 140, 90, 850, 600, vec![("Immortal Steel", 0.05)]),
            ],
            boss: Some(monster("Eternal King", 99, 2000, 180, 100, 1500, 1000, vec![("Eternal Crown", 0.15), ("King's Soul", 0.02)])),
            unlock_condition: "Reach level 75".into(),
        },
        Zone {
            id: "the_final_forge".into(), name: "The Final Forge".into(),
            description: "The heart of ImpForge itself. Here, productivity becomes legend.".into(),
            level_min: 99, level_max: 100,
            monsters: vec![
                monster("Compile Error", 99, 999, 150, 80, 1000, 500, vec![("Debug Token", 0.5)]),
                monster("Merge Conflict", 99, 888, 160, 70, 1000, 500, vec![("Resolution Gem", 0.4)]),
            ],
            boss: Some(monster("ImpForge Itself", 100, 5000, 200, 120, 5000, 3000, vec![("Mythic Forge Hammer", 0.1), ("ImpForge Core", 0.01)])),
            unlock_condition: "Reach level 99".into(),
        },
        // -- Expanded zones (10) ---
        Zone {
            id: "fungal_wastes".into(), name: "Fungal Wastes".into(),
            description: "A rotting landscape where colossal fungi release hallucinogenic spores.".into(),
            level_min: 25, level_max: 35,
            monsters: vec![
                monster("Spore Beast", 26, 150, 30, 18, 100, 60, vec![("Spore Sac", 0.4)]),
                monster("Fungal Horror", 30, 180, 35, 22, 130, 80, vec![("Mycelium Thread", 0.35), ("Fungal Core", 0.1)]),
                monster("Mycoid Titan", 34, 240, 42, 28, 170, 100, vec![("Titan Spore", 0.2)]),
            ],
            boss: Some(monster("Elder Mycoid", 35, 400, 50, 35, 280, 180, vec![("Ancient Spore Heart", 0.3), ("Mycoid Crown", 0.08)])),
            unlock_condition: "Reach level 25".into(),
        },
        Zone {
            id: "ice_moon_alpha".into(), name: "Ice Moon Alpha".into(),
            description: "A frozen satellite where crystalline creatures lurk beneath the ice.".into(),
            level_min: 30, level_max: 40,
            monsters: vec![
                monster("Frost Worm", 31, 170, 32, 20, 120, 70, vec![("Frozen Fang", 0.4)]),
                monster("Crystal Spider", 35, 160, 38, 16, 150, 90, vec![("Ice Crystal", 0.35), ("Spider Ice Silk", 0.15)]),
                monster("Ice Colossus", 39, 300, 45, 35, 200, 120, vec![("Colossus Shard", 0.2)]),
            ],
            boss: Some(monster("Glacier Wyrm", 40, 500, 58, 40, 350, 220, vec![("Wyrm Frost Heart", 0.25), ("Glacial Gem", 0.06)])),
            unlock_condition: "Reach level 30".into(),
        },
        Zone {
            id: "volcanic_depths".into(), name: "Volcanic Depths".into(),
            description: "Molten caverns deep beneath a volcanic world. The heat is lethal.".into(),
            level_min: 35, level_max: 45,
            monsters: vec![
                monster("Magma Drake", 36, 200, 42, 25, 160, 95, vec![("Magma Scale", 0.35)]),
                monster("Obsidian Golem", 40, 280, 38, 40, 180, 110, vec![("Obsidian Core", 0.3), ("Volcanic Glass", 0.2)]),
                monster("Fire Elemental", 44, 180, 52, 18, 210, 130, vec![("Elemental Ember", 0.25)]),
            ],
            boss: Some(monster("Pyroclasm Lord", 45, 600, 65, 45, 400, 280, vec![("Pyroclasm Heart", 0.2), ("Eternal Flame Gem", 0.05)])),
            unlock_condition: "Reach level 35".into(),
        },
        Zone {
            id: "nebula_graveyard".into(), name: "Nebula Graveyard".into(),
            description: "A starship graveyard within a dying nebula. Ghostly echoes haunt the wrecks.".into(),
            level_min: 40, level_max: 55,
            monsters: vec![
                monster("Ghost Ship", 42, 250, 50, 30, 220, 140, vec![("Spectral Hull Fragment", 0.3)]),
                monster("Void Phantom", 48, 220, 60, 25, 280, 180, vec![("Phantom Core", 0.2), ("Void Echo", 0.1)]),
                monster("Nebula Serpent", 53, 350, 65, 35, 340, 220, vec![("Nebula Scale", 0.15)]),
            ],
            boss: Some(monster("Dreadnought Revenant", 55, 800, 80, 50, 550, 400, vec![("Revenant Cannon", 0.2), ("Dreadnought Core", 0.04)])),
            unlock_condition: "Reach level 40".into(),
        },
        Zone {
            id: "hive_world_nexus".into(), name: "Hive World Nexus".into(),
            description: "The central nexus of an ancient hive civilization. The walls pulse with life.".into(),
            level_min: 50, level_max: 65,
            monsters: vec![
                monster("Hive Warrior", 52, 320, 60, 35, 300, 200, vec![("Chitin Plate", 0.35)]),
                monster("Bio-Construct", 58, 400, 70, 45, 380, 250, vec![("Living Metal", 0.2)]),
                monster("Synapse Beast", 63, 350, 80, 40, 420, 280, vec![("Synapse Node", 0.15)]),
            ],
            boss: Some(monster("Hive Tyrant", 65, 1000, 95, 60, 650, 500, vec![("Tyrant Claw", 0.25), ("Hive Crown", 0.06)])),
            unlock_condition: "Reach level 50".into(),
        },
        Zone {
            id: "dark_matter_rift".into(), name: "Dark Matter Rift".into(),
            description: "A tear in space-time where dark matter pools into tangible form.".into(),
            level_min: 55, level_max: 70,
            monsters: vec![
                monster("Rift Walker", 57, 380, 70, 40, 350, 230, vec![("Dark Matter Shard", 0.3)]),
                monster("Entropy Wraith", 63, 320, 85, 30, 420, 280, vec![("Entropy Fragment", 0.2)]),
                monster("Gravity Bender", 68, 450, 78, 50, 480, 320, vec![("Gravity Core", 0.12)]),
            ],
            boss: Some(monster("Rift Guardian", 70, 1200, 110, 70, 750, 550, vec![("Guardian Matrix", 0.2), ("Rift Keystone", 0.04)])),
            unlock_condition: "Reach level 55".into(),
        },
        Zone {
            id: "ancient_forge_zone".into(), name: "Ancient Forge".into(),
            description: "Ruins of a forge-god's workshop. Sentient weapons guard the anvils.".into(),
            level_min: 60, level_max: 75,
            monsters: vec![
                monster("Animated Blade", 62, 350, 80, 35, 400, 260, vec![("Sentient Steel", 0.25)]),
                monster("Forge Elemental", 68, 500, 75, 55, 480, 320, vec![("Forge Ember", 0.2)]),
                monster("Anvil Construct", 73, 600, 70, 65, 550, 380, vec![("Construct Plate", 0.15)]),
            ],
            boss: Some(monster("Forge Keeper", 75, 1500, 120, 80, 850, 600, vec![("Keeper's Hammer", 0.2), ("Divine Ingot", 0.03)])),
            unlock_condition: "Reach level 60".into(),
        },
        Zone {
            id: "neural_wasteland".into(), name: "Neural Wasteland".into(),
            description: "A psychic wasteland where thoughts become monsters.".into(),
            level_min: 70, level_max: 85,
            monsters: vec![
                monster("Thought Parasite", 72, 400, 90, 40, 500, 350, vec![("Psionic Crystal", 0.25)]),
                monster("Mind Flayer", 78, 450, 100, 50, 600, 420, vec![("Neural Cortex", 0.18)]),
                monster("Psychic Storm", 83, 380, 120, 35, 700, 480, vec![("Storm Essence", 0.12)]),
            ],
            boss: Some(monster("Psionic Overlord", 85, 1800, 140, 85, 1000, 750, vec![("Overlord Crown", 0.15), ("Psionic Nexus", 0.03)])),
            unlock_condition: "Reach level 70".into(),
        },
        Zone {
            id: "extinction_point".into(), name: "Extinction Point".into(),
            description: "The edge of annihilation. Only the strongest survive here.".into(),
            level_min: 85, level_max: 95,
            monsters: vec![
                monster("Apocalypse Herald", 87, 700, 130, 70, 800, 550, vec![("Herald Sigil", 0.2)]),
                monster("Extinction Beast", 91, 800, 145, 80, 900, 650, vec![("Extinction Fang", 0.15)]),
                monster("Omega Sentinel", 94, 900, 155, 90, 1000, 750, vec![("Omega Core", 0.1)]),
            ],
            boss: Some(monster("World Devourer", 95, 2500, 175, 100, 1400, 1000, vec![("Devourer Maw", 0.15), ("World Seed", 0.02)])),
            unlock_condition: "Reach level 85".into(),
        },
        Zone {
            id: "origin_core".into(), name: "Origin Core".into(),
            description: "The primordial core where the first swarm awakened. Time has no meaning here.".into(),
            level_min: 95, level_max: 100,
            monsters: vec![
                monster("Primordial Spawn", 96, 1000, 160, 95, 1200, 800, vec![("Primordial Essence", 0.2)]),
                monster("Genesis Construct", 98, 1200, 170, 100, 1400, 900, vec![("Genesis Shard", 0.12)]),
            ],
            boss: Some(monster("The First Swarm", 100, 4000, 195, 115, 4000, 2500, vec![("First Swarm Core", 0.1), ("Origin Spark", 0.01)])),
            unlock_condition: "Reach level 95".into(),
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn monster(
    name: &str, level: u32, hp: u32, attack: u32, defense: u32,
    xp: u64, gold: u64, loot: Vec<(&str, f32)>,
) -> Monster {
    Monster {
        name: name.to_string(),
        level, hp, attack, defense,
        xp_reward: xp, gold_reward: gold,
        loot_table: loot.into_iter().map(|(n, c)| (n.to_string(), c)).collect(),
    }
}

pub(crate) fn all_recipes() -> Vec<CraftingRecipe> {
    vec![
        CraftingRecipe {
            id: "recipe_iron_sword".into(), name: "Iron Sword".into(),
            result_item_id: "iron_sword".into(),
            materials: vec![("Iron Ore".into(), 3), ("Gear".into(), 1)],
            required_level: 3,
        },
        CraftingRecipe {
            id: "recipe_crystal_staff".into(), name: "Crystal Staff".into(),
            result_item_id: "crystal_staff".into(),
            materials: vec![("Crystal".into(), 5), ("Parchment".into(), 2)],
            required_level: 5,
        },
        CraftingRecipe {
            id: "recipe_leather_armor".into(), name: "Leather Armor".into(),
            result_item_id: "leather_armor".into(),
            materials: vec![("Wolf Pelt".into(), 3), ("Banner".into(), 1)],
            required_level: 6,
        },
        CraftingRecipe {
            id: "recipe_spider_bow".into(), name: "Spider Silk Bow".into(),
            result_item_id: "spider_bow".into(),
            materials: vec![("Spider Silk".into(), 4), ("Gear".into(), 2)],
            required_level: 8,
        },
        CraftingRecipe {
            id: "recipe_golem_shield".into(), name: "Golem Shield".into(),
            result_item_id: "golem_shield".into(),
            materials: vec![("Stone Shard".into(), 3), ("Golem Core".into(), 1)],
            required_level: 12,
        },
        CraftingRecipe {
            id: "recipe_dragon_blade".into(), name: "Dragon Blade".into(),
            result_item_id: "dragon_blade".into(),
            materials: vec![("Dragon Scale".into(), 2), ("Iron Ore".into(), 5), ("Gear".into(), 3)],
            required_level: 16,
        },
        CraftingRecipe {
            id: "recipe_shadow_cloak".into(), name: "Shadow Cloak".into(),
            result_item_id: "shadow_cloak".into(),
            materials: vec![("Shadow Steel".into(), 2), ("Wraith Essence".into(), 1)],
            required_level: 22,
        },
        CraftingRecipe {
            id: "recipe_healing_potion".into(), name: "Healing Potion".into(),
            result_item_id: "healing_potion".into(),
            materials: vec![("Slime Gel".into(), 2), ("Crystal".into(), 1)],
            required_level: 1,
        },
    ]
}

pub(crate) fn generate_item_from_recipe(recipe: &CraftingRecipe, crafter_level: u32) -> Item {
    let bonus = (crafter_level as i32 - recipe.required_level as i32).max(0);
    let (atk, def, mag, hp, itype) = match recipe.result_item_id.as_str() {
        "iron_sword" => (12 + bonus, 0, 0, 0, "weapon"),
        "crystal_staff" => (3, 0, 15 + bonus, 0, "weapon"),
        "leather_armor" => (0, 10 + bonus, 0, 20, "armor"),
        "spider_bow" => (14 + bonus, 0, 2, 0, "weapon"),
        "golem_shield" => (0, 18 + bonus, 0, 30, "armor"),
        "dragon_blade" => (25 + bonus, 0, 5, 0, "weapon"),
        "shadow_cloak" => (0, 12, 10 + bonus, 15, "accessory"),
        "healing_potion" => (0, 0, 0, 50, "potion"),
        _ => (5, 5, 5, 10, "weapon"),
    };

    let rarity = if bonus > 15 {
        ItemRarity::Epic
    } else if bonus > 8 {
        ItemRarity::Rare
    } else if bonus > 3 {
        ItemRarity::Uncommon
    } else {
        ItemRarity::Common
    };

    Item {
        id: format!("crafted_{}_{}", recipe.result_item_id, Utc::now().timestamp_millis()),
        name: recipe.name.clone(),
        item_type: itype.to_string(),
        rarity,
        stats: ItemStats { attack: atk, defense: def, magic: mag, hp_bonus: hp },
        level_req: recipe.required_level,
        description: format!("Crafted at the forge (level {crafter_level})."),
    }
}

// ---------------------------------------------------------------------------
// Swarm static data: Evolution paths, resource mapping
// ---------------------------------------------------------------------------

pub(crate) fn all_evolution_paths() -> Vec<EvolutionPath> {
    vec![
        // Tier 1 -> Tier 2
        EvolutionPath {
            from: "forge_drone".into(), to: "viper".into(),
            essence_cost: 200, level_requirement: 15,
            materials: vec![("Crystal".into(), 3)],
        },
        EvolutionPath {
            from: "forge_drone".into(), to: "shadow_weaver".into(),
            essence_cost: 200, level_requirement: 15,
            materials: vec![("Shadow Steel".into(), 2)],
        },
        EvolutionPath {
            from: "imp_scout".into(), to: "skyweaver".into(),
            essence_cost: 200, level_requirement: 15,
            materials: vec![("Cloud Pearl".into(), 2)],
        },
        EvolutionPath {
            from: "imp_scout".into(), to: "overseer".into(),
            essence_cost: 200, level_requirement: 15,
            materials: vec![("Golem Core".into(), 2)],
        },
        // Tier 2 -> Tier 3
        EvolutionPath {
            from: "viper".into(), to: "titan".into(),
            essence_cost: 500, level_requirement: 30,
            materials: vec![("Dragon Scale".into(), 3), ("Legendary Ingot".into(), 2)],
        },
        EvolutionPath {
            from: "viper".into(), to: "swarm_mother".into(),
            essence_cost: 500, level_requirement: 30,
            materials: vec![("Wraith Essence".into(), 3)],
        },
        EvolutionPath {
            from: "skyweaver".into(), to: "ravager".into(),
            essence_cost: 500, level_requirement: 30,
            materials: vec![("Demon Horn".into(), 2), ("Infernal Gem".into(), 1)],
        },
        EvolutionPath {
            from: "shadow_weaver".into(), to: "titan".into(),
            essence_cost: 500, level_requirement: 30,
            materials: vec![("Void Shard".into(), 2)],
        },
        EvolutionPath {
            from: "overseer".into(), to: "swarm_mother".into(),
            essence_cost: 500, level_requirement: 30,
            materials: vec![("Neural Fragment".into(), 3)],
        },
        // Tier 3 -> Tier 4 (Matriarch, only 1 allowed)
        EvolutionPath {
            from: "titan".into(), to: "matriarch".into(),
            essence_cost: 2000, level_requirement: 50,
            materials: vec![("Mythic Core".into(), 1), ("Void Orb".into(), 1)],
        },
        EvolutionPath {
            from: "swarm_mother".into(), to: "matriarch".into(),
            essence_cost: 2000, level_requirement: 50,
            materials: vec![("Mythic Core".into(), 1), ("Eternal Crown".into(), 1)],
        },
        EvolutionPath {
            from: "ravager".into(), to: "matriarch".into(),
            essence_cost: 2000, level_requirement: 50,
            materials: vec![("Mythic Core".into(), 1), ("King's Soul".into(), 1)],
        },
    ]
}

pub(crate) fn swarm_resources_for_action(action: &str) -> SwarmResources {
    match action {
        "create_document" => SwarmResources { essence: 10, minerals: 0, vespene: 0, biomass: 5, dark_matter: 0 },
        "run_workflow" => SwarmResources { essence: 20, minerals: 0, vespene: 0, biomass: 0, dark_matter: 3 },
        "ai_query" => SwarmResources { essence: 5, minerals: 0, vespene: 3, biomass: 0, dark_matter: 0 },
        "create_spreadsheet" => SwarmResources { essence: 15, minerals: 8, vespene: 0, biomass: 0, dark_matter: 0 },
        "send_email" => SwarmResources { essence: 5, minerals: 0, vespene: 0, biomass: 0, dark_matter: 0 },
        "social_post" => SwarmResources { essence: 8, minerals: 0, vespene: 0, biomass: 2, dark_matter: 0 },
        "team_contribution" => SwarmResources { essence: 12, minerals: 0, vespene: 0, biomass: 0, dark_matter: 0 },
        "create_note" => SwarmResources { essence: 8, minerals: 0, vespene: 0, biomass: 3, dark_matter: 0 },
        "create_slide" => SwarmResources { essence: 10, minerals: 2, vespene: 0, biomass: 0, dark_matter: 0 },
        "import_file" => SwarmResources { essence: 5, minerals: 3, vespene: 0, biomass: 0, dark_matter: 0 },
        "complete_quest" => SwarmResources { essence: 30, minerals: 5, vespene: 5, biomass: 5, dark_matter: 2 },
        _ => SwarmResources { essence: 2, minerals: 0, vespene: 0, biomass: 0, dark_matter: 0 },
    }
}

// ---------------------------------------------------------------------------
// OGame Shop items (Dark Matter — NO microtransactions, earned in-game only!)
// ---------------------------------------------------------------------------

pub(crate) fn all_shop_items() -> Vec<ShopItem> {
    vec![
        ShopItem {
            id: "boost_production_25".into(),
            name: "+25% Production (24h)".into(),
            description: "All resource production increased by 25% for 24 hours.".into(),
            cost_dark_matter: 50,
            effect: ShopEffect::ProductionBoost(0.25),
            duration_hours: Some(24),
        },
        ShopItem {
            id: "boost_research_30".into(),
            name: "-30% Research Time (24h)".into(),
            description: "All research completes 30% faster for 24 hours.".into(),
            cost_dark_matter: 75,
            effect: ShopEffect::ResearchSpeed(0.30),
            duration_hours: Some(24),
        },
        ShopItem {
            id: "boost_build_30".into(),
            name: "-30% Build Time (24h)".into(),
            description: "All building upgrades complete 30% faster for 24 hours.".into(),
            cost_dark_matter: 75,
            effect: ShopEffect::BuildSpeed(0.30),
            duration_hours: Some(24),
        },
        ShopItem {
            id: "boost_fleet_50".into(),
            name: "+50% Fleet Speed (12h)".into(),
            description: "Fleet travels 50% faster for 12 hours.".into(),
            cost_dark_matter: 40,
            effect: ShopEffect::FleetSpeed(0.50),
            duration_hours: Some(12),
        },
        ShopItem {
            id: "extra_queue_perm".into(),
            name: "Extra Build Queue (Permanent)".into(),
            description: "Build two things at once, forever. A must-have upgrade.".into(),
            cost_dark_matter: 500,
            effect: ShopEffect::ExtraQueue,
            duration_hours: None,
        },
        ShopItem {
            id: "boost_creep_50".into(),
            name: "+50% Creep Spread (48h)".into(),
            description: "Creep spreads 50% faster for 48 hours.".into(),
            cost_dark_matter: 60,
            effect: ShopEffect::CreepBoost(0.50),
            duration_hours: Some(48),
        },
        ShopItem {
            id: "boost_production_50".into(),
            name: "+50% Production (8h)".into(),
            description: "All resource production increased by 50% for 8 hours. Short but powerful.".into(),
            cost_dark_matter: 30,
            effect: ShopEffect::ProductionBoost(0.50),
            duration_hours: Some(8),
        },
    ]
}

