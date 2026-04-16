// SPDX-License-Identifier: Elastic-2.0
//! SwarmForge Combat -- Defense types (DefenseType, DefenseStats, DefenseCost)
//! + their impls.  Defensive structures that protect colonies from attack.

use serde::{Deserialize, Serialize};

use super::engine::simulate_battle;
use super::types::BattleResult;

/// Health declaration for this sub-module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_combat::defense", "Game");

/// Planetary defense structure types.
///
/// Each defense has fixed stats (HP, shield, damage) and a resource cost.
/// Defenses fire during battle as stationary units that cannot be targeted
/// by fleet movement (they stay on the planet).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DefenseType {
    /// Cheap, high-volume basic defense.
    MissileLauncher,
    /// Anti-fighter laser turret.
    LightLaser,
    /// General purpose heavy laser.
    HeavyLaser,
    /// Anti-armor railgun with high damage.
    GaussCannon,
    /// Shield-disrupting ion weapon.
    IonCannon,
    /// Strongest conventional turret, most expensive.
    PlasmaTurret,
    /// Planetary shield dome (2000 shield points).
    SmallShieldDome,
    /// Large planetary shield dome (10000 shield points).
    LargeShieldDome,
    /// Intercepts incoming missiles (80% success rate).
    AntiBallistic,
    /// Long-range missile that attacks enemy defenses remotely.
    InterplanetaryMissile,
}

impl DefenseType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissileLauncher => "missile_launcher",
            Self::LightLaser => "light_laser",
            Self::HeavyLaser => "heavy_laser",
            Self::GaussCannon => "gauss_cannon",
            Self::IonCannon => "ion_cannon",
            Self::PlasmaTurret => "plasma_turret",
            Self::SmallShieldDome => "small_shield_dome",
            Self::LargeShieldDome => "large_shield_dome",
            Self::AntiBallistic => "anti_ballistic",
            Self::InterplanetaryMissile => "interplanetary_missile",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "missile_launcher" => Some(Self::MissileLauncher),
            "light_laser" => Some(Self::LightLaser),
            "heavy_laser" => Some(Self::HeavyLaser),
            "gauss_cannon" => Some(Self::GaussCannon),
            "ion_cannon" => Some(Self::IonCannon),
            "plasma_turret" => Some(Self::PlasmaTurret),
            "small_shield_dome" => Some(Self::SmallShieldDome),
            "large_shield_dome" => Some(Self::LargeShieldDome),
            "anti_ballistic" => Some(Self::AntiBallistic),
            "interplanetary_missile" => Some(Self::InterplanetaryMissile),
            _ => None,
        }
    }

    /// All defense types in build order.
    pub fn all() -> &'static [DefenseType] {
        &[
            Self::MissileLauncher,
            Self::LightLaser,
            Self::HeavyLaser,
            Self::GaussCannon,
            Self::IonCannon,
            Self::PlasmaTurret,
            Self::SmallShieldDome,
            Self::LargeShieldDome,
            Self::AntiBallistic,
            Self::InterplanetaryMissile,
        ]
    }
}

/// Combat statistics for a defense structure (per unit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefenseStats {
    pub defense_type: String,
    pub name: String,
    pub hp: u32,
    pub shield: u32,
    pub damage: u32,
    pub cost_biomass: u32,
    pub cost_minerals: u32,
    pub cost_spore_gas: u32,
    pub description: String,
}

/// Resource cost for building defense structures.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DefenseCost {
    pub biomass: u64,
    pub minerals: u64,
    pub spore_gas: u64,
    pub total_units: u32,
}

/// Return the combat stats for a defense type (OGame-accurate values).
pub fn defense_stats(def_type: &DefenseType) -> DefenseStats {
    match def_type {
        DefenseType::MissileLauncher => DefenseStats {
            defense_type: def_type.as_str().to_string(),
            name: "Missile Launcher".to_string(),
            hp: 200, shield: 20, damage: 80,
            cost_biomass: 2000, cost_minerals: 0, cost_spore_gas: 0,
            description: "Cheap rocket launcher. Best built in large numbers for volume fire.".to_string(),
        },
        DefenseType::LightLaser => DefenseStats {
            defense_type: def_type.as_str().to_string(),
            name: "Light Laser".to_string(),
            hp: 200, shield: 25, damage: 100,
            cost_biomass: 1500, cost_minerals: 500, cost_spore_gas: 0,
            description: "Fast-tracking laser effective against small fighters.".to_string(),
        },
        DefenseType::HeavyLaser => DefenseStats {
            defense_type: def_type.as_str().to_string(),
            name: "Heavy Laser".to_string(),
            hp: 800, shield: 100, damage: 250,
            cost_biomass: 6000, cost_minerals: 2000, cost_spore_gas: 0,
            description: "General-purpose laser turret with solid damage and durability.".to_string(),
        },
        DefenseType::GaussCannon => DefenseStats {
            defense_type: def_type.as_str().to_string(),
            name: "Gauss Cannon".to_string(),
            hp: 3500, shield: 200, damage: 1100,
            cost_biomass: 20000, cost_minerals: 15000, cost_spore_gas: 2000,
            description: "Electromagnetic railgun that punches through heavy armor.".to_string(),
        },
        DefenseType::IonCannon => DefenseStats {
            defense_type: def_type.as_str().to_string(),
            name: "Ion Cannon".to_string(),
            hp: 800, shield: 500, damage: 150,
            cost_biomass: 5000, cost_minerals: 3000, cost_spore_gas: 0,
            description: "Disrupts enemy shields with concentrated ion bursts.".to_string(),
        },
        DefenseType::PlasmaTurret => DefenseStats {
            defense_type: def_type.as_str().to_string(),
            name: "Plasma Turret".to_string(),
            hp: 10000, shield: 300, damage: 3000,
            cost_biomass: 50000, cost_minerals: 50000, cost_spore_gas: 30000,
            description: "The strongest conventional defense. Devastating damage at extreme cost.".to_string(),
        },
        DefenseType::SmallShieldDome => DefenseStats {
            defense_type: def_type.as_str().to_string(),
            name: "Small Shield Dome".to_string(),
            hp: 2000, shield: 2000, damage: 1,
            cost_biomass: 10000, cost_minerals: 10000, cost_spore_gas: 0,
            description: "Projects a 2000-point shield over the colony. Only one allowed.".to_string(),
        },
        DefenseType::LargeShieldDome => DefenseStats {
            defense_type: def_type.as_str().to_string(),
            name: "Large Shield Dome".to_string(),
            hp: 10000, shield: 10000, damage: 1,
            cost_biomass: 50000, cost_minerals: 50000, cost_spore_gas: 0,
            description: "Projects a 10000-point shield over the colony. Only one allowed.".to_string(),
        },
        DefenseType::AntiBallistic => DefenseStats {
            defense_type: def_type.as_str().to_string(),
            name: "Anti-Ballistic Missile".to_string(),
            hp: 100, shield: 1, damage: 1,
            cost_biomass: 8000, cost_minerals: 0, cost_spore_gas: 2000,
            description: "Intercepts incoming interplanetary missiles with 80% success rate.".to_string(),
        },
        DefenseType::InterplanetaryMissile => DefenseStats {
            defense_type: def_type.as_str().to_string(),
            name: "Interplanetary Missile".to_string(),
            hp: 15000, shield: 1, damage: 12000,
            cost_biomass: 12500, cost_minerals: 2500, cost_spore_gas: 10000,
            description: "Long-range missile that destroys enemy defenses from orbit.".to_string(),
        },
    }
}

/// Calculate the total resource cost for building `count` units of a defense type.
pub fn defense_cost(def_type: &DefenseType, count: u32) -> DefenseCost {
    let stats = defense_stats(def_type);
    DefenseCost {
        biomass: stats.cost_biomass as u64 * count as u64,
        minerals: stats.cost_minerals as u64 * count as u64,
        spore_gas: stats.cost_spore_gas as u64 * count as u64,
        total_units: count,
    }
}

/// Simulate a battle between planetary defenses and an attacking fleet.
///
/// Defenses are converted to stationary combat units and fight alongside any
/// existing defender fleet in a standard OGame-style 6-round battle.
///
/// After battle, 70% of destroyed defenses are rebuilt (OGame mechanic:
/// defenses have a 70% chance to be restored after each battle).
pub fn defense_vs_fleet(
    defenses: &[(DefenseType, u32)],
    attacker_fleet: &[(String, u32)],
) -> BattleResult {
    let defender_fleet: Vec<(String, u32)> = defenses
        .iter()
        .map(|(dt, count)| (dt.as_str().to_string(), *count))
        .collect();

    simulate_battle(attacker_fleet, &defender_fleet)
}

