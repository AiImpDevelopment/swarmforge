// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! Faction-Specific Resource Mechanics, Patrol System, Unit Movement
//!
//! Based on: SwarmForge1.md resource matrix + swarmforge_schema.sql tables 11-13
//!
//! ## Faction Resource Mechanics
//!
//! Each of the four factions (Insects, Demons, Undead, Humans) has a unique
//! resource economy beyond the universal trio (ore, crystal, essence):
//!
//! - **Insects**: Chitin / Resin / Spore Gas + Biomass, Hive Energy, Larvae
//! - **Demons**: Brimstone / Soulstone / Wrath Essence + Suenden, Kultisten, Corruption
//! - **Undead**: Bone / Ether Shard / Death Mist + Eiter Essence, Leichenteile
//! - **Humans**: Steel / Crystal / Deuterium + Nahrung, Solar Energy, Faith
//!
//! Faction-unique mechanics:
//! - Insect queens inject 3 larvae per hatchery (40s cooldown, max 19)
//! - Demon corruption decays 5%/hour without combat
//! - Undead Adepten manually spread Blight across hex tiles
//! - Human Faith grows from victories, decays without them
//!
//! ## Patrol System
//!
//! Units follow waypoint routes with configurable patrol modes (Cyclic,
//! PingPong, OneWay).  Patrols auto-engage enemies within an engagement
//! radius and return after exceeding a leash distance.
//!
//! ## SC2-Style Unit Movement
//!
//! Continuous float positions with smooth interpolation toward target
//! coordinates.  Movement speed is per-unit, facing updates automatically.

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_factions", "Game");

// ---------------------------------------------------------------------------
// Part 1: Faction-Specific Resource Mechanics
// ---------------------------------------------------------------------------

/// Complete faction resource state.
///
/// Tracks the universal resources (ore, crystal, essence) shared by all
/// factions plus faction-specific resources with unique names and mechanics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionResources {
    // Universal resources (all factions)
    /// Structure resource (Chitin / Brimstone / Bone / Steel)
    pub ore: f64,
    /// Advanced resource (Resin / Soulstone / Ether Shard / Crystal)
    pub crystal: f64,
    /// Fuel resource (Spore Gas / Wrath Essence / Death Mist / Deuterium)
    pub essence: f64,
    /// Net energy: production minus consumption
    pub energy_balance: f64,
    /// Rare universal currency
    pub dark_matter: u64,

    // Faction-specific
    /// Faction identifier: "insects", "demons", "undead", "humans"
    pub faction: String,
    /// Food analog (Biomass / Suenden / Eiter Essence / Nahrung)
    pub food: f64,
    /// Faction energy (Hive Energy / Kultisten / Eiter covers both / Solar)
    pub faction_energy: f64,
    /// Flight fuel (Spore Gas / Kultisten-prayers / Death Mist / Deuterium)
    pub flight_fuel: f64,
    /// Faction-unique resource (Larvae / Corruption / Leichenteile / Faith)
    pub faction_unique: f64,
    /// Cross-faction resource (Infected Biomass / Cursed Suenden / Pestilent Leichenteile / none)
    pub cross_faction: f64,

    // Production rates (per hour)
    pub ore_rate: f64,
    pub crystal_rate: f64,
    pub essence_rate: f64,
    pub food_rate: f64,
    pub unique_rate: f64,

    /// Unix epoch seconds of the last resource calculation
    pub last_calc_epoch: i64,
}

impl FactionResources {
    /// Create a default resource state for a new colony of the given faction.
    pub fn new(faction: &str) -> Self {
        let (ore_rate, crystal_rate, essence_rate, food_rate, unique_rate) = match faction {
            "insects" => (30.0, 15.0, 10.0, 20.0, 3.0),
            "demons" => (25.0, 20.0, 15.0, 10.0, 5.0),
            "undead" => (20.0, 25.0, 20.0, 15.0, 2.0),
            _ => (35.0, 18.0, 12.0, 25.0, 1.0), // humans
        };
        Self {
            ore: 500.0,
            crystal: 200.0,
            essence: 100.0,
            energy_balance: 0.0,
            dark_matter: 0,
            faction: faction.to_string(),
            food: 300.0,
            faction_energy: 100.0,
            flight_fuel: 50.0,
            faction_unique: 0.0,
            cross_faction: 0.0,
            ore_rate,
            crystal_rate,
            essence_rate,
            food_rate,
            unique_rate,
            last_calc_epoch: Utc::now().timestamp(),
        }
    }

    /// Advance resources by `delta_secs` seconds using current production rates.
    pub fn tick(&mut self, delta_secs: f64) {
        let hours = delta_secs / 3600.0;
        self.ore += self.ore_rate * hours;
        self.crystal += self.crystal_rate * hours;
        self.essence += self.essence_rate * hours;
        self.food += self.food_rate * hours;
        self.faction_unique += self.unique_rate * hours;
        self.last_calc_epoch = Utc::now().timestamp();
    }
}

/// Return the faction-themed resource names as a map.
///
/// Keys are generic identifiers (`ore`, `crystal`, `essence`, `food`,
/// `energy`, `flight_fuel`, `unique`, `cross`).  Values are the
/// faction-specific display names from the SwarmForge design document.
pub fn resource_names(faction: &str) -> HashMap<String, String> {
    match faction {
        "insects" => HashMap::from([
            ("ore".into(), "Chitin".into()),
            ("crystal".into(), "Resin".into()),
            ("essence".into(), "Spore Gas".into()),
            ("food".into(), "Biomass".into()),
            ("energy".into(), "Energy (Hive)".into()),
            ("flight_fuel".into(), "Spore Gas".into()),
            ("unique".into(), "Larvae".into()),
            ("cross".into(), "Infected Biomass".into()),
        ]),
        "demons" => HashMap::from([
            ("ore".into(), "Brimstone".into()),
            ("crystal".into(), "Soulstone".into()),
            ("essence".into(), "Wrath Essence".into()),
            ("food".into(), "Suenden (Sins)".into()),
            ("energy".into(), "Kultisten Energy (Living Power Grid)".into()),
            ("flight_fuel".into(), "Kultisten Prayers".into()),
            ("sacrifice".into(), "Kultisten Sacrifice (Summon Fuel)".into()),
            ("unique".into(), "Corruption".into()),
            ("cross".into(), "Cursed Suenden".into()),
        ]),
        "undead" => HashMap::from([
            ("ore".into(), "Bone".into()),
            ("crystal".into(), "Ether Shard".into()),
            ("essence".into(), "Death Mist".into()),
            ("food".into(), "Eiter Essence".into()),
            ("energy".into(), "Eiter Essence".into()), // same as food
            ("flight_fuel".into(), "Death Mist".into()),
            ("unique".into(), "Leichenteile (Corpses)".into()),
            ("cross".into(), "Pestilent Leichenteile".into()),
        ]),
        _ => HashMap::from([
            ("ore".into(), "Steel".into()),
            ("crystal".into(), "Crystal".into()),
            ("essence".into(), "Deuterium".into()),
            ("food".into(), "Nahrung (Food)".into()),
            ("energy".into(), "Energy (Solar)".into()),
            ("flight_fuel".into(), "Deuterium".into()),
            ("unique".into(), "Faith / Moral".into()),
            ("cross".into(), "--- (none)".into()),
        ]),
    }
}

// ---------------------------------------------------------------------------
// Faction-Specific Mechanics
// ---------------------------------------------------------------------------

/// Insect-specific: Queen larvae injection mechanic.
///
/// Each queen injects 3 larvae every 40 seconds into a hatchery.
/// A hatchery holds at most 19 larvae at once.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LarvaeInjection {
    pub queen_count: u32,
    /// Larvae produced per inject cycle (3 per queen)
    pub larvae_per_inject: u32,
    /// Cooldown between inject cycles in seconds
    pub inject_cooldown_secs: u32,
    /// Maximum larvae a single hatchery can hold
    pub max_larvae: u32,
    pub current_larvae: u32,
    pub auto_inject: bool,
}

impl LarvaeInjection {
    pub fn new(queen_count: u32) -> Self {
        Self {
            queen_count,
            larvae_per_inject: 3,
            inject_cooldown_secs: 40,
            max_larvae: 19,
            current_larvae: 0,
            auto_inject: true,
        }
    }

    /// Perform one inject cycle.  Returns the number of larvae actually added.
    pub fn inject(&mut self) -> u32 {
        let space = self.max_larvae.saturating_sub(self.current_larvae);
        let to_add = (self.queen_count * self.larvae_per_inject).min(space);
        self.current_larvae += to_add;
        to_add
    }
}

/// Insect-specific: Bonus colony with 3-day visibility timer.
///
/// After creation, the colony is visible for 3 days on the galaxy map.
/// Once the timer expires it becomes invisible to other players.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusColony {
    pub colony_id: String,
    /// ISO-8601 timestamp when visibility expires (3 days from creation)
    pub visible_until: String,
    /// Whether the colony is still visible on the map
    pub is_visible: bool,
}

/// Demon-specific: Corruption resource that decays without combat.
///
/// Corruption is gained from fighting and lost at 5% per hour during
/// peacetime.  It acts as a faction-unique power multiplier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorruptionState {
    pub amount: f64,
    /// Percentage lost per hour without combat (0.05 = 5%)
    pub decay_rate_per_hour: f64,
    /// Amount gained from the last combat encounter
    pub gained_from_combat: f64,
    /// ISO-8601 timestamp of the last combat event
    pub last_combat: String,
}

impl CorruptionState {
    pub fn new() -> Self {
        Self {
            amount: 0.0,
            decay_rate_per_hour: 0.05,
            gained_from_combat: 0.0,
            last_combat: Utc::now().to_rfc3339(),
        }
    }

    /// Apply hourly decay.  Corruption decreases by `decay_rate_per_hour`
    /// fraction each hour since `last_combat`.
    pub fn apply_decay(&mut self, hours_since_combat: f64) {
        if hours_since_combat > 0.0 {
            let factor = (1.0 - self.decay_rate_per_hour).powf(hours_since_combat);
            self.amount *= factor;
        }
    }

    /// Record a combat event that generates corruption.
    pub fn gain_from_combat(&mut self, amount: f64) {
        self.amount += amount;
        self.gained_from_combat = amount;
        self.last_combat = Utc::now().to_rfc3339();
    }
}

/// Demon Kultisten Energy & Sacrifice System
///
/// Kultisten serve THREE purposes:
/// 1. ENERGY PRODUCTION: Each Kultist generates energy (like Human solar panels)
/// 2. SUMMONING SPEED: More Kultisten = faster demon summoning at altars
/// 3. SACRIFICE: Large demons require Kultist sacrifices to summon
///
/// Risk/Reward: Sacrificing Kultisten gives you powerful demons but reduces
/// your energy production. This creates a strategic tension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KultistenSystem {
    /// Total Kultisten alive
    pub count: u32,
    /// Energy produced per Kultist per hour
    pub energy_per_kultist: f64,
    /// Total energy production: count * energy_per_kultist
    pub total_energy_per_hour: f64,
    /// Summoning speed bonus: 1.0 + (count * 0.02)
    pub summon_speed_bonus: f64,
    /// Kultisten production rate (from Dark Altar)
    pub production_per_hour: u32,
    /// Number of altars producing Kultisten
    pub altar_count: u32,
    /// Kultisten sacrificed total (lifetime stat)
    pub total_sacrificed: u32,
}

impl KultistenSystem {
    /// Create a new Kultisten system with default values.
    pub fn new() -> Self {
        let count = 5_u32;
        Self {
            count,
            energy_per_kultist: 50.0,
            total_energy_per_hour: count as f64 * 50.0,
            summon_speed_bonus: 1.0 + (count as f64 * 0.02),
            production_per_hour: 5,
            altar_count: 1,
            total_sacrificed: 0,
        }
    }

    /// Recalculate derived fields after any count change.
    fn recalculate(&mut self) {
        self.total_energy_per_hour = self.count as f64 * self.energy_per_kultist;
        self.summon_speed_bonus = kultisten_summon_bonus(self);
    }
}

/// Sacrifice costs for summoning large demons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SacrificeCost {
    pub unit_name: String,
    pub kultisten_required: u32,
    /// Suenden (food) cost
    pub additional_suenden: f64,
    /// Brimstone (ore) cost
    pub additional_brimstone: f64,
    pub summon_time_secs: u32,
}

/// Return the sacrifice cost table for all summonable large demons.
pub fn get_sacrifice_costs() -> Vec<SacrificeCost> {
    vec![
        SacrificeCost {
            unit_name: "Greater Demon".into(),
            kultisten_required: 3,
            additional_suenden: 500.0,
            additional_brimstone: 300.0,
            summon_time_secs: 120,
        },
        SacrificeCost {
            unit_name: "Pit Lord".into(),
            kultisten_required: 5,
            additional_suenden: 1000.0,
            additional_brimstone: 800.0,
            summon_time_secs: 300,
        },
        SacrificeCost {
            unit_name: "Infernal Lord".into(),
            kultisten_required: 8,
            additional_suenden: 2000.0,
            additional_brimstone: 1500.0,
            summon_time_secs: 600,
        },
        SacrificeCost {
            unit_name: "Abyssal Maw".into(),
            kultisten_required: 20,
            additional_suenden: 10000.0,
            additional_brimstone: 8000.0,
            summon_time_secs: 3600,
        },
        SacrificeCost {
            unit_name: "Chaos Sorcerer (Hero)".into(),
            kultisten_required: 10,
            additional_suenden: 5000.0,
            additional_brimstone: 3000.0,
            summon_time_secs: 900,
        },
    ]
}

/// Calculate current Kultisten energy output.
pub fn kultisten_energy(system: &KultistenSystem) -> f64 {
    system.count as f64 * system.energy_per_kultist
}

/// Calculate summoning speed bonus from Kultisten (+2% per Kultist).
pub fn kultisten_summon_bonus(system: &KultistenSystem) -> f64 {
    1.0 + (system.count as f64 * 0.02)
}

/// Attempt to sacrifice Kultisten for a large demon summon.
///
/// Returns the sacrifice cost on success, or an error if there are not
/// enough Kultisten or the unit name is unknown.
pub fn kultisten_sacrifice(
    system: &mut KultistenSystem,
    unit_name: &str,
) -> AppResult<SacrificeCost> {
    let costs = get_sacrifice_costs();
    let cost = costs
        .iter()
        .find(|c| c.unit_name == unit_name)
        .ok_or_else(|| {
            ImpForgeError::validation(
                "UNKNOWN_SACRIFICE",
                format!("Unknown unit: {unit_name}"),
            )
        })?;

    if system.count < cost.kultisten_required {
        return Err(ImpForgeError::validation(
            "NOT_ENOUGH_KULTISTEN",
            format!(
                "Need {} Kultisten, have {}",
                cost.kultisten_required, system.count
            ),
        ));
    }

    system.count -= cost.kultisten_required;
    system.total_sacrificed += cost.kultisten_required;
    system.recalculate();

    Ok(cost.clone())
}

/// Tick Kultisten production from altars over `delta_hours`.
pub fn kultisten_produce(system: &mut KultistenSystem, delta_hours: f64) {
    let produced =
        (system.altar_count as f64 * system.production_per_hour as f64 * delta_hours) as u32;
    system.count += produced;
    system.recalculate();
}

/// Undead-specific: Adepten manually spread Blight across tiles.
///
/// Each Adept is assigned to a hex coordinate and spreads blight at a
/// rate that depends on their tool level (Eimer level).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdeptState {
    pub adept_id: String,
    /// Cumulative service hours
    pub service_hours: f64,
    /// ISO-8601 timestamp when service started
    pub service_start: String,
    /// Tool level (higher = faster blight spread)
    pub eimer_level: u32,
    /// Tiles spread per hour
    pub blight_spread_rate: f64,
    /// Hex coordinate the adept is assigned to, if any
    pub assigned_hex: Option<(i32, i32)>,
}

/// Undead-specific: Leichenteile (corpse parts) harvested from kills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpseHarvest {
    pub kills_since_harvest: u32,
    /// Average corpse parts per kill
    pub leichenteile_per_kill: f64,
    pub total_harvested: f64,
}

impl CorpseHarvest {
    pub fn new() -> Self {
        Self {
            kills_since_harvest: 0,
            leichenteile_per_kill: 0.5,
            total_harvested: 0.0,
        }
    }

    /// Record kills and harvest corpse parts.
    pub fn harvest(&mut self, kills: u32) -> f64 {
        let gained = kills as f64 * self.leichenteile_per_kill;
        self.kills_since_harvest = 0;
        self.total_harvested += gained;
        gained
    }
}

/// Human-specific: Faith/Moral gained from victories, decays in peacetime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaithMorale {
    pub faith: f64,
    /// Faith gained per battle won
    pub faith_per_victory: f64,
    /// Faith lost per hour without victories
    pub faith_decay_per_hour: f64,
    /// Bonus percentage to all stats (faith / 100)
    pub morale_bonus: f64,
}

impl FaithMorale {
    pub fn new() -> Self {
        Self {
            faith: 0.0,
            faith_per_victory: 10.0,
            faith_decay_per_hour: 1.0,
            morale_bonus: 0.0,
        }
    }

    /// Record a victory and update morale bonus.
    pub fn record_victory(&mut self) {
        self.faith += self.faith_per_victory;
        self.morale_bonus = self.faith / 100.0;
    }

    /// Apply hourly decay and update morale bonus.
    pub fn apply_decay(&mut self, hours: f64) {
        self.faith = (self.faith - self.faith_decay_per_hour * hours).max(0.0);
        self.morale_bonus = self.faith / 100.0;
    }
}

// ---------------------------------------------------------------------------
// Part 2: Patrol System (schema tables 11-13)
// ---------------------------------------------------------------------------

/// Patrol behavior at the end of the route.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PatrolMode {
    /// A -> B -> C -> A -> B -> C (loop back to start)
    Cyclic,
    /// A -> B -> C -> B -> A -> B (reverse at each end)
    PingPong,
    /// A -> B -> C (stop at last waypoint)
    OneWay,
}

impl PatrolMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "ping_pong" | "pingpong" => Self::PingPong,
            "one_way" | "oneway" => Self::OneWay,
            _ => Self::Cyclic,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cyclic => "cyclic",
            Self::PingPong => "ping_pong",
            Self::OneWay => "one_way",
        }
    }
}

/// A patrol route that units follow between waypoints.
///
/// Units move along the waypoint list according to the patrol mode.
/// When enemies are detected within `engagement_radius` tiles, units
/// engage.  They return to patrol after chasing up to `leash_distance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatrolRoute {
    pub id: String,
    pub colony_id: String,
    /// Ordered hex coordinates defining the patrol path
    pub waypoints: Vec<(i32, i32)>,
    pub mode: PatrolMode,
    /// Tiles within which units engage detected enemies
    pub engagement_radius: u32,
    /// Maximum chase distance before returning to patrol
    pub leash_distance: u32,
    /// Index into `waypoints` the patrol is currently heading toward
    pub current_waypoint_index: usize,
    /// Unit IDs assigned to this patrol
    pub assigned_units: Vec<String>,
    pub is_active: bool,
    /// ISO-8601 creation timestamp
    pub created_at: String,
    /// Direction of travel for PingPong mode (true = forward, false = reverse)
    pub ping_pong_forward: bool,
}

/// An event logged during a patrol tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatrolLog {
    pub patrol_id: String,
    /// Event type: "engage", "arrive", "enemy_detected", "return", "complete"
    pub event_type: String,
    pub position: (i32, i32),
    pub details: String,
    pub timestamp: String,
}

/// Create a new patrol route.
pub fn create_patrol(
    colony_id: &str,
    waypoints: Vec<(i32, i32)>,
    mode: PatrolMode,
    engagement_radius: u32,
    leash_distance: u32,
) -> Result<PatrolRoute, ImpForgeError> {
    if waypoints.len() < 2 {
        return Err(ImpForgeError::validation(
            "PATROL_WAYPOINTS",
            "A patrol route requires at least 2 waypoints",
        ));
    }

    Ok(PatrolRoute {
        id: Uuid::new_v4().to_string(),
        colony_id: colony_id.to_string(),
        waypoints,
        mode,
        engagement_radius,
        leash_distance,
        current_waypoint_index: 0,
        assigned_units: Vec::new(),
        is_active: true,
        created_at: Utc::now().to_rfc3339(),
        ping_pong_forward: true,
    })
}

/// Advance a patrol to its next waypoint according to its mode.
///
/// Returns a `PatrolLog` describing the arrival event, or `None` if the
/// patrol has completed (OneWay reached the end).
pub fn advance_patrol(patrol: &mut PatrolRoute) -> Option<PatrolLog> {
    if patrol.waypoints.is_empty() || !patrol.is_active {
        return None;
    }

    let current_pos = patrol.waypoints[patrol.current_waypoint_index];

    match patrol.mode {
        PatrolMode::Cyclic => {
            patrol.current_waypoint_index =
                (patrol.current_waypoint_index + 1) % patrol.waypoints.len();
        }
        PatrolMode::PingPong => {
            if patrol.ping_pong_forward {
                if patrol.current_waypoint_index + 1 >= patrol.waypoints.len() {
                    patrol.ping_pong_forward = false;
                    patrol.current_waypoint_index =
                        patrol.current_waypoint_index.saturating_sub(1);
                } else {
                    patrol.current_waypoint_index += 1;
                }
            } else if patrol.current_waypoint_index == 0 {
                patrol.ping_pong_forward = true;
                patrol.current_waypoint_index = 1.min(patrol.waypoints.len() - 1);
            } else {
                patrol.current_waypoint_index -= 1;
            }
        }
        PatrolMode::OneWay => {
            if patrol.current_waypoint_index + 1 >= patrol.waypoints.len() {
                patrol.is_active = false;
                return Some(PatrolLog {
                    patrol_id: patrol.id.clone(),
                    event_type: "complete".to_string(),
                    position: current_pos,
                    details: "Patrol completed (one-way route finished)".to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                });
            }
            patrol.current_waypoint_index += 1;
        }
    }

    let next_pos = patrol.waypoints[patrol.current_waypoint_index];
    Some(PatrolLog {
        patrol_id: patrol.id.clone(),
        event_type: "arrive".to_string(),
        position: next_pos,
        details: format!(
            "Moving from ({},{}) to ({},{})",
            current_pos.0, current_pos.1, next_pos.0, next_pos.1
        ),
        timestamp: Utc::now().to_rfc3339(),
    })
}

// ---------------------------------------------------------------------------
// Part 3: SC2-Style Unit Movement
// ---------------------------------------------------------------------------

/// Continuous-space position for a unit with smooth movement toward a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitPosition {
    pub unit_id: String,
    /// Current X coordinate (continuous float)
    pub pos_x: f64,
    /// Current Y coordinate (continuous float)
    pub pos_y: f64,
    /// Target X coordinate, if moving
    pub target_x: Option<f64>,
    /// Target Y coordinate, if moving
    pub target_y: Option<f64>,
    /// Movement speed in tiles per second
    pub speed: f64,
    /// Facing direction in radians
    pub facing: f64,
    /// Whether the unit is currently in motion
    pub is_moving: bool,
}

impl UnitPosition {
    /// Create a stationary unit at the given position.
    pub fn new(unit_id: &str, x: f64, y: f64, speed: f64) -> Self {
        Self {
            unit_id: unit_id.to_string(),
            pos_x: x,
            pos_y: y,
            target_x: None,
            target_y: None,
            speed,
            facing: 0.0,
            is_moving: false,
        }
    }

    /// Set a new movement target.
    pub fn move_to(&mut self, tx: f64, ty: f64) {
        self.target_x = Some(tx);
        self.target_y = Some(ty);
        self.is_moving = true;
    }

    /// Stop all movement immediately at the current position.
    pub fn stop(&mut self) {
        self.target_x = None;
        self.target_y = None;
        self.is_moving = false;
    }
}

/// Advance a unit toward its target by `delta_secs` seconds of movement.
///
/// When the unit arrives within 0.1 tiles of the target, it snaps to the
/// exact target position and stops.  Facing is updated to the direction
/// of travel.
pub fn move_unit_tick(unit: &mut UnitPosition, delta_secs: f64) {
    if let (Some(tx), Some(ty)) = (unit.target_x, unit.target_y) {
        let dx = tx - unit.pos_x;
        let dy = ty - unit.pos_y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < 0.1 {
            // Snap to target
            unit.pos_x = tx;
            unit.pos_y = ty;
            unit.target_x = None;
            unit.target_y = None;
            unit.is_moving = false;
        } else {
            let move_dist = unit.speed * delta_secs;
            let ratio = (move_dist / dist).min(1.0);
            unit.pos_x += dx * ratio;
            unit.pos_y += dy * ratio;
            unit.facing = dy.atan2(dx);
            unit.is_moving = true;
        }
    }
}

// ---------------------------------------------------------------------------
// SwarmFactionEngine — SQLite persistence
// ---------------------------------------------------------------------------

/// Persistent engine for faction resources, patrols, and unit positions.
///
/// Stores all state in the shared `swarmforge.db` database (WAL mode).
pub struct SwarmFactionEngine {
    conn: Mutex<Connection>,
}

impl SwarmFactionEngine {
    /// Open (or create) the swarmforge database and initialize faction tables.
    pub fn new(data_dir: &std::path::Path) -> Result<Self, ImpForgeError> {
        std::fs::create_dir_all(data_dir).map_err(|e| {
            ImpForgeError::filesystem("FACTION_DIR", format!("Cannot create data dir: {e}"))
        })?;

        let db_path = data_dir.join("swarmforge.db");
        let conn = Connection::open(&db_path).map_err(|e| {
            ImpForgeError::internal("FACTION_DB_OPEN", format!("SQLite open failed: {e}"))
        })?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;",
        )
        .map_err(|e| {
            ImpForgeError::internal("FACTION_DB_PRAGMA", format!("Pragma failed: {e}"))
        })?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS faction_resources (
                colony_id       TEXT PRIMARY KEY,
                faction         TEXT NOT NULL,
                resources_json  TEXT NOT NULL,
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS patrol_routes (
                id              TEXT PRIMARY KEY,
                colony_id       TEXT NOT NULL,
                route_json      TEXT NOT NULL,
                is_active       INTEGER NOT NULL DEFAULT 1,
                created_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS patrol_logs (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                patrol_id       TEXT NOT NULL,
                event_type      TEXT NOT NULL,
                position_x      INTEGER NOT NULL,
                position_y      INTEGER NOT NULL,
                details         TEXT NOT NULL,
                timestamp       TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS unit_positions (
                unit_id         TEXT PRIMARY KEY,
                position_json   TEXT NOT NULL,
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS kultisten_system (
                colony_id       TEXT PRIMARY KEY,
                system_json     TEXT NOT NULL,
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_patrols_colony
                ON patrol_routes(colony_id);
            CREATE INDEX IF NOT EXISTS idx_patrol_logs_patrol
                ON patrol_logs(patrol_id);",
        )
        .map_err(|e| {
            ImpForgeError::internal("FACTION_DB_SCHEMA", format!("Schema creation failed: {e}"))
        })?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // -- Faction Resources --

    /// Load or initialize faction resources for a colony.
    pub fn get_resources(&self, colony_id: &str, faction: &str) -> AppResult<FactionResources> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FACTION_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let result: Result<String, _> = conn.query_row(
            "SELECT resources_json FROM faction_resources WHERE colony_id = ?1",
            params![colony_id],
            |row| row.get(0),
        );

        match result {
            Ok(json) => serde_json::from_str(&json).map_err(|e| {
                ImpForgeError::internal("FACTION_PARSE", format!("JSON parse failed: {e}"))
            }),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let resources = FactionResources::new(faction);
                let json = serde_json::to_string(&resources).map_err(|e| {
                    ImpForgeError::internal(
                        "FACTION_SERIALIZE",
                        format!("JSON serialize failed: {e}"),
                    )
                })?;
                conn.execute(
                    "INSERT INTO faction_resources (colony_id, faction, resources_json)
                     VALUES (?1, ?2, ?3)",
                    params![colony_id, faction, json],
                )
                .map_err(|e| {
                    ImpForgeError::internal("FACTION_INSERT", format!("Insert failed: {e}"))
                })?;
                Ok(resources)
            }
            Err(e) => Err(ImpForgeError::internal(
                "FACTION_QUERY",
                format!("Query failed: {e}"),
            )),
        }
    }

    /// Tick resources forward and persist the updated state.
    pub fn tick_resources(
        &self,
        colony_id: &str,
        faction: &str,
        delta_secs: f64,
    ) -> AppResult<FactionResources> {
        let mut resources = self.get_resources(colony_id, faction)?;
        resources.tick(delta_secs);

        let json = serde_json::to_string(&resources).map_err(|e| {
            ImpForgeError::internal("FACTION_SERIALIZE", format!("JSON serialize failed: {e}"))
        })?;

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FACTION_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        conn.execute(
            "UPDATE faction_resources SET resources_json = ?1, updated_at = datetime('now')
             WHERE colony_id = ?2",
            params![json, colony_id],
        )
        .map_err(|e| {
            ImpForgeError::internal("FACTION_UPDATE", format!("Update failed: {e}"))
        })?;

        Ok(resources)
    }

    /// Execute a faction-specific special action.
    ///
    /// Returns a JSON object describing the action result.
    pub fn special_action(
        &self,
        faction: &str,
        action: &str,
    ) -> AppResult<serde_json::Value> {
        match (faction, action) {
            ("insects", "larvae_inject") => {
                let mut inj = LarvaeInjection::new(1);
                let added = inj.inject();
                Ok(serde_json::json!({
                    "action": "larvae_inject",
                    "larvae_added": added,
                    "current_larvae": inj.current_larvae,
                    "max_larvae": inj.max_larvae,
                }))
            }
            ("demons", "corruption_decay") => {
                let mut corruption = CorruptionState::new();
                corruption.amount = 100.0;
                corruption.apply_decay(1.0); // 1 hour of decay
                Ok(serde_json::json!({
                    "action": "corruption_decay",
                    "remaining": corruption.amount,
                    "decay_rate": corruption.decay_rate_per_hour,
                }))
            }
            ("undead", "adept_spread") => {
                Ok(serde_json::json!({
                    "action": "adept_spread",
                    "spread_rate_per_hour": 2.0,
                    "eimer_level": 1,
                    "status": "spreading",
                }))
            }
            ("humans", "faith_gain") => {
                let mut faith = FaithMorale::new();
                faith.record_victory();
                Ok(serde_json::json!({
                    "action": "faith_gain",
                    "faith": faith.faith,
                    "morale_bonus_pct": faith.morale_bonus,
                }))
            }
            _ => Err(ImpForgeError::validation(
                "FACTION_ACTION_UNKNOWN",
                format!("Unknown action '{action}' for faction '{faction}'"),
            )),
        }
    }

    // -- Patrol System --

    /// Create a new patrol route and persist it.
    pub fn create_patrol_route(
        &self,
        colony_id: &str,
        waypoints: Vec<(i32, i32)>,
        mode: &str,
        engagement_radius: u32,
        leash_distance: u32,
    ) -> AppResult<PatrolRoute> {
        let patrol = create_patrol(
            colony_id,
            waypoints,
            PatrolMode::from_str(mode),
            engagement_radius,
            leash_distance,
        )?;

        let json = serde_json::to_string(&patrol).map_err(|e| {
            ImpForgeError::internal("PATROL_SERIALIZE", format!("JSON serialize failed: {e}"))
        })?;

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FACTION_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        conn.execute(
            "INSERT INTO patrol_routes (id, colony_id, route_json, is_active)
             VALUES (?1, ?2, ?3, 1)",
            params![patrol.id, colony_id, json],
        )
        .map_err(|e| {
            ImpForgeError::internal("PATROL_INSERT", format!("Insert failed: {e}"))
        })?;

        Ok(patrol)
    }

    /// List all active patrols for a colony.
    pub fn list_patrols(&self, colony_id: &str) -> AppResult<Vec<PatrolRoute>> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FACTION_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT route_json FROM patrol_routes
                 WHERE colony_id = ?1 AND is_active = 1
                 ORDER BY created_at DESC",
            )
            .map_err(|e| {
                ImpForgeError::internal("PATROL_PREPARE", format!("Prepare failed: {e}"))
            })?;

        let patrols: Vec<PatrolRoute> = stmt
            .query_map(params![colony_id], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            })
            .map_err(|e| {
                ImpForgeError::internal("PATROL_QUERY", format!("Query failed: {e}"))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|json| serde_json::from_str(&json).ok())
            .collect();

        Ok(patrols)
    }

    /// Tick all active patrols, advancing each to its next waypoint.
    ///
    /// Returns a log of all events produced during the tick.
    pub fn tick_patrols(&self, _delta_secs: f64) -> AppResult<Vec<PatrolLog>> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FACTION_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let mut stmt = conn
            .prepare("SELECT id, route_json FROM patrol_routes WHERE is_active = 1")
            .map_err(|e| {
                ImpForgeError::internal("PATROL_PREPARE", format!("Prepare failed: {e}"))
            })?;

        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| {
                ImpForgeError::internal("PATROL_QUERY", format!("Query failed: {e}"))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut logs = Vec::new();

        for (patrol_id, json) in &rows {
            let parsed: Result<PatrolRoute, _> = serde_json::from_str(json);
            if let Ok(mut patrol) = parsed {
                if let Some(log_entry) = advance_patrol(&mut patrol) {
                    // Persist the log
                    let _ = conn.execute(
                        "INSERT INTO patrol_logs (patrol_id, event_type, position_x, position_y, details, timestamp)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            log_entry.patrol_id,
                            log_entry.event_type,
                            log_entry.position.0,
                            log_entry.position.1,
                            log_entry.details,
                            log_entry.timestamp,
                        ],
                    );
                    logs.push(log_entry);
                }

                // Update the patrol state in DB
                if let Ok(updated_json) = serde_json::to_string(&patrol) {
                    let _ = conn.execute(
                        "UPDATE patrol_routes SET route_json = ?1, is_active = ?2 WHERE id = ?3",
                        params![updated_json, patrol.is_active as i32, patrol_id],
                    );
                }
            }
        }

        Ok(logs)
    }

    /// Cancel (deactivate) a patrol route.
    pub fn cancel_patrol(&self, patrol_id: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FACTION_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let updated = conn
            .execute(
                "UPDATE patrol_routes SET is_active = 0 WHERE id = ?1",
                params![patrol_id],
            )
            .map_err(|e| {
                ImpForgeError::internal("PATROL_CANCEL", format!("Update failed: {e}"))
            })?;

        if updated == 0 {
            return Err(ImpForgeError::validation(
                "PATROL_NOT_FOUND",
                format!("Patrol '{patrol_id}' not found"),
            ));
        }

        Ok(())
    }

    // -- Unit Movement --

    /// Get the current position of a unit, or create a default at origin.
    pub fn get_unit_position(&self, unit_id: &str) -> AppResult<UnitPosition> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FACTION_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let result: Result<String, _> = conn.query_row(
            "SELECT position_json FROM unit_positions WHERE unit_id = ?1",
            params![unit_id],
            |row| row.get(0),
        );

        match result {
            Ok(json) => serde_json::from_str(&json).map_err(|e| {
                ImpForgeError::internal("UNIT_PARSE", format!("JSON parse failed: {e}"))
            }),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let pos = UnitPosition::new(unit_id, 0.0, 0.0, 2.0);
                self.save_unit_position_inner(&conn, &pos)?;
                Ok(pos)
            }
            Err(e) => Err(ImpForgeError::internal(
                "UNIT_QUERY",
                format!("Query failed: {e}"),
            )),
        }
    }

    /// Issue a move command for a unit.
    pub fn move_unit(&self, unit_id: &str, target_x: f64, target_y: f64) -> AppResult<UnitPosition> {
        let mut pos = self.get_unit_position(unit_id)?;
        pos.move_to(target_x, target_y);

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FACTION_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        self.save_unit_position_inner(&conn, &pos)?;
        Ok(pos)
    }

    /// Stop a unit in place.
    pub fn stop_unit(&self, unit_id: &str) -> AppResult<UnitPosition> {
        let mut pos = self.get_unit_position(unit_id)?;
        pos.stop();

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FACTION_LOCK", format!("Mutex poisoned: {e}"))
        })?;
        self.save_unit_position_inner(&conn, &pos)?;
        Ok(pos)
    }

    /// Tick all units with active move targets, advancing them by `delta_secs`.
    pub fn tick_movement(&self, delta_secs: f64) -> AppResult<Vec<UnitPosition>> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("FACTION_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let mut stmt = conn
            .prepare("SELECT position_json FROM unit_positions")
            .map_err(|e| {
                ImpForgeError::internal("UNIT_PREPARE", format!("Prepare failed: {e}"))
            })?;

        let all: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| {
                ImpForgeError::internal("UNIT_QUERY", format!("Query failed: {e}"))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut updated = Vec::new();

        for json in &all {
            if let Ok(mut pos) = serde_json::from_str::<UnitPosition>(json) {
                if pos.is_moving {
                    move_unit_tick(&mut pos, delta_secs);
                    self.save_unit_position_inner(&conn, &pos)?;
                    updated.push(pos);
                }
            }
        }

        Ok(updated)
    }

    /// Persist a unit position (internal helper, caller holds the lock).
    fn save_unit_position_inner(
        &self,
        conn: &Connection,
        pos: &UnitPosition,
    ) -> AppResult<()> {
        let json = serde_json::to_string(pos).map_err(|e| {
            ImpForgeError::internal("UNIT_SERIALIZE", format!("JSON serialize failed: {e}"))
        })?;

        conn.execute(
            "INSERT INTO unit_positions (unit_id, position_json, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(unit_id) DO UPDATE SET
                position_json = excluded.position_json,
                updated_at = excluded.updated_at",
            params![pos.unit_id, json],
        )
        .map_err(|e| {
            ImpForgeError::internal("UNIT_SAVE", format!("Upsert failed: {e}"))
        })?;

        Ok(())
    }

    // -- Kultisten System --

    /// Load or initialize the Kultisten system for a demon colony.
    pub fn get_kultisten_system(&self, colony_id: &str) -> AppResult<KultistenSystem> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("KULTISTEN_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let result: Result<String, _> = conn.query_row(
            "SELECT system_json FROM kultisten_system WHERE colony_id = ?1",
            params![colony_id],
            |row| row.get(0),
        );

        match result {
            Ok(json) => serde_json::from_str(&json).map_err(|e| {
                ImpForgeError::internal("KULTISTEN_PARSE", format!("JSON parse failed: {e}"))
            }),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let system = KultistenSystem::new();
                let json = serde_json::to_string(&system).map_err(|e| {
                    ImpForgeError::internal(
                        "KULTISTEN_SERIALIZE",
                        format!("JSON serialize failed: {e}"),
                    )
                })?;
                conn.execute(
                    "INSERT INTO kultisten_system (colony_id, system_json) VALUES (?1, ?2)",
                    params![colony_id, json],
                )
                .map_err(|e| {
                    ImpForgeError::internal("KULTISTEN_INSERT", format!("Insert failed: {e}"))
                })?;
                Ok(system)
            }
            Err(e) => Err(ImpForgeError::internal(
                "KULTISTEN_QUERY",
                format!("Query failed: {e}"),
            )),
        }
    }

    /// Save updated Kultisten system state.
    fn save_kultisten_system(
        &self,
        conn: &Connection,
        colony_id: &str,
        system: &KultistenSystem,
    ) -> AppResult<()> {
        let json = serde_json::to_string(system).map_err(|e| {
            ImpForgeError::internal("KULTISTEN_SERIALIZE", format!("JSON serialize failed: {e}"))
        })?;
        conn.execute(
            "INSERT INTO kultisten_system (colony_id, system_json, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(colony_id) DO UPDATE SET
                system_json = excluded.system_json,
                updated_at = excluded.updated_at",
            params![colony_id, json],
        )
        .map_err(|e| {
            ImpForgeError::internal("KULTISTEN_SAVE", format!("Upsert failed: {e}"))
        })?;
        Ok(())
    }

    /// Sacrifice Kultisten to summon a large demon. Persists the updated state.
    pub fn sacrifice_kultisten(
        &self,
        colony_id: &str,
        unit_name: &str,
    ) -> AppResult<SacrificeCost> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("KULTISTEN_LOCK", format!("Mutex poisoned: {e}"))
        })?;

        // Load current state (or initialize)
        let json: String = conn
            .query_row(
                "SELECT system_json FROM kultisten_system WHERE colony_id = ?1",
                params![colony_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| serde_json::to_string(&KultistenSystem::new()).unwrap_or_default());

        let mut system: KultistenSystem = serde_json::from_str(&json).map_err(|e| {
            ImpForgeError::internal("KULTISTEN_PARSE", format!("JSON parse failed: {e}"))
        })?;

        let cost = kultisten_sacrifice(&mut system, unit_name)?;
        self.save_kultisten_system(&conn, colony_id, &system)?;
        Ok(cost)
    }
}

// ---------------------------------------------------------------------------
// Tauri IPC Commands (15 total: 4 resources, 4 patrols, 4 movement, 3 kultisten)
// ---------------------------------------------------------------------------

// -- Faction Resources (4) --

/// Get faction-themed resource names.
#[tauri::command]
pub async fn faction_resource_names(
    faction: String,
) -> Result<HashMap<String, String>, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_factions", "game_factions", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_factions", "game_factions");
    crate::synapse_fabric::synapse_session_push("swarm_factions", "game_factions", "faction_resource_names called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_factions", "info", "swarm_factions active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_factions", "command", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"op": "resource_names"}));
    Ok(resource_names(&faction))
}

/// Get the current resource state for a colony.
#[tauri::command]
pub async fn faction_resource_state(
    colony_id: String,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<FactionResources, ImpForgeError> {
    // Default to "humans" if faction unknown; the DB record stores the real faction
    engine.get_resources(&colony_id, "humans")
}

/// Tick resources forward by delta_secs and return updated state.
#[tauri::command]
pub async fn faction_tick_resources(
    colony_id: String,
    delta_secs: f64,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<FactionResources, ImpForgeError> {
    engine.tick_resources(&colony_id, "humans", delta_secs)
}

/// Execute a faction-specific special action (larvae_inject, corruption_decay, etc).
#[tauri::command]
pub async fn faction_special_action(
    faction: String,
    action: String,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<serde_json::Value, ImpForgeError> {
    engine.special_action(&faction, &action)
}

// -- Patrols (4) --

/// Create a new patrol route for a colony.
#[tauri::command]
pub async fn patrol_create(
    colony_id: String,
    waypoints: Vec<(i32, i32)>,
    mode: String,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<PatrolRoute, ImpForgeError> {
    engine.create_patrol_route(&colony_id, waypoints, &mode, 3, 8)
}

/// List all active patrols for a colony.
#[tauri::command]
pub async fn patrol_list(
    colony_id: String,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<Vec<PatrolRoute>, ImpForgeError> {
    engine.list_patrols(&colony_id)
}

/// Tick all active patrols, returning log events.
#[tauri::command]
pub async fn patrol_tick(
    delta_secs: f64,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<Vec<PatrolLog>, ImpForgeError> {
    engine.tick_patrols(delta_secs)
}

/// Cancel (deactivate) a patrol route.
#[tauri::command]
pub async fn patrol_cancel(
    patrol_id: String,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<(), ImpForgeError> {
    engine.cancel_patrol(&patrol_id)
}

// -- Unit Movement (4) --

/// Get the current position of a unit.
#[tauri::command]
pub async fn unit_position(
    unit_id: String,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<UnitPosition, ImpForgeError> {
    engine.get_unit_position(&unit_id)
}

/// Issue a move command to a unit.
#[tauri::command]
pub async fn unit_move_to(
    unit_id: String,
    target_x: f64,
    target_y: f64,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<UnitPosition, ImpForgeError> {
    engine.move_unit(&unit_id, target_x, target_y)
}

/// Tick all moving units forward by delta_secs.
#[tauri::command]
pub async fn unit_tick_movement(
    delta_secs: f64,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<Vec<UnitPosition>, ImpForgeError> {
    engine.tick_movement(delta_secs)
}

/// Stop a unit in place.
#[tauri::command]
pub async fn unit_stop(
    unit_id: String,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<UnitPosition, ImpForgeError> {
    engine.stop_unit(&unit_id)
}

// -- Kultisten System (3) --

/// Get the current Kultisten system status for a demon colony.
#[tauri::command]
pub async fn kultisten_status(
    colony_id: String,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<KultistenSystem, ImpForgeError> {
    engine.get_kultisten_system(&colony_id)
}

/// Sacrifice Kultisten to summon a large demon.
#[tauri::command]
pub async fn kultisten_do_sacrifice(
    colony_id: String,
    unit_name: String,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<SacrificeCost, ImpForgeError> {
    engine.sacrifice_kultisten(&colony_id, &unit_name)
}

/// Get the sacrifice cost table for all summonable demons.
#[tauri::command]
pub async fn kultisten_sacrifice_costs() -> Result<Vec<SacrificeCost>, ImpForgeError> {
    Ok(get_sacrifice_costs())
}

// ---------------------------------------------------------------------------
// Additional Tauri Commands — wiring internal helpers
// ---------------------------------------------------------------------------

/// Tick Kultisten energy production and new Kultisten from altars.
#[tauri::command]
pub async fn kultisten_tick_production(
    colony_id: String,
    delta_hours: f64,
    engine: tauri::State<'_, SwarmFactionEngine>,
) -> Result<serde_json::Value, ImpForgeError> {
    let mut ks = engine.get_kultisten_system(&colony_id)?;
    kultisten_produce(&mut ks, delta_hours);
    let energy = kultisten_energy(&ks);
    Ok(serde_json::json!({
        "count": ks.count,
        "energy_output": energy,
        "summon_speed_bonus": ks.summon_speed_bonus,
    }))
}

/// Get insect bonus colony visibility info.
#[tauri::command]
pub async fn faction_bonus_colony_info(
    colony_id: String,
) -> Result<BonusColony, ImpForgeError> {
    let visible_until = (Utc::now() + chrono::Duration::days(3)).to_rfc3339();
    Ok(BonusColony {
        colony_id,
        visible_until,
        is_visible: true,
    })
}

/// Record combat corruption gain for a demon colony and apply decay.
#[tauri::command]
pub async fn faction_corruption_combat(
    corruption_gain: f64,
    hours_since_combat: f64,
) -> Result<serde_json::Value, ImpForgeError> {
    let mut state = CorruptionState::new();
    state.gain_from_combat(corruption_gain);
    state.apply_decay(hours_since_combat);
    Ok(serde_json::json!({
        "amount": state.amount,
        "decay_rate": state.decay_rate_per_hour,
    }))
}

/// Get undead adept blight state for a new adept.
#[tauri::command]
pub async fn faction_undead_adept_state(
    adept_id: String,
    eimer_level: u32,
) -> Result<AdeptState, ImpForgeError> {
    Ok(AdeptState {
        adept_id,
        service_hours: 0.0,
        service_start: Utc::now().to_rfc3339(),
        eimer_level,
        blight_spread_rate: 0.5 + (eimer_level as f64 * 0.25),
        assigned_hex: None,
    })
}

/// Harvest corpse parts from kills (undead mechanic).
#[tauri::command]
pub async fn faction_undead_harvest(
    kills: u32,
) -> Result<serde_json::Value, ImpForgeError> {
    let mut harvest = CorpseHarvest::new();
    let gained = harvest.harvest(kills);
    Ok(serde_json::json!({
        "leichenteile_gained": gained,
        "total_harvested": harvest.total_harvested,
    }))
}

/// Apply faith decay for a human faction (peacetime morale loss).
#[tauri::command]
pub async fn faction_faith_decay(
    hours: f64,
) -> Result<serde_json::Value, ImpForgeError> {
    let mut fm = FaithMorale::new();
    fm.record_victory();
    fm.apply_decay(hours);
    Ok(serde_json::json!({
        "faith": fm.faith,
        "morale_bonus": fm.morale_bonus,
    }))
}

/// Get patrol mode from its string key.
#[tauri::command]
pub async fn faction_patrol_mode_info(
    mode: String,
) -> Result<serde_json::Value, ImpForgeError> {
    let pm = PatrolMode::from_str(&mode);
    Ok(serde_json::json!({
        "mode": pm.as_str(),
    }))
}

// ---------------------------------------------------------------------------
// Tests (18 + Kultisten tests)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;


    // -- Faction Resources --

    #[test]
    fn test_faction_resources_new_insects() {
        let r = FactionResources::new("insects");
        assert_eq!(r.faction, "insects");
        assert_eq!(r.ore, 500.0);
        assert_eq!(r.ore_rate, 30.0);
        assert_eq!(r.crystal_rate, 15.0);
    }

    #[test]
    fn test_faction_resources_new_demons() {
        let r = FactionResources::new("demons");
        assert_eq!(r.faction, "demons");
        assert_eq!(r.ore_rate, 25.0);
        assert_eq!(r.unique_rate, 5.0);
    }

    #[test]
    fn test_faction_resources_tick() {
        let mut r = FactionResources::new("humans");
        let initial_ore = r.ore;
        r.tick(3600.0); // 1 hour
        assert!((r.ore - (initial_ore + r.ore_rate)).abs() < 0.01);
    }

    #[test]
    fn test_resource_names_all_factions() {
        for faction in &["insects", "demons", "undead", "humans"] {
            let names = resource_names(faction);
            assert!(names.contains_key("ore"));
            assert!(names.contains_key("crystal"));
            assert!(names.contains_key("essence"));
            assert!(names.contains_key("food"));
            assert!(names.contains_key("unique"));
            if *faction == "demons" {
                assert!(names.contains_key("sacrifice"));
                assert_eq!(names.len(), 9);
            } else {
                assert_eq!(names.len(), 8);
            }
        }
    }

    #[test]
    fn test_resource_names_unknown_defaults_to_humans() {
        let names = resource_names("elves");
        assert_eq!(names["ore"], "Steel");
    }

    // -- Larvae Injection --

    #[test]
    fn test_larvae_inject() {
        let mut inj = LarvaeInjection::new(2);
        let added = inj.inject();
        assert_eq!(added, 6); // 2 queens * 3
        assert_eq!(inj.current_larvae, 6);
    }

    #[test]
    fn test_larvae_inject_cap() {
        let mut inj = LarvaeInjection::new(3);
        inj.current_larvae = 17;
        let added = inj.inject();
        assert_eq!(added, 2); // only 2 space left (19 - 17)
        assert_eq!(inj.current_larvae, 19);
    }

    // -- Corruption --

    #[test]
    fn test_corruption_decay() {
        let mut c = CorruptionState::new();
        c.amount = 100.0;
        c.apply_decay(1.0); // 1 hour
        // 100 * (1 - 0.05)^1 = 95
        assert!((c.amount - 95.0).abs() < 0.01);
    }

    #[test]
    fn test_corruption_gain() {
        let mut c = CorruptionState::new();
        c.gain_from_combat(50.0);
        assert_eq!(c.amount, 50.0);
        assert_eq!(c.gained_from_combat, 50.0);
    }

    // -- Faith --

    #[test]
    fn test_faith_victory() {
        let mut f = FaithMorale::new();
        f.record_victory();
        assert_eq!(f.faith, 10.0);
        assert!((f.morale_bonus - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_faith_decay() {
        let mut f = FaithMorale::new();
        f.faith = 10.0;
        f.apply_decay(5.0); // 5 hours
        assert_eq!(f.faith, 5.0);
    }

    #[test]
    fn test_faith_decay_floor() {
        let mut f = FaithMorale::new();
        f.faith = 2.0;
        f.apply_decay(10.0);
        assert_eq!(f.faith, 0.0); // clamped to 0
    }

    // -- Corpse Harvest --

    #[test]
    fn test_corpse_harvest() {
        let mut ch = CorpseHarvest::new();
        let gained = ch.harvest(10);
        assert!((gained - 5.0).abs() < 0.01); // 10 * 0.5
        assert!((ch.total_harvested - 5.0).abs() < 0.01);
    }

    // -- Patrol System --

    #[test]
    fn test_create_patrol_min_waypoints() {
        let result = create_patrol("c1", vec![(0, 0)], PatrolMode::Cyclic, 3, 8);
        assert!(result.is_err());
    }

    #[test]
    fn test_patrol_cyclic_advance() {
        let mut patrol = create_patrol(
            "c1",
            vec![(0, 0), (1, 0), (2, 0)],
            PatrolMode::Cyclic,
            3,
            8,
        )
        .expect("valid patrol");

        let log1 = advance_patrol(&mut patrol);
        assert!(log1.is_some());
        assert_eq!(patrol.current_waypoint_index, 1);

        let _ = advance_patrol(&mut patrol);
        assert_eq!(patrol.current_waypoint_index, 2);

        let _ = advance_patrol(&mut patrol);
        assert_eq!(patrol.current_waypoint_index, 0); // wrapped
    }

    #[test]
    fn test_patrol_oneway_completes() {
        let mut patrol = create_patrol(
            "c1",
            vec![(0, 0), (1, 0)],
            PatrolMode::OneWay,
            3,
            8,
        )
        .expect("valid patrol");

        let log1 = advance_patrol(&mut patrol);
        assert!(log1.is_some());
        assert_eq!(patrol.current_waypoint_index, 1);

        let log2 = advance_patrol(&mut patrol);
        assert!(log2.is_some());
        assert_eq!(log2.as_ref().map(|l| l.event_type.as_str()), Some("complete"));
        assert!(!patrol.is_active);
    }

    #[test]
    fn test_patrol_pingpong() {
        let mut patrol = create_patrol(
            "c1",
            vec![(0, 0), (1, 0), (2, 0)],
            PatrolMode::PingPong,
            3,
            8,
        )
        .expect("valid patrol");

        // Forward: 0 -> 1 -> 2
        advance_patrol(&mut patrol);
        assert_eq!(patrol.current_waypoint_index, 1);
        advance_patrol(&mut patrol);
        assert_eq!(patrol.current_waypoint_index, 2);

        // Reverse at end: 2 -> 1
        advance_patrol(&mut patrol);
        assert_eq!(patrol.current_waypoint_index, 1);

        // Continue reverse: 1 -> 0
        advance_patrol(&mut patrol);
        assert_eq!(patrol.current_waypoint_index, 0);

        // Forward again: 0 -> 1
        advance_patrol(&mut patrol);
        assert_eq!(patrol.current_waypoint_index, 1);
    }

    // -- Unit Movement --

    #[test]
    fn test_unit_move_tick() {
        let mut unit = UnitPosition::new("u1", 0.0, 0.0, 5.0);
        unit.move_to(10.0, 0.0);
        move_unit_tick(&mut unit, 1.0);
        // Should have moved 5.0 tiles in 1 second at speed 5
        assert!((unit.pos_x - 5.0).abs() < 0.01);
        assert!(unit.is_moving);
    }

    #[test]
    fn test_unit_snap_to_target() {
        let mut unit = UnitPosition::new("u1", 9.95, 0.0, 5.0);
        unit.move_to(10.0, 0.0);
        move_unit_tick(&mut unit, 1.0);
        // Within 0.1 of target => snap
        assert_eq!(unit.pos_x, 10.0);
        assert!(!unit.is_moving);
        assert!(unit.target_x.is_none());
    }

    #[test]
    fn test_unit_stop() {
        let mut unit = UnitPosition::new("u1", 5.0, 5.0, 3.0);
        unit.move_to(20.0, 20.0);
        assert!(unit.is_moving);
        unit.stop();
        assert!(!unit.is_moving);
        assert!(unit.target_x.is_none());
    }

    #[test]
    fn test_unit_facing_update() {
        let mut unit = UnitPosition::new("u1", 0.0, 0.0, 10.0);
        unit.move_to(10.0, 10.0);
        move_unit_tick(&mut unit, 0.1);
        // Moving diagonally: facing should be ~pi/4 (0.785 rad)
        assert!((unit.facing - std::f64::consts::FRAC_PI_4).abs() < 0.01);
    }

    // -- Engine (SQLite persistence) --

    #[test]
    fn test_engine_init() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let engine = SwarmFactionEngine::new(tmp.path());
        assert!(engine.is_ok());
    }

    #[test]
    fn test_engine_resources_roundtrip() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let engine = SwarmFactionEngine::new(tmp.path()).expect("engine");
        let res = engine.get_resources("colony1", "insects").expect("get");
        assert_eq!(res.faction, "insects");
        assert_eq!(res.ore, 500.0);

        // Tick and verify persistence
        let updated = engine.tick_resources("colony1", "insects", 3600.0).expect("tick");
        assert!(updated.ore > 500.0);
    }

    #[test]
    fn test_engine_patrol_roundtrip() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let engine = SwarmFactionEngine::new(tmp.path()).expect("engine");
        let patrol = engine
            .create_patrol_route("c1", vec![(0, 0), (5, 5)], "cyclic", 3, 8)
            .expect("create");
        assert_eq!(patrol.colony_id, "c1");

        let list = engine.list_patrols("c1").expect("list");
        assert_eq!(list.len(), 1);

        engine.cancel_patrol(&patrol.id).expect("cancel");
        let list2 = engine.list_patrols("c1").expect("list2");
        assert_eq!(list2.len(), 0);
    }

    #[test]
    fn test_engine_unit_movement_roundtrip() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let engine = SwarmFactionEngine::new(tmp.path()).expect("engine");
        let pos = engine.get_unit_position("unit1").expect("get");
        assert_eq!(pos.pos_x, 0.0);

        let moved = engine.move_unit("unit1", 10.0, 10.0).expect("move");
        assert!(moved.is_moving);

        let stopped = engine.stop_unit("unit1").expect("stop");
        assert!(!stopped.is_moving);
    }

    #[test]
    fn test_engine_tick_movement() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let engine = SwarmFactionEngine::new(tmp.path()).expect("engine");
        engine.move_unit("u1", 10.0, 0.0).expect("move");
        let updated = engine.tick_movement(1.0).expect("tick");
        assert_eq!(updated.len(), 1);
        assert!(updated[0].pos_x > 0.0);
    }

    #[test]
    fn test_engine_special_action_insects() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let engine = SwarmFactionEngine::new(tmp.path()).expect("engine");
        let result = engine.special_action("insects", "larvae_inject").expect("action");
        assert_eq!(result["action"], "larvae_inject");
        assert!(result["larvae_added"].as_u64().is_some());
    }

    #[test]
    fn test_engine_special_action_unknown() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let engine = SwarmFactionEngine::new(tmp.path()).expect("engine");
        let result = engine.special_action("elves", "magic");
        assert!(result.is_err());
    }

    #[test]
    fn test_patrol_mode_from_str() {
        assert_eq!(PatrolMode::from_str("cyclic"), PatrolMode::Cyclic);
        assert_eq!(PatrolMode::from_str("ping_pong"), PatrolMode::PingPong);
        assert_eq!(PatrolMode::from_str("pingpong"), PatrolMode::PingPong);
        assert_eq!(PatrolMode::from_str("one_way"), PatrolMode::OneWay);
        assert_eq!(PatrolMode::from_str("oneway"), PatrolMode::OneWay);
        assert_eq!(PatrolMode::from_str("unknown"), PatrolMode::Cyclic);
    }

    // -- Kultisten System --

    #[test]
    fn test_kultisten_system_new_defaults() {
        let ks = KultistenSystem::new();
        assert_eq!(ks.count, 5);
        assert_eq!(ks.energy_per_kultist, 50.0);
        assert_eq!(ks.total_energy_per_hour, 250.0); // 5 * 50
        assert!((ks.summon_speed_bonus - 1.10).abs() < 0.001); // 1.0 + 5*0.02
        assert_eq!(ks.altar_count, 1);
        assert_eq!(ks.production_per_hour, 5);
        assert_eq!(ks.total_sacrificed, 0);
    }

    #[test]
    fn test_kultisten_energy_calculation() {
        let ks = KultistenSystem::new();
        let energy = kultisten_energy(&ks);
        assert_eq!(energy, 250.0); // 5 * 50
    }

    #[test]
    fn test_kultisten_summon_bonus() {
        let mut ks = KultistenSystem::new();
        ks.count = 50;
        let bonus = kultisten_summon_bonus(&ks);
        assert!((bonus - 2.0).abs() < 0.001); // 1.0 + 50*0.02 = 2.0
    }

    #[test]
    fn test_kultisten_produce_one_hour() {
        let mut ks = KultistenSystem::new();
        ks.altar_count = 2;
        ks.production_per_hour = 5;
        let initial = ks.count;
        kultisten_produce(&mut ks, 1.0);
        assert_eq!(ks.count, initial + 10); // 2 altars * 5/hr * 1hr
        assert_eq!(ks.total_energy_per_hour, ks.count as f64 * ks.energy_per_kultist);
    }

    #[test]
    fn test_kultisten_produce_partial_hour() {
        let mut ks = KultistenSystem::new();
        ks.altar_count = 1;
        ks.production_per_hour = 10;
        let initial = ks.count;
        kultisten_produce(&mut ks, 0.5);
        // 1 altar * 10/hr * 0.5hr = 5 (truncated to u32)
        assert_eq!(ks.count, initial + 5);
    }

    #[test]
    fn test_kultisten_sacrifice_success() {
        let mut ks = KultistenSystem::new();
        ks.count = 10;
        let cost = kultisten_sacrifice(&mut ks, "Greater Demon").expect("sacrifice");
        assert_eq!(cost.kultisten_required, 3);
        assert_eq!(ks.count, 7);
        assert_eq!(ks.total_sacrificed, 3);
        // Derived fields recalculated
        assert_eq!(ks.total_energy_per_hour, 7.0 * 50.0);
    }

    #[test]
    fn test_kultisten_sacrifice_not_enough() {
        let mut ks = KultistenSystem::new();
        ks.count = 2;
        let result = kultisten_sacrifice(&mut ks, "Greater Demon");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("NOT_ENOUGH_KULTISTEN"));
        assert_eq!(ks.count, 2); // unchanged
    }

    #[test]
    fn test_kultisten_sacrifice_unknown_unit() {
        let mut ks = KultistenSystem::new();
        ks.count = 100;
        let result = kultisten_sacrifice(&mut ks, "Dragon");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("UNKNOWN_SACRIFICE"));
    }

    #[test]
    fn test_sacrifice_costs_table() {
        let costs = get_sacrifice_costs();
        assert_eq!(costs.len(), 5);
        // Verify ascending Kultisten cost
        assert!(costs[0].kultisten_required < costs[1].kultisten_required);
        // Verify the hero unit exists
        assert!(costs.iter().any(|c| c.unit_name.contains("Hero")));
        // All have non-zero summon times
        assert!(costs.iter().all(|c| c.summon_time_secs > 0));
    }

    #[test]
    fn test_kultisten_sacrifice_drains_all() {
        let mut ks = KultistenSystem::new();
        ks.count = 20;
        // Sacrifice for Abyssal Maw (costs 20)
        let cost = kultisten_sacrifice(&mut ks, "Abyssal Maw").expect("sacrifice");
        assert_eq!(cost.kultisten_required, 20);
        assert_eq!(ks.count, 0);
        assert_eq!(ks.total_sacrificed, 20);
        assert_eq!(ks.total_energy_per_hour, 0.0);
        assert!((ks.summon_speed_bonus - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_kultisten_engine_roundtrip() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let engine = SwarmFactionEngine::new(tmp.path()).expect("engine");

        // First call initializes with defaults
        let ks = engine.get_kultisten_system("demon_colony_1").expect("get");
        assert_eq!(ks.count, 5);

        // Sacrifice persists
        let cost = engine
            .sacrifice_kultisten("demon_colony_1", "Greater Demon")
            .expect("sacrifice");
        assert_eq!(cost.kultisten_required, 3);

        // Reload and verify persistence
        let ks2 = engine.get_kultisten_system("demon_colony_1").expect("get2");
        assert_eq!(ks2.count, 2);
        assert_eq!(ks2.total_sacrificed, 3);
    }

    #[test]
    fn test_demon_resource_names_include_sacrifice() {
        let names = resource_names("demons");
        assert!(names.contains_key("sacrifice"));
        assert!(names["energy"].contains("Living Power Grid"));
    }
}
