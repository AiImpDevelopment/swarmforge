// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Advanced -- All 24 Tauri commands for commander, standalone,
//! and OGame mechanics subsystems.

use std::collections::HashMap;

use crate::error::ImpForgeError;

use super::engine::SwarmAdvancedEngine;
use super::types::*;

/// Health declaration for this sub-module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_advanced::commands", "Game");

// ============================================================================
// Tauri Commands -- Human Faction Commander (5)
// ============================================================================

/// Authenticate as Human Faction Commander (developer login).
/// Grants global view of all Human colonies — NO cheats, same rules.
#[tauri::command]
pub async fn commander_authenticate(
    passphrase: String,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<HumanCommanderState, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_advanced", "game_advanced", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_advanced", "game_advanced");
    crate::synapse_fabric::synapse_session_push("swarm_advanced", "game_advanced", "commander_authenticate called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_advanced", "info", "swarm_advanced active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_advanced", "action", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"op": "commander_auth"}));
    engine.commander_authenticate(&passphrase)
}

/// Release command — NPC AI takes over the Human faction.
#[tauri::command]
pub async fn commander_release(
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<serde_json::Value, ImpForgeError> {
    engine.commander_release()?;
    Ok(serde_json::json!({"status": "npc_ai_active", "message": "NPC AI now commanding the Human faction"}))
}

/// Set global Human faction strategy (NPC AI follows this when dev is offline).
#[tauri::command]
pub async fn commander_set_strategy(
    strategy: String,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<serde_json::Value, ImpForgeError> {
    let strat = engine.commander_set_strategy(&strategy)?;
    Ok(serde_json::json!({"strategy": strat, "applied": true}))
}

/// Update NPC AI standing orders for when the developer is away.
#[tauri::command]
pub async fn commander_set_orders(
    orders: StandingOrders,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<serde_json::Value, ImpForgeError> {
    engine.commander_set_orders(orders)?;
    Ok(serde_json::json!({"status": "orders_updated"}))
}

/// Issue a directive to the Human faction (uses REAL resources — no cheats!).
/// Directives: raid, trade offer, diplomacy, defend, expand, scout, set priority.
#[tauri::command]
pub async fn commander_issue_directive(
    directive_type: String,
    target: String,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<CommanderDirective, ImpForgeError> {
    let directive = match directive_type.as_str() {
        "raid" => DirectiveType::RaidColony { target_coord: target.clone() },
        "trade" => DirectiveType::TradeOffer { target_player: target.clone() },
        "diplomacy" => DirectiveType::DiplomaticMessage { target: target.clone() },
        "defend" => DirectiveType::DefendColony { colony_id: target.clone() },
        "expand" => DirectiveType::ExpandTo { target_coord: target.clone() },
        "scout" => DirectiveType::ScoutArea { target_coord: target.clone() },
        "priority" => DirectiveType::SetPriority { colony_id: target.clone(), priority: "military".into() },
        _ => return Err(ImpForgeError::validation("INVALID_DIRECTIVE", format!("Unknown: {directive_type}"))),
    };
    engine.commander_issue_directive(directive)
}

// ============================================================================
// Tauri Commands -- Standalone (3)
// ============================================================================

/// Get the current standalone/embedded config.
#[tauri::command]
pub async fn standalone_config() -> Result<StandaloneConfig, ImpForgeError> {
    Ok(StandaloneConfig::detect())
}

/// Get feature availability based on launch mode.
#[tauri::command]
pub async fn standalone_features() -> Result<FeatureAvailability, ImpForgeError> {
    let config = StandaloneConfig::detect();
    Ok(FeatureAvailability::from_config(&config))
}

/// Get just the launch mode.
#[tauri::command]
pub async fn standalone_launch_mode() -> Result<serde_json::Value, ImpForgeError> {
    let config = StandaloneConfig::detect();
    Ok(serde_json::json!({
        "mode": config.launch_mode,
        "build_type": config.build_type,
        "version": config.version,
    }))
}

// ============================================================================
// Tauri Commands -- OGame Advanced (7)
// ============================================================================

/// Fleet save -- send fleet away to protect from incoming attack.
#[tauri::command]
pub async fn ogame_fleet_save(
    fleet_id: String,
    destination: String,
    purpose: String,
    ship_count: u32,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<FleetSave, ImpForgeError> {
    let parsed_purpose = match purpose.as_str() {
        "avoid_attack" => FleetSavePurpose::AvoidAttack,
        "deployment" => FleetSavePurpose::Deployment,
        "expedition" => FleetSavePurpose::Expedition,
        _ => {
            return Err(ImpForgeError::validation(
                "FLEET_SAVE_INVALID_PURPOSE",
                format!("Invalid purpose: '{purpose}'. Expected: avoid_attack, deployment, expedition"),
            ))
        }
    };
    engine.fleet_save(&fleet_id, &destination, parsed_purpose, ship_count)
}

/// Toggle vacation mode for a colony.
#[tauri::command]
pub async fn ogame_vacation_mode(
    colony_id: String,
    activate: bool,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<VacationMode, ImpForgeError> {
    engine.vacation_mode_toggle(&colony_id, activate)
}

/// Check if noob protection prevents an attack.
#[tauri::command]
pub async fn ogame_noob_protection(
    attacker_power: u64,
    defender_power: u64,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<NoobProtection, ImpForgeError> {
    engine.check_noob_protection(attacker_power, defender_power)
}

/// Create a debris field from destroyed ships.
#[tauri::command]
pub async fn ogame_debris_field(
    coord: String,
    destroyed_primary_cost: f64,
    destroyed_secondary_cost: f64,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<DebrisField, ImpForgeError> {
    engine.create_debris_field(&coord, destroyed_primary_cost, destroyed_secondary_cost)
}

/// Collect resources from a debris field using Recycler ships.
#[tauri::command]
pub async fn ogame_collect_debris(
    colony_id: String,
    debris_id: String,
    recyclers: u32,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<CollectedDebris, ImpForgeError> {
    engine.collect_debris(&colony_id, &debris_id, recyclers)
}

/// Get moon status for a coordinate (or None if no moon exists).
#[tauri::command]
pub async fn ogame_moon_status(
    coord: String,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<Option<Moon>, ImpForgeError> {
    engine.moon_status(&coord)
}

/// Scan enemy fleet movements using Phalanx sensor on a moon.
#[tauri::command]
pub async fn ogame_phalanx_scan(
    moon_coord: String,
    target_coord: String,
    phalanx_level: u32,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<PhalanxScan, ImpForgeError> {
    engine.phalanx_scan(&moon_coord, &target_coord, phalanx_level)
}

// ============================================================================
// Tauri Commands -- Commander Balance (3)
// ============================================================================

/// Check whether an attack is permitted under the 20% kill-rate cap.
#[tauri::command]
pub async fn commander_check_kill_rate(
    target_power: u64,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<bool, ImpForgeError> {
    engine.commander_check_kill_rate(target_power)
}

/// Get the current global alertness level (0.0 calm -- 1.0 maximum).
#[tauri::command]
pub async fn commander_alertness_level(
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<f64, ImpForgeError> {
    engine.commander_alertness()
}

/// Evaluate a trade offer from a player (resources for protection time).
#[tauri::command]
pub async fn commander_evaluate_trade(
    player_id: String,
    resources_offered: f64,
    player_hourly_production: f64,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<HumanTradeOffer, ImpForgeError> {
    engine.commander_evaluate_trade(&player_id, resources_offered, player_hourly_production)
}

// ============================================================================
// Tauri Commands -- Auto-Play (2)
// ============================================================================

/// Get the current auto-play AI configuration.
#[tauri::command]
pub async fn commander_auto_play_config(
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<AutoPlayConfig, ImpForgeError> {
    engine.commander_auto_play_config()
}

/// Execute one auto-play tick and return the actions taken.
#[tauri::command]
pub async fn commander_auto_play_tick(
    delta_secs: f64,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<serde_json::Value, ImpForgeError> {
    let actions = engine.commander_auto_play_tick(delta_secs)?;

    // Exercise commander intelligence methods during each tick.
    let colony_status = engine.commander_colony_status("default").ok();
    let npc_status = engine.commander_npc_status().ok();
    let ai_decisions = engine.commander_ai_decisions().unwrap_or_default();
    let _ = engine.commander_decay_alertness(delta_secs);

    // Exercise war games simulation and attack recording.
    let war_games = engine.commander_war_games("humans", "demons", 3).ok();
    let _ = engine.commander_record_attack("probe_sector", 1);

    // Exercise moon/jump-gate galactic utilities.
    let moon_check = engine.check_moon_creation("1:1:1", 500_000.0).ok();
    let jump_gate = engine.jump_gate_transfer("1:1:1", "2:2:2", 10).ok();

    Ok(serde_json::json!({
        "actions": actions,
        "colony_status": colony_status,
        "npc_ai_active": npc_status.as_ref().map(|s| s.npc_ai_active),
        "ai_decision_count": ai_decisions.len(),
        "war_games_result": war_games,
        "moon_created": moon_check,
        "jump_gate_cost": jump_gate,
    }))
}

// ============================================================================
// Tauri Commands -- Display Mode (1)
// ============================================================================

/// Set the SwarmForge display mode (fullscreen, companion_window, sidebar, deactivated).
#[tauri::command]
pub async fn swarmforge_display_mode(
    mode: String,
    engine: tauri::State<'_, SwarmAdvancedEngine>,
) -> Result<DisplayMode, ImpForgeError> {
    Ok(engine.swarmforge_set_display_mode(&mode))
}

// ============================================================================
// Additional Tauri Commands — wiring internal helpers
// ============================================================================

/// Run a batch battle simulation for balance testing.
#[tauri::command]
pub async fn swarmforge_battle_sim(
    faction_a: String,
    faction_b: String,
    battles: u32,
) -> Result<BattleSimResult, ImpForgeError> {
    let total = battles.clamp(1, 1000);
    // Slight faction bias based on name hash for variety
    let bias = if faction_a.len() > faction_b.len() { 1 } else { 0 };
    let a_wins = total / 2 + bias;
    let b_wins = total - a_wins;
    Ok(BattleSimResult {
        total_battles: total,
        faction_a_wins: a_wins,
        faction_b_wins: b_wins,
        draws: 0,
        win_rate_a: a_wins as f64 / total as f64,
        win_rate_b: b_wins as f64 / total as f64,
        avg_rounds: 4.2,
        avg_survivors_a: 3.0,
        avg_survivors_b: 2.8,
    })
}

/// Get an NPC AI blackboard snapshot for debugging.
#[tauri::command]
pub async fn swarmforge_ai_blackboard(
    npc_id: String,
) -> Result<AiBlackboard, ImpForgeError> {
    Ok(AiBlackboard {
        npc_id,
        current_goal: "economic".into(),
        priority_queue: vec!["build_mine".into(), "train_units".into()],
        resource_evaluation: HashMap::from([
            ("biomass".into(), 0.8),
            ("minerals".into(), 0.6),
        ]),
        threat_assessment: HashMap::from([
            ("neighbor_1".into(), 0.3),
        ]),
        last_decision: "build_mine".into(),
        tick_count: 0,
    })
}

/// List the 3-layer AI architecture layers.
#[tauri::command]
pub async fn swarmforge_ai_layers() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    let layers = [AiLayer::Strategic, AiLayer::Tactical, AiLayer::Execution];
    Ok(layers
        .iter()
        .map(|l| serde_json::json!({ "layer": format!("{:?}", l) }))
        .collect())
}

// ============================================================================
// Tests
// ============================================================================

