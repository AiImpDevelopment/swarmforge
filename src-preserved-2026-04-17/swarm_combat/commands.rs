// SPDX-License-Identifier: Elastic-2.0
//! SwarmForge Combat -- All 21 Tauri commands: combat, terrain, defense,
//! fleet movement, battle simulation, siege.

use crate::error::ImpForgeError;

use super::defense::*;
use super::engine::{simulate_battle, SwarmCombatEngine};
use super::terrain::*;
use super::types::*;

/// Health declaration for this sub-module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_combat::commands", "Game");

#[tauri::command]
pub async fn swarm_dispatch_fleet(
    origin: (u32, u32, u32),
    target: (u32, u32, u32),
    ships: Vec<(String, u32)>,
    mission_type: String,
    cargo: Resources,
    engine: tauri::State<'_, SwarmCombatEngine>,
) -> Result<FleetMission, ImpForgeError> {
    engine.dispatch_fleet(origin, target, ships, &mission_type, cargo, 1.0)
}

/// Get current status of a fleet mission.
#[tauri::command]
pub async fn swarm_fleet_status(
    mission_id: String,
    engine: tauri::State<'_, SwarmCombatEngine>,
) -> Result<FleetMission, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_combat", "game_combat", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_combat", "game_combat");
    crate::synapse_fabric::synapse_session_push("swarm_combat", "game_combat", "swarm_fleet_status called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_combat", "info", "swarm_combat active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    engine.get_fleet_status(&mission_id)
}

/// List all active (non-completed) fleet missions.
#[tauri::command]
pub async fn swarm_list_fleets(
    engine: tauri::State<'_, SwarmCombatEngine>,
) -> Result<Vec<FleetMission>, ImpForgeError> {
    engine.list_fleets()
}

/// Recall a fleet that is still outbound.
#[tauri::command]
pub async fn swarm_recall_fleet(
    mission_id: String,
    engine: tauri::State<'_, SwarmCombatEngine>,
) -> Result<FleetMission, ImpForgeError> {
    engine.recall_fleet(&mission_id)
}

/// Preview a battle without persisting (combat simulator).
#[tauri::command]
pub async fn swarm_simulate_combat(
    attacker_ships: Vec<(String, u32)>,
    defender_ships: Vec<(String, u32)>,
) -> Result<BattleResult, ImpForgeError> {
    if attacker_ships.is_empty() || defender_ships.is_empty() {
        return Err(ImpForgeError::validation(
            "COMBAT_EMPTY",
            "Both sides must have at least one ship.",
        ));
    }
    crate::cortex_wiring::cortex_event(
        "swarm_combat", "action",
        crate::cortex_wiring::EventCategory::Creative,
        serde_json::json!({"attackers": attacker_ships.len(), "defenders": defender_ships.len()}),
    );
    Ok(simulate_battle(&attacker_ships, &defender_ships))
}

/// Retrieve a saved battle report by ID.
#[tauri::command]
pub async fn swarm_battle_report(
    battle_id: String,
    engine: tauri::State<'_, SwarmCombatEngine>,
) -> Result<BattleResult, ImpForgeError> {
    engine.get_battle_report(&battle_id)
}

// ---------------------------------------------------------------------------
// Terrain Tauri commands (FactionTerrain + TerrainTile + TerrainEffect live
// in `super::terrain`).
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn swarm_terrain_effects(
    terrain_type: String,
) -> Result<TerrainEffect, ImpForgeError> {
    let terrain = FactionTerrain::from_str(&terrain_type);
    Ok(terrain_get_effect(&terrain))
}

/// Get a terrain map for a colony (returns a 7x7 test grid).
#[tauri::command]
pub async fn swarm_terrain_map(
    colony_id: String,
) -> Result<Vec<TerrainTile>, ImpForgeError> {
    if colony_id.is_empty() {
        return Err(ImpForgeError::validation(
            "TERRAIN_EMPTY_ID",
            "Colony ID must not be empty.",
        ));
    }
    Ok(generate_test_terrain_map(&colony_id))
}

/// Find contested zones (border-war tiles) on a colony's terrain map.
#[tauri::command]
pub async fn swarm_terrain_contested_zones(
    colony_id: String,
) -> Result<Vec<serde_json::Value>, ImpForgeError> {
    if colony_id.is_empty() {
        return Err(ImpForgeError::validation(
            "TERRAIN_EMPTY_ID",
            "Colony ID must not be empty.",
        ));
    }

    let tiles = generate_test_terrain_map(&colony_id);
    let contested = terrain_check_contested(&tiles);

    let result: Vec<serde_json::Value> = contested
        .into_iter()
        .map(|(x, y)| {
            serde_json::json!({
                "x": x,
                "y": y,
                "contested": true
            })
        })
        .collect();

    Ok(result)
}

// ---------------------------------------------------------------------------
// Defense Tauri commands (DefenseType + DefenseStats + DefenseCost live in
// `super::defense`).
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn swarm_defense_stats(defense_type: String) -> Result<DefenseStats, ImpForgeError> {
    let dt = DefenseType::from_str(&defense_type).ok_or_else(|| {
        ImpForgeError::validation(
            "DEFENSE_UNKNOWN_TYPE",
            format!("Unknown defense type: '{defense_type}'"),
        )
        .with_suggestion(
            "Valid types: missile_launcher, light_laser, heavy_laser, gauss_cannon, \
             ion_cannon, plasma_turret, small_shield_dome, large_shield_dome, \
             anti_ballistic, interplanetary_missile",
        )
    })?;

    Ok(defense_stats(&dt))
}

/// Calculate cost for building N defense units.
#[tauri::command]
pub async fn swarm_defense_cost(
    defense_type: String,
    count: u32,
) -> Result<serde_json::Value, ImpForgeError> {
    let dt = DefenseType::from_str(&defense_type).ok_or_else(|| {
        ImpForgeError::validation(
            "DEFENSE_UNKNOWN_TYPE",
            format!("Unknown defense type: '{defense_type}'"),
        )
    })?;

    if count == 0 {
        return Err(ImpForgeError::validation(
            "DEFENSE_ZERO_COUNT",
            "Count must be at least 1",
        ));
    }

    // Shield domes are unique (max 1 each)
    if matches!(dt, DefenseType::SmallShieldDome | DefenseType::LargeShieldDome) && count > 1 {
        return Err(ImpForgeError::validation(
            "DEFENSE_DOME_LIMIT",
            "Shield domes are unique structures. Only 1 of each type allowed per colony.",
        ));
    }

    let stats = defense_stats(&dt);
    let cost = defense_cost(&dt, count);

    Ok(serde_json::json!({
        "defense_type": defense_type,
        "name": stats.name,
        "count": count,
        "cost_per_unit": {
            "biomass": stats.cost_biomass,
            "minerals": stats.cost_minerals,
            "spore_gas": stats.cost_spore_gas,
        },
        "total_cost": {
            "biomass": cost.biomass,
            "minerals": cost.minerals,
            "spore_gas": cost.spore_gas,
        },
    }))
}

/// Build defenses on a colony (persists to SQLite).
#[tauri::command]
pub async fn swarm_build_defense(
    colony_id: String,
    defense_type: String,
    count: u32,
    engine: tauri::State<'_, SwarmCombatEngine>,
) -> Result<serde_json::Value, ImpForgeError> {
    let dt = DefenseType::from_str(&defense_type).ok_or_else(|| {
        ImpForgeError::validation(
            "DEFENSE_UNKNOWN_TYPE",
            format!("Unknown defense type: '{defense_type}'"),
        )
    })?;

    if count == 0 {
        return Err(ImpForgeError::validation(
            "DEFENSE_ZERO_COUNT",
            "Count must be at least 1",
        ));
    }

    if colony_id.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "DEFENSE_NO_COLONY",
            "Colony ID is required",
        ));
    }

    // Shield domes are unique
    if matches!(dt, DefenseType::SmallShieldDome | DefenseType::LargeShieldDome) && count > 1 {
        return Err(ImpForgeError::validation(
            "DEFENSE_DOME_LIMIT",
            "Shield domes are unique structures. Only 1 of each type allowed per colony.",
        ));
    }

    let cost = defense_cost(&dt, count);
    let stats = defense_stats(&dt);

    engine.build_defense(&colony_id, &dt, count)?;

    Ok(serde_json::json!({
        "colony_id": colony_id,
        "defense_type": defense_type,
        "name": stats.name,
        "count": count,
        "total_cost": {
            "biomass": cost.biomass,
            "minerals": cost.minerals,
            "spore_gas": cost.spore_gas,
        },
        "status": "built",
    }))
}

// ---------------------------------------------------------------------------
// Additional Tauri Commands — wiring internal helpers
// ---------------------------------------------------------------------------

/// List all damage types with their string representations.
#[tauri::command]
pub async fn swarm_damage_types() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    Ok(DamageType::all()
        .iter()
        .map(|dt| serde_json::json!({ "id": dt.as_str(), "name": dt.as_str() }))
        .collect())
}

/// List all armor types with their string representations.
#[tauri::command]
pub async fn swarm_armor_types() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    let all = [
        ArmorType::Chitin, ArmorType::Scale, ArmorType::Ethereal,
        ArmorType::Hellforged, ArmorType::Bone, ArmorType::Crystal, ArmorType::Void,
    ];
    Ok(all.iter().map(|at| {
        serde_json::json!({ "id": at.as_str(), "name": at.as_str() })
    }).collect())
}

/// Look up a damage type from its string key.
#[tauri::command]
pub async fn swarm_damage_type_info(name: String) -> Result<serde_json::Value, ImpForgeError> {
    let dt = DamageType::from_str(&name);
    let at = ArmorType::from_str(&name); // fallback parse for armor
    Ok(serde_json::json!({
        "damage_type": dt.as_str(),
        "armor_fallback": at.as_str(),
    }))
}

/// Simulate combat and persist the battle report.
#[tauri::command]
pub async fn swarm_simulate_and_save(
    attacker_ships: Vec<(String, u32)>,
    defender_ships: Vec<(String, u32)>,
    mission_id: Option<String>,
    engine: tauri::State<'_, SwarmCombatEngine>,
) -> Result<BattleResult, ImpForgeError> {
    if attacker_ships.is_empty() || defender_ships.is_empty() {
        return Err(ImpForgeError::validation(
            "COMBAT_EMPTY",
            "Both sides must have at least one ship.",
        ));
    }
    let result = simulate_battle(&attacker_ships, &defender_ships);
    engine.save_battle_report(&result, mission_id.as_deref())?;
    Ok(result)
}

/// Get all defenses built on a specific colony.
#[tauri::command]
pub async fn swarm_colony_defenses(
    colony_id: String,
    engine: tauri::State<'_, SwarmCombatEngine>,
) -> Result<Vec<(String, u32)>, ImpForgeError> {
    if colony_id.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "DEFENSE_NO_COLONY",
            "Colony ID is required",
        ));
    }
    engine.get_colony_defenses(&colony_id)
}

/// Advance terrain spread and return the updated map including cross-faction damage.
#[tauri::command]
pub async fn swarm_terrain_tick(
    colony_id: String,
    delta_secs: f64,
    unit_faction: String,
) -> Result<serde_json::Value, ImpForgeError> {
    if colony_id.is_empty() {
        return Err(ImpForgeError::validation(
            "TERRAIN_EMPTY_ID",
            "Colony ID must not be empty.",
        ));
    }
    let mut tiles = generate_test_terrain_map(&colony_id);
    terrain_spread_tick(&mut tiles, delta_secs);
    let contested = terrain_check_contested(&tiles);

    let tile_data: Vec<serde_json::Value> = tiles
        .iter()
        .map(|t| {
            let cross_dmg = terrain_cross_faction_damage(&unit_faction, t);
            serde_json::json!({
                "x": t.x,
                "y": t.y,
                "terrain": t.terrain_type.as_str(),
                "strength": t.strength,
                "owner": &t.owner_faction,
                "damage_to_unit": cross_dmg,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "tiles": tile_data,
        "contested_count": contested.len(),
    }))
}

/// List all defense types available for building.
#[tauri::command]
pub async fn swarm_all_defense_types() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    Ok(DefenseType::all()
        .iter()
        .map(|dt| {
            let stats = defense_stats(dt);
            serde_json::json!({
                "id": dt.as_str(),
                "name": stats.name,
                "description": stats.description,
            })
        })
        .collect())
}

/// Simulate a battle between colony defenses and an attacking fleet.
#[tauri::command]
pub async fn swarm_defense_battle(
    defenses: Vec<(String, u32)>,
    attacker_fleet: Vec<(String, u32)>,
) -> Result<BattleResult, ImpForgeError> {
    if defenses.is_empty() || attacker_fleet.is_empty() {
        return Err(ImpForgeError::validation(
            "COMBAT_EMPTY",
            "Both sides must have at least one unit.",
        ));
    }
    let typed_defenses: Vec<(DefenseType, u32)> = defenses
        .iter()
        .filter_map(|(name, count)| {
            DefenseType::from_str(name).map(|dt| (dt, *count))
        })
        .collect();
    Ok(defense_vs_fleet(&typed_defenses, &attacker_fleet))
}

/// Get fleet cargo capacity and resource totals for a set of ships.
#[tauri::command]
pub async fn swarm_fleet_cargo_info(
    ships: Vec<(String, u32)>,
) -> Result<serde_json::Value, ImpForgeError> {
    if ships.is_empty() {
        return Err(ImpForgeError::validation(
            "FLEET_EMPTY",
            "Fleet must contain at least one ship.",
        ));
    }
    let mut total_cargo = 0.0_f64;
    let mut total_cost = Resources::default();
    for (ship_type, count) in &ships {
        let profile = ship_profile(ship_type);
        total_cargo += profile.cargo * (*count as f64);
        let mut cost = profile.cost;
        cost.scale(*count as f64);
        total_cost.add(&cost);
    }
    Ok(serde_json::json!({
        "total_cargo_capacity": total_cargo,
        "total_fleet_cost": total_cost.total(),
    }))
}

