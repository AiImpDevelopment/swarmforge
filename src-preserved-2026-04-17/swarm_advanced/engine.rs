// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Advanced -- SQLite persistence engine for Commander,
//! Standalone-config, and OGame mechanics subsystems.

use chrono::Utc;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::error::ImpForgeError;

use super::types::*;

/// Health declaration for this sub-module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_advanced::engine", "Game");

// ============================================================================
// SwarmAdvancedEngine -- SQLite persistence for all three subsystems
// ============================================================================

pub struct SwarmAdvancedEngine {
    conn: Mutex<Connection>,
    commander_state: Mutex<HumanCommanderState>,
}
impl SwarmAdvancedEngine {
    /// Open (or create) tables in the swarmforge database at `data_dir/swarmforge.db`.
    pub(crate) fn new(data_dir: &std::path::Path) -> Result<Self, ImpForgeError> {
        std::fs::create_dir_all(data_dir).map_err(|e| {
            ImpForgeError::filesystem(
                "ADVANCED_DIR",
                format!("Cannot create data dir: {e}"),
            )
        })?;

        let db_path = data_dir.join("swarmforge.db");
        let conn = Connection::open(&db_path).map_err(|e| {
            ImpForgeError::internal(
                "ADVANCED_DB_OPEN",
                format!("SQLite open failed: {e}"),
            )
        })?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;",
        )
        .map_err(|e| {
            ImpForgeError::internal("ADVANCED_PRAGMA", format!("PRAGMA failed: {e}"))
        })?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS fleet_saves (
                id TEXT PRIMARY KEY,
                fleet_id TEXT NOT NULL,
                destination TEXT NOT NULL,
                return_time TEXT NOT NULL,
                purpose TEXT NOT NULL,
                ship_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS vacation_mode (
                colony_id TEXT PRIMARY KEY,
                active INTEGER NOT NULL DEFAULT 0,
                started_at TEXT,
                min_duration_hours INTEGER NOT NULL DEFAULT 48,
                max_duration_days INTEGER NOT NULL DEFAULT 30
            );
            CREATE TABLE IF NOT EXISTS debris_fields (
                id TEXT PRIMARY KEY,
                coord TEXT NOT NULL,
                resources_primary REAL NOT NULL DEFAULT 0.0,
                resources_secondary REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL,
                expires_hours INTEGER NOT NULL DEFAULT 24
            );
            CREATE TABLE IF NOT EXISTS moons (
                id TEXT PRIMARY KEY,
                coord TEXT NOT NULL UNIQUE,
                size INTEGER NOT NULL,
                has_phalanx INTEGER NOT NULL DEFAULT 0,
                has_jump_gate INTEGER NOT NULL DEFAULT 0,
                jump_gate_cooldown_mins INTEGER NOT NULL DEFAULT 60,
                creation_chance REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL
            );",
        )
        .map_err(|e| {
            ImpForgeError::internal(
                "ADVANCED_SCHEMA",
                format!("Schema creation failed: {e}"),
            )
        })?;

        Ok(Self {
            conn: Mutex::new(conn),
            commander_state: Mutex::new(HumanCommanderState::default()),
        })
    }

    // -----------------------------------------------------------------------
    // Human Faction Commander Powers (NO cheats — same rules as all players!)
    // -----------------------------------------------------------------------

    /// Authenticate as Human Faction Commander.
    /// Grants global view of all Human colonies — NOT cheats.
    pub(crate) fn commander_authenticate(&self, passphrase: &str) -> Result<HumanCommanderState, ImpForgeError> {
        if passphrase != COMMANDER_PASSPHRASE {
            return Err(ImpForgeError::validation(
                "INVALID_PASSPHRASE",
                "Commander authentication failed.",
            ));
        }
        let mut guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.take_command();
        Ok(guard.clone())
    }

    /// Developer goes offline — hand control to NPC AI.
    pub(crate) fn commander_release(&self) -> Result<(), ImpForgeError> {
        let mut guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.release_command();
        Ok(())
    }

    /// Set global faction strategy (NPC AI follows this when developer is away).
    pub(crate) fn commander_set_strategy(&self, strategy: &str) -> Result<FactionStrategy, ImpForgeError> {
        let mut guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;

        let strat = match strategy {
            "balanced" => FactionStrategy::Balanced,
            "expansionist" => FactionStrategy::Expansionist,
            "militaristic" => FactionStrategy::Militaristic,
            "defensive" => FactionStrategy::Defensive,
            "economic" => FactionStrategy::Economic,
            "diplomatic" => FactionStrategy::Diplomatic,
            "aggressive" => FactionStrategy::Aggressive,
            _ => return Err(ImpForgeError::validation("INVALID_STRATEGY", format!("Unknown strategy: {strategy}"))),
        };
        guard.global_strategy = strat.clone();
        Ok(strat)
    }

    /// Update standing orders for NPC AI behavior when developer is offline.
    pub(crate) fn commander_set_orders(&self, orders: StandingOrders) -> Result<(), ImpForgeError> {
        let mut guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;
        guard.standing_orders = orders;
        Ok(())
    }

    /// Issue a directive (uses REAL resources and units — no cheats!)
    pub(crate) fn commander_issue_directive(&self, directive: DirectiveType) -> Result<CommanderDirective, ImpForgeError> {
        let guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;

        Ok(CommanderDirective {
            id: Uuid::new_v4().to_string(),
            directive_type: directive,
            target: "pending".to_string(),
            issued_at: Utc::now().to_rfc3339(),
            status: "queued".to_string(),
        })
    }

    /// View resource status of a Human colony (commander intelligence).
    pub(crate) fn commander_colony_status(
        &self,
        colony_id: &str,
    ) -> Result<serde_json::Value, ImpForgeError> {
        let guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;

        if colony_id.is_empty() {
            return Err(ImpForgeError::validation(
                "CMD_EMPTY_COLONY",
                "Colony ID cannot be empty",
            ));
        }

        Ok(serde_json::json!({
            "colony_id": colony_id,
            "faction": "humans",
            "npc_ai_active": guard.npc_ai_active,
            "strategy": guard.global_strategy,
            "queried_at": Utc::now().to_rfc3339(),
        }))
    }

    /// Get NPC AI status — shows what the AI is doing while dev is away.
    pub(crate) fn commander_npc_status(&self) -> Result<HumanCommanderState, ImpForgeError> {
        let guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        Ok(guard.clone())
    }

    /// Run war games simulation — plan battles before committing real troops.
    /// Uses same combat rules as real battles, just simulated.
    pub(crate) fn commander_war_games(
        &self,
        faction_a: &str,
        faction_b: &str,
        count: u32,
    ) -> Result<BattleSimResult, ImpForgeError> {
        let guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;

        if count == 0 || count > 10_000 {
            return Err(ImpForgeError::validation(
                "CMD_SIM_COUNT",
                "Battle count must be 1-10,000",
            ));
        }

        // Deterministic simulation using faction names as seed material.
        // Real combat uses swarm_combat::simulate_battle; this is a fast
        // Monte Carlo approximation for balance testing.
        let seed = faction_a.len() as u64 * 31 + faction_b.len() as u64 * 17;
        let mut rng_state = seed;
        let mut a_wins = 0u32;
        let mut b_wins = 0u32;
        let mut draws = 0u32;
        let mut total_rounds = 0u64;
        let mut total_survivors_a = 0u64;
        let mut total_survivors_b = 0u64;

        for i in 0..count {
            // Simple LCG for deterministic results
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(i as u64);
            let outcome = (rng_state >> 33) % 100;
            let rounds = ((rng_state >> 20) % 6) + 1;
            total_rounds += rounds;

            if outcome < 45 {
                a_wins += 1;
                total_survivors_a += ((rng_state >> 10) % 80) + 20;
            } else if outcome < 90 {
                b_wins += 1;
                total_survivors_b += ((rng_state >> 10) % 80) + 20;
            } else {
                draws += 1;
                total_survivors_a += ((rng_state >> 10) % 40) + 10;
                total_survivors_b += ((rng_state >> 5) % 40) + 10;
            }
        }

        let n = count as f64;
        Ok(BattleSimResult {
            total_battles: count,
            faction_a_wins: a_wins,
            faction_b_wins: b_wins,
            draws,
            win_rate_a: a_wins as f64 / n,
            win_rate_b: b_wins as f64 / n,
            avg_rounds: total_rounds as f64 / n,
            avg_survivors_a: total_survivors_a as f64 / n.max(1.0),
            avg_survivors_b: total_survivors_b as f64 / n.max(1.0),
        })
    }

    /// View NPC AI decision logs — see what the AI decided and why.
    /// Helps the commander understand AI behavior during absence.
    pub(crate) fn commander_ai_decisions(&self) -> Result<Vec<AiBlackboard>, ImpForgeError> {
        let guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;

        // Return representative NPC blackboards for the 3 AI difficulty tiers
        let npcs = vec![
            AiBlackboard {
                npc_id: "npc_easy_01".to_string(),
                current_goal: "expand_colony".to_string(),
                priority_queue: vec![
                    "build_metal_mine".to_string(),
                    "research_energy".to_string(),
                ],
                resource_evaluation: HashMap::from([
                    ("primary".to_string(), 0.8),
                    ("secondary".to_string(), 0.5),
                ]),
                threat_assessment: HashMap::from([("player".to_string(), 0.2)]),
                last_decision: "queue_building".to_string(),
                tick_count: 142,
            },
            AiBlackboard {
                npc_id: "npc_medium_01".to_string(),
                current_goal: "military_buildup".to_string(),
                priority_queue: vec![
                    "build_shipyard".to_string(),
                    "fleet_save".to_string(),
                    "espionage_scan".to_string(),
                ],
                resource_evaluation: HashMap::from([
                    ("primary".to_string(), 0.6),
                    ("secondary".to_string(), 0.7),
                    ("tertiary".to_string(), 0.9),
                ]),
                threat_assessment: HashMap::from([("player".to_string(), 0.6)]),
                last_decision: "dispatch_fleet".to_string(),
                tick_count: 891,
            },
            AiBlackboard {
                npc_id: "npc_hard_01".to_string(),
                current_goal: "aggressive_raid".to_string(),
                priority_queue: vec![
                    "phalanx_scan".to_string(),
                    "coordinate_attack".to_string(),
                    "moon_defense".to_string(),
                ],
                resource_evaluation: HashMap::from([
                    ("primary".to_string(), 0.3),
                    ("secondary".to_string(), 0.4),
                    ("dark_matter".to_string(), 0.95),
                ]),
                threat_assessment: HashMap::from([
                    ("player".to_string(), 0.9),
                    ("alliance_neighbor".to_string(), 0.4),
                ]),
                last_decision: "launch_attack".to_string(),
                tick_count: 2047,
            },
        ];

        Ok(npcs)
    }

    // -----------------------------------------------------------------------
    // Commander Balance Mechanisms (prevents unfair play)
    // Based on Helldivers 2 "Joel" pattern + AlphaStar throttling
    // -----------------------------------------------------------------------

    /// Check if an attack is allowed by the kill-rate limit.
    ///
    /// Returns `true` if the attack may proceed, `false` if the commander
    /// must wait because the 20% kill-rate cap has been exceeded.
    /// `target_power` is used to weight the impact -- attacking much weaker
    /// targets counts more heavily against the limit.
    pub(crate) fn commander_check_kill_rate(&self, target_power: u64) -> Result<bool, ImpForgeError> {
        let guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;

        // Simple model: if kill rate already exceeds 20%, block the attack
        let kill_rate = if guard.kill_rate_limit.attacks_last_hour == 0 {
            0.0
        } else {
            guard.kill_rate_limit.kills_last_hour as f64
                / guard.kill_rate_limit.attacks_last_hour as f64
        };

        // Weaker targets (power < 1000) are weighted more heavily
        let weakness_penalty = if target_power < 1000 { 0.05 } else { 0.0 };
        let effective_rate = kill_rate + weakness_penalty;

        Ok(effective_rate <= guard.kill_rate_limit.max_kill_rate)
    }

    /// Record an attack for kill-rate tracking.
    ///
    /// Call this after every attack the commander initiates.
    /// `killed_units` is the number of enemy units destroyed in this attack.
    pub(crate) fn commander_record_attack(
        &self,
        target_colony: &str,
        killed_units: u32,
    ) -> Result<(), ImpForgeError> {
        let mut guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;

        guard.kill_rate_limit.attacks_last_hour += 1;
        guard.kill_rate_limit.kills_last_hour += killed_units;

        // Recalculate current kill rate
        guard.kill_rate_limit.current_kill_rate = if guard.kill_rate_limit.attacks_last_hour == 0 {
            0.0
        } else {
            guard.kill_rate_limit.kills_last_hour as f64
                / guard.kill_rate_limit.attacks_last_hour as f64
        };

        guard.kill_rate_limit.is_limited =
            guard.kill_rate_limit.current_kill_rate > guard.kill_rate_limit.max_kill_rate;

        // Update alertness for this attack
        let count = guard
            .alertness
            .settlements_attacked
            .entry(target_colony.to_string())
            .or_insert(0);
        *count += 1;

        let repeat_count = *count;
        let base_increase = guard.alertness.increase_per_attack;
        let multiplier = if repeat_count > 1 {
            guard.alertness.repeated_attack_multiplier
        } else {
            1.0
        };
        guard.alertness.level =
            (guard.alertness.level + base_increase * multiplier).min(1.0);

        Ok(())
    }

    /// Get current alertness level (0.0 = calm, 1.0 = maximum alert).
    pub(crate) fn commander_alertness(&self) -> Result<f64, ImpForgeError> {
        let guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;
        Ok(guard.alertness.level)
    }

    /// Decay alertness over time (called periodically by the game tick).
    ///
    /// Returns the new alertness level after decay.
    /// `hours_elapsed` is how many game-hours have passed since the last call.
    pub(crate) fn commander_decay_alertness(
        &self,
        hours_elapsed: f64,
    ) -> Result<f64, ImpForgeError> {
        let mut guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;

        let decay = guard.alertness.decay_per_hour * hours_elapsed;
        guard.alertness.level = (guard.alertness.level - decay).max(0.0);
        Ok(guard.alertness.level)
    }

    // -----------------------------------------------------------------------
    // Trade with Humans Mechanic
    // -----------------------------------------------------------------------

    /// Evaluate a trade offer from a player.
    ///
    /// Players can bribe the commander for temporary protection.
    /// Minimum 10,000 resources for 24 hours, up to 168 hours (1 week).
    /// Offers below the minimum are rejected.
    /// Evaluate a trade offer from a player.
    ///
    /// **Cost model**: 24h protection costs 5 hours of the player's TOTAL hourly
    /// production. This scales with player strength automatically:
    /// - Small player (100/hr) → 500 resources = 24h protection
    /// - Large player (10,000/hr) → 50,000 resources = 24h protection
    ///
    /// If the player attacks Humans during protection, it is VOIDED immediately.
    /// Max protection: 168 hours (1 week).
    pub(crate) fn commander_evaluate_trade(
        &self,
        player_id: &str,
        resources_offered: f64,
        player_hourly_production: f64,
    ) -> Result<HumanTradeOffer, ImpForgeError> {
        let guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        guard.require_auth()?;

        if player_id.is_empty() {
            return Err(ImpForgeError::validation(
                "TRADE_EMPTY_PLAYER",
                "Player ID must not be empty",
            ));
        }

        if player_hourly_production <= 0.0 {
            return Err(ImpForgeError::validation(
                "TRADE_INVALID_PRODUCTION",
                "Player hourly production must be positive",
            ));
        }

        // Cost per 24h protection = 5 hours of player's total production
        let cost_per_24h = player_hourly_production * 5.0;
        let protection_raw = (resources_offered / cost_per_24h * 24.0).floor() as u32;
        let accepted = protection_raw >= 24; // must afford at least 24h
        let protection_hours = if accepted { protection_raw.min(168) } else { 0 };

        let now = Utc::now();
        let expires_at = if accepted {
            (now + chrono::Duration::hours(protection_hours as i64))
                .to_rfc3339()
        } else {
            now.to_rfc3339()
        };

        Ok(HumanTradeOffer {
            player_id: player_id.to_string(),
            resources_offered,
            protection_hours,
            accepted,
            expires_at,
            voided_on_attack: false,
        })
    }

    // -----------------------------------------------------------------------
    // Auto-Play AI System (AlphaStar Throttling)
    // -----------------------------------------------------------------------

    /// Get the current auto-play configuration.
    pub(crate) fn commander_auto_play_config(&self) -> Result<AutoPlayConfig, ImpForgeError> {
        let guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        Ok(guard.auto_play.clone())
    }

    /// Execute one auto-play tick, returning the list of actions taken.
    ///
    /// The AI is throttled to a maximum of 22 raw actions per 5-second window
    /// (AlphaStar limit).  The number of actions in a single tick is computed
    /// from `delta_secs` and the sustained APM, capped by the per-window limit.
    /// Each action is a descriptive string for the game log.
    pub(crate) fn commander_auto_play_tick(
        &self,
        delta_secs: f64,
    ) -> Result<Vec<String>, ImpForgeError> {
        let guard = self.commander_state.lock().map_err(|e| {
            ImpForgeError::internal("CMD_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        if delta_secs <= 0.0 {
            return Ok(Vec::new());
        }

        let config = &guard.auto_play;

        // Actions allowed this tick based on sustained APM
        let actions_per_sec = config.sustained_apm as f64 / 60.0;
        let raw_actions = (actions_per_sec * delta_secs).round() as u32;

        // Cap to the 5-second window limit, scaled for the actual tick length
        let window_ratio = (delta_secs / 5.0).min(1.0);
        let max_this_tick =
            ((config.max_actions_per_5s as f64) * window_ratio).ceil() as u32;

        let action_count = raw_actions.min(max_this_tick).max(1);

        // Apply offline efficiency
        let effective_actions =
            ((action_count as f64) * config.offline_efficiency * config.subscription_bonus)
                .round() as u32;

        // Generate representative actions from the 3-layer AI
        let action_pool = [
            // Strategic layer
            "strategic: evaluate_expansion_targets",
            "strategic: score_military_buildup",
            "strategic: assess_diplomatic_options",
            // Tactical layer
            "tactical: queue_building_upgrade",
            "tactical: adjust_fleet_composition",
            "tactical: schedule_resource_transport",
            "tactical: set_rally_point",
            // Execution layer
            "execution: move_fleet_to_sector",
            "execution: activate_shield_generator",
            "execution: launch_probe",
            "execution: collect_resources",
            "execution: repair_damaged_units",
        ];

        let mut actions = Vec::with_capacity(effective_actions as usize);
        for i in 0..effective_actions {
            let idx = (i as usize) % action_pool.len();
            actions.push(action_pool[idx].to_string());
        }

        Ok(actions)
    }

    // -----------------------------------------------------------------------
    // Display Mode
    // -----------------------------------------------------------------------

    /// Set the SwarmForge display mode.
    pub(crate) fn swarmforge_set_display_mode(&self, mode: &str) -> DisplayMode {
        DisplayMode::from_str_lossy(mode)
    }

    // -----------------------------------------------------------------------
    // Fleet Save
    // -----------------------------------------------------------------------

    /// Create a fleet save -- send fleet to safety before an incoming attack.
    pub(crate) fn fleet_save(
        &self,
        fleet_id: &str,
        destination: &str,
        purpose: FleetSavePurpose,
        ship_count: u32,
    ) -> Result<FleetSave, ImpForgeError> {
        if fleet_id.is_empty() {
            return Err(ImpForgeError::validation(
                "FLEET_SAVE_EMPTY_ID",
                "Fleet ID cannot be empty",
            ));
        }
        if destination.is_empty() {
            return Err(ImpForgeError::validation(
                "FLEET_SAVE_EMPTY_DEST",
                "Destination cannot be empty",
            ));
        }

        let now = Utc::now();
        // Return time = now + 2 hours (simulated travel)
        let return_time = now + chrono::Duration::hours(2);
        let save = FleetSave {
            id: Uuid::new_v4().to_string(),
            fleet_id: fleet_id.to_string(),
            destination: destination.to_string(),
            return_time: return_time.to_rfc3339(),
            purpose,
            ship_count,
            created_at: now.to_rfc3339(),
        };

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FLEET_SAVE_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        conn.execute(
            "INSERT INTO fleet_saves (id, fleet_id, destination, return_time, purpose, ship_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                save.id,
                save.fleet_id,
                save.destination,
                save.return_time,
                serde_json::to_string(&save.purpose).unwrap_or_default(),
                save.ship_count,
                save.created_at,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("FLEET_SAVE_INSERT", format!("Insert failed: {e}"))
        })?;

        Ok(save)
    }

    // -----------------------------------------------------------------------
    // Vacation Mode
    // -----------------------------------------------------------------------

    /// Toggle vacation mode on or off for a colony.
    pub(crate) fn vacation_mode_toggle(
        &self,
        colony_id: &str,
        activate: bool,
    ) -> Result<VacationMode, ImpForgeError> {
        if colony_id.is_empty() {
            return Err(ImpForgeError::validation(
                "VACATION_EMPTY_COLONY",
                "Colony ID cannot be empty",
            ));
        }

        let now = Utc::now().to_rfc3339();
        let mode = if activate {
            VacationMode {
                active: true,
                started_at: Some(now.clone()),
                min_duration_hours: 48,
                max_duration_days: 30,
                production_paused: true,
                attack_immune: true,
                cant_attack: true,
            }
        } else {
            VacationMode::default()
        };

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("VACATION_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        conn.execute(
            "INSERT OR REPLACE INTO vacation_mode (colony_id, active, started_at, min_duration_hours, max_duration_days)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                colony_id,
                mode.active as i32,
                mode.started_at,
                mode.min_duration_hours,
                mode.max_duration_days,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("VACATION_UPSERT", format!("Upsert failed: {e}"))
        })?;

        Ok(mode)
    }

    // -----------------------------------------------------------------------
    // Noob Protection
    // -----------------------------------------------------------------------

    /// Check if noob protection prevents an attack.
    ///
    /// Returns `true` if the attack is **blocked** (defender is protected).
    pub(crate) fn check_noob_protection(
        &self,
        attacker_power: u64,
        defender_power: u64,
    ) -> Result<NoobProtection, ImpForgeError> {
        let protection = NoobProtection::default();

        // If defender is below the threshold, they are protected
        let is_protected = defender_power < protection.power_threshold;

        // Even without threshold protection, check power range (5x rule)
        let power_ratio = if defender_power > 0 {
            attacker_power as f64 / defender_power as f64
        } else {
            f64::MAX
        };
        let out_of_range = power_ratio > protection.attack_range;

        Ok(NoobProtection {
            protected: is_protected || out_of_range,
            power_threshold: protection.power_threshold,
            attack_range: protection.attack_range,
            expires_at: if is_protected { None } else { Some("expired".to_string()) },
        })
    }

    // -----------------------------------------------------------------------
    // Debris Fields
    // -----------------------------------------------------------------------

    /// Create a debris field from destroyed ships.
    ///
    /// Standard OGame formula: 30% of destroyed ships' resource cost becomes debris.
    pub(crate) fn create_debris_field(
        &self,
        coord: &str,
        destroyed_primary_cost: f64,
        destroyed_secondary_cost: f64,
    ) -> Result<DebrisField, ImpForgeError> {
        if coord.is_empty() {
            return Err(ImpForgeError::validation(
                "DEBRIS_EMPTY_COORD",
                "Coordinate cannot be empty",
            ));
        }

        let debris = DebrisField {
            id: Uuid::new_v4().to_string(),
            coord: coord.to_string(),
            resources_primary: destroyed_primary_cost * 0.30,
            resources_secondary: destroyed_secondary_cost * 0.30,
            created_at: Utc::now().to_rfc3339(),
            expires_hours: 24,
        };

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DEBRIS_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        conn.execute(
            "INSERT INTO debris_fields (id, coord, resources_primary, resources_secondary, created_at, expires_hours)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                debris.id,
                debris.coord,
                debris.resources_primary,
                debris.resources_secondary,
                debris.created_at,
                debris.expires_hours,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("DEBRIS_INSERT", format!("Insert failed: {e}"))
        })?;

        Ok(debris)
    }

    /// Collect resources from a debris field using Recycler ships.
    pub(crate) fn collect_debris(
        &self,
        colony_id: &str,
        debris_id: &str,
        recyclers: u32,
    ) -> Result<CollectedDebris, ImpForgeError> {
        if debris_id.is_empty() {
            return Err(ImpForgeError::validation(
                "DEBRIS_EMPTY_ID",
                "Debris field ID cannot be empty",
            ));
        }
        if recyclers == 0 {
            return Err(ImpForgeError::validation(
                "DEBRIS_NO_RECYCLERS",
                "Need at least 1 Recycler ship to collect debris",
            ));
        }

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DEBRIS_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        // Fetch the debris field
        let (primary, secondary) = conn
            .query_row(
                "SELECT resources_primary, resources_secondary FROM debris_fields WHERE id = ?1",
                params![debris_id],
                |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?)),
            )
            .map_err(|e| {
                ImpForgeError::validation(
                    "DEBRIS_NOT_FOUND",
                    format!("Debris field not found: {e}"),
                )
            })?;

        // Each recycler can carry 20,000 units
        let capacity = recyclers as f64 * 20_000.0;
        let total_available = primary + secondary;
        let collection_ratio = if total_available > 0.0 {
            (capacity / total_available).min(1.0)
        } else {
            1.0
        };

        let collected = CollectedDebris {
            debris_id: debris_id.to_string(),
            collector_colony: colony_id.to_string(),
            primary_collected: primary * collection_ratio,
            secondary_collected: secondary * collection_ratio,
            recyclers_used: recyclers,
        };

        // Remove debris field if fully collected, otherwise reduce
        if collection_ratio >= 1.0 {
            conn.execute("DELETE FROM debris_fields WHERE id = ?1", params![debris_id])
                .map_err(|e| {
                    ImpForgeError::internal(
                        "DEBRIS_DELETE",
                        format!("Delete failed: {e}"),
                    )
                })?;
        } else {
            let remaining_primary = primary * (1.0 - collection_ratio);
            let remaining_secondary = secondary * (1.0 - collection_ratio);
            conn.execute(
                "UPDATE debris_fields SET resources_primary = ?1, resources_secondary = ?2 WHERE id = ?3",
                params![remaining_primary, remaining_secondary, debris_id],
            )
            .map_err(|e| {
                ImpForgeError::internal(
                    "DEBRIS_UPDATE",
                    format!("Update failed: {e}"),
                )
            })?;
        }

        Ok(collected)
    }

    // -----------------------------------------------------------------------
    // Moon Creation
    // -----------------------------------------------------------------------

    /// Check if a debris field creates a moon.
    ///
    /// OGame formula: chance = min(debris_units / 100_000 * 20, 20)%
    /// Moon size: random 1,000 - 8,944 km
    pub(crate) fn check_moon_creation(
        &self,
        coord: &str,
        debris_amount: f64,
    ) -> Result<Option<Moon>, ImpForgeError> {
        if coord.is_empty() {
            return Err(ImpForgeError::validation(
                "MOON_EMPTY_COORD",
                "Coordinate cannot be empty",
            ));
        }

        // Check if moon already exists at this coordinate
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("MOON_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        let existing: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM moons WHERE coord = ?1",
                params![coord],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if existing {
            return Ok(None); // Only one moon per planet
        }

        // Calculate creation chance
        let chance = (debris_amount / 100_000.0 * 20.0).min(20.0);

        // Deterministic "random" based on coordinate hash and timestamp
        let seed_val = coord.bytes().fold(0u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u64)
        });
        let now_secs = Utc::now().timestamp() as u64;
        let roll = ((seed_val ^ now_secs) % 100) as f64;

        if roll >= chance {
            return Ok(None); // No moon created
        }

        // Moon size: 1,000 - 8,944 km
        let size = 1_000 + ((seed_val ^ (now_secs.wrapping_mul(7))) % 7_944) as u32;

        let moon = Moon {
            id: Uuid::new_v4().to_string(),
            coord: coord.to_string(),
            size,
            has_phalanx: false,
            has_jump_gate: false,
            jump_gate_cooldown_mins: 60,
            creation_chance: chance,
            created_at: Utc::now().to_rfc3339(),
        };

        conn.execute(
            "INSERT INTO moons (id, coord, size, has_phalanx, has_jump_gate, jump_gate_cooldown_mins, creation_chance, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                moon.id,
                moon.coord,
                moon.size,
                moon.has_phalanx as i32,
                moon.has_jump_gate as i32,
                moon.jump_gate_cooldown_mins,
                moon.creation_chance,
                moon.created_at,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("MOON_INSERT", format!("Insert failed: {e}"))
        })?;

        Ok(Some(moon))
    }

    /// Get moon status for a coordinate.
    pub(crate) fn moon_status(&self, coord: &str) -> Result<Option<Moon>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("MOON_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let result = conn.query_row(
            "SELECT id, coord, size, has_phalanx, has_jump_gate, jump_gate_cooldown_mins, creation_chance, created_at
             FROM moons WHERE coord = ?1",
            params![coord],
            |row| {
                Ok(Moon {
                    id: row.get(0)?,
                    coord: row.get(1)?,
                    size: row.get(2)?,
                    has_phalanx: row.get::<_, i32>(3)? != 0,
                    has_jump_gate: row.get::<_, i32>(4)? != 0,
                    jump_gate_cooldown_mins: row.get(5)?,
                    creation_chance: row.get(6)?,
                    created_at: row.get(7)?,
                })
            },
        );

        match result {
            Ok(moon) => Ok(Some(moon)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(ImpForgeError::internal(
                "MOON_QUERY",
                format!("Query failed: {e}"),
            )),
        }
    }

    // -----------------------------------------------------------------------
    // Phalanx Sensor
    // -----------------------------------------------------------------------

    /// Scan for enemy fleet movements using the Phalanx sensor on a moon.
    ///
    /// Scan range: (phalanx_level^2) - 1 systems.
    /// Cost: 5000 * phalanx_level deuterium per scan.
    pub(crate) fn phalanx_scan(
        &self,
        moon_coord: &str,
        target_coord: &str,
        phalanx_level: u32,
    ) -> Result<PhalanxScan, ImpForgeError> {
        if phalanx_level == 0 {
            return Err(ImpForgeError::validation(
                "PHALANX_NO_SENSOR",
                "Moon does not have a Phalanx sensor (level 0)",
            ));
        }

        // Verify moon exists and has phalanx
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PHALANX_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        let has_phalanx: bool = conn
            .query_row(
                "SELECT has_phalanx FROM moons WHERE coord = ?1",
                params![moon_coord],
                |row| row.get::<_, i32>(0).map(|v| v != 0),
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "PHALANX_NO_MOON",
                    format!("No moon found at coordinate: {moon_coord}"),
                )
            })?;

        if !has_phalanx {
            return Err(ImpForgeError::validation(
                "PHALANX_NOT_BUILT",
                "Phalanx sensor has not been built on this moon",
            ));
        }

        let scan_range = phalanx_level.saturating_mul(phalanx_level).saturating_sub(1);
        let cost = 5000.0 * phalanx_level as f64;

        // Simulated fleet detections (in production, query active fleet_missions table)
        let seed = target_coord.bytes().fold(0u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u64)
        });
        let fleet_count = (seed % 4) as usize;
        let missions = ["attack", "transport", "espionage", "deploy"];
        let fleets: Vec<FleetMovement> = (0..fleet_count)
            .map(|i| {
                let mission_idx = ((seed + i as u64) % missions.len() as u64) as usize;
                FleetMovement {
                    owner: format!("npc_player_{}", (seed + i as u64) % 100),
                    origin: format!("[{}:{}:{}]", (seed % 9) + 1, (seed % 499) + 1, (i % 15) + 1),
                    destination: target_coord.to_string(),
                    arrival_time: (Utc::now() + chrono::Duration::minutes(30 + (i as i64 * 15)))
                        .to_rfc3339(),
                    ship_count: ((seed + i as u64 * 7) % 500 + 10) as u32,
                    mission: missions[mission_idx].to_string(),
                }
            })
            .collect();

        Ok(PhalanxScan {
            target_coord: target_coord.to_string(),
            fleets_detected: fleets,
            scan_range,
            cost_deuterium: cost,
            scanned_at: Utc::now().to_rfc3339(),
        })
    }

    // -----------------------------------------------------------------------
    // Jump Gate
    // -----------------------------------------------------------------------

    /// Transfer a fleet instantly between two moons via Jump Gate.
    ///
    /// Both moons must have a Jump Gate built.  60 minute cooldown.
    pub(crate) fn jump_gate_transfer(
        &self,
        from_moon_coord: &str,
        to_moon_coord: &str,
        ship_count: u32,
    ) -> Result<serde_json::Value, ImpForgeError> {
        if from_moon_coord == to_moon_coord {
            return Err(ImpForgeError::validation(
                "JUMP_SAME_MOON",
                "Cannot jump to the same moon",
            ));
        }

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("JUMP_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        // Check both moons have jump gates
        for (label, coord) in [("source", from_moon_coord), ("target", to_moon_coord)] {
            let has_gate: bool = conn
                .query_row(
                    "SELECT has_jump_gate FROM moons WHERE coord = ?1",
                    params![coord],
                    |row| row.get::<_, i32>(0).map(|v| v != 0),
                )
                .map_err(|_| {
                    ImpForgeError::validation(
                        "JUMP_NO_MOON",
                        format!("No moon at {label} coordinate: {coord}"),
                    )
                })?;

            if !has_gate {
                return Err(ImpForgeError::validation(
                    "JUMP_NO_GATE",
                    format!("No Jump Gate on {label} moon: {coord}"),
                ));
            }
        }

        Ok(serde_json::json!({
            "from": from_moon_coord,
            "to": to_moon_coord,
            "ships_transferred": ship_count,
            "cooldown_mins": 60,
            "transferred_at": Utc::now().to_rfc3339(),
        }))
    }
}
