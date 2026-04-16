// SPDX-License-Identifier: Elastic-2.0
//! EvoSys -- Novelty Multiplier + Governing Attributes for SwarmForge units.

use serde::{Deserialize, Serialize};

use super::swarm_types::{SwarmUnit, UnitType};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("forge_quest::evosys", "EvoSys");

/// Novelty multiplier for XP earning.
/// First-time actions get a 5x bonus, fresh actions 2x,
/// heavily repeated actions get diminishing returns.
pub(crate) fn novelty_multiplier(action_count: u32) -> f64 {
    if action_count == 0 {
        5.0 // First time = 5x
    } else if action_count < 10 {
        2.0 // Still fresh = 2x
    } else if action_count > 50 {
        0.1 // Grind penalty
    } else {
        1.0 // Normal
    }
}

/// Governing attributes calculated from a SwarmUnit's stats, mutations, and level.
/// These provide a high-level summary of a unit's capabilities across five dimensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoverningAttributes {
    pub strength: f32,
    pub speed: f32,
    pub intelligence: f32,
    pub resilience: f32,
    pub charisma: f32,
}

pub(crate) fn calculate_governing(unit: &SwarmUnit) -> GoverningAttributes {
    let tier = unit.unit_type.tier() as f32;
    let level = unit.level as f32;

    // Strength: derived from attack + tier scaling
    let strength = (unit.attack as f32 * 1.2) + (level * 0.5) + (tier * 3.0);

    // Speed: scouts and skyweaver are fastest; base off efficiency + tier
    let speed_base = match unit.unit_type {
        UnitType::ImpScout | UnitType::Skyweaver | UnitType::Gargoyle => 1.5,
        UnitType::Ravager | UnitType::Broodling => 1.2,
        UnitType::Viper | UnitType::RipperSwarm => 1.3,
        UnitType::NydusWorm => 1.8,
        _ => 1.0,
    };
    let speed = (unit.efficiency * 10.0 * speed_base) + (level * 0.3);

    // Intelligence: mages, overseers, and matriarch have higher base
    let int_base = match unit.unit_type {
        UnitType::Overseer | UnitType::Matriarch | UnitType::Dominatrix => 1.6,
        UnitType::ShadowWeaver | UnitType::Infestor => 1.4,
        UnitType::SwarmMother | UnitType::HiveGuard => 1.3,
        _ => 1.0,
    };
    let intelligence = (level * 0.8 * int_base) + (tier * 4.0);

    // Resilience: derived from hp + defense
    let resilience = (unit.hp as f32 * 0.3) + (unit.defense as f32 * 1.5) + (tier * 2.0);

    // Charisma: leadership units (matriarch, swarm mother) + level scaling
    let char_base = match unit.unit_type {
        UnitType::Matriarch | UnitType::Dominatrix => 2.0,
        UnitType::SwarmMother => 1.5,
        UnitType::Overseer | UnitType::Infestor => 1.3,
        _ => 1.0,
    };
    let charisma = (level * 0.5 * char_base) + (tier * 2.5);

    GoverningAttributes {
        strength,
        speed,
        intelligence,
        resilience,
        charisma,
    }
}
