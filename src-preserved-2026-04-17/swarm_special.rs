// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Special Buildings, Resource Conversion, Terrain Spreaders, Visual Swarm
//!
//! Four subsystems that fill the remaining SwarmForge gaps:
//!
//! ## Part 1: Special Faction Buildings
//!
//! Unique buildings per faction that go beyond the standard OGame production
//! pattern.  Four categories:
//!
//! - **Sensor** — Vision / espionage detection (Sensor Phalanx, Tremorsense
//!   Spire, All-Seeing Eye, Observatory Tower).
//! - **JumpGate** — Instant fleet transfer between owned colonies
//!   (Wormtunnel, Hellgate, Death Gate, Warp Gate).
//! - **TerrainSpreader** — Faction-specific terrain expansion (Creep Tumor,
//!   Panik Keim, Blightspread Monolith).
//! - **LandReclamation** — Human terraforming (Land Reclamation Works).
//!
//! Each faction gets 3-4 specials for a total of 13 buildings.
//!
//! ## Part 2: Resource Conversion Matrix
//!
//! Cross-resource exchange rates.  Demon Kultisten are a premium convertible
//! currency (1 Kultist = 5,000 Spore Gas = 2,200 Eiter Essence).  Universal
//! marketplace rates follow the classic OGame 3:2:1 ratio for ore:crystal:essence.
//!
//! ## Part 3: Terrain Spreader Mechanics
//!
//! Each faction spreads terrain differently:
//!
//! - **Insects**: Auto-spread via chainable Creep Tumors (fast, fragile).
//! - **Demons**: Diffusion-based Panik Keim (requires active Corruption).
//! - **Undead**: Manual placement by Adepten (slow but **permanent**).
//! - **Humans**: Explicit terraform via Land Reclamation (slowest, deliberate).
//!
//! ## Part 4: Visual Swarm Overlay
//!
//! Maps ImpForge AI agent activity to faction-themed creature visuals.
//! The orchestrator's workers appear as Swarmlings, Imps, Ghouls, or Soldiers
//! depending on the selected faction theme.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use crate::error::{AppResult, ImpForgeError};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_special", "Game");

// ============================================================================
// PART 1: Special Faction Buildings
// ============================================================================

/// Category of special building — determines behaviour and UI grouping.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpecialBuildingCategory {
    /// Vision / detection (Sensor Phalanx, All-Seeing Eye, etc.)
    Sensor,
    /// Instant fleet transfer between colonies (Wormtunnel, Hellgate, etc.)
    JumpGate,
    /// Faction terrain expansion (Creep Tumor, Panik Keim, etc.)
    TerrainSpreader,
    /// Human terraform — removes enemy terrain, converts to Settlement
    LandReclamation,
}

/// A unique faction building that does not fit standard OGame production.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialBuilding {
    /// Machine-readable identifier (e.g. `"sensor_phalanx"`).
    pub id: String,
    /// Display name shown in the UI.
    pub name: String,
    /// Owning faction (`"insects"`, `"demons"`, `"undead"`, `"humans"`).
    pub faction: String,
    /// Building category for grouping and behaviour.
    pub category: SpecialBuildingCategory,
    /// Base hit points.
    pub hp: u32,
    /// Cost in the faction's primary resource (ore / chitin / brimstone / bone).
    pub cost_primary: f64,
    /// Cost in the faction's secondary resource (crystal / resin / soulstone / ether).
    pub cost_secondary: f64,
    /// Cost in the faction's tertiary resource (essence / spore gas / wrath / death mist).
    pub cost_tertiary: f64,
    /// Construction time in seconds.
    pub build_time_secs: u32,
    /// Human-readable description of the building's effect.
    pub effect: String,
    /// Operational range in systems / hexes (sensors, spreaders).
    pub range: Option<u32>,
    /// Cooldown between uses in seconds (jump gates).
    pub cooldown_secs: Option<u32>,
}

/// Return all 13 special buildings across the four factions.
pub(crate) fn get_special_buildings() -> Vec<SpecialBuilding> {
    vec![
        // -- SENSORS (vision / espionage detection) ----------------------------
        SpecialBuilding {
            id: "sensor_phalanx".into(),
            name: "Sensor Phalanx".into(),
            faction: "insects".into(),
            category: SpecialBuildingCategory::Sensor,
            hp: 1000,
            cost_primary: 20_000.0,
            cost_secondary: 40_000.0,
            cost_tertiary: 20_000.0,
            build_time_secs: 7200,
            effect: "Scan enemy fleet movements. Range = level^2 - 1 systems. \
                     Cost: 5,000 deuterium per scan."
                .into(),
            range: Some(5),
            cooldown_secs: None,
        },
        SpecialBuilding {
            id: "tremorsense_spire".into(),
            name: "Tremorsense Spire".into(),
            faction: "demons".into(),
            category: SpecialBuildingCategory::Sensor,
            hp: 900,
            cost_primary: 18_000.0,
            cost_secondary: 42_000.0,
            cost_tertiary: 18_000.0,
            build_time_secs: 7200,
            effect: "Detect enemy fleet via warp vibrations. Range = level^2 - 1. \
                     Reveals fleet composition."
                .into(),
            range: Some(5),
            cooldown_secs: None,
        },
        SpecialBuilding {
            id: "all_seeing_eye".into(),
            name: "All-Seeing Eye".into(),
            faction: "undead".into(),
            category: SpecialBuildingCategory::Sensor,
            hp: 800,
            cost_primary: 22_000.0,
            cost_secondary: 38_000.0,
            cost_tertiary: 22_000.0,
            build_time_secs: 7200,
            effect: "Wraith Watchtower upgrade. See through fog of war in range. \
                     Reveals hidden units."
                .into(),
            range: Some(7),
            cooldown_secs: None,
        },
        SpecialBuilding {
            id: "observatory_tower".into(),
            name: "Observatory Tower".into(),
            faction: "humans".into(),
            category: SpecialBuildingCategory::Sensor,
            hp: 1200,
            cost_primary: 25_000.0,
            cost_secondary: 35_000.0,
            cost_tertiary: 15_000.0,
            build_time_secs: 7200,
            effect: "Advanced telescope array. Detects incoming fleets 2x earlier. \
                     Strategic early warning."
                .into(),
            range: Some(6),
            cooldown_secs: None,
        },
        // -- JUMP GATES (instant fleet transfer between own colonies) ----------
        SpecialBuilding {
            id: "wormtunnel".into(),
            name: "Wormtunnel".into(),
            faction: "insects".into(),
            category: SpecialBuildingCategory::JumpGate,
            hp: 2000,
            cost_primary: 2_000_000.0,
            cost_secondary: 4_000_000.0,
            cost_tertiary: 2_000_000.0,
            build_time_secs: 86_400,
            effect: "Instant fleet transfer between own colonies with Wormtunnels. \
                     60-minute cooldown."
                .into(),
            range: None,
            cooldown_secs: Some(3600),
        },
        SpecialBuilding {
            id: "hellgate".into(),
            name: "Hellgate".into(),
            faction: "demons".into(),
            category: SpecialBuildingCategory::JumpGate,
            hp: 1800,
            cost_primary: 1_800_000.0,
            cost_secondary: 4_200_000.0,
            cost_tertiary: 1_800_000.0,
            build_time_secs: 86_400,
            effect: "Tear open a portal to another Hellgate. Instant fleet transfer. \
                     45-minute cooldown (faster via Kultisten)."
                .into(),
            range: None,
            cooldown_secs: Some(2700),
        },
        SpecialBuilding {
            id: "death_gate".into(),
            name: "Death Gate".into(),
            faction: "undead".into(),
            category: SpecialBuildingCategory::JumpGate,
            hp: 1500,
            cost_primary: 2_200_000.0,
            cost_secondary: 3_800_000.0,
            cost_tertiary: 2_200_000.0,
            build_time_secs: 86_400,
            effect: "Open a gateway through the Realm of Death. Instant transfer. \
                     90-minute cooldown but ships gain +10% HP buff."
                .into(),
            range: None,
            cooldown_secs: Some(5400),
        },
        SpecialBuilding {
            id: "warp_gate".into(),
            name: "Warp Gate".into(),
            faction: "humans".into(),
            category: SpecialBuildingCategory::JumpGate,
            hp: 2500,
            cost_primary: 2_500_000.0,
            cost_secondary: 3_500_000.0,
            cost_tertiary: 2_500_000.0,
            build_time_secs: 86_400,
            effect: "Stable wormhole between Warp Gates. 60-minute cooldown. \
                     Most reliable but expensive."
                .into(),
            range: None,
            cooldown_secs: Some(3600),
        },
        // -- TERRAIN SPREADERS ------------------------------------------------
        SpecialBuilding {
            id: "creep_tumor".into(),
            name: "Creep Tumor".into(),
            faction: "insects".into(),
            category: SpecialBuildingCategory::TerrainSpreader,
            hp: 200,
            cost_primary: 100.0,
            cost_secondary: 0.0,
            cost_tertiary: 0.0,
            build_time_secs: 15,
            effect: "Auto-spreads Chitinous Resin to adjacent hexes. Creates chain. \
                     Destroyed = resin recedes."
                .into(),
            range: Some(3),
            cooldown_secs: None,
        },
        SpecialBuilding {
            id: "panik_keim".into(),
            name: "Panik Keim".into(),
            faction: "demons".into(),
            category: SpecialBuildingCategory::TerrainSpreader,
            hp: 150,
            cost_primary: 80.0,
            cost_secondary: 50.0,
            cost_tertiary: 0.0,
            build_time_secs: 20,
            effect: "Diffuses Hellfire Corruption to nearby tiles. Corruption decays \
                     without new Corruption supply."
                .into(),
            range: Some(2),
            cooldown_secs: None,
        },
        // NOTE: Undead have NO auto-spreader.  Adepten spread Blight MANUALLY.
        // Blight is PERMANENT once placed (never decays).
        SpecialBuilding {
            id: "blightspread_monolith".into(),
            name: "Blightspread Monolith".into(),
            faction: "undead".into(),
            category: SpecialBuildingCategory::TerrainSpreader,
            hp: 500,
            cost_primary: 500.0,
            cost_secondary: 300.0,
            cost_tertiary: 0.0,
            build_time_secs: 120,
            effect: "Marks a hex for Adept blight work. Adepten must manually spread \
                     from here. Blight is PERMANENT."
                .into(),
            range: Some(1),
            cooldown_secs: None,
        },
        // -- LAND RECLAMATION & HUMAN TERRAIN (Humans) -------------------------
        SpecialBuilding {
            id: "settlement_beacon".into(),
            name: "Settlement Beacon".into(),
            faction: "humans".into(),
            category: SpecialBuildingCategory::TerrainSpreader,
            hp: 800,
            cost_primary: 2000.0,
            cost_secondary: 1500.0,
            cost_tertiary: 500.0,
            build_time_secs: 300,
            effect: "Marks territory as Human Settlement. Engineers slowly extend \
                     the civilised zone to adjacent hexes."
                .into(),
            range: Some(2),
            cooldown_secs: None,
        },
        SpecialBuilding {
            id: "land_reclamation_works".into(),
            name: "Land Reclamation Works".into(),
            faction: "humans".into(),
            category: SpecialBuildingCategory::LandReclamation,
            hp: 1500,
            cost_primary: 5000.0,
            cost_secondary: 3000.0,
            cost_tertiary: 1000.0,
            build_time_secs: 600,
            effect: "Explicitly terraform hexes. Removes enemy terrain. Slow but \
                     permanent. Converts to Human Settlement."
                .into(),
            range: Some(2),
            cooldown_secs: None,
        },
    ]
}

// ============================================================================
// PART 2: Resource Conversion Matrix
// ============================================================================

/// A single resource conversion rule.
///
/// When `faction_required` is `Some`, only that faction can perform the
/// conversion.  When `None`, it is available on the universal marketplace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConversion {
    /// Source resource name (e.g. `"kultisten"`).
    pub from_resource: String,
    /// Target resource name (e.g. `"spore_gas"`).
    pub to_resource: String,
    /// How many units of `to_resource` you get per 1 unit of `from_resource`.
    pub rate: f64,
    /// Faction lock.  `None` means any faction can convert via marketplace.
    pub faction_required: Option<String>,
}

/// Return all 8 resource conversion rules.
///
/// Key reference: 1 Kultist = 5,000 Spore Gas = 2,200 Eiter Essence.
/// Universal marketplace follows the classic OGame 3:2:1 ratio.
pub(crate) fn get_conversion_rates() -> Vec<ResourceConversion> {
    vec![
        // Demon Kultisten conversions
        ResourceConversion {
            from_resource: "kultisten".into(),
            to_resource: "spore_gas".into(),
            rate: 5000.0,
            faction_required: Some("demons".into()),
        },
        ResourceConversion {
            from_resource: "kultisten".into(),
            to_resource: "eiter_essence".into(),
            rate: 2200.0,
            faction_required: Some("demons".into()),
        },
        // Cross-faction resource effects
        ResourceConversion {
            from_resource: "biomass".into(),
            to_resource: "infected_biomass".into(),
            rate: 1.0,
            faction_required: Some("insects".into()),
        },
        ResourceConversion {
            from_resource: "suenden".into(),
            to_resource: "cursed_suenden".into(),
            rate: 1.0,
            faction_required: Some("demons".into()),
        },
        ResourceConversion {
            from_resource: "leichenteile".into(),
            to_resource: "pestilent_leichenteile".into(),
            rate: 1.0,
            faction_required: Some("undead".into()),
        },
        // Universal marketplace conversions (OGame 3:2:1 ratio)
        ResourceConversion {
            from_resource: "ore".into(),
            to_resource: "crystal".into(),
            rate: 1.5, // 3 ore -> 2 crystal
            faction_required: None,
        },
        ResourceConversion {
            from_resource: "ore".into(),
            to_resource: "essence".into(),
            rate: 3.0, // 3 ore -> 1 essence
            faction_required: None,
        },
        ResourceConversion {
            from_resource: "crystal".into(),
            to_resource: "essence".into(),
            rate: 2.0, // 2 crystal -> 1 essence
            faction_required: None,
        },
    ]
}

/// Execute a resource conversion: spend `amount` of `from_resource` and
/// receive the equivalent in `to_resource` at the configured rate.
///
/// Returns a JSON object with `from_spent`, `to_received`, and `rate`.
/// Validates that the conversion pair exists and that the faction (if
/// required) matches.
pub(crate) fn convert_resources(
    from: &str,
    to: &str,
    amount: f64,
    player_faction: Option<&str>,
) -> Result<serde_json::Value, ImpForgeError> {
    if amount <= 0.0 {
        return Err(ImpForgeError::validation(
            "INVALID_AMOUNT",
            "Conversion amount must be positive",
        ));
    }

    let rates = get_conversion_rates();
    let rule = rates
        .iter()
        .find(|r| r.from_resource == from && r.to_resource == to)
        .ok_or_else(|| {
            ImpForgeError::validation(
                "NO_CONVERSION",
                format!("No conversion rule from '{from}' to '{to}'"),
            )
        })?;

    // Check faction lock
    if let Some(required) = &rule.faction_required {
        match player_faction {
            Some(pf) if pf == required => {} // OK
            _ => {
                return Err(ImpForgeError::validation(
                    "FACTION_LOCKED",
                    format!("Conversion from '{}' to '{}' requires faction '{}'", from, to, required),
                ));
            }
        }
    }

    let received = amount / rule.rate;

    Ok(serde_json::json!({
        "from_resource": from,
        "to_resource": to,
        "from_spent": amount,
        "to_received": received,
        "rate": rule.rate,
    }))
}

// ============================================================================
// PART 3: Terrain Spreader Mechanics
// ============================================================================

/// Describes how a faction's terrain spreading works.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainSpreaderBehavior {
    /// Faction identifier.
    pub faction: String,
    /// Name of the spreader mechanism.
    pub spreader_name: String,
    /// `true` = terrain spreads automatically; `false` = requires manual action.
    pub auto_spread: bool,
    /// Average tiles covered per in-game hour.
    pub spread_speed_tiles_per_hour: f64,
    /// `true` = terrain never decays once placed.
    pub permanent: bool,
    /// Condition under which terrain recedes (if not permanent).
    pub decays_when: Option<String>,
    /// Design rationale or lore note.
    pub special_note: String,
}

/// Return the terrain-spreading rules for all four factions.
pub(crate) fn get_terrain_spreader_rules() -> Vec<TerrainSpreaderBehavior> {
    vec![
        TerrainSpreaderBehavior {
            faction: "insects".into(),
            spreader_name: "Creep Tumor".into(),
            auto_spread: true,
            spread_speed_tiles_per_hour: 2.0,
            permanent: false,
            decays_when: Some("source destroyed".into()),
            special_note: "Chainable -- each tumor can spawn new tumors. Fast but fragile.".into(),
        },
        TerrainSpreaderBehavior {
            faction: "demons".into(),
            spreader_name: "Panik Keim".into(),
            auto_spread: true,
            spread_speed_tiles_per_hour: 1.5,
            permanent: false,
            decays_when: Some("no Corruption supply nearby".into()),
            special_note: "Diffusion-based. Requires active Corruption production to sustain."
                .into(),
        },
        TerrainSpreaderBehavior {
            faction: "undead".into(),
            spreader_name: "Adepten (manual)".into(),
            auto_spread: false,
            spread_speed_tiles_per_hour: 0.5,
            permanent: true,
            decays_when: None,
            special_note: "SLOWEST but PERMANENT. Blight never dies. Adepten must manually \
                           place it."
                .into(),
        },
        TerrainSpreaderBehavior {
            faction: "humans".into(),
            spreader_name: "Land Reclamation Works".into(),
            auto_spread: false,
            spread_speed_tiles_per_hour: 0.3,
            permanent: true,
            decays_when: None,
            special_note: "Explicit terraform. Removes enemy terrain. Slowest but most \
                           deliberate."
                .into(),
        },
    ]
}

// ============================================================================
// PART 4: Visual Swarm Overlay
// ============================================================================

/// Where the swarm overlay is rendered in the ImpForge UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverlayPosition {
    /// Behind the main content area.
    Background,
    /// Inside the sidebar panel.
    Sidebar,
    /// In the bottom status bar.
    StatusBar,
    /// Free-floating overlay window.
    Floating,
}

/// Configuration for the visual swarm that shows ImpForge AI agent activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmOverlayConfig {
    /// Which faction theme to use for creature visuals.
    pub faction: String,
    /// Whether the overlay is currently visible.
    pub enabled: bool,
    /// Opacity (0.0 = invisible, 1.0 = fully opaque).
    pub opacity: f64,
    /// Animation speed multiplier (1.0 = normal).
    pub animation_speed: f64,
    /// Show the agent name label above each creature.
    pub show_agent_labels: bool,
    /// Show the total active worker count badge.
    pub show_worker_count: bool,
    /// Where the overlay is positioned in the UI.
    pub position: OverlayPosition,
}

impl Default for SwarmOverlayConfig {
    fn default() -> Self {
        Self {
            faction: "insects".into(),
            enabled: true,
            opacity: 0.6,
            animation_speed: 1.0,
            show_agent_labels: true,
            show_worker_count: true,
            position: OverlayPosition::StatusBar,
        }
    }
}

/// A single AI agent / worker visualised as a faction creature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmCreature {
    /// Internal agent name (e.g. `"auto_labeler"`, `"ollama"`).
    pub agent_name: String,
    /// Faction-specific creature type (e.g. `"Worker Drone"`, `"Imp Worker"`).
    pub creature_type: String,
    /// Current activity state (`"working"`, `"idle"`, `"waiting"`, `"error"`).
    pub activity: String,
    /// Human-readable description of the current task.
    pub task_description: String,
    /// Progress of the current task (0.0 .. 1.0).
    pub progress: f64,
    /// Animated X position (screen coordinates).
    pub position_x: f64,
    /// Animated Y position (screen coordinates).
    pub position_y: f64,
}

/// Map an ImpForge agent name to its faction-themed creature type.
pub(crate) fn agent_to_creature(agent_name: &str, faction: &str) -> String {
    match faction {
        "insects" => match agent_name {
            "auto_labeler" | "indexer" => "Worker Drone",
            "context_enricher" | "memory" => "Scribe Beetle",
            "ollama" | "llm" | "ai" => "Brood Mother",
            "git" | "build" | "test" => "Soldier Ant",
            "orchestrator" | "scheduler" => "Hive Queen",
            _ => "Swarmling",
        },
        "demons" => match agent_name {
            "auto_labeler" | "indexer" => "Imp Worker",
            "context_enricher" | "memory" => "Knowledge Fiend",
            "ollama" | "llm" | "ai" => "Chaos Sorcerer",
            "git" | "build" | "test" => "Hell Knight",
            "orchestrator" | "scheduler" => "Infernal Lord",
            _ => "Lesser Imp",
        },
        "undead" => match agent_name {
            "auto_labeler" | "indexer" => "Ghoul Laborer",
            "context_enricher" | "memory" => "Lich Scholar",
            "ollama" | "llm" | "ai" => "Death Knight",
            "git" | "build" | "test" => "Skeleton Warrior",
            "orchestrator" | "scheduler" => "Necromancer",
            _ => "Shambling Dead",
        },
        // "humans" or any unknown faction
        _ => match agent_name {
            "auto_labeler" | "indexer" => "Clerk",
            "context_enricher" | "memory" => "Archivist",
            "ollama" | "llm" | "ai" => "Sage",
            "git" | "build" | "test" => "Engineer",
            "orchestrator" | "scheduler" => "Commander",
            _ => "Soldier",
        },
    }
    .into()
}

/// Return the current swarm state -- all active agents as faction creatures.
///
/// In production this queries the live orchestrator / agent system.  The
/// current implementation returns representative mock data so the frontend
/// can render the overlay immediately.
pub(crate) fn get_swarm_state(faction: &str) -> Vec<SwarmCreature> {
    let agents: &[(&str, &str)] = &[
        ("ollama", "Responding to user chat"),
        ("indexer", "Indexing project files"),
        ("auto_labeler", "Classifying documents"),
        ("memory", "Updating knowledge graph"),
        ("scheduler", "Coordinating tasks"),
        ("build", "Compiling project"),
    ];

    agents
        .iter()
        .enumerate()
        .map(|(i, (name, task))| SwarmCreature {
            agent_name: (*name).to_string(),
            creature_type: agent_to_creature(name, faction),
            activity: "working".to_string(),
            task_description: (*task).to_string(),
            progress: (i as f64 * 0.15 + 0.1).min(0.9),
            position_x: 50.0 + (i as f64 * 80.0),
            position_y: 300.0 + ((i % 3) as f64 * 40.0),
        })
        .collect()
}

// ============================================================================
// Engine (managed Tauri state)
// ============================================================================

/// Holds mutable overlay configuration as managed Tauri state.
pub struct SwarmSpecialEngine {
    config: Mutex<SwarmOverlayConfig>,
}

impl SwarmSpecialEngine {
    /// Create a new engine with default overlay settings.
    pub(crate) fn new() -> Self {
        Self {
            config: Mutex::new(SwarmOverlayConfig::default()),
        }
    }

    /// Read the current overlay configuration.
    pub(crate) fn get_config(&self) -> SwarmOverlayConfig {
        self.config
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Replace the overlay configuration.
    pub(crate) fn set_config(&self, new_config: SwarmOverlayConfig) -> AppResult<()> {
        if !(0.0..=1.0).contains(&new_config.opacity) {
            return Err(ImpForgeError::validation(
                "INVALID_OPACITY",
                "Opacity must be between 0.0 and 1.0",
            ));
        }
        if new_config.animation_speed < 0.0 {
            return Err(ImpForgeError::validation(
                "INVALID_SPEED",
                "Animation speed must be non-negative",
            ));
        }
        let mut guard = self.config.lock().unwrap_or_else(|p| p.into_inner());
        *guard = new_config;
        Ok(())
    }
}

// ============================================================================
// Tauri Commands (10 total)
// ============================================================================

// -- Special Buildings (3) ----------------------------------------------------

/// List all 13 special buildings across all factions.
#[tauri::command]
pub async fn special_buildings_list() -> Result<Vec<SpecialBuilding>, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_special", "game_special", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_special", "game_special");
    crate::synapse_fabric::synapse_session_push("swarm_special", "game_special", "special_buildings_list called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_special", "info", "swarm_special active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_special", "activate", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"action": "list"}));
    Ok(get_special_buildings())
}

/// List special buildings belonging to a single faction.
#[tauri::command]
pub async fn special_buildings_by_faction(
    faction: String,
) -> Result<Vec<SpecialBuilding>, ImpForgeError> {
    let f = faction.to_ascii_lowercase();
    Ok(get_special_buildings()
        .into_iter()
        .filter(|b| b.faction == f)
        .collect())
}

/// Look up a single special building by its id.
#[tauri::command]
pub async fn special_building_info(
    building_id: String,
) -> Result<Option<SpecialBuilding>, ImpForgeError> {
    Ok(get_special_buildings().into_iter().find(|b| b.id == building_id))
}

// -- Resource Conversion (3) --------------------------------------------------

/// Return all resource conversion rates.
#[tauri::command]
pub async fn resource_conversion_rates() -> Result<Vec<ResourceConversion>, ImpForgeError> {
    Ok(get_conversion_rates())
}

/// Execute a resource conversion: spend `amount` of `from` and receive the
/// equivalent in `to` at the configured rate.
#[tauri::command]
pub async fn resource_convert(
    from: String,
    to: String,
    amount: f64,
    faction: Option<String>,
) -> Result<serde_json::Value, ImpForgeError> {
    convert_resources(&from, &to, amount, faction.as_deref())
}

/// Return the terrain-spreading rules for all four factions.
#[tauri::command]
pub async fn terrain_spreader_rules() -> Result<Vec<TerrainSpreaderBehavior>, ImpForgeError> {
    Ok(get_terrain_spreader_rules())
}

// -- Visual Swarm Overlay (4) -------------------------------------------------

/// Get the current swarm overlay configuration.
#[tauri::command]
pub async fn swarm_overlay_config(
    engine: tauri::State<'_, SwarmSpecialEngine>,
) -> Result<SwarmOverlayConfig, ImpForgeError> {
    Ok(engine.get_config())
}

/// Update the swarm overlay configuration.
#[tauri::command]
pub async fn swarm_overlay_set_config(
    config: SwarmOverlayConfig,
    engine: tauri::State<'_, SwarmSpecialEngine>,
) -> Result<(), ImpForgeError> {
    engine.set_config(config)
}

/// Get the current swarm state -- all active agents as faction creatures.
#[tauri::command]
pub async fn swarm_overlay_state(
    faction: String,
) -> Result<Vec<SwarmCreature>, ImpForgeError> {
    Ok(get_swarm_state(&faction))
}

/// Map a single agent name to its faction creature type.
#[tauri::command]
pub async fn swarm_overlay_agent_mapping(
    agent: String,
    faction: String,
) -> Result<String, ImpForgeError> {
    Ok(agent_to_creature(&agent, &faction))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;


    // -- Special Buildings ---------------------------------------------------

    #[test]
    fn test_special_buildings_total_count() {
        let buildings = get_special_buildings();
        assert_eq!(buildings.len(), 13, "Expected 13 special buildings total");
    }

    #[test]
    fn test_special_buildings_per_faction() {
        let buildings = get_special_buildings();
        let insects: Vec<_> = buildings.iter().filter(|b| b.faction == "insects").collect();
        let demons: Vec<_> = buildings.iter().filter(|b| b.faction == "demons").collect();
        let undead: Vec<_> = buildings.iter().filter(|b| b.faction == "undead").collect();
        let humans: Vec<_> = buildings.iter().filter(|b| b.faction == "humans").collect();

        assert_eq!(insects.len(), 3, "Insects should have 3 special buildings");
        assert_eq!(demons.len(), 3, "Demons should have 3 special buildings");
        assert_eq!(undead.len(), 3, "Undead should have 3 special buildings");
        assert_eq!(humans.len(), 4, "Humans should have 4 special buildings (incl. Land Reclamation)");
    }

    #[test]
    fn test_special_buildings_categories() {
        let buildings = get_special_buildings();
        let sensors: Vec<_> = buildings
            .iter()
            .filter(|b| b.category == SpecialBuildingCategory::Sensor)
            .collect();
        let gates: Vec<_> = buildings
            .iter()
            .filter(|b| b.category == SpecialBuildingCategory::JumpGate)
            .collect();
        let spreaders: Vec<_> = buildings
            .iter()
            .filter(|b| b.category == SpecialBuildingCategory::TerrainSpreader)
            .collect();
        let reclamation: Vec<_> = buildings
            .iter()
            .filter(|b| b.category == SpecialBuildingCategory::LandReclamation)
            .collect();

        assert_eq!(sensors.len(), 4, "One sensor per faction");
        assert_eq!(gates.len(), 4, "One jump gate per faction");
        assert_eq!(spreaders.len(), 4, "One terrain spreader per faction");
        assert_eq!(reclamation.len(), 1, "One land reclamation (humans only)");
    }

    #[test]
    fn test_special_buildings_unique_ids() {
        let buildings = get_special_buildings();
        let mut ids: Vec<&str> = buildings.iter().map(|b| b.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), buildings.len(), "All building ids must be unique");
    }

    #[test]
    fn test_special_building_lookup() {
        let buildings = get_special_buildings();
        let phalanx = buildings.iter().find(|b| b.id == "sensor_phalanx");
        assert!(phalanx.is_some(), "sensor_phalanx should exist");
        let p = phalanx.expect("checked above");
        assert_eq!(p.faction, "insects");
        assert_eq!(p.category, SpecialBuildingCategory::Sensor);
        assert_eq!(p.range, Some(5));
    }

    #[test]
    fn test_jump_gates_have_cooldowns() {
        let buildings = get_special_buildings();
        for b in buildings
            .iter()
            .filter(|b| b.category == SpecialBuildingCategory::JumpGate)
        {
            assert!(
                b.cooldown_secs.is_some(),
                "Jump gate '{}' must have a cooldown",
                b.name
            );
            assert!(
                b.cooldown_secs.expect("checked above") > 0,
                "Jump gate '{}' cooldown must be positive",
                b.name
            );
        }
    }

    #[test]
    fn test_sensors_have_range() {
        let buildings = get_special_buildings();
        for b in buildings
            .iter()
            .filter(|b| b.category == SpecialBuildingCategory::Sensor)
        {
            assert!(
                b.range.is_some(),
                "Sensor '{}' must have a range",
                b.name
            );
        }
    }

    // -- Resource Conversion -------------------------------------------------

    #[test]
    fn test_conversion_rates_count() {
        let rates = get_conversion_rates();
        assert_eq!(rates.len(), 8, "Expected 8 conversion rules");
    }

    #[test]
    fn test_kultist_conversion_rates() {
        let rates = get_conversion_rates();
        let kultist_spore = rates
            .iter()
            .find(|r| r.from_resource == "kultisten" && r.to_resource == "spore_gas");
        assert!(kultist_spore.is_some());
        assert!((kultist_spore.expect("checked").rate - 5000.0).abs() < f64::EPSILON);

        let kultist_eiter = rates
            .iter()
            .find(|r| r.from_resource == "kultisten" && r.to_resource == "eiter_essence");
        assert!(kultist_eiter.is_some());
        assert!((kultist_eiter.expect("checked").rate - 2200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_universal_marketplace_ore_crystal() {
        let rates = get_conversion_rates();
        let ore_crystal = rates
            .iter()
            .find(|r| r.from_resource == "ore" && r.to_resource == "crystal")
            .expect("ore->crystal rule must exist");
        assert!(ore_crystal.faction_required.is_none(), "Marketplace is universal");
        assert!((ore_crystal.rate - 1.5).abs() < f64::EPSILON, "3 ore -> 2 crystal");
    }

    #[test]
    fn test_convert_resources_valid() {
        let result = convert_resources("ore", "crystal", 30.0, None);
        assert!(result.is_ok());
        let val = result.expect("checked");
        let received = val["to_received"].as_f64().expect("to_received");
        assert!((received - 20.0).abs() < f64::EPSILON, "30 ore / 1.5 = 20 crystal");
    }

    #[test]
    fn test_convert_resources_invalid_amount() {
        let result = convert_resources("ore", "crystal", -5.0, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "INVALID_AMOUNT");
    }

    #[test]
    fn test_convert_resources_no_rule() {
        let result = convert_resources("gold", "platinum", 10.0, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "NO_CONVERSION");
    }

    #[test]
    fn test_convert_resources_faction_locked() {
        // Kultisten->spore_gas requires demons faction
        let result = convert_resources("kultisten", "spore_gas", 1.0, Some("humans"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "FACTION_LOCKED");
    }

    #[test]
    fn test_convert_resources_faction_correct() {
        let result = convert_resources("kultisten", "spore_gas", 1.0, Some("demons"));
        assert!(result.is_ok());
        let val = result.expect("checked");
        let received = val["to_received"].as_f64().expect("to_received");
        assert!((received - 0.0002).abs() < 0.0001, "1 kultist / 5000 = 0.0002 spore gas");
    }

    // -- Terrain Spreader Rules ----------------------------------------------

    #[test]
    fn test_terrain_rules_count() {
        let rules = get_terrain_spreader_rules();
        assert_eq!(rules.len(), 4, "One terrain rule per faction");
    }

    #[test]
    fn test_terrain_undead_permanent() {
        let rules = get_terrain_spreader_rules();
        let undead = rules.iter().find(|r| r.faction == "undead").expect("undead rule");
        assert!(!undead.auto_spread, "Undead Blight is manual");
        assert!(undead.permanent, "Undead Blight is permanent");
        assert!(undead.decays_when.is_none(), "Permanent terrain has no decay condition");
    }

    #[test]
    fn test_terrain_insects_fastest() {
        let rules = get_terrain_spreader_rules();
        let insects = rules.iter().find(|r| r.faction == "insects").expect("insect rule");
        let max_speed = rules
            .iter()
            .map(|r| r.spread_speed_tiles_per_hour)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (insects.spread_speed_tiles_per_hour - max_speed).abs() < f64::EPSILON,
            "Insects should have the fastest terrain spread"
        );
    }

    #[test]
    fn test_terrain_auto_spread_flags() {
        let rules = get_terrain_spreader_rules();
        for r in &rules {
            match r.faction.as_str() {
                "insects" | "demons" => assert!(r.auto_spread, "{} should auto-spread", r.faction),
                "undead" | "humans" => assert!(!r.auto_spread, "{} should be manual", r.faction),
                _ => panic!("Unknown faction: {}", r.faction),
            }
        }
    }

    // -- Visual Swarm Overlay ------------------------------------------------

    #[test]
    fn test_agent_mapping_insects() {
        assert_eq!(agent_to_creature("ollama", "insects"), "Brood Mother");
        assert_eq!(agent_to_creature("indexer", "insects"), "Worker Drone");
        assert_eq!(agent_to_creature("orchestrator", "insects"), "Hive Queen");
        assert_eq!(agent_to_creature("unknown_agent", "insects"), "Swarmling");
    }

    #[test]
    fn test_agent_mapping_demons() {
        assert_eq!(agent_to_creature("ollama", "demons"), "Chaos Sorcerer");
        assert_eq!(agent_to_creature("git", "demons"), "Hell Knight");
        assert_eq!(agent_to_creature("scheduler", "demons"), "Infernal Lord");
        assert_eq!(agent_to_creature("something", "demons"), "Lesser Imp");
    }

    #[test]
    fn test_agent_mapping_undead() {
        assert_eq!(agent_to_creature("memory", "undead"), "Lich Scholar");
        assert_eq!(agent_to_creature("build", "undead"), "Skeleton Warrior");
        assert_eq!(agent_to_creature("orchestrator", "undead"), "Necromancer");
    }

    #[test]
    fn test_agent_mapping_humans() {
        assert_eq!(agent_to_creature("ai", "humans"), "Sage");
        assert_eq!(agent_to_creature("test", "humans"), "Engineer");
        assert_eq!(agent_to_creature("scheduler", "humans"), "Commander");
        assert_eq!(agent_to_creature("random", "humans"), "Soldier");
    }

    #[test]
    fn test_agent_mapping_unknown_faction() {
        // Unknown faction falls through to the humans/default branch
        assert_eq!(agent_to_creature("ollama", "elves"), "Sage");
        assert_eq!(agent_to_creature("unknown", "elves"), "Soldier");
    }

    #[test]
    fn test_swarm_state_returns_all_agents() {
        let state = get_swarm_state("insects");
        assert_eq!(state.len(), 6, "Mock swarm state should have 6 agents");
        for creature in &state {
            assert_eq!(creature.activity, "working");
            assert!(creature.progress >= 0.0 && creature.progress <= 1.0);
        }
    }

    #[test]
    fn test_swarm_state_faction_theming() {
        let insects = get_swarm_state("insects");
        let demons = get_swarm_state("demons");

        // The same agent should get different creature types per faction
        let insect_ollama = insects.iter().find(|c| c.agent_name == "ollama").expect("ollama");
        let demon_ollama = demons.iter().find(|c| c.agent_name == "ollama").expect("ollama");
        assert_eq!(insect_ollama.creature_type, "Brood Mother");
        assert_eq!(demon_ollama.creature_type, "Chaos Sorcerer");
    }

    #[test]
    fn test_overlay_config_default() {
        let cfg = SwarmOverlayConfig::default();
        assert!(cfg.enabled);
        assert!((cfg.opacity - 0.6).abs() < f64::EPSILON);
        assert!((cfg.animation_speed - 1.0).abs() < f64::EPSILON);
        assert_eq!(cfg.position, OverlayPosition::StatusBar);
    }

    #[test]
    fn test_engine_config_roundtrip() {
        let engine = SwarmSpecialEngine::new();
        let default = engine.get_config();
        assert!(default.enabled);

        let mut custom = default.clone();
        custom.opacity = 0.8;
        custom.faction = "demons".into();
        custom.position = OverlayPosition::Floating;
        engine.set_config(custom.clone()).expect("valid config");

        let read_back = engine.get_config();
        assert_eq!(read_back.faction, "demons");
        assert!((read_back.opacity - 0.8).abs() < f64::EPSILON);
        assert_eq!(read_back.position, OverlayPosition::Floating);
    }

    #[test]
    fn test_engine_rejects_invalid_opacity() {
        let engine = SwarmSpecialEngine::new();
        let mut bad = SwarmOverlayConfig::default();
        bad.opacity = 1.5;
        let result = engine.set_config(bad);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "INVALID_OPACITY");
    }

    #[test]
    fn test_engine_rejects_negative_speed() {
        let engine = SwarmSpecialEngine::new();
        let mut bad = SwarmOverlayConfig::default();
        bad.animation_speed = -0.1;
        let result = engine.set_config(bad);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "INVALID_SPEED");
    }
}
