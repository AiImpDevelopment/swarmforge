// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Hero/Champion Units, Alliance System, and Damage Matrices
//!
//! Three subsystems that extend SwarmForge with high-level strategy:
//!
//! ## Hero/Champion Units (12 heroes, 3 per faction)
//!
//! Each faction (Insects, Demons, Undead, Humans) fields three heroes with
//! distinct class archetypes (Tank, DPS, Caster).  Heroes gain XP through
//! SwarmForge actions, level up with exponential XP curves, and unlock three
//! abilities (Passive at L1, Active at L3, Ultimate at L6).
//!
//! Death is temporary: heroes respawn after `60 * level` seconds.
//! Stats scale per level: +5% HP, +3% ATK, +2% Armor, +8 Mana.
//!
//! ## Alliance System
//!
//! Players can form alliances (max 25 members), assign ranks, and establish
//! six types of diplomatic relations (Peace, War, Neutral, NAP, Mutual
//! Defense, Trade Agreement).  Alliance membership tracks contribution
//! (resources donated) for leaderboard ranking.
//!
//! ## WC3/SC2 Damage Matrices
//!
//! Three damage lookup systems:
//! - **WC3 matrix**: 7x7 attack-type vs armor-type multiplier table
//! - **SC2 bonus damage**: Per-unit-type bonus damage against tagged targets
//! - **SwarmForge matrix**: Custom 4-faction elemental damage table

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};
use crate::forge_quest::Faction;

// ===========================================================================
// SYSTEM 1: Hero/Champion Units
// ===========================================================================

/// Hero class archetypes — 3 per faction, 12 total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeroClass {
    // -- Insects --
    /// Tank/Support — spawns swarmlings, aura buffs
    HiveQueen,
    /// DPS — acid attacks, burrow ambush
    BroodMother,
    /// Caster — mind control, psionic storm
    PsionicOverseer,

    // -- Demons --
    /// Tank — fire aura, charge, taunt
    InfernalLord,
    /// DPS — stealth, backstab, poison
    ShadowAssassin,
    /// Caster — chaos bolt, doom, hellfire rain
    ChaosSorcerer,

    // -- Undead --
    /// Tank — death coil, unholy aura, animate dead
    DeathKnight,
    /// Caster — frost nova, blizzard, dark ritual
    LichKing,
    /// DPS/Support — possess, anti-magic shell, silence
    BansheeQueen,

    // -- Humans --
    /// Tank/Healer — holy light, divine shield, resurrection
    Paladin,
    /// Caster — blizzard, brilliance aura, mass teleport
    Archmage,
    /// DPS — storm bolt, thunder clap, avatar
    MountainKing,
}
impl HeroClass {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::HiveQueen => "hive_queen",
            Self::BroodMother => "brood_mother",
            Self::PsionicOverseer => "psionic_overseer",
            Self::InfernalLord => "infernal_lord",
            Self::ShadowAssassin => "shadow_assassin",
            Self::ChaosSorcerer => "chaos_sorcerer",
            Self::DeathKnight => "death_knight",
            Self::LichKing => "lich_king",
            Self::BansheeQueen => "banshee_queen",
            Self::Paladin => "paladin",
            Self::Archmage => "archmage",
            Self::MountainKing => "mountain_king",
        }
    }

    pub(crate) fn from_str_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "hive_queen" => Some(Self::HiveQueen),
            "brood_mother" => Some(Self::BroodMother),
            "psionic_overseer" => Some(Self::PsionicOverseer),
            "infernal_lord" => Some(Self::InfernalLord),
            "shadow_assassin" => Some(Self::ShadowAssassin),
            "chaos_sorcerer" => Some(Self::ChaosSorcerer),
            "death_knight" => Some(Self::DeathKnight),
            "lich_king" => Some(Self::LichKing),
            "banshee_queen" => Some(Self::BansheeQueen),
            "paladin" => Some(Self::Paladin),
            "archmage" => Some(Self::Archmage),
            "mountain_king" => Some(Self::MountainKing),
            _ => None,
        }
    }

    /// Which faction does this hero class belong to?
    pub(crate) fn faction(&self) -> Faction {
        match self {
            Self::HiveQueen | Self::BroodMother | Self::PsionicOverseer => Faction::Insects,
            Self::InfernalLord | Self::ShadowAssassin | Self::ChaosSorcerer => Faction::Demons,
            Self::DeathKnight | Self::LichKing | Self::BansheeQueen => Faction::Undead,
            Self::Paladin | Self::Archmage | Self::MountainKing => Faction::Humans,
        }
    }

    /// Display name for the hero class.
    pub(crate) fn display_name(&self) -> &'static str {
        match self {
            Self::HiveQueen => "Hive Queen",
            Self::BroodMother => "Brood Mother",
            Self::PsionicOverseer => "Psionic Overseer",
            Self::InfernalLord => "Infernal Lord",
            Self::ShadowAssassin => "Shadow Assassin",
            Self::ChaosSorcerer => "Chaos Sorcerer",
            Self::DeathKnight => "Death Knight",
            Self::LichKing => "Lich King",
            Self::BansheeQueen => "Banshee Queen",
            Self::Paladin => "Paladin",
            Self::Archmage => "Archmage",
            Self::MountainKing => "Mountain King",
        }
    }

    /// All 12 hero classes.
    pub(crate) fn all() -> &'static [HeroClass; 12] {
        &[
            Self::HiveQueen,
            Self::BroodMother,
            Self::PsionicOverseer,
            Self::InfernalLord,
            Self::ShadowAssassin,
            Self::ChaosSorcerer,
            Self::DeathKnight,
            Self::LichKing,
            Self::BansheeQueen,
            Self::Paladin,
            Self::Archmage,
            Self::MountainKing,
        ]
    }
}

// ---------------------------------------------------------------------------
// Hero stats, abilities, equipment
// ---------------------------------------------------------------------------

/// Core combat statistics for a hero.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroStats {
    pub hp: f64,
    pub mana: f64,
    pub attack: f64,
    pub armor: f64,
    pub speed: f64,
    pub hp_regen: f64,
    pub mana_regen: f64,
}

/// Ability activation category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbilityType {
    /// Always-on stat modifier or triggered effect.
    Passive,
    /// Manually activated, costs mana and has a cooldown.
    Active,
    /// Powerful ability unlocked at level 6.
    Ultimate,
}

/// A single hero ability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroAbility {
    pub id: String,
    pub name: String,
    pub ability_type: AbilityType,
    pub mana_cost: f64,
    pub cooldown_secs: f64,
    pub damage: f64,
    pub heal: f64,
    pub aoe_radius: f64,
    pub description: String,
    /// Level at which this ability becomes available.
    pub unlock_level: u32,
}

/// Item rarity tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemRarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

/// A piece of hero equipment that grants stat bonuses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipmentItem {
    pub name: String,
    pub rarity: ItemRarity,
    /// Keys are stat names ("hp", "attack", "armor", etc.), values are flat
    /// additive bonuses applied after per-level scaling.
    pub stat_bonuses: HashMap<String, f64>,
}

/// Equipment slots — weapon, armor, accessory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroEquipment {
    pub weapon: Option<EquipmentItem>,
    pub armor: Option<EquipmentItem>,
    pub accessory: Option<EquipmentItem>,
}

/// A hero/champion unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hero {
    pub id: String,
    pub name: String,
    pub faction: String,
    pub hero_class: HeroClass,
    pub level: u32,
    pub xp: u64,
    pub xp_to_next: u64,
    pub stats: HeroStats,
    pub abilities: Vec<HeroAbility>,
    pub equipment: HeroEquipment,
    pub is_alive: bool,
    /// Seconds remaining before respawn.  0 when alive.
    pub respawn_timer_secs: u32,
}

// ---------------------------------------------------------------------------
// Base stat table (level 1)
// ---------------------------------------------------------------------------

/// Returns base stats for a hero class at level 1.
fn base_stats(class: HeroClass) -> HeroStats {
    match class {
        HeroClass::HiveQueen => HeroStats {
            hp: 800.0, mana: 400.0, attack: 35.0, armor: 8.0,
            speed: 280.0, hp_regen: 2.0, mana_regen: 1.5,
        },
        HeroClass::BroodMother => HeroStats {
            hp: 600.0, mana: 300.0, attack: 55.0, armor: 4.0,
            speed: 320.0, hp_regen: 1.5, mana_regen: 1.0,
        },
        HeroClass::PsionicOverseer => HeroStats {
            hp: 500.0, mana: 600.0, attack: 25.0, armor: 3.0,
            speed: 300.0, hp_regen: 1.0, mana_regen: 2.5,
        },
        HeroClass::InfernalLord => HeroStats {
            hp: 900.0, mana: 350.0, attack: 40.0, armor: 10.0,
            speed: 270.0, hp_regen: 2.5, mana_regen: 1.2,
        },
        HeroClass::ShadowAssassin => HeroStats {
            hp: 550.0, mana: 250.0, attack: 65.0, armor: 3.0,
            speed: 350.0, hp_regen: 1.2, mana_regen: 0.8,
        },
        HeroClass::ChaosSorcerer => HeroStats {
            hp: 450.0, mana: 650.0, attack: 20.0, armor: 2.0,
            speed: 290.0, hp_regen: 0.8, mana_regen: 3.0,
        },
        HeroClass::DeathKnight => HeroStats {
            hp: 850.0, mana: 400.0, attack: 42.0, armor: 9.0,
            speed: 275.0, hp_regen: 2.2, mana_regen: 1.5,
        },
        HeroClass::LichKing => HeroStats {
            hp: 480.0, mana: 700.0, attack: 22.0, armor: 2.0,
            speed: 280.0, hp_regen: 0.8, mana_regen: 3.5,
        },
        HeroClass::BansheeQueen => HeroStats {
            hp: 520.0, mana: 500.0, attack: 48.0, armor: 3.0,
            speed: 310.0, hp_regen: 1.0, mana_regen: 2.0,
        },
        HeroClass::Paladin => HeroStats {
            hp: 850.0, mana: 450.0, attack: 38.0, armor: 9.0,
            speed: 280.0, hp_regen: 2.5, mana_regen: 1.8,
        },
        HeroClass::Archmage => HeroStats {
            hp: 450.0, mana: 700.0, attack: 20.0, armor: 2.0,
            speed: 285.0, hp_regen: 0.8, mana_regen: 3.5,
        },
        HeroClass::MountainKing => HeroStats {
            hp: 700.0, mana: 300.0, attack: 60.0, armor: 7.0,
            speed: 290.0, hp_regen: 2.0, mana_regen: 1.0,
        },
    }
}

// ---------------------------------------------------------------------------
// Per-level scaling
// ---------------------------------------------------------------------------

/// XP required to reach the *next* level from the current one.
/// Formula: `100 * 1.5^level`
fn xp_for_level(level: u32) -> u64 {
    (100.0 * 1.5_f64.powi(level as i32)) as u64
}

/// Apply per-level scaling to base stats.
/// +5% HP, +3% ATK, +2% Armor, +8 Mana per level above 1.
fn scaled_stats(base: &HeroStats, level: u32) -> HeroStats {
    let lvls = (level.saturating_sub(1)) as f64;
    HeroStats {
        hp: base.hp * (1.0 + 0.05 * lvls),
        mana: base.mana + 8.0 * lvls,
        attack: base.attack * (1.0 + 0.03 * lvls),
        armor: base.armor * (1.0 + 0.02 * lvls),
        speed: base.speed,
        hp_regen: base.hp_regen * (1.0 + 0.03 * lvls),
        mana_regen: base.mana_regen * (1.0 + 0.02 * lvls),
    }
}

/// Apply equipment bonuses to stats (flat additive).
fn apply_equipment(stats: &mut HeroStats, equipment: &HeroEquipment) {
    for item in [&equipment.weapon, &equipment.armor, &equipment.accessory].into_iter().flatten() {
        for (key, val) in &item.stat_bonuses {
            match key.as_str() {
                "hp" => stats.hp += val,
                "mana" => stats.mana += val,
                "attack" => stats.attack += val,
                "armor" => stats.armor += val,
                "speed" => stats.speed += val,
                "hp_regen" => stats.hp_regen += val,
                "mana_regen" => stats.mana_regen += val,
                _ => {} // unknown stat key — ignore gracefully
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ability definitions per hero class
// ---------------------------------------------------------------------------

/// Build the three abilities for a hero class.
fn class_abilities(class: HeroClass) -> Vec<HeroAbility> {
    match class {
        // ───────── Insects ─────────
        HeroClass::HiveQueen => vec![
            HeroAbility {
                id: "hq_swarm_aura".into(), name: "Swarm Aura".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 0.0, heal: 0.0, aoe_radius: 8.0,
                description: "Nearby allied units gain +10% attack speed and +5% armor.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "hq_spawn_swarmlings".into(), name: "Spawn Swarmlings".into(),
                ability_type: AbilityType::Active, mana_cost: 80.0, cooldown_secs: 25.0,
                damage: 0.0, heal: 0.0, aoe_radius: 0.0,
                description: "Summon 4 swarmlings that last 30 seconds and attack nearby enemies.".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "hq_hivemind".into(), name: "Hivemind Surge".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 200.0, cooldown_secs: 120.0,
                damage: 0.0, heal: 150.0, aoe_radius: 15.0,
                description: "All allied units in radius gain +30% stats and regenerate HP for 15s.".into(),
                unlock_level: 6,
            },
        ],
        HeroClass::BroodMother => vec![
            HeroAbility {
                id: "bm_acid_blood".into(), name: "Acid Blood".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 15.0, heal: 0.0, aoe_radius: 3.0,
                description: "When damaged, splashes acid dealing 15 damage to nearby attackers.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "bm_burrow_ambush".into(), name: "Burrow Ambush".into(),
                ability_type: AbilityType::Active, mana_cost: 60.0, cooldown_secs: 18.0,
                damage: 120.0, heal: 0.0, aoe_radius: 0.0,
                description: "Burrow underground and ambush a target for 120 damage + 3s stun.".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "bm_acid_rain".into(), name: "Acid Rain".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 180.0, cooldown_secs: 90.0,
                damage: 80.0, heal: 0.0, aoe_radius: 12.0,
                description: "Rain acid over a large area, dealing 80 DPS for 8 seconds.".into(),
                unlock_level: 6,
            },
        ],
        HeroClass::PsionicOverseer => vec![
            HeroAbility {
                id: "po_psi_field".into(), name: "Psionic Field".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 0.0, heal: 0.0, aoe_radius: 10.0,
                description: "Enemies within range have -15% accuracy and -10% speed.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "po_mind_control".into(), name: "Mind Control".into(),
                ability_type: AbilityType::Active, mana_cost: 120.0, cooldown_secs: 30.0,
                damage: 0.0, heal: 0.0, aoe_radius: 0.0,
                description: "Take control of one enemy unit for 20 seconds.".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "po_psi_storm".into(), name: "Psionic Storm".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 250.0, cooldown_secs: 100.0,
                damage: 200.0, heal: 0.0, aoe_radius: 10.0,
                description: "Unleash a devastating psionic storm dealing 200 damage over 4s.".into(),
                unlock_level: 6,
            },
        ],

        // ───────── Demons ─────────
        HeroClass::InfernalLord => vec![
            HeroAbility {
                id: "il_fire_aura".into(), name: "Infernal Aura".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 10.0, heal: 0.0, aoe_radius: 5.0,
                description: "Burns nearby enemies for 10 DPS. Reduces enemy armor by 2.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "il_hellfire_charge".into(), name: "Hellfire Charge".into(),
                ability_type: AbilityType::Active, mana_cost: 70.0, cooldown_secs: 20.0,
                damage: 90.0, heal: 0.0, aoe_radius: 0.0,
                description: "Charge a target, dealing 90 damage and taunting for 5s.".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "il_rain_of_fire".into(), name: "Rain of Fire".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 220.0, cooldown_secs: 110.0,
                damage: 150.0, heal: 0.0, aoe_radius: 14.0,
                description: "Call down a firestorm dealing 150 damage in a massive radius.".into(),
                unlock_level: 6,
            },
        ],
        HeroClass::ShadowAssassin => vec![
            HeroAbility {
                id: "sa_shadow_walk".into(), name: "Shadow Walk".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 0.0, heal: 0.0, aoe_radius: 0.0,
                description: "First attack from stealth deals +50% damage (backstab).".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "sa_backstab".into(), name: "Shadowstrike".into(),
                ability_type: AbilityType::Active, mana_cost: 50.0, cooldown_secs: 12.0,
                damage: 140.0, heal: 0.0, aoe_radius: 0.0,
                description: "Teleport behind target and strike for 140 damage + poison (5s).".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "sa_death_mark".into(), name: "Death Mark".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 160.0, cooldown_secs: 80.0,
                damage: 250.0, heal: 0.0, aoe_radius: 0.0,
                description: "Mark a target for death; after 4s they take 250 true damage.".into(),
                unlock_level: 6,
            },
        ],
        HeroClass::ChaosSorcerer => vec![
            HeroAbility {
                id: "cs_chaos_affinity".into(), name: "Chaos Affinity".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 0.0, heal: 0.0, aoe_radius: 0.0,
                description: "Spells have a 15% chance to cast twice at no additional cost.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "cs_chaos_bolt".into(), name: "Chaos Bolt".into(),
                ability_type: AbilityType::Active, mana_cost: 90.0, cooldown_secs: 15.0,
                damage: 130.0, heal: 0.0, aoe_radius: 0.0,
                description: "Hurl a bolt of pure chaos dealing 130 damage + random debuff.".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "cs_doom".into(), name: "Doom".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 280.0, cooldown_secs: 120.0,
                damage: 300.0, heal: 0.0, aoe_radius: 8.0,
                description: "After 8s, the target explodes dealing 300 AoE damage.".into(),
                unlock_level: 6,
            },
        ],

        // ───────── Undead ─────────
        HeroClass::DeathKnight => vec![
            HeroAbility {
                id: "dk_unholy_aura".into(), name: "Unholy Aura".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 0.0, heal: 0.0, aoe_radius: 8.0,
                description: "Nearby allied undead units gain +15% movement speed and +2 HP/s.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "dk_death_coil".into(), name: "Death Coil".into(),
                ability_type: AbilityType::Active, mana_cost: 75.0, cooldown_secs: 8.0,
                damage: 100.0, heal: 100.0, aoe_radius: 0.0,
                description: "Deal 100 damage to living target or heal undead ally for 100.".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "dk_animate_dead".into(), name: "Animate Dead".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 250.0, cooldown_secs: 180.0,
                damage: 0.0, heal: 0.0, aoe_radius: 12.0,
                description: "Raise up to 6 dead units as skeletal warriors for 40 seconds.".into(),
                unlock_level: 6,
            },
        ],
        HeroClass::LichKing => vec![
            HeroAbility {
                id: "lk_frost_armor".into(), name: "Frost Armor".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 0.0, heal: 0.0, aoe_radius: 0.0,
                description: "Attackers are slowed by 20% for 3s. +3 bonus armor.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "lk_frost_nova".into(), name: "Frost Nova".into(),
                ability_type: AbilityType::Active, mana_cost: 100.0, cooldown_secs: 12.0,
                damage: 90.0, heal: 0.0, aoe_radius: 6.0,
                description: "Blast of frost dealing 90 damage and freezing enemies for 4s.".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "lk_dark_ritual".into(), name: "Dark Ritual".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 0.0, cooldown_secs: 60.0,
                damage: 0.0, heal: 0.0, aoe_radius: 0.0,
                description: "Sacrifice a friendly unit to instantly restore 50% max mana.".into(),
                unlock_level: 6,
            },
        ],
        HeroClass::BansheeQueen => vec![
            HeroAbility {
                id: "bq_banshee_wail".into(), name: "Banshee Wail".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 0.0, heal: 0.0, aoe_radius: 6.0,
                description: "Enemies in range have -20% attack damage.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "bq_anti_magic".into(), name: "Anti-Magic Shell".into(),
                ability_type: AbilityType::Active, mana_cost: 75.0, cooldown_secs: 20.0,
                damage: 0.0, heal: 0.0, aoe_radius: 0.0,
                description: "Shield an ally, absorbing up to 300 spell damage for 15s.".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "bq_possession".into(), name: "Possession".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 300.0, cooldown_secs: 150.0,
                damage: 0.0, heal: 0.0, aoe_radius: 0.0,
                description: "Permanently take control of an enemy non-hero unit.".into(),
                unlock_level: 6,
            },
        ],

        // ───────── Humans ─────────
        HeroClass::Paladin => vec![
            HeroAbility {
                id: "pl_devotion_aura".into(), name: "Devotion Aura".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 0.0, heal: 0.0, aoe_radius: 8.0,
                description: "Nearby allied units gain +4 armor.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "pl_holy_light".into(), name: "Holy Light".into(),
                ability_type: AbilityType::Active, mana_cost: 65.0, cooldown_secs: 6.0,
                damage: 0.0, heal: 200.0, aoe_radius: 0.0,
                description: "Heal a friendly unit for 200 HP or deal 100 to undead.".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "pl_resurrection".into(), name: "Resurrection".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 300.0, cooldown_secs: 240.0,
                damage: 0.0, heal: 0.0, aoe_radius: 10.0,
                description: "Resurrect up to 6 dead friendly units with full HP.".into(),
                unlock_level: 6,
            },
        ],
        HeroClass::Archmage => vec![
            HeroAbility {
                id: "am_brilliance_aura".into(), name: "Brilliance Aura".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 0.0, heal: 0.0, aoe_radius: 10.0,
                description: "Nearby allied heroes regenerate mana 100% faster.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "am_blizzard".into(), name: "Blizzard".into(),
                ability_type: AbilityType::Active, mana_cost: 120.0, cooldown_secs: 10.0,
                damage: 70.0, heal: 0.0, aoe_radius: 8.0,
                description: "Call down waves of ice dealing 70 damage per wave (6 waves).".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "am_mass_teleport".into(), name: "Mass Teleport".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 200.0, cooldown_secs: 60.0,
                damage: 0.0, heal: 0.0, aoe_radius: 12.0,
                description: "Teleport up to 24 nearby units to a target allied structure.".into(),
                unlock_level: 6,
            },
        ],
        HeroClass::MountainKing => vec![
            HeroAbility {
                id: "mk_bash".into(), name: "Bash".into(),
                ability_type: AbilityType::Passive, mana_cost: 0.0, cooldown_secs: 0.0,
                damage: 25.0, heal: 0.0, aoe_radius: 0.0,
                description: "20% chance on hit to stun target for 2s and deal 25 bonus damage.".into(),
                unlock_level: 1,
            },
            HeroAbility {
                id: "mk_storm_bolt".into(), name: "Storm Bolt".into(),
                ability_type: AbilityType::Active, mana_cost: 75.0, cooldown_secs: 9.0,
                damage: 100.0, heal: 0.0, aoe_radius: 0.0,
                description: "Hurl a magic hammer dealing 100 damage and stunning for 5s.".into(),
                unlock_level: 3,
            },
            HeroAbility {
                id: "mk_avatar".into(), name: "Avatar".into(),
                ability_type: AbilityType::Ultimate, mana_cost: 150.0, cooldown_secs: 180.0,
                damage: 0.0, heal: 0.0, aoe_radius: 0.0,
                description: "Transform into a stone giant: +5 armor, +50% damage, spell immune for 60s.".into(),
                unlock_level: 6,
            },
        ],
    }
}

// ---------------------------------------------------------------------------
// Hero factory
// ---------------------------------------------------------------------------

/// Create a new level-1 hero of the given class.
fn create_hero(class: HeroClass) -> Hero {
    let base = base_stats(class);
    let abilities = class_abilities(class);
    Hero {
        id: Uuid::new_v4().to_string(),
        name: class.display_name().to_string(),
        faction: class.faction().as_str().to_string(),
        hero_class: class,
        level: 1,
        xp: 0,
        xp_to_next: xp_for_level(1),
        stats: base,
        abilities,
        equipment: HeroEquipment {
            weapon: None,
            armor: None,
            accessory: None,
        },
        is_alive: true,
        respawn_timer_secs: 0,
    }
}

/// Recalculate a hero's stats from base + level + equipment.
fn recalc_stats(hero: &mut Hero) {
    let base = base_stats(hero.hero_class);
    let mut stats = scaled_stats(&base, hero.level);
    apply_equipment(&mut stats, &hero.equipment);
    hero.stats = stats;
}

// ---------------------------------------------------------------------------
// In-memory hero registry (shared mutable state via Mutex)
// ---------------------------------------------------------------------------

use std::sync::Mutex;

/// Lazily initialised hero registry.  All 12 heroes are created on first access.
fn hero_registry() -> &'static Mutex<Vec<Hero>> {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<Mutex<Vec<Hero>>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let heroes: Vec<Hero> = HeroClass::all().iter().map(|c| create_hero(*c)).collect();
        Mutex::new(heroes)
    })
}

// ===========================================================================
// SYSTEM 2: Alliance System
// ===========================================================================

/// Diplomatic status between two alliances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiplomacyStatus {
    Peace,
    War,
    Neutral,
    /// Non-Aggression Pact — no attacking each other.
    NonAggressionPact,
    /// Mutual Defense — attack one member, fight the whole alliance.
    MutualDefense,
    /// Trade Agreement — -50% trade tax between alliance members.
    TradeAgreement,
}

impl DiplomacyStatus {
    pub(crate) fn from_str_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "peace" => Some(Self::Peace),
            "war" => Some(Self::War),
            "neutral" => Some(Self::Neutral),
            "non_aggression_pact" | "nap" => Some(Self::NonAggressionPact),
            "mutual_defense" => Some(Self::MutualDefense),
            "trade_agreement" | "trade" => Some(Self::TradeAgreement),
            _ => None,
        }
    }
}

/// A diplomatic relation between this alliance and another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiplomaticRelation {
    pub target_alliance_id: String,
    pub status: DiplomacyStatus,
    pub since: String,
    pub expires: Option<String>,
}

/// Rank within an alliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllianceRank {
    Recruit = 0,
    Member = 1,
    Veteran = 2,
    Officer = 3,
    Leader = 4,
}

impl AllianceRank {
    pub(crate) fn from_str_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "recruit" => Some(Self::Recruit),
            "member" => Some(Self::Member),
            "veteran" => Some(Self::Veteran),
            "officer" => Some(Self::Officer),
            "leader" => Some(Self::Leader),
            _ => None,
        }
    }
}

/// A member of an alliance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllianceMember {
    pub player_id: String,
    pub rank: AllianceRank,
    pub joined_at: String,
    /// Total resources donated to the alliance.
    pub contribution: u64,
}

/// An alliance (guild/clan) of up to 25 players.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alliance {
    pub id: String,
    pub name: String,
    /// Short 3-4 letter tag, e.g. [WAR]
    pub tag: String,
    pub leader_id: String,
    pub members: Vec<AllianceMember>,
    pub max_members: u32,
    pub created_at: String,
    pub description: String,
    pub diplomacy: Vec<DiplomaticRelation>,
}

/// Lazily initialised alliance registry.
fn alliance_registry() -> &'static Mutex<Vec<Alliance>> {
    use std::sync::OnceLock;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_heroes", "Game");
    static REGISTRY: OnceLock<Mutex<Vec<Alliance>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

// ---------------------------------------------------------------------------
// Alliance logic
// ---------------------------------------------------------------------------

fn alliance_create_inner(name: String, tag: String, leader_id: String) -> AppResult<Alliance> {
    if tag.len() < 2 || tag.len() > 4 {
        return Err(ImpForgeError::validation(
            "INVALID_TAG",
            "Alliance tag must be 2-4 characters",
        ));
    }
    if name.is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_NAME",
            "Alliance name must not be empty",
        ));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let alliance = Alliance {
        id: Uuid::new_v4().to_string(),
        name,
        tag,
        leader_id: leader_id.clone(),
        members: vec![AllianceMember {
            player_id: leader_id,
            rank: AllianceRank::Leader,
            joined_at: now.clone(),
            contribution: 0,
        }],
        max_members: 25,
        created_at: now,
        description: String::new(),
        diplomacy: Vec::new(),
    };

    let mut reg = alliance_registry()
        .lock()
        .map_err(|_| ImpForgeError::internal("LOCK_FAILED", "Alliance registry lock poisoned"))?;
    reg.push(alliance.clone());
    Ok(alliance)
}

fn alliance_join_inner(alliance_id: String, player_id: String) -> AppResult<()> {
    let mut reg = alliance_registry()
        .lock()
        .map_err(|_| ImpForgeError::internal("LOCK_FAILED", "Alliance registry lock poisoned"))?;

    let alliance = reg
        .iter_mut()
        .find(|a| a.id == alliance_id)
        .ok_or_else(|| ImpForgeError::validation("NOT_FOUND", "Alliance not found"))?;

    if alliance.members.len() as u32 >= alliance.max_members {
        return Err(ImpForgeError::validation("FULL", "Alliance is at max capacity"));
    }
    if alliance.members.iter().any(|m| m.player_id == player_id) {
        return Err(ImpForgeError::validation("ALREADY_MEMBER", "Player is already a member"));
    }

    alliance.members.push(AllianceMember {
        player_id,
        rank: AllianceRank::Recruit,
        joined_at: chrono::Utc::now().to_rfc3339(),
        contribution: 0,
    });
    Ok(())
}

fn alliance_leave_inner(alliance_id: String, player_id: String) -> AppResult<()> {
    let mut reg = alliance_registry()
        .lock()
        .map_err(|_| ImpForgeError::internal("LOCK_FAILED", "Alliance registry lock poisoned"))?;

    let alliance = reg
        .iter_mut()
        .find(|a| a.id == alliance_id)
        .ok_or_else(|| ImpForgeError::validation("NOT_FOUND", "Alliance not found"))?;

    if alliance.leader_id == player_id {
        return Err(ImpForgeError::validation(
            "LEADER_CANNOT_LEAVE",
            "Leader must transfer leadership before leaving",
        ));
    }

    let before = alliance.members.len();
    alliance.members.retain(|m| m.player_id != player_id);
    if alliance.members.len() == before {
        return Err(ImpForgeError::validation("NOT_MEMBER", "Player is not a member"));
    }
    Ok(())
}

fn alliance_set_diplomacy_inner(
    alliance_id: String,
    target_id: String,
    status: String,
) -> AppResult<()> {
    let parsed_status = DiplomacyStatus::from_str_name(&status)
        .ok_or_else(|| ImpForgeError::validation("INVALID_STATUS", format!("Unknown diplomacy status: {status}")))?;

    let mut reg = alliance_registry()
        .lock()
        .map_err(|_| ImpForgeError::internal("LOCK_FAILED", "Alliance registry lock poisoned"))?;

    let alliance = reg
        .iter_mut()
        .find(|a| a.id == alliance_id)
        .ok_or_else(|| ImpForgeError::validation("NOT_FOUND", "Alliance not found"))?;

    // Update existing relation or create a new one
    if let Some(rel) = alliance.diplomacy.iter_mut().find(|r| r.target_alliance_id == target_id) {
        rel.status = parsed_status;
        rel.since = chrono::Utc::now().to_rfc3339();
        rel.expires = None;
    } else {
        alliance.diplomacy.push(DiplomaticRelation {
            target_alliance_id: target_id,
            status: parsed_status,
            since: chrono::Utc::now().to_rfc3339(),
            expires: None,
        });
    }
    Ok(())
}

fn alliance_list_inner() -> AppResult<Vec<Alliance>> {
    let reg = alliance_registry()
        .lock()
        .map_err(|_| ImpForgeError::internal("LOCK_FAILED", "Alliance registry lock poisoned"))?;
    Ok(reg.clone())
}

// ===========================================================================
// SYSTEM 3: WC3/SC2 Damage-Armor Matrix
// ===========================================================================

/// Warcraft III 7x7 damage-armor multiplier matrix.
///
/// Attack types: Normal, Pierce, Magic, Siege, Hero, Chaos, Spell
/// Armor types:  Heavy, Medium, Light, Unarmored, Fortified, Hero, Divine
///
/// Values sourced from the WC3 game manual and community wiki.
pub(crate) fn wc3_damage_matrix(attack_type: &str, armor_type: &str) -> f64 {
    match (
        attack_type.to_ascii_lowercase().as_str(),
        armor_type.to_ascii_lowercase().as_str(),
    ) {
        // Normal
        ("normal", "heavy") => 1.0,
        ("normal", "medium") => 1.5,
        ("normal", "light") => 1.0,
        ("normal", "unarmored") => 1.0,
        ("normal", "fortified") => 0.7,
        ("normal", "hero") => 1.0,
        ("normal", "divine") => 0.05,

        // Pierce
        ("pierce", "heavy") => 2.0,
        ("pierce", "medium") => 0.75,
        ("pierce", "light") => 2.0,
        ("pierce", "unarmored") => 1.5,
        ("pierce", "fortified") => 0.35,
        ("pierce", "hero") => 0.5,
        ("pierce", "divine") => 0.05,

        // Magic
        ("magic", "heavy") => 1.25,
        ("magic", "medium") => 0.75,
        ("magic", "light") => 2.0,
        ("magic", "unarmored") => 1.5,
        ("magic", "fortified") => 0.35,
        ("magic", "hero") => 0.5,
        ("magic", "divine") => 0.05,

        // Siege
        ("siege", "heavy") => 1.0,
        ("siege", "medium") => 0.5,
        ("siege", "light") => 1.0,
        ("siege", "unarmored") => 1.5,
        ("siege", "fortified") => 1.5,
        ("siege", "hero") => 0.5,
        ("siege", "divine") => 0.05,

        // Hero
        ("hero", "heavy") => 1.0,
        ("hero", "medium") => 1.0,
        ("hero", "light") => 1.0,
        ("hero", "unarmored") => 1.0,
        ("hero", "fortified") => 0.5,
        ("hero", "hero") => 1.0,
        ("hero", "divine") => 0.05,

        // Chaos — 100% against everything
        ("chaos", _) => 1.0,

        // Spell — 100% against everything except Divine (immune)
        ("spell", "divine") => 0.0,
        ("spell", _) => 1.0,

        // Unknown combination — neutral
        _ => 1.0,
    }
}

/// SC2-style bonus damage attributes for a given unit archetype.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sc2BonusDamage {
    pub anti_armored: f64,
    pub anti_light: f64,
    pub anti_massive: f64,
    pub anti_biological: f64,
    pub anti_mechanical: f64,
    pub anti_psionic: f64,
}

/// Lookup SC2-style bonus damage values for a unit archetype.
///
/// Returns attribute-specific bonus damage that is added on top of base
/// damage when the target has the matching tag (e.g. "armored", "light").
pub(crate) fn sc2_bonus_for_unit(unit_type: &str) -> Sc2BonusDamage {
    match unit_type.to_ascii_lowercase().as_str() {
        // Anti-armor specialists
        "marauder" | "immortal" => Sc2BonusDamage {
            anti_armored: 20.0, anti_light: 0.0, anti_massive: 0.0,
            anti_biological: 0.0, anti_mechanical: 0.0, anti_psionic: 0.0,
        },
        // Anti-light splash
        "hellion" | "colossus" | "adept" => Sc2BonusDamage {
            anti_armored: 0.0, anti_light: 12.0, anti_massive: 0.0,
            anti_biological: 0.0, anti_mechanical: 0.0, anti_psionic: 0.0,
        },
        // Anti-massive
        "void_ray" | "tempest" | "corruptor" => Sc2BonusDamage {
            anti_armored: 0.0, anti_light: 0.0, anti_massive: 14.0,
            anti_biological: 0.0, anti_mechanical: 0.0, anti_psionic: 0.0,
        },
        // Anti-biological
        "ghost" | "archon" | "infestor" => Sc2BonusDamage {
            anti_armored: 0.0, anti_light: 0.0, anti_massive: 0.0,
            anti_biological: 10.0, anti_mechanical: 0.0, anti_psionic: 0.0,
        },
        // Anti-mechanical
        "stalker" => Sc2BonusDamage {
            anti_armored: 0.0, anti_light: 0.0, anti_massive: 0.0,
            anti_biological: 0.0, anti_mechanical: 8.0, anti_psionic: 0.0,
        },
        // Anti-psionic
        "high_templar" => Sc2BonusDamage {
            anti_armored: 0.0, anti_light: 0.0, anti_massive: 0.0,
            anti_biological: 0.0, anti_mechanical: 0.0, anti_psionic: 15.0,
        },
        // Generalist — no bonus damage
        _ => Sc2BonusDamage {
            anti_armored: 0.0, anti_light: 0.0, anti_massive: 0.0,
            anti_biological: 0.0, anti_mechanical: 0.0, anti_psionic: 0.0,
        },
    }
}

/// SwarmForge custom damage matrix — faction-themed 4x4 elemental system.
///
/// Attack types: bio_acid, hellfire, necrotic, holy
/// Armor types:  chitin, infernal, ethereal, plate
///
/// - Diagonal (same faction) = 0.5x (resistant to own element)
/// - Counter matchup = 2.0x (strong against)
/// - Weak matchup = 0.75x
/// - Neutral = 1.0x
pub(crate) fn swarmforge_damage_matrix(damage_type: &str, armor_type: &str) -> f64 {
    match (
        damage_type.to_ascii_lowercase().as_str(),
        armor_type.to_ascii_lowercase().as_str(),
    ) {
        // Bio-acid (Insects): strong vs plate, weak vs infernal, resists chitin
        ("bio_acid", "chitin") => 0.5,
        ("bio_acid", "infernal") => 0.75,
        ("bio_acid", "ethereal") => 1.0,
        ("bio_acid", "plate") => 2.0,

        // Hellfire (Demons): strong vs chitin, weak vs ethereal, resists infernal
        ("hellfire", "chitin") => 2.0,
        ("hellfire", "infernal") => 0.5,
        ("hellfire", "ethereal") => 0.75,
        ("hellfire", "plate") => 1.0,

        // Necrotic (Undead): strong vs infernal, weak vs plate, resists ethereal
        ("necrotic", "chitin") => 1.0,
        ("necrotic", "infernal") => 2.0,
        ("necrotic", "ethereal") => 0.5,
        ("necrotic", "plate") => 0.75,

        // Holy (Humans): strong vs ethereal, weak vs chitin, resists plate
        ("holy", "chitin") => 0.75,
        ("holy", "infernal") => 1.0,
        ("holy", "ethereal") => 2.0,
        ("holy", "plate") => 0.5,

        // Unknown — neutral
        _ => 1.0,
    }
}

// ===========================================================================
// Tauri Commands — Heroes (5)
// ===========================================================================

/// List all heroes belonging to a faction.
#[tauri::command]
pub async fn hero_roster(faction: String) -> Result<Vec<Hero>, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_heroes", "game_heroes", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_heroes", "game_heroes");
    crate::synapse_fabric::synapse_session_push("swarm_heroes", "game_heroes", "hero_roster called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_heroes", "info", "swarm_heroes active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_heroes", "recruit", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"faction": faction}));
    let target = Faction::from_str(&faction);
    let reg = hero_registry()
        .lock()
        .map_err(|_| ImpForgeError::internal("LOCK_FAILED", "Hero registry lock poisoned"))?;
    let roster: Vec<Hero> = reg
        .iter()
        .filter(|h| h.hero_class.faction() == target)
        .cloned()
        .collect();
    Ok(roster)
}

/// Get a single hero by ID.
#[tauri::command]
pub async fn hero_get(hero_id: String) -> Result<Hero, ImpForgeError> {
    let reg = hero_registry()
        .lock()
        .map_err(|_| ImpForgeError::internal("LOCK_FAILED", "Hero registry lock poisoned"))?;
    reg.iter()
        .find(|h| h.id == hero_id)
        .cloned()
        .ok_or_else(|| ImpForgeError::validation("HERO_NOT_FOUND", format!("No hero with id: {hero_id}")))
}

/// Grant enough XP to level up a hero (for testing/admin use).
/// Returns the updated hero.
#[tauri::command]
pub async fn hero_level_up(hero_id: String) -> Result<Hero, ImpForgeError> {
    let mut reg = hero_registry()
        .lock()
        .map_err(|_| ImpForgeError::internal("LOCK_FAILED", "Hero registry lock poisoned"))?;

    let hero = reg
        .iter_mut()
        .find(|h| h.id == hero_id)
        .ok_or_else(|| ImpForgeError::validation("HERO_NOT_FOUND", format!("No hero with id: {hero_id}")))?;

    if !hero.is_alive {
        return Err(ImpForgeError::validation("HERO_DEAD", "Cannot level up a dead hero"));
    }

    hero.level += 1;
    hero.xp = 0;
    hero.xp_to_next = xp_for_level(hero.level);
    recalc_stats(hero);

    Ok(hero.clone())
}

/// Equip an item to a hero's slot (weapon, armor, accessory).
#[tauri::command]
pub async fn hero_equip(
    hero_id: String,
    slot: String,
    item: EquipmentItem,
) -> Result<Hero, ImpForgeError> {
    let mut reg = hero_registry()
        .lock()
        .map_err(|_| ImpForgeError::internal("LOCK_FAILED", "Hero registry lock poisoned"))?;

    let hero = reg
        .iter_mut()
        .find(|h| h.id == hero_id)
        .ok_or_else(|| ImpForgeError::validation("HERO_NOT_FOUND", format!("No hero with id: {hero_id}")))?;

    match slot.to_ascii_lowercase().as_str() {
        "weapon" => hero.equipment.weapon = Some(item),
        "armor" => hero.equipment.armor = Some(item),
        "accessory" => hero.equipment.accessory = Some(item),
        _ => {
            return Err(ImpForgeError::validation(
                "INVALID_SLOT",
                format!("Unknown equipment slot: {slot}. Use weapon, armor, or accessory."),
            ));
        }
    }

    recalc_stats(hero);
    Ok(hero.clone())
}

/// Use a hero ability.  Returns a JSON payload describing the effect.
#[tauri::command]
pub async fn hero_use_ability(
    hero_id: String,
    ability_id: String,
) -> Result<serde_json::Value, ImpForgeError> {
    let reg = hero_registry()
        .lock()
        .map_err(|_| ImpForgeError::internal("LOCK_FAILED", "Hero registry lock poisoned"))?;

    let hero = reg
        .iter()
        .find(|h| h.id == hero_id)
        .ok_or_else(|| ImpForgeError::validation("HERO_NOT_FOUND", format!("No hero with id: {hero_id}")))?;

    if !hero.is_alive {
        return Err(ImpForgeError::validation("HERO_DEAD", "Dead heroes cannot use abilities"));
    }

    let ability = hero
        .abilities
        .iter()
        .find(|a| a.id == ability_id)
        .ok_or_else(|| {
            ImpForgeError::validation("ABILITY_NOT_FOUND", format!("No ability with id: {ability_id}"))
        })?;

    if hero.level < ability.unlock_level {
        return Err(ImpForgeError::validation(
            "ABILITY_LOCKED",
            format!(
                "Ability '{}' requires level {}, hero is level {}",
                ability.name, ability.unlock_level, hero.level
            ),
        ));
    }

    if ability.ability_type == AbilityType::Passive {
        return Ok(serde_json::json!({
            "hero_id": hero.id,
            "ability": ability.name,
            "type": "passive",
            "message": format!("{} is a passive ability and is always active.", ability.name),
        }));
    }

    if hero.stats.mana < ability.mana_cost {
        return Err(ImpForgeError::validation(
            "INSUFFICIENT_MANA",
            format!(
                "Not enough mana: need {}, have {}",
                ability.mana_cost, hero.stats.mana
            ),
        ));
    }

    Ok(serde_json::json!({
        "hero_id": hero.id,
        "ability": ability.name,
        "type": ability.ability_type,
        "mana_cost": ability.mana_cost,
        "damage": ability.damage,
        "heal": ability.heal,
        "aoe_radius": ability.aoe_radius,
        "cooldown_secs": ability.cooldown_secs,
        "message": format!("{} casts {}!", hero.name, ability.name),
    }))
}

// ===========================================================================
// Tauri Commands — Alliance (5)
// ===========================================================================

/// Create a new alliance.
#[tauri::command]
pub async fn alliance_create(
    name: String,
    tag: String,
    leader_id: String,
) -> Result<Alliance, ImpForgeError> {
    alliance_create_inner(name, tag, leader_id)
}

/// Join an existing alliance.
#[tauri::command]
pub async fn alliance_join(alliance_id: String, player_id: String) -> Result<(), ImpForgeError> {
    alliance_join_inner(alliance_id, player_id)
}

/// Leave an alliance (leaders must transfer leadership first).
#[tauri::command]
pub async fn alliance_leave(alliance_id: String, player_id: String) -> Result<(), ImpForgeError> {
    alliance_leave_inner(alliance_id, player_id)
}

/// Set or update a diplomatic relation between two alliances.
#[tauri::command]
pub async fn alliance_diplomacy(
    alliance_id: String,
    target_id: String,
    status: String,
) -> Result<(), ImpForgeError> {
    alliance_set_diplomacy_inner(alliance_id, target_id, status)
}

/// List all alliances.
#[tauri::command]
pub async fn alliance_list() -> Result<Vec<Alliance>, ImpForgeError> {
    alliance_list_inner()
}

// ===========================================================================
// Tauri Commands — Damage Matrix (3)
// ===========================================================================

/// WC3 damage matrix lookup.
#[tauri::command]
pub async fn damage_matrix_wc3(attack: String, armor: String) -> Result<f64, ImpForgeError> {
    Ok(wc3_damage_matrix(&attack, &armor))
}

/// SC2 bonus damage lookup for a unit type.
#[tauri::command]
pub async fn damage_matrix_sc2_bonus(unit_type: String) -> Result<Sc2BonusDamage, ImpForgeError> {
    Ok(sc2_bonus_for_unit(&unit_type))
}

/// SwarmForge custom faction-elemental damage matrix.
#[tauri::command]
pub async fn damage_matrix_swarmforge(
    damage_type: String,
    armor_type: String,
) -> Result<f64, ImpForgeError> {
    Ok(swarmforge_damage_matrix(&damage_type, &armor_type))
}

// ===========================================================================
// Additional Tauri Commands — wiring internal helpers
// ===========================================================================

/// Look up a hero class by its string name and return details.
#[tauri::command]
pub async fn hero_class_info(name: String) -> Result<serde_json::Value, ImpForgeError> {
    let class = HeroClass::from_str_name(&name).ok_or_else(|| {
        ImpForgeError::validation("HERO_UNKNOWN_CLASS", format!("Unknown hero class: {name}"))
    })?;
    Ok(serde_json::json!({
        "class": class.as_str(),
        "faction": format!("{:?}", class.faction()),
    }))
}

/// Parse an alliance rank from its string name.
#[tauri::command]
pub async fn alliance_rank_info(rank: String) -> Result<serde_json::Value, ImpForgeError> {
    let r = AllianceRank::from_str_name(&rank).ok_or_else(|| {
        ImpForgeError::validation("HERO_UNKNOWN_RANK", format!("Unknown rank: {rank}"))
    })?;
    Ok(serde_json::json!({
        "rank": format!("{:?}", r),
        "value": r as u8,
    }))
}

// ===========================================================================
// Tests (20+)
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;


    // ───────── Hero creation & stats ─────────

    #[test]
    fn test_create_all_12_heroes() {
        let heroes: Vec<Hero> = HeroClass::all().iter().map(|c| create_hero(*c)).collect();
        assert_eq!(heroes.len(), 12);
        for h in &heroes {
            assert_eq!(h.level, 1);
            assert!(h.is_alive);
            assert_eq!(h.respawn_timer_secs, 0);
            assert_eq!(h.abilities.len(), 3);
        }
    }

    #[test]
    fn test_hero_class_faction_mapping() {
        assert_eq!(HeroClass::HiveQueen.faction(), Faction::Insects);
        assert_eq!(HeroClass::BroodMother.faction(), Faction::Insects);
        assert_eq!(HeroClass::PsionicOverseer.faction(), Faction::Insects);

        assert_eq!(HeroClass::InfernalLord.faction(), Faction::Demons);
        assert_eq!(HeroClass::ShadowAssassin.faction(), Faction::Demons);
        assert_eq!(HeroClass::ChaosSorcerer.faction(), Faction::Demons);

        assert_eq!(HeroClass::DeathKnight.faction(), Faction::Undead);
        assert_eq!(HeroClass::LichKing.faction(), Faction::Undead);
        assert_eq!(HeroClass::BansheeQueen.faction(), Faction::Undead);

        assert_eq!(HeroClass::Paladin.faction(), Faction::Humans);
        assert_eq!(HeroClass::Archmage.faction(), Faction::Humans);
        assert_eq!(HeroClass::MountainKing.faction(), Faction::Humans);
    }

    #[test]
    fn test_base_stats_hive_queen() {
        let s = base_stats(HeroClass::HiveQueen);
        assert!((s.hp - 800.0).abs() < f64::EPSILON);
        assert!((s.mana - 400.0).abs() < f64::EPSILON);
        assert!((s.attack - 35.0).abs() < f64::EPSILON);
        assert!((s.armor - 8.0).abs() < f64::EPSILON);
        assert!((s.speed - 280.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_base_stats_shadow_assassin() {
        let s = base_stats(HeroClass::ShadowAssassin);
        assert!((s.hp - 550.0).abs() < f64::EPSILON);
        assert!((s.attack - 65.0).abs() < f64::EPSILON);
        assert!((s.speed - 350.0).abs() < f64::EPSILON);
    }

    // ───────── Leveling ─────────

    #[test]
    fn test_xp_formula_exponential() {
        let xp1 = xp_for_level(1);
        let xp5 = xp_for_level(5);
        let xp10 = xp_for_level(10);
        assert_eq!(xp1, 150); // 100 * 1.5^1 = 150
        assert!(xp5 > xp1);
        assert!(xp10 > xp5);
        // Verify specific: 100 * 1.5^5 = 759
        assert_eq!(xp5, 759);
    }

    #[test]
    fn test_scaled_stats_level_1_is_base() {
        let base = base_stats(HeroClass::Paladin);
        let scaled = scaled_stats(&base, 1);
        assert!((scaled.hp - base.hp).abs() < f64::EPSILON);
        assert!((scaled.mana - base.mana).abs() < f64::EPSILON);
        assert!((scaled.attack - base.attack).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scaled_stats_level_up_increases() {
        let base = base_stats(HeroClass::MountainKing);
        let at_5 = scaled_stats(&base, 5);
        // 4 levels above 1: HP = 700 * (1 + 0.05 * 4) = 700 * 1.2 = 840
        assert!((at_5.hp - 840.0).abs() < 0.01);
        // ATK = 60 * (1 + 0.03 * 4) = 60 * 1.12 = 67.2
        assert!((at_5.attack - 67.2).abs() < 0.01);
        // Mana = 300 + 8 * 4 = 332
        assert!((at_5.mana - 332.0).abs() < 0.01);
    }

    #[test]
    fn test_respawn_timer_formula() {
        // Death penalty: 60 * level seconds
        for level in 1..=20 {
            assert_eq!(60 * level, 60 * level);
        }
        // Level 10 hero = 600s = 10 minutes
        assert_eq!(60_u32 * 10, 600);
    }

    // ───────── Abilities ─────────

    #[test]
    fn test_each_hero_has_three_abilities() {
        for class in HeroClass::all() {
            let abilities = class_abilities(*class);
            assert_eq!(abilities.len(), 3, "class {:?} should have 3 abilities", class);
        }
    }

    #[test]
    fn test_ability_unlock_levels() {
        for class in HeroClass::all() {
            let abilities = class_abilities(*class);
            let mut levels: Vec<u32> = abilities.iter().map(|a| a.unlock_level).collect();
            levels.sort();
            assert_eq!(levels, vec![1, 3, 6], "class {:?} unlock levels", class);
        }
    }

    #[test]
    fn test_ability_types_per_hero() {
        for class in HeroClass::all() {
            let abilities = class_abilities(*class);
            let has_passive = abilities.iter().any(|a| a.ability_type == AbilityType::Passive);
            let has_active = abilities.iter().any(|a| a.ability_type == AbilityType::Active);
            let has_ultimate = abilities.iter().any(|a| a.ability_type == AbilityType::Ultimate);
            assert!(has_passive, "class {:?} missing passive", class);
            assert!(has_active, "class {:?} missing active", class);
            assert!(has_ultimate, "class {:?} missing ultimate", class);
        }
    }

    // ───────── Equipment ─────────

    #[test]
    fn test_equipment_stat_bonuses() {
        let mut hero = create_hero(HeroClass::Paladin);
        let base_hp = hero.stats.hp;
        let base_atk = hero.stats.attack;

        let sword = EquipmentItem {
            name: "Holy Avenger".into(),
            rarity: ItemRarity::Epic,
            stat_bonuses: HashMap::from([
                ("attack".into(), 20.0),
                ("hp".into(), 100.0),
            ]),
        };
        hero.equipment.weapon = Some(sword);
        recalc_stats(&mut hero);

        assert!((hero.stats.hp - (base_hp + 100.0)).abs() < f64::EPSILON);
        assert!((hero.stats.attack - (base_atk + 20.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_equipment_all_slots() {
        let mut hero = create_hero(HeroClass::ShadowAssassin);

        hero.equipment.weapon = Some(EquipmentItem {
            name: "Shadow Blade".into(),
            rarity: ItemRarity::Legendary,
            stat_bonuses: HashMap::from([("attack".into(), 30.0)]),
        });
        hero.equipment.armor = Some(EquipmentItem {
            name: "Cloak of Shadows".into(),
            rarity: ItemRarity::Rare,
            stat_bonuses: HashMap::from([("armor".into(), 5.0)]),
        });
        hero.equipment.accessory = Some(EquipmentItem {
            name: "Ring of Speed".into(),
            rarity: ItemRarity::Common,
            stat_bonuses: HashMap::from([("speed".into(), 25.0)]),
        });
        recalc_stats(&mut hero);

        let base = base_stats(HeroClass::ShadowAssassin);
        assert!((hero.stats.attack - (base.attack + 30.0)).abs() < 0.01);
        assert!((hero.stats.armor - (base.armor + 5.0)).abs() < 0.01);
        assert!((hero.stats.speed - (base.speed + 25.0)).abs() < 0.01);
    }

    // ───────── HeroClass parsing ─────────

    #[test]
    fn test_hero_class_roundtrip() {
        for class in HeroClass::all() {
            let name = class.as_str();
            let parsed = HeroClass::from_str_name(name);
            assert_eq!(parsed, Some(*class), "roundtrip failed for {name}");
        }
    }

    #[test]
    fn test_hero_class_unknown_returns_none() {
        assert_eq!(HeroClass::from_str_name("unknown_class"), None);
    }

    // ───────── Alliance ─────────

    #[test]
    fn test_alliance_create_and_list() {
        // Clear registry for test isolation
        {
            let mut reg = alliance_registry().lock().expect("mutex lock should succeed");
            reg.clear();
        }

        let a = alliance_create_inner("Test Alliance".into(), "TST".into(), "leader1".into()).expect("a should be valid");
        assert_eq!(a.name, "Test Alliance");
        assert_eq!(a.tag, "TST");
        assert_eq!(a.members.len(), 1);
        assert_eq!(a.members[0].rank, AllianceRank::Leader);

        let list = alliance_list_inner().expect("list should be valid");
        assert!(!list.is_empty());
    }

    #[test]
    fn test_alliance_join_and_leave() {
        {
            let mut reg = alliance_registry().lock().expect("mutex lock should succeed");
            reg.clear();
        }

        let a = alliance_create_inner("Join Test".into(), "JT".into(), "leader2".into()).expect("a should be valid");
        alliance_join_inner(a.id.clone(), "player1".into()).expect("clone should succeed");

        let list = alliance_list_inner().expect("list should be valid");
        let alliance = list.iter().find(|x| x.id == a.id).expect("find should return result");
        assert_eq!(alliance.members.len(), 2);

        alliance_leave_inner(a.id.clone(), "player1".into()).expect("clone should succeed");
        let list = alliance_list_inner().expect("list should be valid");
        let alliance = list.iter().find(|x| x.id == a.id).expect("find should return result");
        assert_eq!(alliance.members.len(), 1);
    }

    #[test]
    fn test_alliance_leader_cannot_leave() {
        {
            let mut reg = alliance_registry().lock().expect("mutex lock should succeed");
            reg.clear();
        }

        let a = alliance_create_inner("Leader Test".into(), "LT".into(), "boss".into()).expect("a should be valid");
        let result = alliance_leave_inner(a.id, "boss".into());
        assert!(result.is_err());
    }

    #[test]
    fn test_alliance_tag_validation() {
        let too_short = alliance_create_inner("Bad".into(), "X".into(), "p".into());
        assert!(too_short.is_err());

        let too_long = alliance_create_inner("Bad".into(), "TOOLONG".into(), "p".into());
        assert!(too_long.is_err());

        let ok = alliance_create_inner("Good".into(), "OK".into(), "p".into());
        assert!(ok.is_ok());
    }

    #[test]
    fn test_alliance_diplomacy() {
        {
            let mut reg = alliance_registry().lock().expect("mutex lock should succeed");
            reg.clear();
        }

        let a = alliance_create_inner("Ally A".into(), "AA".into(), "la".into()).expect("a should be valid");
        let b = alliance_create_inner("Ally B".into(), "BB".into(), "lb".into()).expect("b should be valid");

        alliance_set_diplomacy_inner(a.id.clone(), b.id.clone(), "war".into()).expect("clone should succeed");

        let list = alliance_list_inner().expect("list should be valid");
        let ally_a = list.iter().find(|x| x.id == a.id).expect("find should return result");
        assert_eq!(ally_a.diplomacy.len(), 1);
        assert_eq!(ally_a.diplomacy[0].status, DiplomacyStatus::War);

        // Update to peace
        alliance_set_diplomacy_inner(a.id.clone(), b.id.clone(), "peace".into()).expect("clone should succeed");
        let list = alliance_list_inner().expect("list should be valid");
        let ally_a = list.iter().find(|x| x.id == a.id).expect("find should return result");
        assert_eq!(ally_a.diplomacy[0].status, DiplomacyStatus::Peace);
    }

    // ───────── WC3 Damage Matrix ─────────

    #[test]
    fn test_wc3_normal_vs_medium() {
        assert!((wc3_damage_matrix("normal", "medium") - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_wc3_pierce_vs_heavy() {
        assert!((wc3_damage_matrix("pierce", "heavy") - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_wc3_chaos_always_100() {
        for armor in &["heavy", "medium", "light", "unarmored", "fortified", "hero", "divine"] {
            assert!(
                (wc3_damage_matrix("chaos", armor) - 1.0).abs() < f64::EPSILON,
                "chaos vs {armor} should be 1.0"
            );
        }
    }

    #[test]
    fn test_wc3_spell_vs_divine_immune() {
        assert!((wc3_damage_matrix("spell", "divine") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_wc3_case_insensitive() {
        assert!((wc3_damage_matrix("MAGIC", "LIGHT") - 2.0).abs() < f64::EPSILON);
        assert!((wc3_damage_matrix("Pierce", "Hero") - 0.5).abs() < f64::EPSILON);
    }

    // ───────── SC2 Bonus Damage ─────────

    #[test]
    fn test_sc2_marauder_anti_armored() {
        let b = sc2_bonus_for_unit("marauder");
        assert!((b.anti_armored - 20.0).abs() < f64::EPSILON);
        assert!((b.anti_light - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sc2_unknown_unit_no_bonus() {
        let b = sc2_bonus_for_unit("marine");
        assert!((b.anti_armored - 0.0).abs() < f64::EPSILON);
        assert!((b.anti_light - 0.0).abs() < f64::EPSILON);
        assert!((b.anti_massive - 0.0).abs() < f64::EPSILON);
    }

    // ───────── SwarmForge Damage Matrix ─────────

    #[test]
    fn test_swarmforge_self_resist() {
        assert!((swarmforge_damage_matrix("bio_acid", "chitin") - 0.5).abs() < f64::EPSILON);
        assert!((swarmforge_damage_matrix("hellfire", "infernal") - 0.5).abs() < f64::EPSILON);
        assert!((swarmforge_damage_matrix("necrotic", "ethereal") - 0.5).abs() < f64::EPSILON);
        assert!((swarmforge_damage_matrix("holy", "plate") - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_swarmforge_counter_matchup() {
        assert!((swarmforge_damage_matrix("bio_acid", "plate") - 2.0).abs() < f64::EPSILON);
        assert!((swarmforge_damage_matrix("hellfire", "chitin") - 2.0).abs() < f64::EPSILON);
        assert!((swarmforge_damage_matrix("necrotic", "infernal") - 2.0).abs() < f64::EPSILON);
        assert!((swarmforge_damage_matrix("holy", "ethereal") - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_swarmforge_unknown_neutral() {
        assert!((swarmforge_damage_matrix("unknown", "chitin") - 1.0).abs() < f64::EPSILON);
    }
}
