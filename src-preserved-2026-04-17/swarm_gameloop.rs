// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Game Loop -- Fixed-rate tick simulation + delta-compressed state streaming
//!
//! The game loop runs at a fixed 20 ticks/sec in a dedicated tokio task.
//! State snapshots are delta-compressed and streamed to Svelte/PixiJS via
//! Tauri event channels at display rate.  The frontend renders at 60fps with
//! interpolation between ticks.
//!
//! ## Architecture
//!
//! - Dedicated `tokio::spawn` task running at configurable tick rate (default 20 Hz)
//! - Each tick runs 7 sub-systems: resources, buildings, research, units, terrain, combat, patrols
//! - Delta-compressed `TickDelta` emitted via `app_handle.emit("swarmforge:tick", &delta)`
//! - Full `GameStateSnapshot` emitted every N ticks (default: every 20 = once per second)
//! - Frontend interpolates between snapshots for smooth 60fps rendering
//!
//! ## Sub-system stubs
//!
//! The tick sub-systems (`tick_resources`, `tick_buildings`, etc.) currently return
//! empty `Vec`s.  They will be wired to the actual game state (SwarmDatabase,
//! SwarmFactions, SwarmCombat) in a future pass.  The architecture -- dedicated
//! thread, delta compression, and event streaming pattern -- is what matters now.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::sync::RwLock;

use crate::error::{AppResult, ImpForgeError};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_gameloop", "Game");

// ============================================================================
// Configuration
// ============================================================================

/// Game simulation configuration.
///
/// Controls tick rate, pausing, speed multiplier, and feature toggles
/// for delta compression and frontend interpolation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLoopConfig {
    /// Ticks per second (default 20 = 50ms per tick).
    pub tick_rate: u32,
    /// Maximum allowed tick duration in ms before a "slow tick" warning.
    pub max_tick_duration_ms: u64,
    /// Whether the frontend should interpolate (lerp) between ticks.
    pub interpolation_enabled: bool,
    /// Only send changed state to the frontend (vs full snapshot every tick).
    pub delta_compression: bool,
    /// When true the simulation is frozen (no ticks processed).
    pub paused: bool,
    /// Game speed multiplier. 1.0 = normal, 2.0 = double, 0.5 = half.
    /// Clamped to [0.5, 10.0].
    pub speed_multiplier: f64,
}

impl Default for GameLoopConfig {
    fn default() -> Self {
        Self {
            tick_rate: 20,
            max_tick_duration_ms: 50,
            interpolation_enabled: true,
            delta_compression: true,
            paused: true, // starts paused until the player loads a game
            speed_multiplier: 1.0,
        }
    }
}

// ============================================================================
// Delta types -- one struct per sub-system
// ============================================================================

/// A single game tick's worth of state changes.
///
/// Emitted on every tick via `swarmforge:tick` Tauri event.
/// The frontend applies deltas on top of its last known state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickDelta {
    pub tick_number: u64,
    pub timestamp_ms: u64,
    pub resource_changes: Vec<ResourceDelta>,
    pub unit_movements: Vec<UnitMoveDelta>,
    pub building_progress: Vec<BuildingDelta>,
    pub research_progress: Vec<ResearchDelta>,
    pub combat_events: Vec<CombatDelta>,
    pub terrain_changes: Vec<TerrainDelta>,
    pub notifications: Vec<GameNotification>,
}

/// Change in a colony's resource amount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDelta {
    pub colony_id: String,
    pub resource: String,
    pub old_value: f64,
    pub new_value: f64,
    pub rate_per_hour: f64,
}

/// Movement of a single unit between two positions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitMoveDelta {
    pub unit_id: String,
    pub old_x: f64,
    pub old_y: f64,
    pub new_x: f64,
    pub new_y: f64,
    /// Facing angle in radians.
    pub facing: f64,
}

/// Construction progress on a building.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingDelta {
    pub building_id: String,
    pub colony_id: String,
    /// Progress fraction in [0.0, 1.0].
    pub progress: f64,
    pub completed: bool,
    pub building_type: String,
    pub level: u32,
}

/// Research progress on a technology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchDelta {
    pub tech_id: String,
    /// Progress fraction in [0.0, 1.0].
    pub progress: f64,
    pub completed: bool,
}

/// A combat event from one tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatDelta {
    pub battle_id: String,
    pub attacker: String,
    pub defender: String,
    pub damage_dealt: f64,
    pub units_destroyed: u32,
}

/// A terrain change on a hex tile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainDelta {
    pub hex_q: i32,
    pub hex_r: i32,
    pub terrain_type: String,
    pub strength_change: f64,
}

/// In-game notification surfaced to the player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameNotification {
    /// One of: "build_complete", "research_done", "under_attack", "fleet_arrived", etc.
    pub notification_type: String,
    pub message: String,
    /// 1 = low, 3 = medium, 5 = critical.
    pub priority: u8,
    pub colony_id: Option<String>,
}

// ============================================================================
// Full snapshot (initial sync or reconnect)
// ============================================================================

/// Full game state snapshot sent on first connect or when the client
/// requests a full resync (e.g. after a tab was backgrounded).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateSnapshot {
    pub tick_number: u64,
    pub colonies: Vec<ColonySnapshot>,
    pub active_fleets: u32,
    pub active_patrols: u32,
    pub active_research: u32,
    pub active_builds: u32,
    pub total_units: u32,
    pub game_time_hours: f64,
}

/// Snapshot of a single colony's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonySnapshot {
    pub id: String,
    pub name: String,
    pub faction: String,
    pub resources: HashMap<String, f64>,
    pub production_rates: HashMap<String, f64>,
    pub building_count: u32,
    pub unit_count: u32,
    pub defense_power: f64,
}

// ============================================================================
// Game Loop Engine
// ============================================================================

/// The core simulation engine.
///
/// Holds all shared state behind `Arc<RwLock<_>>` so the spawned tokio task
/// and the Tauri command handlers can both access it safely.
pub(crate) struct GameLoopEngine {
    config: Arc<RwLock<GameLoopConfig>>,
    tick_count: Arc<RwLock<u64>>,
    running: Arc<RwLock<bool>>,
    last_delta: Arc<RwLock<Option<TickDelta>>>,
    state_snapshot: Arc<RwLock<Option<GameStateSnapshot>>>,
}
impl GameLoopEngine {
    pub(crate) fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(GameLoopConfig::default())),
            tick_count: Arc::new(RwLock::new(0)),
            running: Arc::new(RwLock::new(false)),
            last_delta: Arc::new(RwLock::new(None)),
            state_snapshot: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the game loop in a dedicated tokio task.
    ///
    /// Idempotent: calling `start` when already running is a no-op.
    /// The task emits `swarmforge:tick` events on every tick and
    /// `swarmforge:snapshot` every `tick_rate` ticks (once per second at 20 Hz).
    pub(crate) async fn start(&self, app_handle: tauri::AppHandle) -> AppResult<()> {
        {
            let running = self.running.read().await;
            if *running {
                return Ok(()); // already running -- idempotent
            }
        }

        *self.running.write().await = true;

        let config = Arc::clone(&self.config);
        let tick_count = Arc::clone(&self.tick_count);
        let running = Arc::clone(&self.running);
        let last_delta = Arc::clone(&self.last_delta);
        let state_snapshot = Arc::clone(&self.state_snapshot);

        tokio::spawn(async move {
            loop {
                // --- Check if we should stop ---
                if !*running.read().await {
                    break;
                }

                let cfg = config.read().await.clone();

                // --- Paused: sleep longer, don't tick ---
                if cfg.paused {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }

                let tick_start = Instant::now();
                let tick_interval_ms = 1000u64
                    .checked_div(cfg.tick_rate.max(1) as u64)
                    .unwrap_or(50);

                // === ADVANCE TICK COUNTER ===
                let tick_num = {
                    let mut count = tick_count.write().await;
                    *count += 1;
                    *count
                };

                let delta_time = (1.0 / cfg.tick_rate.max(1) as f64) * cfg.speed_multiplier;

                // === RUN SUB-SYSTEMS ===
                // Each returns a Vec of deltas (currently stubs).
                let resource_changes = Self::tick_resources(delta_time);
                let building_progress = Self::tick_buildings(delta_time);
                let research_progress = Self::tick_research(delta_time);
                let unit_movements = Self::tick_units(delta_time);
                let terrain_changes = Self::tick_terrain(delta_time);
                let combat_events = Self::tick_combat(delta_time);
                let _patrol_events = Self::tick_patrols(delta_time);

                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                // === BUILD DELTA ===
                let delta = TickDelta {
                    tick_number: tick_num,
                    timestamp_ms: now_ms,
                    resource_changes,
                    unit_movements,
                    building_progress,
                    research_progress,
                    combat_events,
                    terrain_changes,
                    notifications: Vec::new(),
                };

                // Store for polling commands
                *last_delta.write().await = Some(delta.clone());

                // Emit to frontend via Tauri event channel
                let _ = app_handle.emit("swarmforge:tick", &delta);

                // === FULL SNAPSHOT every tick_rate ticks (once per second) ===
                let snapshot_interval = cfg.tick_rate.max(1) as u64;
                if tick_num % snapshot_interval == 0 {
                    let snapshot = GameStateSnapshot {
                        tick_number: tick_num,
                        colonies: Vec::new(), // populated from DB in future pass
                        active_fleets: 0,
                        active_patrols: 0,
                        active_research: 0,
                        active_builds: 0,
                        total_units: 0,
                        game_time_hours: tick_num as f64
                            / (cfg.tick_rate.max(1) as f64 * 3600.0),
                    };
                    *state_snapshot.write().await = Some(snapshot);
                }

                // === TICK BUDGET ENFORCEMENT ===
                let elapsed_ms = tick_start.elapsed().as_millis() as u64;
                if elapsed_ms > cfg.max_tick_duration_ms {
                    log::warn!(
                        "Slow tick #{tick_num}: {elapsed_ms}ms (budget: {}ms)",
                        cfg.max_tick_duration_ms
                    );
                }
                if elapsed_ms < tick_interval_ms {
                    tokio::time::sleep(tokio::time::Duration::from_millis(
                        tick_interval_ms - elapsed_ms,
                    ))
                    .await;
                }
            }
            log::info!("SwarmForge game loop stopped");
        });

        log::info!("SwarmForge game loop started");
        Ok(())
    }

    /// Stop the game loop.  The spawned task will exit on its next iteration.
    pub(crate) async fn stop(&self) {
        *self.running.write().await = false;
    }

    /// Pause or resume the simulation.
    pub(crate) async fn set_paused(&self, paused: bool) {
        self.config.write().await.paused = paused;
    }

    /// Set game speed multiplier (clamped to [0.5, 10.0]).
    pub(crate) async fn set_speed(&self, multiplier: f64) {
        self.config.write().await.speed_multiplier = multiplier.clamp(0.5, 10.0);
    }

    /// Whether the loop task is currently running.
    pub(crate) async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Current tick count.
    pub(crate) async fn tick_count(&self) -> u64 {
        *self.tick_count.read().await
    }

    /// Last emitted delta (if any).
    pub(crate) async fn last_delta(&self) -> Option<TickDelta> {
        self.last_delta.read().await.clone()
    }

    /// Last full state snapshot (if any).
    pub(crate) async fn snapshot(&self) -> Option<GameStateSnapshot> {
        self.state_snapshot.read().await.clone()
    }

    /// Current configuration (read-only copy).
    pub(crate) async fn get_config(&self) -> GameLoopConfig {
        self.config.read().await.clone()
    }

    // ========================================================================
    // Tick sub-systems (stubs -- return empty Vecs)
    //
    // These will be wired to SwarmDatabase / SwarmFactions / SwarmCombat
    // in a future pass.  The empty implementations let the loop architecture
    // compile and run end-to-end now.
    // ========================================================================
    fn tick_resources(_dt: f64) -> Vec<ResourceDelta> {
        Vec::new()
    }
    fn tick_buildings(_dt: f64) -> Vec<BuildingDelta> {
        Vec::new()
    }
    fn tick_research(_dt: f64) -> Vec<ResearchDelta> {
        Vec::new()
    }
    fn tick_units(_dt: f64) -> Vec<UnitMoveDelta> {
        Vec::new()
    }
    fn tick_terrain(_dt: f64) -> Vec<TerrainDelta> {
        Vec::new()
    }
    fn tick_combat(_dt: f64) -> Vec<CombatDelta> {
        Vec::new()
    }
    fn tick_patrols(_dt: f64) -> Vec<serde_json::Value> {
        Vec::new()
    }
}

// ============================================================================
// Static engine instance (process-wide singleton)
// ============================================================================

static GAME_LOOP: Lazy<GameLoopEngine> = Lazy::new(GameLoopEngine::new);

// ============================================================================
// Tauri Commands (8)
// ============================================================================

/// Start the game loop.  Idempotent -- calling when already running is a no-op.
#[tauri::command]
pub(crate) async fn gameloop_start(app: tauri::AppHandle) -> AppResult<String> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_gameloop", "game_gameloop", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_gameloop", "game_gameloop");
    crate::synapse_fabric::synapse_session_push("swarm_gameloop", "game_gameloop", "gameloop_start called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_gameloop", "info", "swarm_gameloop active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_gameloop", "tick", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"action": "start"}));
    GAME_LOOP.start(app).await?;
    Ok("Game loop started".to_string())
}

/// Stop the game loop.  The spawned task exits on its next iteration.
#[tauri::command]
pub(crate) async fn gameloop_stop() -> AppResult<String> {
    GAME_LOOP.stop().await;
    Ok("Game loop stopped".to_string())
}

/// Pause or resume the simulation.
#[tauri::command]
pub(crate) async fn gameloop_pause(paused: bool) -> AppResult<String> {
    GAME_LOOP.set_paused(paused).await;
    let state = if paused { "paused" } else { "resumed" };
    Ok(format!("Game loop {state}"))
}

/// Set the game speed multiplier (clamped to [0.5, 10.0]).
#[tauri::command]
pub(crate) async fn gameloop_set_speed(multiplier: f64) -> AppResult<String> {
    if !multiplier.is_finite() {
        return Err(ImpForgeError::validation(
            "INVALID_SPEED",
            "Speed multiplier must be a finite number",
        ));
    }
    GAME_LOOP.set_speed(multiplier).await;
    let actual = GAME_LOOP.get_config().await.speed_multiplier;
    Ok(format!("Speed set to {actual:.1}x"))
}

/// Get the current game loop configuration.
#[tauri::command]
pub(crate) async fn gameloop_config() -> AppResult<GameLoopConfig> {
    Ok(GAME_LOOP.get_config().await)
}

/// Get the current tick count.
#[tauri::command]
pub(crate) async fn gameloop_tick_count() -> AppResult<u64> {
    Ok(GAME_LOOP.tick_count().await)
}

/// Get the last emitted tick delta (if any).
#[tauri::command]
pub(crate) async fn gameloop_last_delta() -> AppResult<Option<TickDelta>> {
    Ok(GAME_LOOP.last_delta().await)
}

/// Get the last full state snapshot (if any).
#[tauri::command]
pub(crate) async fn gameloop_snapshot() -> AppResult<Option<GameStateSnapshot>> {
    Ok(GAME_LOOP.snapshot().await)
}

// ============================================================================
// Additional Tauri Commands — wiring internal helpers
// ============================================================================

/// Check if the game loop is currently running.
#[tauri::command]
pub(crate) async fn gameloop_is_running() -> AppResult<bool> {
    Ok(GAME_LOOP.is_running().await)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;


    // --- Config ---

    #[test]
    fn test_default_config() {
        let cfg = GameLoopConfig::default();
        assert_eq!(cfg.tick_rate, 20);
        assert_eq!(cfg.max_tick_duration_ms, 50);
        assert!(cfg.interpolation_enabled);
        assert!(cfg.delta_compression);
        assert!(cfg.paused); // starts paused
        assert!((cfg.speed_multiplier - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_serialization() {
        let cfg = GameLoopConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize config");
        let parsed: GameLoopConfig = serde_json::from_str(&json).expect("deserialize config");
        assert_eq!(parsed.tick_rate, cfg.tick_rate);
        assert_eq!(parsed.paused, cfg.paused);
        assert!((parsed.speed_multiplier - cfg.speed_multiplier).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_custom_values() {
        let cfg = GameLoopConfig {
            tick_rate: 30,
            max_tick_duration_ms: 33,
            interpolation_enabled: false,
            delta_compression: false,
            paused: false,
            speed_multiplier: 3.5,
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        assert!(json.contains("30"));
        assert!(json.contains("3.5"));
    }

    // --- Delta types ---

    #[test]
    fn test_tick_delta_serialization() {
        let delta = TickDelta {
            tick_number: 42,
            timestamp_ms: 1_700_000_000_000,
            resource_changes: vec![ResourceDelta {
                colony_id: "c1".to_string(),
                resource: "metal".to_string(),
                old_value: 100.0,
                new_value: 105.0,
                rate_per_hour: 360.0,
            }],
            unit_movements: Vec::new(),
            building_progress: Vec::new(),
            research_progress: Vec::new(),
            combat_events: Vec::new(),
            terrain_changes: Vec::new(),
            notifications: Vec::new(),
        };
        let json = serde_json::to_string(&delta).expect("serialize delta");
        assert!(json.contains("\"tick_number\":42"));
        assert!(json.contains("\"metal\""));
        assert!(json.contains("105"));
    }

    #[test]
    fn test_resource_delta_fields() {
        let rd = ResourceDelta {
            colony_id: "colony_abc".to_string(),
            resource: "crystal".to_string(),
            old_value: 50.0,
            new_value: 55.5,
            rate_per_hour: 120.0,
        };
        assert_eq!(rd.colony_id, "colony_abc");
        assert!((rd.new_value - 55.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_unit_move_delta() {
        let um = UnitMoveDelta {
            unit_id: "u1".to_string(),
            old_x: 0.0,
            old_y: 0.0,
            new_x: 10.0,
            new_y: 5.0,
            facing: std::f64::consts::FRAC_PI_2,
        };
        let json = serde_json::to_string(&um).expect("serialize");
        assert!(json.contains("\"unit_id\":\"u1\""));
    }

    #[test]
    fn test_building_delta() {
        let bd = BuildingDelta {
            building_id: "b1".to_string(),
            colony_id: "c1".to_string(),
            progress: 0.75,
            completed: false,
            building_type: "metal_mine".to_string(),
            level: 3,
        };
        assert!(!bd.completed);
        assert_eq!(bd.level, 3);
    }

    #[test]
    fn test_research_delta() {
        let rd = ResearchDelta {
            tech_id: "laser_tech".to_string(),
            progress: 1.0,
            completed: true,
        };
        assert!(rd.completed);
        assert!((rd.progress - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_combat_delta() {
        let cd = CombatDelta {
            battle_id: "battle_1".to_string(),
            attacker: "player_a".to_string(),
            defender: "player_b".to_string(),
            damage_dealt: 150.0,
            units_destroyed: 3,
        };
        assert_eq!(cd.units_destroyed, 3);
    }

    #[test]
    fn test_terrain_delta() {
        let td = TerrainDelta {
            hex_q: 5,
            hex_r: -3,
            terrain_type: "lava".to_string(),
            strength_change: -0.1,
        };
        let json = serde_json::to_string(&td).expect("serialize");
        assert!(json.contains("\"hex_q\":5"));
        assert!(json.contains("\"lava\""));
    }

    #[test]
    fn test_game_notification() {
        let n = GameNotification {
            notification_type: "build_complete".to_string(),
            message: "Metal Mine Level 5 finished".to_string(),
            priority: 3,
            colony_id: Some("c1".to_string()),
        };
        assert_eq!(n.priority, 3);

        let n_no_colony = GameNotification {
            notification_type: "research_done".to_string(),
            message: "Laser Tech researched".to_string(),
            priority: 2,
            colony_id: None,
        };
        let json = serde_json::to_string(&n_no_colony).expect("serialize");
        assert!(json.contains("\"research_done\""));
    }

    // --- Snapshot ---

    #[test]
    fn test_snapshot_serialization() {
        let snap = GameStateSnapshot {
            tick_number: 1000,
            colonies: vec![ColonySnapshot {
                id: "c1".to_string(),
                name: "Homeworld".to_string(),
                faction: "insects".to_string(),
                resources: HashMap::from([("metal".to_string(), 500.0)]),
                production_rates: HashMap::from([("metal".to_string(), 60.0)]),
                building_count: 5,
                unit_count: 12,
                defense_power: 150.0,
            }],
            active_fleets: 2,
            active_patrols: 1,
            active_research: 1,
            active_builds: 3,
            total_units: 12,
            game_time_hours: 0.5,
        };
        let json = serde_json::to_string(&snap).expect("serialize snapshot");
        assert!(json.contains("\"Homeworld\""));
        assert!(json.contains("\"insects\""));
    }

    #[test]
    fn test_colony_snapshot_empty_maps() {
        let cs = ColonySnapshot {
            id: "c2".to_string(),
            name: "Outpost".to_string(),
            faction: "demons".to_string(),
            resources: HashMap::new(),
            production_rates: HashMap::new(),
            building_count: 0,
            unit_count: 0,
            defense_power: 0.0,
        };
        let json = serde_json::to_string(&cs).expect("serialize");
        assert!(json.contains("\"Outpost\""));
    }

    // --- Engine ---

    #[tokio::test]
    async fn test_engine_new_defaults() {
        let engine = GameLoopEngine::new();
        assert!(!engine.is_running().await);
        assert_eq!(engine.tick_count().await, 0);
        assert!(engine.last_delta().await.is_none());
        assert!(engine.snapshot().await.is_none());
    }

    #[tokio::test]
    async fn test_engine_config_defaults() {
        let engine = GameLoopEngine::new();
        let cfg = engine.get_config().await;
        assert_eq!(cfg.tick_rate, 20);
        assert!(cfg.paused);
    }

    #[tokio::test]
    async fn test_engine_set_speed_clamp() {
        let engine = GameLoopEngine::new();

        engine.set_speed(0.1).await;
        assert!((engine.get_config().await.speed_multiplier - 0.5).abs() < f64::EPSILON);

        engine.set_speed(100.0).await;
        assert!((engine.get_config().await.speed_multiplier - 10.0).abs() < f64::EPSILON);

        engine.set_speed(3.0).await;
        assert!((engine.get_config().await.speed_multiplier - 3.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_engine_pause_unpause() {
        let engine = GameLoopEngine::new();
        assert!(engine.get_config().await.paused);

        engine.set_paused(false).await;
        assert!(!engine.get_config().await.paused);

        engine.set_paused(true).await;
        assert!(engine.get_config().await.paused);
    }

    #[tokio::test]
    async fn test_engine_stop_without_start() {
        // Stopping a loop that was never started should be harmless.
        let engine = GameLoopEngine::new();
        engine.stop().await;
        assert!(!engine.is_running().await);
    }

    // --- Tick sub-system stubs ---

    #[test]
    fn test_tick_resources_stub() {
        let result = GameLoopEngine::tick_resources(0.05);
        assert!(result.is_empty());
    }

    #[test]
    fn test_tick_buildings_stub() {
        let result = GameLoopEngine::tick_buildings(0.05);
        assert!(result.is_empty());
    }

    #[test]
    fn test_tick_research_stub() {
        let result = GameLoopEngine::tick_research(0.05);
        assert!(result.is_empty());
    }

    #[test]
    fn test_tick_units_stub() {
        let result = GameLoopEngine::tick_units(0.05);
        assert!(result.is_empty());
    }

    #[test]
    fn test_tick_terrain_stub() {
        let result = GameLoopEngine::tick_terrain(0.05);
        assert!(result.is_empty());
    }

    #[test]
    fn test_tick_combat_stub() {
        let result = GameLoopEngine::tick_combat(0.05);
        assert!(result.is_empty());
    }

    #[test]
    fn test_tick_patrols_stub() {
        let result = GameLoopEngine::tick_patrols(0.05);
        assert!(result.is_empty());
    }

    // --- Notification priority ---

    #[test]
    fn test_notification_priority_range() {
        for p in [1u8, 2, 3, 4, 5] {
            let n = GameNotification {
                notification_type: "test".to_string(),
                message: format!("priority {p}"),
                priority: p,
                colony_id: None,
            };
            assert!(n.priority >= 1 && n.priority <= 5);
        }
    }

    // --- Delta with all fields populated ---

    #[test]
    fn test_full_tick_delta() {
        let delta = TickDelta {
            tick_number: 999,
            timestamp_ms: 1_700_000_000_000,
            resource_changes: vec![ResourceDelta {
                colony_id: "c1".into(),
                resource: "metal".into(),
                old_value: 100.0,
                new_value: 110.0,
                rate_per_hour: 720.0,
            }],
            unit_movements: vec![UnitMoveDelta {
                unit_id: "u1".into(),
                old_x: 0.0,
                old_y: 0.0,
                new_x: 1.0,
                new_y: 1.0,
                facing: 0.0,
            }],
            building_progress: vec![BuildingDelta {
                building_id: "b1".into(),
                colony_id: "c1".into(),
                progress: 0.5,
                completed: false,
                building_type: "crystal_mine".into(),
                level: 2,
            }],
            research_progress: vec![ResearchDelta {
                tech_id: "shields".into(),
                progress: 0.3,
                completed: false,
            }],
            combat_events: vec![CombatDelta {
                battle_id: "bat1".into(),
                attacker: "p1".into(),
                defender: "p2".into(),
                damage_dealt: 42.0,
                units_destroyed: 1,
            }],
            terrain_changes: vec![TerrainDelta {
                hex_q: 0,
                hex_r: 0,
                terrain_type: "forest".into(),
                strength_change: 0.05,
            }],
            notifications: vec![GameNotification {
                notification_type: "under_attack".into(),
                message: "Colony c1 is under attack!".into(),
                priority: 5,
                colony_id: Some("c1".into()),
            }],
        };

        let json = serde_json::to_string(&delta).expect("serialize full delta");
        assert!(json.contains("\"tick_number\":999"));
        assert!(json.contains("\"under_attack\""));
        assert!(json.contains("\"crystal_mine\""));
        assert!(json.contains("\"shields\""));
    }
}
