// SPDX-License-Identifier: Elastic-2.0
//! SwarmForge Combat — Type definitions (damage/armour matrix, resources,
//! mission types, fleet status, fleet mission, battle result) + their impls.

use serde::{Deserialize, Serialize};

/// Health declaration for this sub-module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_combat::types", "Game");

// ---------------------------------------------------------------------------
// Damage & Armor Types
// ---------------------------------------------------------------------------

/// Seven elemental damage types.  Each interacts differently with armour.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DamageType {
    Fire,
    Plasma,
    Electricity,
    Corrosion,
    Slash,
    Stab,
    Blunt,
}

impl DamageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fire => "fire",
            Self::Plasma => "plasma",
            Self::Electricity => "electricity",
            Self::Corrosion => "corrosion",
            Self::Slash => "slash",
            Self::Stab => "stab",
            Self::Blunt => "blunt",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "fire" => Self::Fire,
            "plasma" => Self::Plasma,
            "electricity" => Self::Electricity,
            "corrosion" => Self::Corrosion,
            "slash" => Self::Slash,
            "stab" => Self::Stab,
            "blunt" => Self::Blunt,
            _ => Self::Blunt,
        }
    }

    pub fn all() -> &'static [DamageType] {
        &[
            Self::Fire,
            Self::Plasma,
            Self::Electricity,
            Self::Corrosion,
            Self::Slash,
            Self::Stab,
            Self::Blunt,
        ]
    }
}

/// Seven armour archetypes with different elemental resistances.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ArmorType {
    Chitin,
    Scale,
    Ethereal,
    Hellforged,
    Bone,
    Crystal,
    Void,
}

impl ArmorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chitin => "chitin",
            Self::Scale => "scale",
            Self::Ethereal => "ethereal",
            Self::Hellforged => "hellforged",
            Self::Bone => "bone",
            Self::Crystal => "crystal",
            Self::Void => "void",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "chitin" => Self::Chitin,
            "scale" => Self::Scale,
            "ethereal" => Self::Ethereal,
            "hellforged" => Self::Hellforged,
            "bone" => Self::Bone,
            "crystal" => Self::Crystal,
            "void" => Self::Void,
            _ => Self::Chitin,
        }
    }
}

// ---------------------------------------------------------------------------
// Damage Calculation (LoL-style)
// ---------------------------------------------------------------------------

/// 7x7 type-effectiveness matrix.
///
/// Rows = DamageType (Fire..Blunt), Columns = ArmorType (Chitin..Void).
/// Values: 1.50 = super-effective, 0.25 = resisted, 1.00 = neutral.
///
/// Design rationale:
/// - Fire melts Chitin (organic) and Bone but bounces off Hellforged metal
/// - Plasma cuts through Scale and Hellforged but Void absorbs it
/// - Electricity shatters Crystal and disrupts Ethereal but Scale grounds it
/// - Corrosion eats Scale and Bone but Crystal is inert
/// - Slash tears Chitin and Ethereal but bounces off Crystal and Hellforged
/// - Stab pierces Scale and Crystal gaps but Chitin flexes, Void swallows
/// - Blunt crushes Crystal and Bone but Ethereal phases through
const TYPE_MATRIX: [[f64; 7]; 7] = [
    //           Chitin  Scale   Ether   Hell    Bone    Cryst   Void
    /* Fire */  [1.50,   1.00,   1.00,   0.25,   1.50,   1.00,   1.00],
    /* Plasma */[1.00,   1.50,   1.00,   1.50,   1.00,   1.00,   0.25],
    /* Elec */  [1.00,   0.25,   1.50,   1.00,   1.00,   1.50,   1.00],
    /* Corr */  [1.00,   1.50,   1.00,   1.00,   1.50,   0.25,   1.00],
    /* Slash */ [1.50,   1.00,   1.50,   0.25,   1.00,   0.25,   1.00],
    /* Stab */  [0.25,   1.50,   1.00,   1.00,   1.00,   1.50,   0.25],
    /* Blunt */ [1.00,   1.00,   0.25,   1.00,   1.50,   1.50,   1.00],
];

/// Return the type-effectiveness multiplier for a damage/armour pair.
pub fn type_multiplier(damage: &DamageType, armor: &ArmorType) -> f64 {
    let row = *damage as usize;
    let col = *armor as usize;
    TYPE_MATRIX[row][col]
}

/// Calculate final damage using LoL-style armour reduction.
///
/// Formula: `raw_atk * multiplier * 100 / (100 + effective_armor)`
///
/// - `attacker_atk`: raw attack power
/// - `defender_def`: raw defence / armour value
/// - `damage_type`:  elemental type of the attack
/// - `armor_type`:   elemental type of the defender's armour
///
/// Returns damage dealt (always >= 1.0 so chip damage is possible).
pub fn calculate_damage(
    attacker_atk: f64,
    defender_def: f64,
    damage_type: DamageType,
    armor_type: ArmorType,
) -> f64 {
    let multiplier = type_multiplier(&damage_type, &armor_type);
    let effective_armor = defender_def.max(0.0);
    let raw = attacker_atk * multiplier * 100.0 / (100.0 + effective_armor);
    raw.max(1.0)
}

// ---------------------------------------------------------------------------
// Fleet Types
// ---------------------------------------------------------------------------

/// Cargo resources carried by or produced from a fleet.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Resources {
    pub biomass: f64,
    pub minerals: f64,
    pub crystal: f64,
    pub spore_gas: f64,
}

impl Resources {
    pub fn total(&self) -> f64 {
        self.biomass + self.minerals + self.crystal + self.spore_gas
    }

    pub fn add(&mut self, other: &Resources) {
        self.biomass += other.biomass;
        self.minerals += other.minerals;
        self.crystal += other.crystal;
        self.spore_gas += other.spore_gas;
    }

    pub fn scale(&mut self, factor: f64) {
        self.biomass *= factor;
        self.minerals *= factor;
        self.crystal *= factor;
        self.spore_gas *= factor;
    }
}

/// Mission types that determine fleet behaviour on arrival.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MissionType {
    Attack,
    Transport,
    Colonize,
    Espionage,
    Deploy,
    Harvest,
    Expedition,
}

impl MissionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Attack => "attack",
            Self::Transport => "transport",
            Self::Colonize => "colonize",
            Self::Espionage => "espionage",
            Self::Deploy => "deploy",
            Self::Harvest => "harvest",
            Self::Expedition => "expedition",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "attack" => Self::Attack,
            "transport" => Self::Transport,
            "colonize" => Self::Colonize,
            "espionage" => Self::Espionage,
            "deploy" => Self::Deploy,
            "harvest" => Self::Harvest,
            "expedition" => Self::Expedition,
            _ => Self::Attack,
        }
    }

    /// Whether the fleet returns home after completing the mission.
    pub fn returns(&self) -> bool {
        match self {
            Self::Attack | Self::Espionage | Self::Harvest | Self::Expedition => true,
            Self::Transport | Self::Colonize | Self::Deploy => false,
        }
    }
}

/// Lifecycle states of a fleet mission.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FleetStatus {
    Outbound,
    Arrived,
    Returning,
    Completed,
}

impl FleetStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Outbound => "outbound",
            Self::Arrived => "arrived",
            Self::Returning => "returning",
            Self::Completed => "completed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "outbound" => Self::Outbound,
            "arrived" => Self::Arrived,
            "returning" => Self::Returning,
            "completed" => Self::Completed,
            _ => Self::Outbound,
        }
    }
}

/// A fleet mission dispatched between two coordinate triples.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetMission {
    pub id: String,
    /// Ships in the fleet: `(ship_type_str, count)`.
    pub fleet: Vec<(String, u32)>,
    /// Origin coordinates: `(galaxy, system, planet)`.
    pub origin: (u32, u32, u32),
    /// Target coordinates: `(galaxy, system, planet)`.
    pub target: (u32, u32, u32),
    pub mission_type: MissionType,
    pub departure_time: String,
    pub arrival_time: String,
    pub return_time: Option<String>,
    pub status: FleetStatus,
    pub cargo: Resources,
}

// ---------------------------------------------------------------------------
// Ship Speed & Cargo (extends forge_quest::ShipType stats)
// ---------------------------------------------------------------------------

/// Per-ship-type speed and cargo capacity.
/// Speed is in "distance units per second" -- higher = faster.
/// Cargo is the total resource volume each individual ship can carry.
pub(crate) struct ShipProfile {
    pub(crate) speed: f64,
    pub(crate) cargo: f64,
    /// Base build cost for debris/loot calculation.
    pub(crate) cost: Resources,
    /// Primary damage type of this ship class.
    pub(crate) damage_type: DamageType,
    /// Armour archetype of this ship class.
    pub(crate) armor_type: ArmorType,
}

pub(crate) fn ship_profile(ship_type: &str) -> ShipProfile {
    match ship_type {
        "bio_fighter" => ShipProfile {
            speed: 12500.0, cargo: 50.0,
            cost: Resources { biomass: 3000.0, minerals: 1000.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Slash, armor_type: ArmorType::Chitin,
        },
        "spore_interceptor" => ShipProfile {
            speed: 10000.0, cargo: 50.0,
            cost: Resources { biomass: 6000.0, minerals: 4000.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Corrosion, armor_type: ArmorType::Scale,
        },
        "kraken_frigate" => ShipProfile {
            speed: 7500.0, cargo: 800.0,
            cost: Resources { biomass: 20000.0, minerals: 7000.0, crystal: 2000.0, spore_gas: 0.0 },
            damage_type: DamageType::Blunt, armor_type: ArmorType::Bone,
        },
        "leviathan" => ShipProfile {
            speed: 5000.0, cargo: 1500.0,
            cost: Resources { biomass: 45000.0, minerals: 15000.0, crystal: 5000.0, spore_gas: 0.0 },
            damage_type: DamageType::Plasma, armor_type: ArmorType::Hellforged,
        },
        "bio_transporter" => ShipProfile {
            speed: 7500.0, cargo: 25000.0,
            cost: Resources { biomass: 2000.0, minerals: 2000.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Blunt, armor_type: ArmorType::Chitin,
        },
        "colony_pod" => ShipProfile {
            speed: 2500.0, cargo: 7500.0,
            cost: Resources { biomass: 10000.0, minerals: 10000.0, crystal: 10000.0, spore_gas: 0.0 },
            damage_type: DamageType::Blunt, armor_type: ArmorType::Scale,
        },
        "devourer" => ShipProfile {
            speed: 5000.0, cargo: 2000.0,
            cost: Resources { biomass: 60000.0, minerals: 50000.0, crystal: 15000.0, spore_gas: 0.0 },
            damage_type: DamageType::Fire, armor_type: ArmorType::Hellforged,
        },
        "world_eater" => ShipProfile {
            speed: 100.0, cargo: 1000000.0,
            cost: Resources { biomass: 5_000_000.0, minerals: 4_000_000.0, crystal: 1_000_000.0, spore_gas: 0.0 },
            damage_type: DamageType::Plasma, armor_type: ArmorType::Void,
        },
        "leech_hauler" => ShipProfile {
            speed: 6000.0, cargo: 50000.0,
            cost: Resources { biomass: 8000.0, minerals: 6000.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Corrosion, armor_type: ArmorType::Scale,
        },
        "spore_carrier" => ShipProfile {
            speed: 4000.0, cargo: 5000.0,
            cost: Resources { biomass: 30000.0, minerals: 15000.0, crystal: 5000.0, spore_gas: 0.0 },
            damage_type: DamageType::Corrosion, armor_type: ArmorType::Ethereal,
        },
        "hive_ship" => ShipProfile {
            speed: 3000.0, cargo: 3000.0,
            cost: Resources { biomass: 80000.0, minerals: 40000.0, crystal: 20000.0, spore_gas: 5000.0 },
            damage_type: DamageType::Electricity, armor_type: ArmorType::Crystal,
        },
        "void_kraken" => ShipProfile {
            speed: 2000.0, cargo: 10000.0,
            cost: Resources { biomass: 200000.0, minerals: 100000.0, crystal: 50000.0, spore_gas: 10000.0 },
            damage_type: DamageType::Plasma, armor_type: ArmorType::Void,
        },
        "mycetic_spore" => ShipProfile {
            speed: 15000.0, cargo: 0.0,
            cost: Resources { biomass: 1000.0, minerals: 0.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Stab, armor_type: ArmorType::Chitin,
        },
        "neural_parasite" => ShipProfile {
            speed: 8000.0, cargo: 500.0,
            cost: Resources { biomass: 15000.0, minerals: 10000.0, crystal: 5000.0, spore_gas: 2000.0 },
            damage_type: DamageType::Electricity, armor_type: ArmorType::Ethereal,
        },
        "narwhal" => ShipProfile {
            speed: 3500.0, cargo: 4000.0,
            cost: Resources { biomass: 40000.0, minerals: 25000.0, crystal: 10000.0, spore_gas: 0.0 },
            damage_type: DamageType::Stab, armor_type: ArmorType::Bone,
        },
        "drone_ship" => ShipProfile {
            speed: 4500.0, cargo: 2000.0,
            cost: Resources { biomass: 50000.0, minerals: 30000.0, crystal: 15000.0, spore_gas: 3000.0 },
            damage_type: DamageType::Fire, armor_type: ArmorType::Crystal,
        },
        "razorfiend" => ShipProfile {
            speed: 14000.0, cargo: 100.0,
            cost: Resources { biomass: 5000.0, minerals: 2000.0, crystal: 500.0, spore_gas: 0.0 },
            damage_type: DamageType::Slash, armor_type: ArmorType::Chitin,
        },
        "hierophant" => ShipProfile {
            speed: 500.0, cargo: 50000.0,
            cost: Resources { biomass: 2_000_000.0, minerals: 1_500_000.0, crystal: 500_000.0, spore_gas: 100_000.0 },
            damage_type: DamageType::Fire, armor_type: ArmorType::Void,
        },
        // --- Defense structures (stationary, speed 0, no cargo) ---
        "missile_launcher" => ShipProfile {
            speed: 0.0, cargo: 0.0,
            cost: Resources { biomass: 2000.0, minerals: 0.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Fire, armor_type: ArmorType::Chitin,
        },
        "light_laser" => ShipProfile {
            speed: 0.0, cargo: 0.0,
            cost: Resources { biomass: 1500.0, minerals: 500.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Electricity, armor_type: ArmorType::Scale,
        },
        "heavy_laser" => ShipProfile {
            speed: 0.0, cargo: 0.0,
            cost: Resources { biomass: 6000.0, minerals: 2000.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Electricity, armor_type: ArmorType::Scale,
        },
        "gauss_cannon" => ShipProfile {
            speed: 0.0, cargo: 0.0,
            cost: Resources { biomass: 20000.0, minerals: 15000.0, crystal: 0.0, spore_gas: 2000.0 },
            damage_type: DamageType::Stab, armor_type: ArmorType::Hellforged,
        },
        "ion_cannon" => ShipProfile {
            speed: 0.0, cargo: 0.0,
            cost: Resources { biomass: 5000.0, minerals: 3000.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Plasma, armor_type: ArmorType::Crystal,
        },
        "plasma_turret" => ShipProfile {
            speed: 0.0, cargo: 0.0,
            cost: Resources { biomass: 50000.0, minerals: 50000.0, crystal: 0.0, spore_gas: 30000.0 },
            damage_type: DamageType::Plasma, armor_type: ArmorType::Hellforged,
        },
        "small_shield_dome" => ShipProfile {
            speed: 0.0, cargo: 0.0,
            cost: Resources { biomass: 10000.0, minerals: 10000.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Blunt, armor_type: ArmorType::Ethereal,
        },
        "large_shield_dome" => ShipProfile {
            speed: 0.0, cargo: 0.0,
            cost: Resources { biomass: 50000.0, minerals: 50000.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Blunt, armor_type: ArmorType::Ethereal,
        },
        "anti_ballistic" => ShipProfile {
            speed: 0.0, cargo: 0.0,
            cost: Resources { biomass: 8000.0, minerals: 0.0, crystal: 0.0, spore_gas: 2000.0 },
            damage_type: DamageType::Fire, armor_type: ArmorType::Chitin,
        },
        "interplanetary_missile" => ShipProfile {
            speed: 0.0, cargo: 0.0,
            cost: Resources { biomass: 12500.0, minerals: 2500.0, crystal: 0.0, spore_gas: 10000.0 },
            damage_type: DamageType::Fire, armor_type: ArmorType::Bone,
        },
        // Fallback for unknown ships -- treat as weak scout
        _ => ShipProfile {
            speed: 10000.0, cargo: 50.0,
            cost: Resources { biomass: 1000.0, minerals: 500.0, crystal: 0.0, spore_gas: 0.0 },
            damage_type: DamageType::Blunt, armor_type: ArmorType::Chitin,
        },
    }
}

/// Combat stats pulled from forge_quest::ShipType::combat_stats().
/// Reproduced here to keep the combat module self-contained and avoid
/// circular dependencies.  (attack, shields, hp)
pub(crate) fn ship_combat_stats(ship_type: &str) -> (u32, u32, u32) {
    match ship_type {
        "bio_fighter"       => (50, 10, 400),
        "spore_interceptor" => (150, 25, 1000),
        "kraken_frigate"    => (400, 50, 2700),
        "leviathan"         => (1000, 200, 6000),
        "bio_transporter"   => (5, 10, 1200),
        "colony_pod"        => (50, 100, 3000),
        "devourer"          => (2000, 500, 11000),
        "world_eater"       => (200000, 50000, 900000),
        "leech_hauler"      => (10, 20, 2500),
        "spore_carrier"     => (100, 80, 4000),
        "hive_ship"         => (800, 400, 15000),
        "void_kraken"       => (5000, 2000, 50000),
        "mycetic_spore"     => (20, 5, 200),
        "neural_parasite"   => (300, 150, 3500),
        "narwhal"           => (200, 300, 8000),
        "drone_ship"        => (600, 250, 10000),
        "razorfiend"        => (120, 30, 800),
        "hierophant"        => (80000, 30000, 500000),
        // --- Defense structures ---
        "missile_launcher"  => (80, 20, 200),
        "light_laser"       => (100, 25, 200),
        "heavy_laser"       => (250, 100, 800),
        "gauss_cannon"      => (1100, 200, 3500),
        "ion_cannon"        => (150, 500, 800),
        "plasma_turret"     => (3000, 300, 10000),
        "small_shield_dome" => (1, 2000, 2000),
        "large_shield_dome" => (1, 10000, 10000),
        "anti_ballistic"    => (1, 1, 100),
        "interplanetary_missile" => (12000, 1, 15000),
        _                   => (10, 5, 100),
    }
}

// ---------------------------------------------------------------------------
// Travel Time Calculation
// ---------------------------------------------------------------------------

/// Calculate one-way travel time in seconds.
///
/// Distance model (OGame-inspired):
/// - Same planet: 0 (nonsensical but handled)
/// - Same system, different planet: `abs(p1 - p2) * 2500 + 5000`
/// - Same galaxy, different system: `abs(s1 - s2) * 19500 + 25000`
/// - Different galaxy: `abs(g1 - g2) * 200000 + 100000`
///
/// travel_time = distance / (speed_factor * slowest_ship_speed) rounded up.
pub fn calculate_travel_time(
    origin: (u32, u32, u32),
    target: (u32, u32, u32),
    fleet: &[(String, u32)],
    speed_factor: f64,
) -> u64 {
    if origin == target {
        return 0;
    }

    let (g1, s1, p1) = origin;
    let (g2, s2, p2) = target;

    let distance: f64 = if g1 != g2 {
        (g1 as i64 - g2 as i64).unsigned_abs() as f64 * 200_000.0 + 100_000.0
    } else if s1 != s2 {
        (s1 as i64 - s2 as i64).unsigned_abs() as f64 * 19_500.0 + 25_000.0
    } else {
        (p1 as i64 - p2 as i64).unsigned_abs() as f64 * 2_500.0 + 5_000.0
    };

    // Slowest ship determines fleet speed
    let slowest_speed = fleet
        .iter()
        .filter(|(_, count)| *count > 0)
        .map(|(st, _)| ship_profile(st).speed)
        .fold(f64::MAX, f64::min);

    // Guard against degenerate values
    let effective_speed = (slowest_speed * speed_factor.max(0.01)).max(1.0);

    (distance / effective_speed).ceil() as u64
}

// ---------------------------------------------------------------------------
// Battle Simulation (OGame-style, up to 6 rounds)
// ---------------------------------------------------------------------------

/// One ship instance alive during battle.
#[derive(Debug, Clone)]
pub(crate) struct CombatUnit {
    pub(crate) ship_type: String,
    pub(crate) attack: f64,
    pub(crate) shields: f64,
    pub(crate) shields_max: f64,
    pub(crate) hp: f64,
    pub(crate) damage_type: DamageType,
    pub(crate) armor_type: ArmorType,
}

/// The outcome of a full battle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleResult {
    pub id: String,
    pub rounds: u32,
    pub attacker_losses: Vec<(String, u32)>,
    pub defender_losses: Vec<(String, u32)>,
    pub attacker_won: bool,
    pub loot: Resources,
    pub debris: Resources,
    pub timestamp: String,
}

/// Expand a fleet spec into individual `CombatUnit`s.
pub(crate) fn expand_fleet(fleet: &[(String, u32)]) -> Vec<CombatUnit> {
    let mut units = Vec::new();
    for (ship_type, count) in fleet {
        let (atk, shd, hp) = ship_combat_stats(ship_type);
        let profile = ship_profile(ship_type);
        for _ in 0..*count {
            units.push(CombatUnit {
                ship_type: ship_type.clone(),
                attack: atk as f64,
                shields: shd as f64,
                shields_max: shd as f64,
                hp: hp as f64,
                damage_type: profile.damage_type,
                armor_type: profile.armor_type,
            });
        }
    }
    units
}

/// Count losses by comparing original fleet to surviving units.
pub(crate) fn count_losses(original: &[(String, u32)], survivors: &[CombatUnit]) -> Vec<(String, u32)> {
    original
        .iter()
        .filter_map(|(st, orig_count)| {
            let alive = survivors.iter().filter(|u| u.ship_type == *st).count() as u32;
            let lost = orig_count.saturating_sub(alive);
            if lost > 0 { Some((st.clone(), lost)) } else { None }
        })
        .collect()
}

/// Calculate total build cost for destroyed ships (for debris & loot).
pub(crate) fn destroyed_cost(losses: &[(String, u32)]) -> Resources {
    let mut total = Resources::default();
    for (st, count) in losses {
        let profile = ship_profile(st);
        total.biomass += profile.cost.biomass * *count as f64;
        total.minerals += profile.cost.minerals * *count as f64;
        total.crystal += profile.cost.crystal * *count as f64;
        total.spore_gas += profile.cost.spore_gas * *count as f64;
    }
    total
}

