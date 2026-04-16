// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Advanced -- Type definitions (commander dashboard, standalone
//! release config, OGame mechanics).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::ImpForgeError;

/// Health declaration for this sub-module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_advanced::types", "Game");

/// Commander authentication passphrase — only the developer can access the
/// Human Faction Commander dashboard.  Grants NO cheats, just a global view.
pub(crate) const COMMANDER_PASSPHRASE: &str = "AiImp-Commander-2026-HumanFaction";

/// Human Faction Commander state — the developer's control panel.
///
/// The developer (Karsten) controls the ENTIRE Human faction with the same
/// rules / resources / combat as every other player — NO cheats, just a
/// global dashboard.  When offline, NPC AI takes over with standing orders.
/// This is NOT a cheat mode. All game rules apply normally.
/// The commander just has a global view of ALL Human colonies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanCommanderState {
    pub authenticated: bool,
    /// Global strategy for the entire Human faction
    pub global_strategy: FactionStrategy,
    /// Whether NPC AI is currently managing (developer offline)
    pub npc_ai_active: bool,
    /// Standing orders that NPC AI follows when developer is away
    pub standing_orders: StandingOrders,
    /// All Human colony IDs under command
    pub colonies: Vec<String>,
    /// Total faction power score
    pub total_power: u64,
    /// Last time the developer was actively commanding
    pub last_active: String,
    /// How long the NPC AI has been running autonomously
    pub npc_autonomous_hours: f64,
    /// Kill-rate limiter (Helldivers 2 "Joel" pattern)
    pub kill_rate_limit: KillRateLimit,
    /// Global alertness counter -- rises with attacks, decays over time
    pub alertness: AlertnessCounter,
    /// Auto-play AI configuration (AlphaStar throttling)
    pub auto_play: AutoPlayConfig,
}

/// Global strategy the Human faction follows
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum FactionStrategy {
    #[default]
    Balanced,          // Even mix of economy and military
    Expansionist,      // Prioritize colonizing new planets
    Militaristic,      // Build army, pressure other factions
    Defensive,         // Fortify existing colonies
    Economic,          // Maximize resource production
    Diplomatic,        // Focus on trade and alliances
    Aggressive,        // Active warfare against other factions
}

/// Standing orders for NPC AI when developer is offline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingOrders {
    /// Auto-build new buildings when resources available
    pub auto_build: bool,
    /// Auto-research next technology in queue
    pub auto_research: bool,
    /// Auto-train military units to maintain army size
    pub auto_train: bool,
    /// Minimum army size to maintain per colony
    pub min_army_per_colony: u32,
    /// Auto-defend: respond to attacks automatically
    pub auto_defend: bool,
    /// Auto-expand: colonize new planets when able
    pub auto_expand: bool,
    /// Auto-trade: accept profitable trade offers
    pub auto_trade: bool,
    /// Auto-espionage: plant agents in enemy colonies
    pub auto_espionage: bool,
    /// Resource priority: which resource to focus on
    pub resource_priority: String,
    /// Diplomatic stance toward each faction
    pub diplomatic_stances: HashMap<String, String>,
}

impl Default for StandingOrders {
    fn default() -> Self {
        let mut stances = HashMap::new();
        stances.insert("insects".into(), "hostile".into());
        stances.insert("demons".into(), "hostile".into());
        stances.insert("undead".into(), "hostile".into());

        Self {
            auto_build: true,
            auto_research: true,
            auto_train: true,
            min_army_per_colony: 100,
            auto_defend: true,
            auto_expand: true,
            auto_trade: true,
            auto_espionage: true,
            resource_priority: "balanced".into(),
            diplomatic_stances: stances,
        }
    }
}

impl Default for HumanCommanderState {
    fn default() -> Self {
        Self {
            authenticated: false,
            global_strategy: FactionStrategy::default(),
            npc_ai_active: true, // NPC runs by default until developer logs in
            standing_orders: StandingOrders::default(),
            colonies: Vec::new(),
            total_power: 0,
            last_active: String::new(),
            npc_autonomous_hours: 0.0,
            kill_rate_limit: KillRateLimit::default(),
            alertness: AlertnessCounter::default(),
            auto_play: AutoPlayConfig::default(),
        }
    }
}

impl HumanCommanderState {
    /// Developer takes command — NPC AI hands off control
    pub(crate) fn take_command(&mut self) {
        self.authenticated = true;
        self.npc_ai_active = false;
        self.last_active = chrono::Utc::now().to_rfc3339();
    }

    /// Developer goes offline — NPC AI takes over with standing orders
    pub(crate) fn release_command(&mut self) {
        self.npc_ai_active = true;
    }

    /// Check authentication
    pub(crate) fn require_auth(&self) -> Result<(), ImpForgeError> {
        if !self.authenticated {
            return Err(ImpForgeError::validation(
                "COMMANDER_NOT_AUTH",
                "Human Faction Commander access requires authentication.",
            ));
        }
        Ok(())
    }
}

/// Faction-wide event that the commander can trigger (same game rules apply!)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommanderDirective {
    pub id: String,
    pub directive_type: DirectiveType,
    pub target: String,
    pub issued_at: String,
    pub status: String,
}

/// Types of directives the commander can issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DirectiveType {
    RaidColony { target_coord: String },     // Send attack fleet (uses real units!)
    TradeOffer { target_player: String },     // Propose trade (real resources)
    DiplomaticMessage { target: String },     // Send diplomacy
    DefendColony { colony_id: String },       // Reinforce a colony
    ExpandTo { target_coord: String },        // Send colony ship
    ScoutArea { target_coord: String },       // Send spy probes
    SetPriority { colony_id: String, priority: String }, // Change colony focus
}

/// Result of a batch battle simulation for balance testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleSimResult {
    pub total_battles: u32,
    pub faction_a_wins: u32,
    pub faction_b_wins: u32,
    pub draws: u32,
    pub win_rate_a: f64,
    pub win_rate_b: f64,
    pub avg_rounds: f64,
    pub avg_survivors_a: f64,
    pub avg_survivors_b: f64,
}

/// NPC AI blackboard snapshot for debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiBlackboard {
    pub npc_id: String,
    pub current_goal: String,
    pub priority_queue: Vec<String>,
    pub resource_evaluation: HashMap<String, f64>,
    pub threat_assessment: HashMap<String, f64>,
    pub last_decision: String,
    pub tick_count: u64,
}

// ============================================================================
// PART 2: Standalone Release System
// ============================================================================

/// How SwarmForge was launched.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchMode {
    /// SwarmForge launched as its own app (e.g. from Steam or desktop shortcut)
    Standalone,
    /// Launched from within ImpForge (gets Dark Matter integration)
    EmbeddedInImpForge,
}

/// Configuration detected at startup based on the launch environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandaloneConfig {
    pub launch_mode: LaunchMode,
    pub impforge_integration: bool,
    /// Only true in embedded mode -- productivity earns Dark Matter
    pub dm_from_productivity: bool,
    /// True if launched from Steam (overlay available)
    pub steam_overlay: bool,
    pub version: String,
    /// "standalone" or "embedded"
    pub build_type: String,
}

impl StandaloneConfig {
    /// Auto-detect launch mode from environment variables.
    pub(crate) fn detect() -> Self {
        let is_embedded = std::env::var("IMPFORGE_EMBEDDED").is_ok();

        Self {
            launch_mode: if is_embedded {
                LaunchMode::EmbeddedInImpForge
            } else {
                LaunchMode::Standalone
            },
            impforge_integration: is_embedded,
            dm_from_productivity: is_embedded,
            steam_overlay: std::env::var("STEAM_OVERLAY").is_ok(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_type: if is_embedded {
                "embedded".to_string()
            } else {
                "standalone".to_string()
            },
        }
    }
}

/// Feature availability matrix based on launch mode and tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureAvailability {
    /// Embedded only -- productivity tasks earn Dark Matter
    pub dark_matter_from_productivity: bool,
    /// Embedded only -- document/spreadsheet integration
    pub office_suite_integration: bool,
    /// Embedded only -- CodeForge IDE integration
    pub ide_integration: bool,
    /// Embedded only -- ForgeFlow workflow triggers
    pub workflow_integration: bool,
    /// Always available
    pub standalone_game: bool,
    /// Always available
    pub multiplayer: bool,
    /// Always available
    pub achievements: bool,
    /// Pro tier only
    pub cloud_save: bool,
}

impl FeatureAvailability {
    /// Build the feature matrix from the current config.
    pub(crate) fn from_config(config: &StandaloneConfig) -> Self {
        let embedded = config.impforge_integration;
        Self {
            dark_matter_from_productivity: embedded,
            office_suite_integration: embedded,
            ide_integration: embedded,
            workflow_integration: embedded,
            standalone_game: true,
            multiplayer: true,
            achievements: true,
            cloud_save: false, // requires Pro subscription
        }
    }
}

// ============================================================================
// PART 4: Commander Balance Mechanisms (Helldivers 2 "Joel" + AlphaStar)
// ============================================================================
//
// Prevents unfair play by limiting kill rates, tracking alertness, enabling
// trade-for-protection, and throttling auto-play AI to human-like APM.

/// 20% Kill-Rate Limitation -- prevents steamrolling weak players.
///
/// Based on Helldivers 2 "Joel" game-master pattern: the system dynamically
/// limits how aggressively the commander can destroy weak opponents.
/// If kill rate exceeds 20% in the last hour, attacks are temporarily blocked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillRateLimit {
    /// Maximum allowed kill rate (0.20 = 20%)
    pub max_kill_rate: f64,
    /// Current rolling average kill rate
    pub current_kill_rate: f64,
    /// Number of units killed in the last hour
    pub kills_last_hour: u32,
    /// Number of attacks launched in the last hour
    pub attacks_last_hour: u32,
    /// True when the kill rate has been exceeded -- attacks blocked
    pub is_limited: bool,
}

impl Default for KillRateLimit {
    fn default() -> Self {
        Self {
            max_kill_rate: 0.20,
            current_kill_rate: 0.0,
            kills_last_hour: 0,
            attacks_last_hour: 0,
            is_limited: false,
        }
    }
}

/// Global Human Alertness Counter -- penalizes aggressive behavior.
///
/// Every attack raises the alert level across all human NPC settlements.
/// Higher alertness means stronger NPC defenders, faster reinforcement,
/// and coordinated counter-attacks.  Repeated attacks on the same settlement
/// carry a 1.5x multiplier.  Alertness slowly decays at 0.05 per hour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertnessCounter {
    /// Current alert level from 0.0 (calm) to 1.0 (maximum alert)
    pub level: f64,
    /// How much alertness decays per game-hour (default 0.05)
    pub decay_per_hour: f64,
    /// How much each new attack raises alertness (default 0.10)
    pub increase_per_attack: f64,
    /// Track how many times each settlement was attacked
    pub settlements_attacked: HashMap<String, u32>,
    /// Multiplier for repeated attacks on the same settlement (default 1.5)
    pub repeated_attack_multiplier: f64,
}

impl Default for AlertnessCounter {
    fn default() -> Self {
        Self {
            level: 0.0,
            decay_per_hour: 0.05,
            increase_per_attack: 0.10,
            settlements_attacked: HashMap::new(),
            repeated_attack_multiplier: 1.5,
        }
    }
}

/// Human trade offer -- costs large resources, grants temporary protection.
///
/// Players can bribe the Human faction commander for a ceasefire.
/// Minimum offer: 10,000 resources = 24 hours protection.
/// Maximum: 70,000+ resources = 168 hours (1 week) protection.
/// Formula: protection_hours = min(168, (resources / 10_000) * 24)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanTradeOffer {
    pub player_id: String,
    /// Resources the player offered
    pub resources_offered: f64,
    /// 24-168 hours based on amount (cost = 5h of player's hourly production per 24h)
    pub protection_hours: u32,
    /// Whether the commander accepted the offer
    pub accepted: bool,
    /// ISO-8601 expiration timestamp
    pub expires_at: String,
    /// If the player attacks Humans during protection, this becomes true and protection is voided
    pub voided_on_attack: bool,
}

// ============================================================================
// PART 5: Auto-Play AI System (AlphaStar Throttling)
// ============================================================================
//
// When the developer is offline the AI plays automatically, but is throttled
// to human-like action rates.  Based on DeepMind's AlphaStar constraints:
// max 22 raw actions per 5-second window, 200-300 sustained APM, 500 burst
// APM for 2-second micro windows, and 200ms artificial reaction delay.

/// AI auto-play configuration (when developer is offline).
///
/// Limits match AlphaStar's published constraints to keep the AI fair.
/// A subscription bonus can raise efficiency slightly but never removes
/// the fundamental APM cap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPlayConfig {
    /// Maximum raw actions per 5-second window (AlphaStar: 22)
    pub max_actions_per_5s: u32,
    /// Sustained actions per minute (200-300)
    pub sustained_apm: u32,
    /// Burst actions per minute for micro windows (up to 500)
    pub burst_apm: u32,
    /// Duration of burst window in seconds (2.0)
    pub burst_window_secs: f64,
    /// Artificial reaction delay in milliseconds (200ms)
    pub reaction_delay_ms: u32,
    /// Base efficiency multiplier when offline (0.8 = 80%)
    pub offline_efficiency: f64,
    /// Subscription tier bonus (1.0 free, 1.2 pro)
    pub subscription_bonus: f64,
}

impl Default for AutoPlayConfig {
    fn default() -> Self {
        Self {
            max_actions_per_5s: 22,
            sustained_apm: 250,
            burst_apm: 500,
            burst_window_secs: 2.0,
            reaction_delay_ms: 200,
            offline_efficiency: 0.8,
            subscription_bonus: 1.0,
        }
    }
}

/// 3-Layer AI Architecture for auto-play decision-making.
///
/// - Strategic: utility-function scoring to evaluate and prioritize directives.
/// - Tactical: behavior-tree nodes for build orders, army composition, scouting.
/// - Execution: finite state machines for unit micro, throttled by APM limits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AiLayer {
    /// Utility scoring -- evaluate directives, prioritize goals
    Strategic,
    /// Behavior trees -- build orders, army management, scouting
    Tactical,
    /// FSMs -- unit micro with APM throttling
    Execution,
}

// ============================================================================
// PART 6: Display Mode Configuration
// ============================================================================

/// How SwarmForge is displayed within ImpForge or as standalone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    /// Full immersive experience (game takes whole window)
    Fullscreen,
    /// Side window while working in ImpForge
    CompanionWindow,
    /// Compact sidebar (minimal, shows key stats only)
    Sidebar,
    /// SwarmForge hidden, only offline progression runs
    Deactivated,
}

impl DisplayMode {
    /// Parse a display mode from a string, defaulting to CompanionWindow.
    pub(crate) fn from_str_lossy(s: &str) -> Self {
        match s {
            "fullscreen" => Self::Fullscreen,
            "companion_window" | "companion" => Self::CompanionWindow,
            "sidebar" => Self::Sidebar,
            "deactivated" | "hidden" | "off" => Self::Deactivated,
            _ => Self::CompanionWindow,
        }
    }
}

// ============================================================================
// PART 7: Advanced OGame Mechanics
// ============================================================================

// ---------------------------------------------------------------------------
// Fleet Save
// ---------------------------------------------------------------------------

/// Why the fleet was saved (sent away).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FleetSavePurpose {
    /// Send fleet away before enemy attack arrives
    AvoidAttack,
    /// Deploy to defend another colony
    Deployment,
    /// Explore expedition slot (position 16)
    Expedition,
}

/// A fleet save record -- fleet sent to safety.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetSave {
    pub id: String,
    pub fleet_id: String,
    pub destination: String,
    pub return_time: String,
    pub purpose: FleetSavePurpose,
    pub ship_count: u32,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Vacation Mode
// ---------------------------------------------------------------------------

/// Vacation mode protects the colony while the player is away.
/// Mirrors OGame's vacation mode rules exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VacationMode {
    pub active: bool,
    pub started_at: Option<String>,
    /// 48 hours minimum before deactivation
    pub min_duration_hours: u32,
    /// 30 days maximum
    pub max_duration_days: u32,
    /// No resources are generated while on vacation
    pub production_paused: bool,
    /// Cannot be attacked by other players
    pub attack_immune: bool,
    /// Cannot attack other players
    pub cant_attack: bool,
}

impl Default for VacationMode {
    fn default() -> Self {
        Self {
            active: false,
            started_at: None,
            min_duration_hours: 48,
            max_duration_days: 30,
            production_paused: false,
            attack_immune: false,
            cant_attack: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Noob Protection
// ---------------------------------------------------------------------------

/// Noob protection prevents strong players from attacking weak ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoobProtection {
    pub protected: bool,
    /// Protection applies below this total power level
    pub power_threshold: u64,
    /// Can only attack players within 5x power range
    pub attack_range: f64,
    /// Protection removed permanently after reaching threshold
    pub expires_at: Option<String>,
}

impl Default for NoobProtection {
    fn default() -> Self {
        Self {
            protected: true,
            power_threshold: 50_000,
            attack_range: 5.0,
            expires_at: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Debris Fields
// ---------------------------------------------------------------------------

/// Debris field created when ships are destroyed in battle.
/// Contains 30% of the destroyed ships' resource cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebrisField {
    pub id: String,
    pub coord: String,
    /// 30% of destroyed ship primary resource cost
    pub resources_primary: f64,
    /// 30% of destroyed ship secondary resource cost
    pub resources_secondary: f64,
    pub created_at: String,
    /// Disappears after 24 hours if not collected
    pub expires_hours: u32,
}

/// Resources collected from a debris field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectedDebris {
    pub debris_id: String,
    pub collector_colony: String,
    pub primary_collected: f64,
    pub secondary_collected: f64,
    pub recyclers_used: u32,
}

// ---------------------------------------------------------------------------
// Moons
// ---------------------------------------------------------------------------

/// A moon orbiting a planet, created from large debris fields.
///
/// Moon creation chance = min(debris_units / 100_000 * 20, 20)%
/// Moon size = random 1,000 - 8,944 km
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Moon {
    pub id: String,
    /// Same coordinate as the parent planet
    pub coord: String,
    /// 1,000 - 8,944 km diameter
    pub size: u32,
    /// Sensor Phalanx for scanning enemy fleet movements
    pub has_phalanx: bool,
    /// Jump Gate for instant fleet transport between moons
    pub has_jump_gate: bool,
    /// 60 minute cooldown between jump gate uses
    pub jump_gate_cooldown_mins: u32,
    /// Chance that created this moon (for display)
    pub creation_chance: f64,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Phalanx Sensor
// ---------------------------------------------------------------------------

/// A detected fleet movement from a Phalanx scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetMovement {
    pub owner: String,
    pub origin: String,
    pub destination: String,
    pub arrival_time: String,
    pub ship_count: u32,
    pub mission: String,
}

/// Result of a Phalanx sensor scan from a moon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhalanxScan {
    pub target_coord: String,
    pub fleets_detected: Vec<FleetMovement>,
    /// System range based on phalanx level: (level^2) - 1 systems
    pub scan_range: u32,
    /// Cost: 5000 * level deuterium per scan
    pub cost_deuterium: f64,
    pub scanned_at: String,
}

