// SPDX-License-Identifier: Elastic-2.0
//! Tauri commands -- thin wrappers around ForgeQuestEngine.

use crate::error::ImpForgeError;

use super::types::*;
use super::evosys::GoverningAttributes;
use super::swarm_types::*;
use super::mutations::*;
use super::colony_types::*;
use super::engine::ForgeQuestEngine;
use super::static_data::*;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("forge_quest::commands", "Tauri Commands");

#[tauri::command]
pub async fn quest_create_character(
    name: String,
    class: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<QuestCharacter, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("forge_quest", "quest", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "forge_quest", "quest");
    crate::synapse_fabric::synapse_session_push("forge_quest", "quest", "quest_create_character called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "forge_quest", "info", "forge_quest active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "office", true, 0);

    engine.create_character(&name, &class)
}

#[tauri::command]
pub async fn quest_get_character(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<QuestCharacter, ImpForgeError> {
    engine.get_character()
}

#[tauri::command]
pub async fn quest_track_action(
    action: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<ActionResult, ImpForgeError> {
    crate::cortex_wiring::cortex_event(
        "forge_quest", "progress",
        crate::cortex_wiring::EventCategory::Creative,
        serde_json::json!({"action": action}),
    );
    engine.track_action(&action)
}

#[tauri::command]
pub async fn quest_auto_battle(
    zone_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<BattleResult, ImpForgeError> {
    engine.auto_battle(&zone_id)
}

#[tauri::command]
pub async fn quest_craft_item(
    recipe_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Item, ImpForgeError> {
    engine.craft_item(&recipe_id)
}

#[tauri::command]
pub async fn quest_equip_item(
    item_id: String,
    slot: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<(), ImpForgeError> {
    engine.equip_item(&item_id, &slot)
}

#[tauri::command]
pub async fn quest_unequip(
    slot: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<(), ImpForgeError> {
    engine.unequip(&slot)
}

#[tauri::command]
pub async fn quest_invest_skill(
    skill_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Skill, ImpForgeError> {
    engine.invest_skill(&skill_id)
}

#[tauri::command]
pub async fn quest_get_zones() -> Result<Vec<Zone>, ImpForgeError> {
    Ok(all_zones())
}

#[tauri::command]
pub async fn quest_get_quests(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<Quest>, ImpForgeError> {
    engine.get_quests()
}

#[tauri::command]
pub async fn quest_get_recipes() -> Result<Vec<CraftingRecipe>, ImpForgeError> {
    Ok(all_recipes())
}

#[tauri::command]
pub async fn quest_get_leaderboard(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<LeaderboardEntry>, ImpForgeError> {
    engine.get_leaderboard()
}

// ---------------------------------------------------------------------------
// Swarm Tauri Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn quest_get_swarm(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<SwarmState, ImpForgeError> {
    engine.get_swarm()
}

#[tauri::command]
pub async fn quest_spawn_larva(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<SwarmUnit, ImpForgeError> {
    engine.spawn_larva()
}

#[tauri::command]
pub async fn quest_evolve_unit(
    unit_id: String,
    target_type: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<SwarmUnit, ImpForgeError> {
    engine.evolve_unit(&unit_id, &target_type)
}

#[tauri::command]
pub async fn quest_upgrade_building(
    building_type: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Building, ImpForgeError> {
    engine.upgrade_building(&building_type)
}

#[tauri::command]
pub async fn quest_assign_mission(
    mission_id: String,
    unit_ids: Vec<String>,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<SwarmMission, ImpForgeError> {
    engine.assign_mission(&mission_id, unit_ids)
}

#[tauri::command]
pub async fn quest_collect_mission(
    mission_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<MissionReward, ImpForgeError> {
    engine.collect_mission(&mission_id)
}

#[tauri::command]
pub async fn quest_get_missions(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<SwarmMission>, ImpForgeError> {
    engine.get_missions()
}

#[tauri::command]
pub async fn quest_swarm_auto_assign(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<SwarmMission>, ImpForgeError> {
    engine.swarm_auto_assign()
}

// ---------------------------------------------------------------------------
// Swarm Mutation Tauri Commands (4 new)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn swarm_get_mutations(
    unit_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<UnitMutations, ImpForgeError> {
    engine.get_unit_mutations(&unit_id)
}

#[tauri::command]
pub async fn swarm_available_mutations(
    unit_type: String,
    level: u32,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<Mutation>, ImpForgeError> {
    engine.get_available_mutations(&unit_type, level)
}

#[tauri::command]
pub async fn swarm_apply_mutation(
    unit_id: String,
    mutation_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<SwarmUnit, ImpForgeError> {
    engine.apply_mutation(&unit_id, &mutation_id)
}

#[tauri::command]
pub async fn swarm_get_mutation_tree(
    unit_type: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<Vec<Mutation>>, ImpForgeError> {
    engine.get_mutation_tree(&unit_type)
}

// ---------------------------------------------------------------------------
// SwarmForge OGame-style Tauri Commands (12 new)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn swarm_get_planet(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Planet, ImpForgeError> {
    engine.get_planet()
}

#[tauri::command]
pub async fn swarm_upgrade_building(
    building_type: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<PlanetBuilding, ImpForgeError> {
    engine.upgrade_planet_building(&building_type)
}

#[tauri::command]
pub async fn swarm_collect_resources(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<PlanetResources, ImpForgeError> {
    engine.collect_planet_resources()
}

#[tauri::command]
pub async fn swarm_start_research(
    tech: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Research, ImpForgeError> {
    engine.start_research(&tech)
}

#[tauri::command]
pub async fn swarm_build_ships(
    ship_type: String,
    count: u32,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Ship, ImpForgeError> {
    engine.build_ships(&ship_type, count)
}

#[tauri::command]
pub async fn swarm_get_fleet(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<Ship>, ImpForgeError> {
    let planet = engine.get_planet()?;
    Ok(planet.fleet)
}

#[tauri::command]
pub async fn swarm_get_research(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<Research>, ImpForgeError> {
    let planet = engine.get_planet()?;
    Ok(planet.research)
}

#[tauri::command]
pub async fn swarm_get_creep(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<CreepStatus, ImpForgeError> {
    engine.get_creep()
}

#[tauri::command]
pub async fn swarm_shop_items() -> Result<Vec<ShopItem>, ImpForgeError> {
    Ok(all_shop_items())
}

#[tauri::command]
pub async fn swarm_shop_buy(
    item_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<ShopItem, ImpForgeError> {
    engine.buy_shop_item(&item_id)
}

#[tauri::command]
pub async fn swarm_get_galaxy(
    galaxy: u32,
    system: u32,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<PlanetSlot>, ImpForgeError> {
    engine.get_galaxy(galaxy, system)
}

#[tauri::command]
pub async fn swarm_check_timers(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<CompletedTimer>, ImpForgeError> {
    engine.check_timers()
}

// ---------------------------------------------------------------------------
// EvoSys Tauri Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn swarm_unit_attributes(
    unit_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<GoverningAttributes, ImpForgeError> {
    engine.get_unit_attributes(&unit_id)
}

// ---------------------------------------------------------------------------
// Dark Matter Earnings Tauri Commands
// ---------------------------------------------------------------------------

/// Earn Dark Matter from a productivity activity.  Returns new total.
#[tauri::command]
pub async fn swarm_earn_dark_matter(
    source: String,
    _colony_id: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<u32, ImpForgeError> {
    let dm_source = DmSource::from_str(&source).ok_or_else(|| {
        ImpForgeError::validation(
            "DM_BAD_SOURCE",
            format!(
                "Unknown Dark Matter source '{}'. Valid: document_written, code_committed, \
                 spreadsheet_created, email_sent, task_completed, active_usage, tests_passed, \
                 build_succeeded, milestone_reached, daily_login, weekly_challenge.",
                source
            ),
        )
    })?;
    engine.earn_dark_matter_from_source(&dm_source)
}

/// Get recent Dark Matter earning history.
#[tauri::command]
pub async fn swarm_dark_matter_history(
    _colony_id: String,
    limit: u32,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<DarkMatterEarnings>, ImpForgeError> {
    let capped = if limit == 0 { 50 } else { limit.min(500) };
    engine.get_dark_matter_history(capped)
}

/// Get all Dark Matter earning rates (source -> amount mapping).
#[tauri::command]
pub async fn swarm_dark_matter_rates() -> Result<serde_json::Value, ImpForgeError> {
    Ok(serde_json::json!({
        "document_written": DmSource::DocumentWritten.dm_amount(),
        "code_committed": DmSource::CodeCommitted.dm_amount(),
        "spreadsheet_created": DmSource::SpreadsheetCreated.dm_amount(),
        "email_sent": DmSource::EmailSent.dm_amount(),
        "task_completed": DmSource::TaskCompleted.dm_amount(),
        "active_usage": DmSource::ActiveUsage.dm_amount(),
        "tests_passed": DmSource::TestsPassed.dm_amount(),
        "build_succeeded": DmSource::BuildSucceeded.dm_amount(),
        "milestone_reached": DmSource::MilestoneReached.dm_amount(),
        "daily_login": DmSource::DailyLogin.dm_amount(),
        "weekly_challenge": DmSource::WeeklyChallenge.dm_amount(),
        "note": "Dark Matter is ONLY earned through ImpForge productivity. NOT purchasable."
    }))
}

// ---------------------------------------------------------------------------
// Additional Tauri Commands — wiring internal helpers
// ---------------------------------------------------------------------------

/// List all item types and their string forms (weapon subtypes, armor slots, etc.).
#[tauri::command]
pub async fn quest_item_types() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    let weapon_types = [
        WeaponType::Sword, WeaponType::Staff, WeaponType::Bow,
        WeaponType::Hammer, WeaponType::Lute, WeaponType::Tome,
    ];
    let armor_slots = [
        ArmorSlot::Head, ArmorSlot::Chest, ArmorSlot::Legs,
        ArmorSlot::Boots, ArmorSlot::Gloves, ArmorSlot::Shield,
    ];

    let mut result = Vec::new();
    for wt in &weapon_types {
        let it = ItemType::Weapon(wt.clone());
        result.push(serde_json::json!({
            "category": it.as_str(),
            "subtype": format!("{:?}", wt),
        }));
    }
    for slot in &armor_slots {
        let it = ItemType::Armor(slot.clone());
        result.push(serde_json::json!({
            "category": it.as_str(),
            "subtype": format!("{:?}", slot),
        }));
    }
    for it in [ItemType::Accessory, ItemType::Material, ItemType::Potion, ItemType::QuestItem] {
        result.push(serde_json::json!({
            "category": it.as_str(),
            "subtype": null,
        }));
    }
    Ok(result)
}

/// List all quest objective variants.
#[tauri::command]
pub async fn quest_objective_types() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    let objectives = [
        QuestObjective::CreateDocuments(1),
        QuestObjective::RunWorkflows(1),
        QuestObjective::AiQueries(1),
        QuestObjective::SlayMonsters(1),
        QuestObjective::CraftItems(1),
        QuestObjective::ReachLevel(1),
        QuestObjective::CompleteStreak(1),
        QuestObjective::UseModules(1),
    ];
    Ok(objectives
        .iter()
        .map(|o| serde_json::json!({ "type": format!("{:?}", o) }))
        .collect())
}

/// List all skill branches with their string keys.
#[tauri::command]
pub async fn quest_skill_branches() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    let branches = ["combat", "defense", "magic", "crafting", "leadership", "wisdom"];
    Ok(branches
        .iter()
        .map(|s| {
            let branch = SkillBranch::from_str(s);
            serde_json::json!({
                "id": branch.as_str(),
                "name": branch.as_str(),
            })
        })
        .collect())
}

/// List all mutation types with their string keys.
#[tauri::command]
pub async fn quest_mutation_types() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    let types = ["defensive", "offensive", "utility", "evolution", "specialization"];
    Ok(types
        .iter()
        .map(|s| {
            let mt = MutationType::from_str(s);
            serde_json::json!({
                "id": mt.as_str(),
                "name": mt.as_str(),
            })
        })
        .collect())
}

/// Get the faction for a unit type by its string key.
#[tauri::command]
pub async fn quest_unit_faction(unit_type: String) -> Result<serde_json::Value, ImpForgeError> {
    let ut = UnitType::from_str(&unit_type);
    let faction = ut.faction();
    Ok(serde_json::json!({
        "unit_type": ut.as_str(),
        "faction": format!("{:?}", faction),
    }))
}

/// Get ship build time per unit by ship type string.
#[tauri::command]
pub async fn quest_ship_build_time(ship_type: String) -> Result<serde_json::Value, ImpForgeError> {
    let st = ShipType::from_str(&ship_type);
    Ok(serde_json::json!({
        "ship_type": st.as_str(),
        "display_name": st.display_name(),
        "build_time_secs": st.build_time_per_unit(),
    }))
}

/// Browse the Dark Matter shop.
#[tauri::command]
pub async fn quest_shop_browse(
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<Vec<ShopItem>, ImpForgeError> {
    engine.get_shop_items()
}

/// Award Dark Matter for completing in-game actions.
#[tauri::command]
pub async fn quest_award_dark_matter(
    amount: u64,
    reason: String,
    engine: tauri::State<'_, ForgeQuestEngine>,
) -> Result<u64, ImpForgeError> {
    if amount == 0 {
        return Err(ImpForgeError::validation(
            "DM_ZERO",
            "Amount must be greater than zero.",
        ));
    }
    engine.award_dark_matter(amount, &reason)
}
