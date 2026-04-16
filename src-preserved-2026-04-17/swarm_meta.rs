// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology
//! SwarmForge Meta-Progression Systems
//!
//! Three subsystems that drive long-term player engagement:
//!
//! ## Prestige System -- "Metamorphosis Cycles"
//!
//! Three-tier prestige inspired by idle game best practices (Cookie Clicker,
//! Idle Heroes, Antimatter Dimensions).  Each tier trades current progress
//! for permanent bonuses that accelerate the next run.
//!
//! - **Molt** (Tier 1): Soft reset.  Keep tech tree, gain Genetic Memory
//!   multiplier.  Requires colony level >= 30 and 1M total resources.
//! - **Chrysalis** (Tier 2): Full reset.  Unlocks new evolution branches.
//!   Requires 5+ Molts and colony level >= 50.
//! - **Transcendence** (Tier 3): Ascend to a new galaxy.  +50% base stats
//!   permanently.  Requires 3+ Chrysalis and colony level >= 100.
//!
//! Permanent currency "Phoenix Ash" = floor(total_resources^0.5 / 1000).
//! Genetic Memory = 1.0 + 0.02 * total_molts (never resets).
//!
//! ## Talent Trees -- 3 Branches per Unit
//!
//! Every unit type gets 45 talent nodes across three branches (Offensive,
//! Defensive, Utility), each with 5 tiers.  Tier N+1 nodes require at least
//! one point in a Tier N node.  Talent points are earned at 1 per 5 unit
//! levels.  Resetting costs 100 Dark Matter.
//!
//! ## Combat Formations
//!
//! Seven formation presets that apply stat modifiers to an army.  The
//! `swarm_formation_recommend` command uses a simple heuristic to suggest
//! the best formation for a given army composition (melee-heavy -> Vanguard,
//! ranged-heavy -> Crescent, mixed -> Line, etc.).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{AppResult, ImpForgeError};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_meta", "Game");

// ===========================================================================
// FEATURE 1: Prestige System -- "Metamorphosis Cycles"
// ===========================================================================

/// The three prestige tiers, ordered by magnitude of reset and reward.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrestigeTier {
    /// Tier 1: Soft-reset.  Keep tech tree, gain Genetic Memory multiplier.
    Molt,
    /// Tier 2: Full reset.  Unlocks new evolution branches.
    Chrysalis,
    /// Tier 3: Ascend to a new galaxy.  +50% base stats permanently.
    Transcendence,
}

impl PrestigeTier {
    /// Parse a tier from its string name (case-insensitive).
    pub(crate) fn from_str_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "molt" => Some(Self::Molt),
            "chrysalis" => Some(Self::Chrysalis),
            "transcendence" => Some(Self::Transcendence),
            _ => None,
        }
    }
}

/// Persistent prestige state for a colony.  All fields survive every reset
/// tier -- they are the *permanent* progression layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrestigeState {
    pub molt_count: u32,
    pub chrysalis_count: u32,
    pub transcendence_count: u32,
    /// Permanent premium currency earned across all resets.
    pub phoenix_ash: u64,
    /// Permanent resource production multiplier (never resets).
    /// Formula: 1.0 + 0.02 * total_molts
    pub genetic_memory: f64,
    /// Points to unlock new evolution branches (earned at Chrysalis).
    pub evolution_points: u32,
    /// Lifetime count of all resets across every tier.
    pub total_resets: u32,
}

impl Default for PrestigeState {
    fn default() -> Self {
        Self {
            molt_count: 0,
            chrysalis_count: 0,
            transcendence_count: 0,
            phoenix_ash: 0,
            genetic_memory: 1.0,
            evolution_points: 0,
            total_resets: 0,
        }
    }
}

/// What the player earns from a single prestige operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrestigeReward {
    pub phoenix_ash_earned: u64,
    pub resource_multiplier: f64,
    pub xp_multiplier: f64,
    pub new_unlocks: Vec<String>,
}

/// Pre-conditions that must be met before a prestige of a given tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrestigeRequirements {
    pub tier: String,
    pub min_colony_level: u32,
    pub min_total_resources: u64,
    pub min_previous_tier_count: u32,
    pub previous_tier_name: Option<String>,
}

// ---- Prestige Formulas ----------------------------------------------------

/// Phoenix Ash earned from a prestige.
/// Formula: floor(total_resources_earned^0.5 / 1000)
pub(crate) fn calculate_phoenix_ash(total_resources_earned: u64) -> u64 {
    ((total_resources_earned as f64).sqrt() / 1000.0).floor() as u64
}

/// Resource production multiplier after N Molts.
/// Formula: 1.0 + 0.05 * molt_count
pub(crate) fn molt_multiplier(molt_count: u32) -> f64 {
    1.0 + 0.05 * molt_count as f64
}

/// XP gain multiplier after N Chrysalis resets.
/// Formula: 1.0 + 0.25 * chrysalis_count
pub(crate) fn chrysalis_multiplier(chrysalis_count: u32) -> f64 {
    1.0 + 0.25 * chrysalis_count as f64
}

/// Genetic Memory multiplier (permanent, across all resets).
/// Formula: 1.0 + 0.02 * total_molts
pub(crate) fn genetic_memory_multiplier(total_molts: u32) -> f64 {
    1.0 + 0.02 * total_molts as f64
}

/// Returns the requirements for a given prestige tier.
pub(crate) fn prestige_requirements(tier: &PrestigeTier) -> PrestigeRequirements {
    match tier {
        PrestigeTier::Molt => PrestigeRequirements {
            tier: "Molt".to_string(),
            min_colony_level: 30,
            min_total_resources: 1_000_000,
            min_previous_tier_count: 0,
            previous_tier_name: None,
        },
        PrestigeTier::Chrysalis => PrestigeRequirements {
            tier: "Chrysalis".to_string(),
            min_colony_level: 50,
            min_total_resources: 1_000_000,
            min_previous_tier_count: 5,
            previous_tier_name: Some("Molt".to_string()),
        },
        PrestigeTier::Transcendence => PrestigeRequirements {
            tier: "Transcendence".to_string(),
            min_colony_level: 100,
            min_total_resources: 1_000_000,
            min_previous_tier_count: 3,
            previous_tier_name: Some("Chrysalis".to_string()),
        },
    }
}

/// Execute a prestige.  Validates requirements against the provided colony
/// snapshot, then returns the reward and an updated `PrestigeState`.
///
/// `colony_level` and `total_resources_earned` come from the game state
/// that the frontend passes in.
pub(crate) fn perform_prestige(
    state: &mut PrestigeState,
    tier: &PrestigeTier,
    colony_level: u32,
    total_resources_earned: u64,
) -> AppResult<PrestigeReward> {
    let reqs = prestige_requirements(tier);

    // Validate colony level
    if colony_level < reqs.min_colony_level {
        return Err(ImpForgeError::validation(
            "PRESTIGE_LEVEL_LOW",
            format!(
                "{} requires colony level {} (current: {})",
                reqs.tier, reqs.min_colony_level, colony_level
            ),
        ));
    }

    // Validate total resources
    if total_resources_earned < reqs.min_total_resources {
        return Err(ImpForgeError::validation(
            "PRESTIGE_RESOURCES_LOW",
            format!(
                "{} requires {} total resources (current: {})",
                reqs.tier, reqs.min_total_resources, total_resources_earned
            ),
        ));
    }

    // Validate previous-tier prerequisite
    let previous_count = match tier {
        PrestigeTier::Molt => 0, // no prerequisite
        PrestigeTier::Chrysalis => state.molt_count,
        PrestigeTier::Transcendence => state.chrysalis_count,
    };
    if previous_count < reqs.min_previous_tier_count {
        return Err(ImpForgeError::validation(
            "PRESTIGE_PREREQ_UNMET",
            format!(
                "{} requires {} {} resets (current: {})",
                reqs.tier,
                reqs.min_previous_tier_count,
                reqs.previous_tier_name.as_deref().unwrap_or("previous"),
                previous_count
            ),
        ));
    }

    // Calculate rewards
    let ash = calculate_phoenix_ash(total_resources_earned);

    let (resource_mult, xp_mult, new_unlocks) = match tier {
        PrestigeTier::Molt => {
            state.molt_count += 1;
            let rm = molt_multiplier(state.molt_count);
            (rm, 1.0, vec!["Genetic Memory boost".to_string()])
        }
        PrestigeTier::Chrysalis => {
            state.chrysalis_count += 1;
            state.evolution_points += 1;
            let xm = chrysalis_multiplier(state.chrysalis_count);
            (
                1.0,
                xm,
                vec![
                    format!("Evolution branch #{}", state.evolution_points),
                    "Chrysalis XP boost".to_string(),
                ],
            )
        }
        PrestigeTier::Transcendence => {
            state.transcendence_count += 1;
            (
                1.5,
                1.5,
                vec![
                    format!("Galaxy #{}", state.transcendence_count + 1),
                    "+50% base stats permanently".to_string(),
                    "New transcendent abilities".to_string(),
                ],
            )
        }
    };

    state.phoenix_ash += ash;
    state.total_resets += 1;
    state.genetic_memory = genetic_memory_multiplier(
        state.molt_count + state.chrysalis_count + state.transcendence_count,
    );

    Ok(PrestigeReward {
        phoenix_ash_earned: ash,
        resource_multiplier: resource_mult,
        xp_multiplier: xp_mult,
        new_unlocks,
    })
}

// ===========================================================================
// FEATURE 2: Talent Trees -- 3 Branches per Unit
// ===========================================================================

/// The three talent specialisation branches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TalentBranch {
    /// Branch A: damage, crit, attack speed.
    Offensive,
    /// Branch B: HP, armor, regen, taunt.
    Defensive,
    /// Branch C: speed, range, aura effects.
    Utility,
}

/// A single node in a talent tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalentNode {
    pub id: String,
    pub name: String,
    pub branch: TalentBranch,
    /// 1-5
    pub tier: u8,
    /// Maximum points that can be allocated (1-3).
    pub max_points: u8,
    /// Bonus per allocated point (e.g. 0.05 = +5%).
    pub effect_per_point: f64,
    /// Stat affected: "damage", "hp", "armor", "speed", etc.
    pub effect_type: String,
    /// ID of the prerequisite node (if any).
    pub requires: Option<String>,
}

/// A complete talent tree for one unit type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalentTree {
    pub unit_type: String,
    pub nodes: Vec<TalentNode>,
    /// node_id -> points currently allocated.
    pub allocated: HashMap<String, u8>,
    /// Total talent points that have ever been spent.
    pub total_points: u8,
    /// Unspent talent points available for allocation.
    pub available_points: u8,
}

/// Cost to reset all talent points (in Dark Matter).
pub const TALENT_RESET_COST: u32 = 100;

/// Talent points earned per N unit levels.
pub const TALENT_POINTS_PER_LEVELS: u8 = 1;
pub const LEVELS_PER_TALENT_POINT: u8 = 5;

// ---- Talent Node Builders -------------------------------------------------

fn offensive_nodes() -> Vec<TalentNode> {
    vec![
        // Tier 1 (3 nodes)
        TalentNode {
            id: "off_t1_damage".into(),
            name: "Sharpened Claws".into(),
            branch: TalentBranch::Offensive,
            tier: 1,
            max_points: 3,
            effect_per_point: 0.05,
            effect_type: "damage".into(),
            requires: None,
        },
        TalentNode {
            id: "off_t1_crit".into(),
            name: "Keen Eyes".into(),
            branch: TalentBranch::Offensive,
            tier: 1,
            max_points: 3,
            effect_per_point: 0.03,
            effect_type: "crit_chance".into(),
            requires: None,
        },
        TalentNode {
            id: "off_t1_aspd".into(),
            name: "Swift Strikes".into(),
            branch: TalentBranch::Offensive,
            tier: 1,
            max_points: 3,
            effect_per_point: 0.04,
            effect_type: "attack_speed".into(),
            requires: None,
        },
        // Tier 2 (3 nodes, require T1)
        TalentNode {
            id: "off_t2_damage".into(),
            name: "Rending Blows".into(),
            branch: TalentBranch::Offensive,
            tier: 2,
            max_points: 3,
            effect_per_point: 0.08,
            effect_type: "damage".into(),
            requires: Some("off_t1_damage".into()),
        },
        TalentNode {
            id: "off_t2_crit".into(),
            name: "Precision".into(),
            branch: TalentBranch::Offensive,
            tier: 2,
            max_points: 3,
            effect_per_point: 0.05,
            effect_type: "crit_chance".into(),
            requires: Some("off_t1_crit".into()),
        },
        TalentNode {
            id: "off_t2_aspd".into(),
            name: "Frenzy".into(),
            branch: TalentBranch::Offensive,
            tier: 2,
            max_points: 3,
            effect_per_point: 0.05,
            effect_type: "attack_speed".into(),
            requires: Some("off_t1_aspd".into()),
        },
        // Tier 3 (3 nodes, require T2)
        TalentNode {
            id: "off_t3_damage".into(),
            name: "Devastating Force".into(),
            branch: TalentBranch::Offensive,
            tier: 3,
            max_points: 3,
            effect_per_point: 0.12,
            effect_type: "damage".into(),
            requires: Some("off_t2_damage".into()),
        },
        TalentNode {
            id: "off_t3_crit_dmg".into(),
            name: "Lethal Precision".into(),
            branch: TalentBranch::Offensive,
            tier: 3,
            max_points: 3,
            effect_per_point: 0.10,
            effect_type: "crit_damage".into(),
            requires: Some("off_t2_crit".into()),
        },
        TalentNode {
            id: "off_t3_aspd".into(),
            name: "Blinding Speed".into(),
            branch: TalentBranch::Offensive,
            tier: 3,
            max_points: 3,
            effect_per_point: 0.08,
            effect_type: "attack_speed".into(),
            requires: Some("off_t2_aspd".into()),
        },
        // Tier 4 (3 nodes, require T3)
        TalentNode {
            id: "off_t4_damage".into(),
            name: "Annihilator".into(),
            branch: TalentBranch::Offensive,
            tier: 4,
            max_points: 3,
            effect_per_point: 0.15,
            effect_type: "damage".into(),
            requires: Some("off_t3_damage".into()),
        },
        TalentNode {
            id: "off_t4_pen".into(),
            name: "Armor Breaker".into(),
            branch: TalentBranch::Offensive,
            tier: 4,
            max_points: 3,
            effect_per_point: 0.15,
            effect_type: "armor_penetration".into(),
            requires: Some("off_t3_crit_dmg".into()),
        },
        TalentNode {
            id: "off_t4_aspd".into(),
            name: "Flurry".into(),
            branch: TalentBranch::Offensive,
            tier: 4,
            max_points: 3,
            effect_per_point: 0.10,
            effect_type: "attack_speed".into(),
            requires: Some("off_t3_aspd".into()),
        },
        // Tier 5 (3 ultimate nodes, require T4)
        TalentNode {
            id: "off_t5_ultimate_dmg".into(),
            name: "Extinction Protocol".into(),
            branch: TalentBranch::Offensive,
            tier: 5,
            max_points: 1,
            effect_per_point: 0.25,
            effect_type: "damage".into(),
            requires: Some("off_t4_damage".into()),
        },
        TalentNode {
            id: "off_t5_double_strike".into(),
            name: "Phantom Strike".into(),
            branch: TalentBranch::Offensive,
            tier: 5,
            max_points: 1,
            effect_per_point: 0.10,
            effect_type: "double_strike_chance".into(),
            requires: Some("off_t4_pen".into()),
        },
        TalentNode {
            id: "off_t5_execute".into(),
            name: "Coup de Grace".into(),
            branch: TalentBranch::Offensive,
            tier: 5,
            max_points: 1,
            effect_per_point: 0.20,
            effect_type: "execute_threshold".into(),
            requires: Some("off_t4_aspd".into()),
        },
    ]
}

fn defensive_nodes() -> Vec<TalentNode> {
    vec![
        // Tier 1
        TalentNode {
            id: "def_t1_hp".into(),
            name: "Thick Carapace".into(),
            branch: TalentBranch::Defensive,
            tier: 1,
            max_points: 3,
            effect_per_point: 0.05,
            effect_type: "hp".into(),
            requires: None,
        },
        TalentNode {
            id: "def_t1_armor".into(),
            name: "Hardened Shell".into(),
            branch: TalentBranch::Defensive,
            tier: 1,
            max_points: 3,
            effect_per_point: 0.04,
            effect_type: "armor".into(),
            requires: None,
        },
        TalentNode {
            id: "def_t1_regen".into(),
            name: "Rapid Healing".into(),
            branch: TalentBranch::Defensive,
            tier: 1,
            max_points: 3,
            effect_per_point: 0.03,
            effect_type: "regen".into(),
            requires: None,
        },
        // Tier 2
        TalentNode {
            id: "def_t2_hp".into(),
            name: "Vitality Surge".into(),
            branch: TalentBranch::Defensive,
            tier: 2,
            max_points: 3,
            effect_per_point: 0.08,
            effect_type: "hp".into(),
            requires: Some("def_t1_hp".into()),
        },
        TalentNode {
            id: "def_t2_armor".into(),
            name: "Iron Plating".into(),
            branch: TalentBranch::Defensive,
            tier: 2,
            max_points: 3,
            effect_per_point: 0.06,
            effect_type: "armor".into(),
            requires: Some("def_t1_armor".into()),
        },
        TalentNode {
            id: "def_t2_block".into(),
            name: "Shield Wall".into(),
            branch: TalentBranch::Defensive,
            tier: 2,
            max_points: 3,
            effect_per_point: 0.05,
            effect_type: "block_chance".into(),
            requires: Some("def_t1_regen".into()),
        },
        // Tier 3
        TalentNode {
            id: "def_t3_hp".into(),
            name: "Colossus".into(),
            branch: TalentBranch::Defensive,
            tier: 3,
            max_points: 3,
            effect_per_point: 0.12,
            effect_type: "hp".into(),
            requires: Some("def_t2_hp".into()),
        },
        TalentNode {
            id: "def_t3_armor".into(),
            name: "Adamantine Scales".into(),
            branch: TalentBranch::Defensive,
            tier: 3,
            max_points: 3,
            effect_per_point: 0.10,
            effect_type: "armor".into(),
            requires: Some("def_t2_armor".into()),
        },
        TalentNode {
            id: "def_t3_regen".into(),
            name: "Regeneration".into(),
            branch: TalentBranch::Defensive,
            tier: 3,
            max_points: 3,
            effect_per_point: 0.08,
            effect_type: "regen".into(),
            requires: Some("def_t2_block".into()),
        },
        // Tier 4
        TalentNode {
            id: "def_t4_hp".into(),
            name: "Undying Fortitude".into(),
            branch: TalentBranch::Defensive,
            tier: 4,
            max_points: 3,
            effect_per_point: 0.15,
            effect_type: "hp".into(),
            requires: Some("def_t3_hp".into()),
        },
        TalentNode {
            id: "def_t4_taunt".into(),
            name: "Provocation".into(),
            branch: TalentBranch::Defensive,
            tier: 4,
            max_points: 3,
            effect_per_point: 0.10,
            effect_type: "taunt_chance".into(),
            requires: Some("def_t3_armor".into()),
        },
        TalentNode {
            id: "def_t4_reflect".into(),
            name: "Thorns".into(),
            branch: TalentBranch::Defensive,
            tier: 4,
            max_points: 3,
            effect_per_point: 0.08,
            effect_type: "damage_reflect".into(),
            requires: Some("def_t3_regen".into()),
        },
        // Tier 5 ultimates
        TalentNode {
            id: "def_t5_immortal".into(),
            name: "Last Stand".into(),
            branch: TalentBranch::Defensive,
            tier: 5,
            max_points: 1,
            effect_per_point: 1.0,
            effect_type: "cheat_death".into(),
            requires: Some("def_t4_hp".into()),
        },
        TalentNode {
            id: "def_t5_fortress".into(),
            name: "Living Fortress".into(),
            branch: TalentBranch::Defensive,
            tier: 5,
            max_points: 1,
            effect_per_point: 0.30,
            effect_type: "damage_reduction".into(),
            requires: Some("def_t4_taunt".into()),
        },
        TalentNode {
            id: "def_t5_absorb".into(),
            name: "Absorption Field".into(),
            branch: TalentBranch::Defensive,
            tier: 5,
            max_points: 1,
            effect_per_point: 0.15,
            effect_type: "lifesteal".into(),
            requires: Some("def_t4_reflect".into()),
        },
    ]
}

fn utility_nodes() -> Vec<TalentNode> {
    vec![
        // Tier 1
        TalentNode {
            id: "utl_t1_speed".into(),
            name: "Fleet Footed".into(),
            branch: TalentBranch::Utility,
            tier: 1,
            max_points: 3,
            effect_per_point: 0.05,
            effect_type: "speed".into(),
            requires: None,
        },
        TalentNode {
            id: "utl_t1_range".into(),
            name: "Eagle Eye".into(),
            branch: TalentBranch::Utility,
            tier: 1,
            max_points: 3,
            effect_per_point: 0.04,
            effect_type: "range".into(),
            requires: None,
        },
        TalentNode {
            id: "utl_t1_resource".into(),
            name: "Scavenger".into(),
            branch: TalentBranch::Utility,
            tier: 1,
            max_points: 3,
            effect_per_point: 0.03,
            effect_type: "resource_bonus".into(),
            requires: None,
        },
        // Tier 2
        TalentNode {
            id: "utl_t2_speed".into(),
            name: "Wind Walker".into(),
            branch: TalentBranch::Utility,
            tier: 2,
            max_points: 3,
            effect_per_point: 0.08,
            effect_type: "speed".into(),
            requires: Some("utl_t1_speed".into()),
        },
        TalentNode {
            id: "utl_t2_range".into(),
            name: "Sharpshooter".into(),
            branch: TalentBranch::Utility,
            tier: 2,
            max_points: 3,
            effect_per_point: 0.06,
            effect_type: "range".into(),
            requires: Some("utl_t1_range".into()),
        },
        TalentNode {
            id: "utl_t2_aura".into(),
            name: "Inspiring Presence".into(),
            branch: TalentBranch::Utility,
            tier: 2,
            max_points: 3,
            effect_per_point: 0.04,
            effect_type: "aura_radius".into(),
            requires: Some("utl_t1_resource".into()),
        },
        // Tier 3
        TalentNode {
            id: "utl_t3_speed".into(),
            name: "Lightning Reflexes".into(),
            branch: TalentBranch::Utility,
            tier: 3,
            max_points: 3,
            effect_per_point: 0.10,
            effect_type: "dodge_chance".into(),
            requires: Some("utl_t2_speed".into()),
        },
        TalentNode {
            id: "utl_t3_range".into(),
            name: "Siege Mastery".into(),
            branch: TalentBranch::Utility,
            tier: 3,
            max_points: 3,
            effect_per_point: 0.10,
            effect_type: "range".into(),
            requires: Some("utl_t2_range".into()),
        },
        TalentNode {
            id: "utl_t3_aura".into(),
            name: "War Drums".into(),
            branch: TalentBranch::Utility,
            tier: 3,
            max_points: 3,
            effect_per_point: 0.06,
            effect_type: "aura_damage".into(),
            requires: Some("utl_t2_aura".into()),
        },
        // Tier 4
        TalentNode {
            id: "utl_t4_vision".into(),
            name: "All-Seeing Eye".into(),
            branch: TalentBranch::Utility,
            tier: 4,
            max_points: 3,
            effect_per_point: 0.15,
            effect_type: "vision_range".into(),
            requires: Some("utl_t3_speed".into()),
        },
        TalentNode {
            id: "utl_t4_resource".into(),
            name: "Master Harvester".into(),
            branch: TalentBranch::Utility,
            tier: 4,
            max_points: 3,
            effect_per_point: 0.12,
            effect_type: "resource_bonus".into(),
            requires: Some("utl_t3_range".into()),
        },
        TalentNode {
            id: "utl_t4_aura".into(),
            name: "Commanding Aura".into(),
            branch: TalentBranch::Utility,
            tier: 4,
            max_points: 3,
            effect_per_point: 0.10,
            effect_type: "aura_all_stats".into(),
            requires: Some("utl_t3_aura".into()),
        },
        // Tier 5 ultimates
        TalentNode {
            id: "utl_t5_haste".into(),
            name: "Time Warp".into(),
            branch: TalentBranch::Utility,
            tier: 5,
            max_points: 1,
            effect_per_point: 0.25,
            effect_type: "cooldown_reduction".into(),
            requires: Some("utl_t4_vision".into()),
        },
        TalentNode {
            id: "utl_t5_income".into(),
            name: "Golden Age".into(),
            branch: TalentBranch::Utility,
            tier: 5,
            max_points: 1,
            effect_per_point: 0.50,
            effect_type: "resource_bonus".into(),
            requires: Some("utl_t4_resource".into()),
        },
        TalentNode {
            id: "utl_t5_aura_ult".into(),
            name: "Nexus of Power".into(),
            branch: TalentBranch::Utility,
            tier: 5,
            max_points: 1,
            effect_per_point: 0.20,
            effect_type: "aura_all_stats".into(),
            requires: Some("utl_t4_aura".into()),
        },
    ]
}

/// Build a fresh talent tree for a given unit type with zero allocations.
/// `unit_level` determines how many talent points are available.
pub(crate) fn build_talent_tree(unit_type: &str, unit_level: u8) -> TalentTree {
    let mut nodes = Vec::with_capacity(45);
    nodes.extend(offensive_nodes());
    nodes.extend(defensive_nodes());
    nodes.extend(utility_nodes());

    let available = unit_level / LEVELS_PER_TALENT_POINT;

    TalentTree {
        unit_type: unit_type.to_string(),
        nodes,
        allocated: HashMap::new(),
        total_points: 0,
        available_points: available,
    }
}

/// Allocate one point to a talent node.  Returns an error if:
/// - No available points remain
/// - The node ID is unknown
/// - The node is already at max points
/// - The prerequisite node has zero points allocated
pub(crate) fn allocate_talent(tree: &mut TalentTree, node_id: &str) -> AppResult<()> {
    if tree.available_points == 0 {
        return Err(ImpForgeError::validation(
            "TALENT_NO_POINTS",
            "No talent points available",
        ));
    }

    let node = tree
        .nodes
        .iter()
        .find(|n| n.id == node_id)
        .ok_or_else(|| {
            ImpForgeError::validation(
                "TALENT_UNKNOWN_NODE",
                format!("Unknown talent node: {node_id}"),
            )
        })?
        .clone();

    // Check prerequisite
    if let Some(ref req_id) = node.requires {
        let prereq_points = tree.allocated.get(req_id).copied().unwrap_or(0);
        if prereq_points == 0 {
            return Err(ImpForgeError::validation(
                "TALENT_PREREQ_UNMET",
                format!(
                    "Talent '{}' requires at least 1 point in '{}'",
                    node.name, req_id
                ),
            ));
        }
    }

    // Check max points
    let current = tree.allocated.get(node_id).copied().unwrap_or(0);
    if current >= node.max_points {
        return Err(ImpForgeError::validation(
            "TALENT_MAX_POINTS",
            format!(
                "Talent '{}' already at max ({}/{})",
                node.name, current, node.max_points
            ),
        ));
    }

    // Allocate
    *tree.allocated.entry(node_id.to_string()).or_insert(0) += 1;
    tree.total_points += 1;
    tree.available_points -= 1;

    Ok(())
}

/// Reset all talent allocations, returning all points.
pub(crate) fn reset_talents(tree: &mut TalentTree) {
    let refunded = tree.total_points;
    tree.allocated.clear();
    tree.available_points += refunded;
    tree.total_points = 0;
}

// ===========================================================================
// FEATURE 3: Combat Formations
// ===========================================================================

/// The seven formation presets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Formation {
    /// +15% charge damage, -10% defense.
    Wedge,
    /// Balanced, no bonus/penalty.
    Line,
    /// -20% AoE damage taken, -10% single target damage.
    Scatter,
    /// -25% damage taken, -15% movement speed.
    Turtle,
    /// +20% damage from sides/rear, -15% frontal defense.
    Flanking,
    /// +10% damage, +10% speed (melee only).
    Vanguard,
    /// Ranged units: +15% damage, +10% range.
    Crescent,
}

impl Formation {
    /// Parse from a string name (case-insensitive).
    pub(crate) fn from_str_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "wedge" => Some(Self::Wedge),
            "line" => Some(Self::Line),
            "scatter" => Some(Self::Scatter),
            "turtle" => Some(Self::Turtle),
            "flanking" => Some(Self::Flanking),
            "vanguard" => Some(Self::Vanguard),
            "crescent" => Some(Self::Crescent),
            _ => None,
        }
    }

    /// All available formations.
    pub(crate) fn all() -> Vec<Self> {
        vec![
            Self::Wedge,
            Self::Line,
            Self::Scatter,
            Self::Turtle,
            Self::Flanking,
            Self::Vanguard,
            Self::Crescent,
        ]
    }

    /// Human-readable name.
    pub(crate) fn display_name(&self) -> &'static str {
        match self {
            Self::Wedge => "Wedge",
            Self::Line => "Line",
            Self::Scatter => "Scatter",
            Self::Turtle => "Turtle",
            Self::Flanking => "Flanking",
            Self::Vanguard => "Vanguard",
            Self::Crescent => "Crescent",
        }
    }

    /// Short tactical description.
    pub(crate) fn description(&self) -> &'static str {
        match self {
            Self::Wedge => "Aggressive spearhead. +15% charge damage, -10% defense.",
            Self::Line => "Balanced formation. No bonus or penalty.",
            Self::Scatter => "Anti-AoE spread. -20% AoE damage taken, -10% single target damage.",
            Self::Turtle => "Defensive shell. -25% damage taken, -15% movement speed.",
            Self::Flanking => "Pincer maneuver. +20% side/rear damage, -15% frontal defense.",
            Self::Vanguard => "Melee rush. +10% damage, +10% speed (melee only).",
            Self::Crescent => "Ranged arc. +15% ranged damage, +10% range.",
        }
    }
}

/// Numerical stat modifiers applied by a formation.  Values are fractional
/// multipliers (e.g. 0.15 = +15%, -0.10 = -10%).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormationEffect {
    pub formation: String,
    pub damage_bonus: f64,
    /// Negative = penalty.
    pub defense_bonus: f64,
    pub speed_bonus: f64,
    pub aoe_reduction: f64,
    pub range_bonus: f64,
    pub special: String,
}

/// Build the effect struct for a given formation.
pub(crate) fn formation_effects(formation: &Formation) -> FormationEffect {
    match formation {
        Formation::Wedge => FormationEffect {
            formation: "Wedge".into(),
            damage_bonus: 0.15,
            defense_bonus: -0.10,
            speed_bonus: 0.0,
            aoe_reduction: 0.0,
            range_bonus: 0.0,
            special: "+15% charge damage".into(),
        },
        Formation::Line => FormationEffect {
            formation: "Line".into(),
            damage_bonus: 0.0,
            defense_bonus: 0.0,
            speed_bonus: 0.0,
            aoe_reduction: 0.0,
            range_bonus: 0.0,
            special: "Balanced, no modifiers".into(),
        },
        Formation::Scatter => FormationEffect {
            formation: "Scatter".into(),
            damage_bonus: -0.10,
            defense_bonus: 0.0,
            speed_bonus: 0.0,
            aoe_reduction: 0.20,
            range_bonus: 0.0,
            special: "-20% AoE damage taken".into(),
        },
        Formation::Turtle => FormationEffect {
            formation: "Turtle".into(),
            damage_bonus: 0.0,
            defense_bonus: 0.25,
            speed_bonus: -0.15,
            aoe_reduction: 0.0,
            range_bonus: 0.0,
            special: "-25% damage taken, -15% speed".into(),
        },
        Formation::Flanking => FormationEffect {
            formation: "Flanking".into(),
            damage_bonus: 0.20,
            defense_bonus: -0.15,
            speed_bonus: 0.0,
            aoe_reduction: 0.0,
            range_bonus: 0.0,
            special: "+20% side/rear damage, -15% frontal defense".into(),
        },
        Formation::Vanguard => FormationEffect {
            formation: "Vanguard".into(),
            damage_bonus: 0.10,
            defense_bonus: 0.0,
            speed_bonus: 0.10,
            aoe_reduction: 0.0,
            range_bonus: 0.0,
            special: "Melee only: +10% damage, +10% speed".into(),
        },
        Formation::Crescent => FormationEffect {
            formation: "Crescent".into(),
            damage_bonus: 0.15,
            defense_bonus: 0.0,
            speed_bonus: 0.0,
            aoe_reduction: 0.0,
            range_bonus: 0.10,
            special: "Ranged only: +15% damage, +10% range".into(),
        },
    }
}

/// Recommend a formation based on army composition.
///
/// The input JSON object is expected to have:
/// - `melee_count` (u32) -- number of melee units
/// - `ranged_count` (u32) -- number of ranged units
/// - `tank_count` (u32)  -- number of tank/defensive units
///
/// Heuristic:
/// - >70% melee: Vanguard
/// - >70% ranged: Crescent
/// - >50% tanks: Turtle
/// - melee > ranged but mixed: Wedge
/// - ranged > melee but mixed: Scatter
/// - even split: Line
pub(crate) fn recommend_formation(melee: u32, ranged: u32, tanks: u32) -> Formation {
    let total = melee + ranged + tanks;
    if total == 0 {
        return Formation::Line;
    }

    let melee_pct = melee as f64 / total as f64;
    let ranged_pct = ranged as f64 / total as f64;
    let tank_pct = tanks as f64 / total as f64;

    if tank_pct > 0.50 {
        Formation::Turtle
    } else if melee_pct > 0.70 {
        Formation::Vanguard
    } else if ranged_pct > 0.70 {
        Formation::Crescent
    } else if melee_pct > ranged_pct {
        Formation::Wedge
    } else if ranged_pct > melee_pct {
        Formation::Scatter
    } else {
        Formation::Line
    }
}

// ===========================================================================
// Tauri Commands
// ===========================================================================

// ---- Prestige Commands ----------------------------------------------------

/// Return the prestige state for a colony.  In a full implementation this
/// would load from SQLite; here we return a default for the prototype.
#[tauri::command]
pub async fn swarm_prestige_state(_colony_id: String) -> AppResult<PrestigeState> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_meta", "game_meta", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_meta", "game_meta");
    crate::synapse_fabric::synapse_session_push("swarm_meta", "game_meta", "swarm_prestige_state called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_meta", "info", "swarm_meta active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_meta", "prestige", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"action": "state"}));
    Ok(PrestigeState::default())
}

/// Execute a prestige operation.  Accepts the tier name and colony snapshot.
#[tauri::command]
pub async fn swarm_prestige_perform(
    _colony_id: String,
    tier: String,
    colony_level: u32,
    total_resources_earned: u64,
) -> AppResult<PrestigeReward> {
    let prestige_tier = PrestigeTier::from_str_name(&tier).ok_or_else(|| {
        ImpForgeError::validation(
            "PRESTIGE_INVALID_TIER",
            format!("Unknown prestige tier: '{tier}'. Valid: Molt, Chrysalis, Transcendence"),
        )
    })?;

    let mut state = PrestigeState::default();
    perform_prestige(&mut state, &prestige_tier, colony_level, total_resources_earned)
}

/// Return the requirements to perform a prestige of the given tier.
#[tauri::command]
pub async fn swarm_prestige_requirements(tier: String) -> AppResult<serde_json::Value> {
    let prestige_tier = PrestigeTier::from_str_name(&tier).ok_or_else(|| {
        ImpForgeError::validation(
            "PRESTIGE_INVALID_TIER",
            format!("Unknown prestige tier: '{tier}'. Valid: Molt, Chrysalis, Transcendence"),
        )
    })?;

    let reqs = prestige_requirements(&prestige_tier);
    serde_json::to_value(&reqs).map_err(|e| {
        ImpForgeError::internal("PRESTIGE_SERIALIZE", format!("Failed to serialize requirements: {e}"))
    })
}

/// Return all active multipliers for a colony based on its prestige state.
#[tauri::command]
pub async fn swarm_prestige_multipliers(_colony_id: String) -> AppResult<serde_json::Value> {
    let state = PrestigeState::default();
    let multipliers = serde_json::json!({
        "molt_resource_multiplier": molt_multiplier(state.molt_count),
        "chrysalis_xp_multiplier": chrysalis_multiplier(state.chrysalis_count),
        "genetic_memory": state.genetic_memory,
        "transcendence_bonus": if state.transcendence_count > 0 { 0.50 * state.transcendence_count as f64 } else { 0.0 },
        "phoenix_ash": state.phoenix_ash,
        "total_resets": state.total_resets,
    });
    Ok(multipliers)
}

// ---- Talent Commands ------------------------------------------------------

/// Return the full talent tree for a unit type.
#[tauri::command]
pub async fn swarm_talent_tree(unit_type: String, unit_level: Option<u8>) -> AppResult<TalentTree> {
    let level = unit_level.unwrap_or(50);
    Ok(build_talent_tree(&unit_type, level))
}

/// Allocate a single talent point to a node.
#[tauri::command]
pub async fn swarm_talent_allocate(
    unit_type: String,
    node_id: String,
    unit_level: Option<u8>,
) -> AppResult<TalentTree> {
    let level = unit_level.unwrap_or(50);
    let mut tree = build_talent_tree(&unit_type, level);
    allocate_talent(&mut tree, &node_id)?;
    Ok(tree)
}

/// Reset all talent allocations (costs 100 Dark Matter -- validated by frontend).
#[tauri::command]
pub async fn swarm_talent_reset(unit_type: String, unit_level: Option<u8>) -> AppResult<TalentTree> {
    let level = unit_level.unwrap_or(50);
    let mut tree = build_talent_tree(&unit_type, level);
    reset_talents(&mut tree);
    Ok(tree)
}

// ---- Formation Commands ---------------------------------------------------

/// Return the stat effects of a specific formation.
#[tauri::command]
pub async fn swarm_formation_effects(formation: String) -> AppResult<FormationEffect> {
    let f = Formation::from_str_name(&formation).ok_or_else(|| {
        ImpForgeError::validation(
            "FORMATION_UNKNOWN",
            format!(
                "Unknown formation: '{formation}'. Valid: Wedge, Line, Scatter, Turtle, Flanking, Vanguard, Crescent"
            ),
        )
    })?;
    Ok(formation_effects(&f))
}

/// List all formations with their descriptions and effects.
#[tauri::command]
pub async fn swarm_formation_list() -> AppResult<Vec<serde_json::Value>> {
    let list: Vec<serde_json::Value> = Formation::all()
        .into_iter()
        .map(|f| {
            let effect = formation_effects(&f);
            serde_json::json!({
                "name": f.display_name(),
                "description": f.description(),
                "effects": effect,
            })
        })
        .collect();
    Ok(list)
}

/// Recommend the best formation given an army composition JSON object.
/// Expected keys: `melee_count`, `ranged_count`, `tank_count` (all u32).
#[tauri::command]
pub async fn swarm_formation_recommend(
    army_composition: serde_json::Value,
) -> AppResult<String> {
    let melee = army_composition
        .get("melee_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let ranged = army_composition
        .get("ranged_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let tanks = army_composition
        .get("tank_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let recommended = recommend_formation(melee, ranged, tanks);
    Ok(recommended.display_name().to_string())
}

// ===========================================================================
// Tests
// ===========================================================================

// ---------------------------------------------------------------------------
// Additional Tauri Commands — wiring internal helpers
// ---------------------------------------------------------------------------

/// Get talent system constants (reset cost, points per levels).
#[tauri::command]
pub async fn swarm_talent_constants() -> AppResult<serde_json::Value> {
    Ok(serde_json::json!({
        "talent_reset_cost_dm": TALENT_RESET_COST,
        "talent_points_per_levels": TALENT_POINTS_PER_LEVELS,
        "levels_per_talent_point": LEVELS_PER_TALENT_POINT,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;


    // ---- Prestige Formula Tests -------------------------------------------

    #[test]
    fn test_phoenix_ash_calculation() {
        // floor(1_000_000^0.5 / 1000) = floor(1000 / 1000) = 1
        assert_eq!(calculate_phoenix_ash(1_000_000), 1);
        // floor(100_000_000^0.5 / 1000) = floor(10_000 / 1000) = 10
        assert_eq!(calculate_phoenix_ash(100_000_000), 10);
        // floor(0^0.5 / 1000) = 0
        assert_eq!(calculate_phoenix_ash(0), 0);
        // floor(999_999^0.5 / 1000) = floor(999.999 / 1000) = 0
        assert_eq!(calculate_phoenix_ash(999_999), 0);
        // floor(4_000_000^0.5 / 1000) = floor(2000 / 1000) = 2
        assert_eq!(calculate_phoenix_ash(4_000_000), 2);
    }

    #[test]
    fn test_molt_multiplier() {
        assert!((molt_multiplier(0) - 1.0).abs() < f64::EPSILON);
        assert!((molt_multiplier(1) - 1.05).abs() < f64::EPSILON);
        assert!((molt_multiplier(10) - 1.50).abs() < f64::EPSILON);
        assert!((molt_multiplier(20) - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_chrysalis_multiplier() {
        assert!((chrysalis_multiplier(0) - 1.0).abs() < f64::EPSILON);
        assert!((chrysalis_multiplier(1) - 1.25).abs() < f64::EPSILON);
        assert!((chrysalis_multiplier(4) - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_genetic_memory_multiplier() {
        assert!((genetic_memory_multiplier(0) - 1.0).abs() < f64::EPSILON);
        assert!((genetic_memory_multiplier(50) - 2.0).abs() < f64::EPSILON);
        assert!((genetic_memory_multiplier(1) - 1.02).abs() < f64::EPSILON);
    }

    #[test]
    fn test_prestige_state_default() {
        let s = PrestigeState::default();
        assert_eq!(s.molt_count, 0);
        assert_eq!(s.chrysalis_count, 0);
        assert_eq!(s.transcendence_count, 0);
        assert_eq!(s.phoenix_ash, 0);
        assert!((s.genetic_memory - 1.0).abs() < f64::EPSILON);
        assert_eq!(s.evolution_points, 0);
        assert_eq!(s.total_resets, 0);
    }

    #[test]
    fn test_prestige_requirements_molt() {
        let reqs = prestige_requirements(&PrestigeTier::Molt);
        assert_eq!(reqs.min_colony_level, 30);
        assert_eq!(reqs.min_total_resources, 1_000_000);
        assert_eq!(reqs.min_previous_tier_count, 0);
        assert!(reqs.previous_tier_name.is_none());
    }

    #[test]
    fn test_prestige_requirements_chrysalis() {
        let reqs = prestige_requirements(&PrestigeTier::Chrysalis);
        assert_eq!(reqs.min_colony_level, 50);
        assert_eq!(reqs.min_previous_tier_count, 5);
        assert_eq!(reqs.previous_tier_name.as_deref(), Some("Molt"));
    }

    #[test]
    fn test_prestige_requirements_transcendence() {
        let reqs = prestige_requirements(&PrestigeTier::Transcendence);
        assert_eq!(reqs.min_colony_level, 100);
        assert_eq!(reqs.min_previous_tier_count, 3);
        assert_eq!(reqs.previous_tier_name.as_deref(), Some("Chrysalis"));
    }

    #[test]
    fn test_perform_molt_success() {
        let mut state = PrestigeState::default();
        let reward = perform_prestige(&mut state, &PrestigeTier::Molt, 30, 1_000_000).expect("reward should be valid");
        assert_eq!(reward.phoenix_ash_earned, 1);
        assert!((reward.resource_multiplier - 1.05).abs() < f64::EPSILON);
        assert!((reward.xp_multiplier - 1.0).abs() < f64::EPSILON);
        assert_eq!(state.molt_count, 1);
        assert_eq!(state.total_resets, 1);
        assert_eq!(state.phoenix_ash, 1);
    }

    #[test]
    fn test_perform_molt_level_too_low() {
        let mut state = PrestigeState::default();
        let err = perform_prestige(&mut state, &PrestigeTier::Molt, 29, 1_000_000).unwrap_err();
        assert_eq!(err.code, "PRESTIGE_LEVEL_LOW");
    }

    #[test]
    fn test_perform_molt_resources_too_low() {
        let mut state = PrestigeState::default();
        let err = perform_prestige(&mut state, &PrestigeTier::Molt, 30, 999_999).unwrap_err();
        assert_eq!(err.code, "PRESTIGE_RESOURCES_LOW");
    }

    #[test]
    fn test_perform_chrysalis_prereq_unmet() {
        let mut state = PrestigeState::default();
        state.molt_count = 4; // need 5
        let err =
            perform_prestige(&mut state, &PrestigeTier::Chrysalis, 50, 1_000_000).unwrap_err();
        assert_eq!(err.code, "PRESTIGE_PREREQ_UNMET");
    }

    #[test]
    fn test_perform_chrysalis_success() {
        let mut state = PrestigeState::default();
        state.molt_count = 5;
        let reward =
            perform_prestige(&mut state, &PrestigeTier::Chrysalis, 50, 4_000_000).expect("perform prestige should succeed");
        assert_eq!(reward.phoenix_ash_earned, 2);
        assert!((reward.xp_multiplier - 1.25).abs() < f64::EPSILON);
        assert_eq!(state.chrysalis_count, 1);
        assert_eq!(state.evolution_points, 1);
    }

    #[test]
    fn test_perform_transcendence_success() {
        let mut state = PrestigeState::default();
        state.molt_count = 10;
        state.chrysalis_count = 3;
        let reward = perform_prestige(
            &mut state,
            &PrestigeTier::Transcendence,
            100,
            100_000_000,
        )
        .expect("test perform transcendence success should succeed");
        assert_eq!(reward.phoenix_ash_earned, 10);
        assert!((reward.resource_multiplier - 1.5).abs() < f64::EPSILON);
        assert!((reward.xp_multiplier - 1.5).abs() < f64::EPSILON);
        assert_eq!(state.transcendence_count, 1);
        assert!(reward.new_unlocks.len() >= 2);
    }

    #[test]
    fn test_perform_transcendence_prereq_unmet() {
        let mut state = PrestigeState::default();
        state.chrysalis_count = 2; // need 3
        let err = perform_prestige(
            &mut state,
            &PrestigeTier::Transcendence,
            100,
            100_000_000,
        )
        .unwrap_err();
        assert_eq!(err.code, "PRESTIGE_PREREQ_UNMET");
    }

    #[test]
    fn test_genetic_memory_updates_after_prestige() {
        let mut state = PrestigeState::default();
        // Perform 3 molts
        for _ in 0..3 {
            let _ = perform_prestige(&mut state, &PrestigeTier::Molt, 30, 1_000_000).expect("prestige molt should succeed");
        }
        // genetic_memory = 1.0 + 0.02 * 3 = 1.06
        assert!((state.genetic_memory - 1.06).abs() < f64::EPSILON);
    }

    // ---- Talent Tree Tests ------------------------------------------------

    #[test]
    fn test_build_talent_tree_has_45_nodes() {
        let tree = build_talent_tree("warrior", 50);
        assert_eq!(tree.nodes.len(), 45);
        assert_eq!(tree.unit_type, "warrior");
        assert_eq!(tree.available_points, 10); // 50 / 5
        assert_eq!(tree.total_points, 0);
    }

    #[test]
    fn test_build_talent_tree_branch_distribution() {
        let tree = build_talent_tree("mage", 25);
        let off_count = tree
            .nodes
            .iter()
            .filter(|n| n.branch == TalentBranch::Offensive)
            .count();
        let def_count = tree
            .nodes
            .iter()
            .filter(|n| n.branch == TalentBranch::Defensive)
            .count();
        let utl_count = tree
            .nodes
            .iter()
            .filter(|n| n.branch == TalentBranch::Utility)
            .count();
        assert_eq!(off_count, 15);
        assert_eq!(def_count, 15);
        assert_eq!(utl_count, 15);
    }

    #[test]
    fn test_talent_tiers_1_through_5() {
        let tree = build_talent_tree("scout", 50);
        for tier in 1..=5 {
            let count = tree.nodes.iter().filter(|n| n.tier == tier).count();
            // 3 nodes per tier per branch = 9 per tier
            assert_eq!(count, 9, "Expected 9 nodes at tier {tier}");
        }
    }

    #[test]
    fn test_allocate_talent_success() {
        let mut tree = build_talent_tree("warrior", 50);
        allocate_talent(&mut tree, "off_t1_damage").expect("allocate talent should succeed");
        assert_eq!(tree.allocated.get("off_t1_damage"), Some(&1));
        assert_eq!(tree.total_points, 1);
        assert_eq!(tree.available_points, 9);
    }

    #[test]
    fn test_allocate_talent_max_points() {
        let mut tree = build_talent_tree("warrior", 50);
        allocate_talent(&mut tree, "off_t1_damage").expect("allocate talent should succeed");
        allocate_talent(&mut tree, "off_t1_damage").expect("allocate talent should succeed");
        allocate_talent(&mut tree, "off_t1_damage").expect("allocate talent should succeed");
        // 4th allocation should fail (max_points = 3)
        let err = allocate_talent(&mut tree, "off_t1_damage").unwrap_err();
        assert_eq!(err.code, "TALENT_MAX_POINTS");
    }

    #[test]
    fn test_allocate_talent_no_points() {
        let mut tree = build_talent_tree("warrior", 0); // 0 / 5 = 0 points
        let err = allocate_talent(&mut tree, "off_t1_damage").unwrap_err();
        assert_eq!(err.code, "TALENT_NO_POINTS");
    }

    #[test]
    fn test_allocate_talent_unknown_node() {
        let mut tree = build_talent_tree("warrior", 50);
        let err = allocate_talent(&mut tree, "nonexistent_node").unwrap_err();
        assert_eq!(err.code, "TALENT_UNKNOWN_NODE");
    }

    #[test]
    fn test_allocate_talent_prereq_unmet() {
        let mut tree = build_talent_tree("warrior", 50);
        // off_t2_damage requires off_t1_damage
        let err = allocate_talent(&mut tree, "off_t2_damage").unwrap_err();
        assert_eq!(err.code, "TALENT_PREREQ_UNMET");
    }

    #[test]
    fn test_allocate_talent_chain_t1_to_t2() {
        let mut tree = build_talent_tree("warrior", 50);
        allocate_talent(&mut tree, "off_t1_damage").expect("allocate talent should succeed");
        allocate_talent(&mut tree, "off_t2_damage").expect("allocate talent should succeed");
        assert_eq!(tree.allocated.get("off_t1_damage"), Some(&1));
        assert_eq!(tree.allocated.get("off_t2_damage"), Some(&1));
        assert_eq!(tree.total_points, 2);
    }

    #[test]
    fn test_allocate_talent_full_chain_to_t5() {
        let mut tree = build_talent_tree("warrior", 100); // 20 points
        allocate_talent(&mut tree, "off_t1_damage").expect("allocate talent should succeed");
        allocate_talent(&mut tree, "off_t2_damage").expect("allocate talent should succeed");
        allocate_talent(&mut tree, "off_t3_damage").expect("allocate talent should succeed");
        allocate_talent(&mut tree, "off_t4_damage").expect("allocate talent should succeed");
        allocate_talent(&mut tree, "off_t5_ultimate_dmg").expect("allocate talent should succeed");
        assert_eq!(tree.total_points, 5);
        assert_eq!(tree.allocated.get("off_t5_ultimate_dmg"), Some(&1));
    }

    #[test]
    fn test_reset_talents() {
        let mut tree = build_talent_tree("warrior", 50);
        allocate_talent(&mut tree, "off_t1_damage").expect("allocate talent should succeed");
        allocate_talent(&mut tree, "off_t1_crit").expect("allocate talent should succeed");
        allocate_talent(&mut tree, "def_t1_hp").expect("allocate talent should succeed");
        assert_eq!(tree.total_points, 3);
        assert_eq!(tree.available_points, 7);

        reset_talents(&mut tree);
        assert_eq!(tree.total_points, 0);
        assert_eq!(tree.available_points, 10);
        assert!(tree.allocated.is_empty());
    }

    #[test]
    fn test_talent_points_per_level() {
        // 0 levels = 0 points
        assert_eq!(build_talent_tree("x", 0).available_points, 0);
        // 4 levels = 0 points (not enough for first point)
        assert_eq!(build_talent_tree("x", 4).available_points, 0);
        // 5 levels = 1 point
        assert_eq!(build_talent_tree("x", 5).available_points, 1);
        // 10 levels = 2 points
        assert_eq!(build_talent_tree("x", 10).available_points, 2);
        // 100 levels = 20 points
        assert_eq!(build_talent_tree("x", 100).available_points, 20);
        // 255 levels = 51 points (max u8)
        assert_eq!(build_talent_tree("x", 255).available_points, 51);
    }

    #[test]
    fn test_tier1_nodes_have_no_prerequisite() {
        let tree = build_talent_tree("test", 50);
        for node in &tree.nodes {
            if node.tier == 1 {
                assert!(
                    node.requires.is_none(),
                    "Tier 1 node '{}' should have no prerequisite",
                    node.id
                );
            }
        }
    }

    #[test]
    fn test_tier2_plus_nodes_have_prerequisites() {
        let tree = build_talent_tree("test", 50);
        for node in &tree.nodes {
            if node.tier >= 2 {
                assert!(
                    node.requires.is_some(),
                    "Tier {} node '{}' should have a prerequisite",
                    node.tier, node.id
                );
            }
        }
    }

    #[test]
    fn test_ultimate_nodes_have_max_1_point() {
        let tree = build_talent_tree("test", 50);
        for node in &tree.nodes {
            if node.tier == 5 {
                assert_eq!(
                    node.max_points, 1,
                    "Ultimate node '{}' should have max 1 point",
                    node.id
                );
            }
        }
    }

    // ---- Formation Tests --------------------------------------------------

    #[test]
    fn test_formation_effects_wedge() {
        let effect = formation_effects(&Formation::Wedge);
        assert!((effect.damage_bonus - 0.15).abs() < f64::EPSILON);
        assert!((effect.defense_bonus - (-0.10)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_formation_effects_line_balanced() {
        let effect = formation_effects(&Formation::Line);
        assert!((effect.damage_bonus).abs() < f64::EPSILON);
        assert!((effect.defense_bonus).abs() < f64::EPSILON);
        assert!((effect.speed_bonus).abs() < f64::EPSILON);
    }

    #[test]
    fn test_formation_effects_scatter() {
        let effect = formation_effects(&Formation::Scatter);
        assert!((effect.damage_bonus - (-0.10)).abs() < f64::EPSILON);
        assert!((effect.aoe_reduction - 0.20).abs() < f64::EPSILON);
    }

    #[test]
    fn test_formation_effects_turtle() {
        let effect = formation_effects(&Formation::Turtle);
        assert!((effect.defense_bonus - 0.25).abs() < f64::EPSILON);
        assert!((effect.speed_bonus - (-0.15)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_formation_effects_flanking() {
        let effect = formation_effects(&Formation::Flanking);
        assert!((effect.damage_bonus - 0.20).abs() < f64::EPSILON);
        assert!((effect.defense_bonus - (-0.15)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_formation_effects_vanguard() {
        let effect = formation_effects(&Formation::Vanguard);
        assert!((effect.damage_bonus - 0.10).abs() < f64::EPSILON);
        assert!((effect.speed_bonus - 0.10).abs() < f64::EPSILON);
    }

    #[test]
    fn test_formation_effects_crescent() {
        let effect = formation_effects(&Formation::Crescent);
        assert!((effect.damage_bonus - 0.15).abs() < f64::EPSILON);
        assert!((effect.range_bonus - 0.10).abs() < f64::EPSILON);
    }

    #[test]
    fn test_formation_all_returns_seven() {
        assert_eq!(Formation::all().len(), 7);
    }

    #[test]
    fn test_formation_from_str_name() {
        assert_eq!(Formation::from_str_name("wedge"), Some(Formation::Wedge));
        assert_eq!(Formation::from_str_name("TURTLE"), Some(Formation::Turtle));
        assert_eq!(
            Formation::from_str_name("Crescent"),
            Some(Formation::Crescent)
        );
        assert_eq!(Formation::from_str_name("invalid"), None);
    }

    #[test]
    fn test_recommend_formation_melee_heavy() {
        // 80% melee -> Vanguard
        assert_eq!(recommend_formation(80, 10, 10), Formation::Vanguard);
    }

    #[test]
    fn test_recommend_formation_ranged_heavy() {
        // 80% ranged -> Crescent
        assert_eq!(recommend_formation(10, 80, 10), Formation::Crescent);
    }

    #[test]
    fn test_recommend_formation_tank_heavy() {
        // >50% tanks -> Turtle
        assert_eq!(recommend_formation(10, 10, 60), Formation::Turtle);
    }

    #[test]
    fn test_recommend_formation_melee_dominant() {
        // melee > ranged but not >70% -> Wedge
        assert_eq!(recommend_formation(50, 30, 20), Formation::Wedge);
    }

    #[test]
    fn test_recommend_formation_ranged_dominant() {
        // ranged > melee but not >70% -> Scatter
        assert_eq!(recommend_formation(30, 50, 20), Formation::Scatter);
    }

    #[test]
    fn test_recommend_formation_even() {
        // Equal split -> Line
        assert_eq!(recommend_formation(33, 33, 33), Formation::Line);
    }

    #[test]
    fn test_recommend_formation_empty() {
        assert_eq!(recommend_formation(0, 0, 0), Formation::Line);
    }

    #[test]
    fn test_prestige_tier_from_str_name() {
        assert_eq!(
            PrestigeTier::from_str_name("molt"),
            Some(PrestigeTier::Molt)
        );
        assert_eq!(
            PrestigeTier::from_str_name("CHRYSALIS"),
            Some(PrestigeTier::Chrysalis)
        );
        assert_eq!(
            PrestigeTier::from_str_name("Transcendence"),
            Some(PrestigeTier::Transcendence)
        );
        assert_eq!(PrestigeTier::from_str_name("invalid"), None);
    }

    #[test]
    fn test_prestige_reward_serializes() {
        let reward = PrestigeReward {
            phoenix_ash_earned: 42,
            resource_multiplier: 1.5,
            xp_multiplier: 2.0,
            new_unlocks: vec!["Galaxy #2".into()],
        };
        let json = serde_json::to_value(&reward).expect("JSON value conversion should succeed");
        assert_eq!(json["phoenix_ash_earned"], 42);
        assert_eq!(json["new_unlocks"][0], "Galaxy #2");
    }

    #[test]
    fn test_talent_tree_serializes() {
        let tree = build_talent_tree("warrior", 50);
        let json = serde_json::to_value(&tree).expect("JSON value conversion should succeed");
        assert_eq!(json["unit_type"], "warrior");
        assert_eq!(json["available_points"], 10);
        assert_eq!(json["nodes"].as_array().expect("value should be an array").len(), 45);
    }

    #[test]
    fn test_formation_effect_serializes() {
        let effect = formation_effects(&Formation::Wedge);
        let json = serde_json::to_value(&effect).expect("JSON value conversion should succeed");
        assert_eq!(json["formation"], "Wedge");
        assert_eq!(json["damage_bonus"], 0.15);
    }
}
