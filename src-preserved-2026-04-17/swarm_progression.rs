// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology
//! SwarmForge Mana System & Unit Tier/Star Progression
//!
//! Two subsystems that extend the SwarmForge RPG layer:
//!
//! ## Mana System (WoW/LoL-inspired)
//!
//! Every caster unit has a mana pool with base regeneration scaled by
//! intelligence and terrain bonuses.  Spells cost mana, have cast times,
//! and can be interrupted by pushback (WoW-style: +10% max cast time per
//! hit while casting, capped at 5 pushbacks).
//!
//! Four factions each have a unique spell roster:
//! - **Insects**: Acid Spray, Pheromone Burst, Swarm Cloud, Hive Mind
//! - **Demons**: Fireball (evolves), Shadow Bolt, Hellfire, Doom
//! - **Undead**: Frost Bolt, Death Coil, Raise Dead, Plague
//! - **Humans**: Holy Light, Blizzard, Thunder Clap, Resurrection
//!
//! ## Unit Tier / Star System (Idle Heroes-inspired)
//!
//! Units progress from 1-star through 10-star, then into Enlightened tiers
//! (E1 through E5).  Each tier grants multiplicative stat bonuses and can
//! unlock abilities.  Progression requires copies of the same unit, faction
//! fodder, or sacrifices of high-tier units.
//!
//! Each faction has themed progression names:
//! - Insects: Larva -> Pupa -> Nymph -> Adult -> Queen -> Metamorphosis
//! - Demons: Lesser -> Minor -> Greater -> Archon -> Overlord -> Ascension
//! - Undead: Remnant -> Shade -> Revenant -> Lich -> Death Knight -> Transcendence
//! - Humans: Recruit -> Veteran -> Elite -> Champion -> Legend -> Ascension

use serde::{Deserialize, Serialize};

use crate::error::{AppResult, ImpForgeError};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_progression", "Game");

// ---------------------------------------------------------------------------
// Mana Pool
// ---------------------------------------------------------------------------

/// A caster unit's mana resource.  Regenerates over time based on the unit's
/// intelligence stat plus any terrain or item bonuses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManaPool {
    /// Current mana available for casting.
    pub current: f64,
    /// Maximum mana capacity.
    pub maximum: f64,
    /// Base mana regenerated per second (before intelligence scaling).
    pub regen_per_sec: f64,
    /// Bonus regeneration from terrain, items, buffs, etc.
    pub regen_bonus: f64,
}

impl ManaPool {
    /// Create a full mana pool with the given capacity and base regen rate.
    pub fn new(maximum: f64, regen_per_sec: f64) -> Self {
        Self {
            current: maximum,
            maximum,
            regen_per_sec,
            regen_bonus: 0.0,
        }
    }

    /// Calculate effective mana regeneration per second.
    ///
    /// Formula: `base_regen + (intelligence * 0.5) + terrain_bonus`
    pub fn effective_regen(&self, intelligence: f64) -> f64 {
        self.regen_per_sec + (intelligence * 0.5) + self.regen_bonus
    }

    /// Tick mana regeneration for `dt` seconds.  Clamps to maximum.
    pub fn tick(&mut self, dt: f64, intelligence: f64) {
        let regen = self.effective_regen(intelligence) * dt;
        self.current = (self.current + regen).min(self.maximum);
    }

    /// Attempt to spend `cost` mana.  Returns `true` if successful,
    /// `false` if insufficient mana (pool unchanged).
    pub fn spend(&mut self, cost: f64) -> bool {
        if self.current >= cost {
            self.current -= cost;
            true
        } else {
            false
        }
    }

    /// Fraction of mana remaining (0.0 to 1.0).
    pub fn fraction(&self) -> f64 {
        if self.maximum <= 0.0 {
            return 0.0;
        }
        (self.current / self.maximum).clamp(0.0, 1.0)
    }
}

// ---------------------------------------------------------------------------
// Spells
// ---------------------------------------------------------------------------

/// A castable spell belonging to a faction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spell {
    /// Unique identifier (e.g. "demon_fireball").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Mana cost to cast.
    pub mana_cost: f64,
    /// Base cast time in seconds (0.0 = instant).
    pub cast_time_secs: f64,
    /// Cooldown in seconds before the spell can be cast again (0.0 = none).
    pub cooldown_secs: f64,
    /// Base damage dealt on hit.
    pub damage: f64,
    /// Element: "fire", "frost", "shadow", "holy", "chaos", "nature", "physical".
    pub damage_type: String,
    /// Area-of-effect radius (0.0 = single target).
    pub aoe_radius: f64,
    /// Maximum casting range in game units.
    pub range: f64,
    /// Flavour text / tooltip.
    pub description: String,
    /// What this spell can evolve into at higher tiers, if any.
    pub evolution_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Casting & Pushback
// ---------------------------------------------------------------------------

/// Tracks the progress of a spell being cast, including WoW-style pushback.
///
/// When a caster takes damage while casting, the remaining cast time increases
/// by 10% of the spell's maximum cast time.  This stacks up to `max_pushbacks`
/// times (default 5), after which the caster is immune to further pushback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastingState {
    /// The spell currently being cast (`None` = idle).
    pub casting_spell: Option<String>,
    /// Progress from 0.0 (just started) to 1.0 (cast complete).
    pub cast_progress: f64,
    /// Number of pushbacks suffered during this cast.
    pub pushback_count: u32,
    /// Maximum pushbacks before immunity (typically 5).
    pub max_pushbacks: u32,
}

impl CastingState {
    /// Create a new casting state for a spell.
    pub fn begin(spell_id: &str) -> Self {
        Self {
            casting_spell: Some(spell_id.to_string()),
            cast_progress: 0.0,
            pushback_count: 0,
            max_pushbacks: 5,
        }
    }

    /// Create an idle (not casting) state.
    pub fn idle() -> Self {
        Self {
            casting_spell: None,
            cast_progress: 0.0,
            pushback_count: 0,
            max_pushbacks: 5,
        }
    }

    /// Apply WoW-style pushback.
    ///
    /// Each hit while casting reduces progress by 10% (of full bar).
    /// After `max_pushbacks` hits the caster becomes immune to further
    /// pushback for the remainder of this cast.
    ///
    /// Returns the updated state.
    pub fn apply_pushback(&mut self) -> &mut Self {
        if self.casting_spell.is_none() {
            return self;
        }
        if self.pushback_count >= self.max_pushbacks {
            return self; // immune
        }
        self.pushback_count += 1;
        // Reduce progress by 10% of the total bar (clamped to 0)
        self.cast_progress = (self.cast_progress - 0.10).max(0.0);
        self
    }

    /// Advance the cast by `dt / total_cast_time` (fraction of full bar).
    /// Returns `true` when the cast reaches 1.0 (complete).
    pub fn advance(&mut self, dt: f64, total_cast_time: f64) -> bool {
        if self.casting_spell.is_none() || total_cast_time <= 0.0 {
            return false;
        }
        self.cast_progress += dt / total_cast_time;
        self.cast_progress >= 1.0
    }

    /// Whether the unit is currently casting.
    pub fn is_casting(&self) -> bool {
        self.casting_spell.is_some()
    }
}

// ---------------------------------------------------------------------------
// Unit Tier / Star System
// ---------------------------------------------------------------------------

/// Unit progression tier, from 1-star (basic summon) through E5 (maximum
/// enlightenment).  Mirrors the Idle Heroes progression path.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnitTier {
    Star1 = 1,
    Star2 = 2,
    Star3 = 3,
    Star4 = 4,
    Star5 = 5,
    Star6 = 6,
    Star7 = 7,
    Star8 = 8,
    Star9 = 9,
    Star10 = 10,
    E1 = 11,
    E2 = 12,
    E3 = 13,
    E4 = 14,
    E5 = 15,
}

impl UnitTier {
    /// Parse a tier from its integer representation (1..=15).
    pub fn from_value(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Star1),
            2 => Some(Self::Star2),
            3 => Some(Self::Star3),
            4 => Some(Self::Star4),
            5 => Some(Self::Star5),
            6 => Some(Self::Star6),
            7 => Some(Self::Star7),
            8 => Some(Self::Star8),
            9 => Some(Self::Star9),
            10 => Some(Self::Star10),
            11 => Some(Self::E1),
            12 => Some(Self::E2),
            13 => Some(Self::E3),
            14 => Some(Self::E4),
            15 => Some(Self::E5),
            _ => None,
        }
    }

    /// Human-readable label (e.g. "5-Star", "E3").
    pub fn label(&self) -> &'static str {
        match self {
            Self::Star1 => "1-Star",
            Self::Star2 => "2-Star",
            Self::Star3 => "3-Star",
            Self::Star4 => "4-Star",
            Self::Star5 => "5-Star",
            Self::Star6 => "6-Star",
            Self::Star7 => "7-Star",
            Self::Star8 => "8-Star",
            Self::Star9 => "9-Star",
            Self::Star10 => "10-Star",
            Self::E1 => "E1",
            Self::E2 => "E2",
            Self::E3 => "E3",
            Self::E4 => "E4",
            Self::E5 => "E5",
        }
    }

    /// Numeric value for database storage and ordering.
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// All tiers in ascending order.
    pub fn all() -> &'static [UnitTier] {
        &[
            Self::Star1, Self::Star2, Self::Star3, Self::Star4, Self::Star5,
            Self::Star6, Self::Star7, Self::Star8, Self::Star9, Self::Star10,
            Self::E1, Self::E2, Self::E3, Self::E4, Self::E5,
        ]
    }
}

// ---------------------------------------------------------------------------
// Tier Requirements
// ---------------------------------------------------------------------------

/// Materials needed to advance a unit to `target_tier`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierRequirement {
    /// The tier being promoted TO.
    pub target_tier: UnitTier,
    /// Copies of the same unit required.
    pub copies_needed: u32,
    /// Faction fodder units required.
    pub fodder_needed: u32,
    /// Minimum tier that each fodder unit must be.
    pub fodder_tier: UnitTier,
    /// Sacrifice units required (E1-E5 promotions only).
    pub sacrifice_needed: u32,
    /// Minimum tier of each sacrifice unit.
    pub sacrifice_tier: UnitTier,
}

/// Look up the promotion requirements for a given target tier.
///
/// ```text
/// 1 -> 2:   1 copy
/// 2 -> 3:   1 copy
/// 3 -> 4:   2 copies + 1 fodder (3-star)
/// 4 -> 5:   2 copies + 2 fodder (4-star)
/// 5 -> 6:   2x 5-star copies + 1x 5-star faction fodder
/// 6 -> 7:   4x 5-star fodder
/// 7 -> 8:   3x 5-star fodder
/// 8 -> 9:   2x 5-star fodder
/// 9 -> 10:  1x 9-star fodder + 2x 5-star fodder  (encoded as fodder=3, sacrifice=0)
/// 10 -> E1: sacrifice 1x 9-star
/// E1 -> E2: sacrifice 1x 10-star
/// E2 -> E3: sacrifice 1x 10-star
/// E3 -> E4: sacrifice 1x 10-star
/// E4 -> E5: sacrifice 1x 10-star
/// ```
pub fn tier_requirements(target: UnitTier) -> Option<TierRequirement> {
    let req = match target {
        UnitTier::Star1 => return None, // starting tier, no promotion needed
        UnitTier::Star2 => TierRequirement {
            target_tier: target,
            copies_needed: 1,
            fodder_needed: 0,
            fodder_tier: UnitTier::Star1,
            sacrifice_needed: 0,
            sacrifice_tier: UnitTier::Star1,
        },
        UnitTier::Star3 => TierRequirement {
            target_tier: target,
            copies_needed: 1,
            fodder_needed: 0,
            fodder_tier: UnitTier::Star1,
            sacrifice_needed: 0,
            sacrifice_tier: UnitTier::Star1,
        },
        UnitTier::Star4 => TierRequirement {
            target_tier: target,
            copies_needed: 2,
            fodder_needed: 1,
            fodder_tier: UnitTier::Star3,
            sacrifice_needed: 0,
            sacrifice_tier: UnitTier::Star1,
        },
        UnitTier::Star5 => TierRequirement {
            target_tier: target,
            copies_needed: 2,
            fodder_needed: 2,
            fodder_tier: UnitTier::Star4,
            sacrifice_needed: 0,
            sacrifice_tier: UnitTier::Star1,
        },
        UnitTier::Star6 => TierRequirement {
            target_tier: target,
            copies_needed: 2,
            fodder_needed: 1,
            fodder_tier: UnitTier::Star5,
            sacrifice_needed: 0,
            sacrifice_tier: UnitTier::Star1,
        },
        UnitTier::Star7 => TierRequirement {
            target_tier: target,
            copies_needed: 0,
            fodder_needed: 4,
            fodder_tier: UnitTier::Star5,
            sacrifice_needed: 0,
            sacrifice_tier: UnitTier::Star1,
        },
        UnitTier::Star8 => TierRequirement {
            target_tier: target,
            copies_needed: 0,
            fodder_needed: 3,
            fodder_tier: UnitTier::Star5,
            sacrifice_needed: 0,
            sacrifice_tier: UnitTier::Star1,
        },
        UnitTier::Star9 => TierRequirement {
            target_tier: target,
            copies_needed: 0,
            fodder_needed: 2,
            fodder_tier: UnitTier::Star5,
            sacrifice_needed: 0,
            sacrifice_tier: UnitTier::Star1,
        },
        UnitTier::Star10 => TierRequirement {
            target_tier: target,
            copies_needed: 0,
            fodder_needed: 2,
            fodder_tier: UnitTier::Star5,
            sacrifice_needed: 1,
            sacrifice_tier: UnitTier::Star9,
        },
        UnitTier::E1 => TierRequirement {
            target_tier: target,
            copies_needed: 0,
            fodder_needed: 0,
            fodder_tier: UnitTier::Star1,
            sacrifice_needed: 1,
            sacrifice_tier: UnitTier::Star9,
        },
        UnitTier::E2 => TierRequirement {
            target_tier: target,
            copies_needed: 0,
            fodder_needed: 0,
            fodder_tier: UnitTier::Star1,
            sacrifice_needed: 1,
            sacrifice_tier: UnitTier::Star10,
        },
        UnitTier::E3 => TierRequirement {
            target_tier: target,
            copies_needed: 0,
            fodder_needed: 0,
            fodder_tier: UnitTier::Star1,
            sacrifice_needed: 1,
            sacrifice_tier: UnitTier::Star10,
        },
        UnitTier::E4 => TierRequirement {
            target_tier: target,
            copies_needed: 0,
            fodder_needed: 0,
            fodder_tier: UnitTier::Star1,
            sacrifice_needed: 1,
            sacrifice_tier: UnitTier::Star10,
        },
        UnitTier::E5 => TierRequirement {
            target_tier: target,
            copies_needed: 0,
            fodder_needed: 0,
            fodder_tier: UnitTier::Star1,
            sacrifice_needed: 1,
            sacrifice_tier: UnitTier::Star10,
        },
    };
    Some(req)
}

// ---------------------------------------------------------------------------
// Tier Stat Bonuses
// ---------------------------------------------------------------------------

/// Multiplicative stat bonuses and ability unlocks granted by a unit's tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierBonus {
    /// HP multiplier (1.0 = base).
    pub hp_multiplier: f64,
    /// Attack multiplier.
    pub attack_multiplier: f64,
    /// Armor multiplier.
    pub armor_multiplier: f64,
    /// Speed multiplier.
    pub speed_multiplier: f64,
    /// Ability unlocked at this tier, if any.
    pub ability_unlock: Option<String>,
}

/// Look up the stat multipliers and ability unlock for a given tier.
///
/// ```text
/// 1-Star:  1.00x
/// 2-Star:  1.10x
/// 3-Star:  1.25x
/// 4-Star:  1.45x
/// 5-Star:  1.70x  — unlock passive ability
/// 6-Star:  2.00x  — unlock active ability
/// 7-Star:  2.30x
/// 8-Star:  2.65x
/// 9-Star:  3.10x  — unlock ultimate ability
/// 10-Star: 3.70x
/// E1:      4.50x  — unlock Ascension passive
/// E2:      5.50x
/// E3:      6.80x
/// E4:      8.50x
/// E5:     10.00x  — unlock Transcendence form
/// ```
pub fn tier_bonus(tier: UnitTier) -> TierBonus {
    let (mult, ability) = match tier {
        UnitTier::Star1 => (1.0, None),
        UnitTier::Star2 => (1.1, None),
        UnitTier::Star3 => (1.25, None),
        UnitTier::Star4 => (1.45, None),
        UnitTier::Star5 => (1.7, Some("Passive Ability")),
        UnitTier::Star6 => (2.0, Some("Active Ability")),
        UnitTier::Star7 => (2.3, None),
        UnitTier::Star8 => (2.65, None),
        UnitTier::Star9 => (3.1, Some("Ultimate Ability")),
        UnitTier::Star10 => (3.7, None),
        UnitTier::E1 => (4.5, Some("Ascension Passive")),
        UnitTier::E2 => (5.5, None),
        UnitTier::E3 => (6.8, None),
        UnitTier::E4 => (8.5, None),
        UnitTier::E5 => (10.0, Some("Transcendence Form")),
    };
    TierBonus {
        hp_multiplier: mult,
        attack_multiplier: mult,
        armor_multiplier: mult,
        speed_multiplier: mult,
        ability_unlock: ability.map(|s| s.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Faction Progression Names
// ---------------------------------------------------------------------------

/// Themed tier names per faction.
///
/// Stars 1-5 get unique names, 6-10 append a roman numeral to the faction's
/// "ascension" keyword, and E1-E5 use "Enlightened I-V".
pub fn faction_tier_names(faction: &str) -> Vec<String> {
    let (base_names, ascension_word) = match faction.to_lowercase().as_str() {
        "insect" | "insects" => (
            vec!["Larva", "Pupa", "Nymph", "Adult", "Queen"],
            "Metamorphosis",
        ),
        "demon" | "demons" => (
            vec!["Lesser", "Minor", "Greater", "Archon", "Overlord"],
            "Ascension",
        ),
        "undead" => (
            vec!["Remnant", "Shade", "Revenant", "Lich", "Death Knight"],
            "Transcendence",
        ),
        "human" | "humans" => (
            vec!["Recruit", "Veteran", "Elite", "Champion", "Legend"],
            "Ascension",
        ),
        _ => (
            vec!["Tier I", "Tier II", "Tier III", "Tier IV", "Tier V"],
            "Advanced",
        ),
    };

    let roman = ["I", "II", "III", "IV", "V"];

    let mut names: Vec<String> = Vec::with_capacity(15);

    // Stars 1-5
    for name in &base_names {
        names.push(name.to_string());
    }

    // Stars 6-10
    for r in &roman {
        names.push(format!("{ascension_word} {r}"));
    }

    // E1-E5
    for r in &roman {
        names.push(format!("Enlightened {r}"));
    }

    names
}

// ---------------------------------------------------------------------------
// Faction Spell Rosters
// ---------------------------------------------------------------------------

/// Build the default spell list for a faction.
pub fn faction_spells(faction: &str) -> Vec<Spell> {
    match faction.to_lowercase().as_str() {
        "insect" | "insects" => insect_spells(),
        "demon" | "demons" => demon_spells(),
        "undead" => undead_spells(),
        "human" | "humans" => human_spells(),
        _ => Vec::new(),
    }
}

fn insect_spells() -> Vec<Spell> {
    vec![
        Spell {
            id: "insect_acid_spray".into(),
            name: "Acid Spray".into(),
            mana_cost: 5.0,
            cast_time_secs: 0.8,
            cooldown_secs: 0.0,
            damage: 25.0,
            damage_type: "nature".into(),
            aoe_radius: 0.0,
            range: 8.0,
            description: "Sprays corrosive acid that deals nature damage and applies a corrosion DOT.".into(),
            evolution_path: Some("insect_corrosive_deluge".into()),
        },
        Spell {
            id: "insect_pheromone_burst".into(),
            name: "Pheromone Burst".into(),
            mana_cost: 15.0,
            cast_time_secs: 1.2,
            cooldown_secs: 12.0,
            damage: 0.0,
            damage_type: "nature".into(),
            aoe_radius: 6.0,
            range: 0.0,
            description: "Releases pheromones that grant nearby allied insects +20% attack speed for 10 seconds.".into(),
            evolution_path: None,
        },
        Spell {
            id: "insect_swarm_cloud".into(),
            name: "Swarm Cloud".into(),
            mana_cost: 25.0,
            cast_time_secs: 2.0,
            cooldown_secs: 18.0,
            damage: 15.0,
            damage_type: "nature".into(),
            aoe_radius: 5.0,
            range: 10.0,
            description: "Summons a cloud of swarming insects that blinds and slows enemies in the area.".into(),
            evolution_path: None,
        },
        Spell {
            id: "insect_hive_mind".into(),
            name: "Hive Mind".into(),
            mana_cost: 40.0,
            cast_time_secs: 3.0,
            cooldown_secs: 45.0,
            damage: 0.0,
            damage_type: "nature".into(),
            aoe_radius: 0.0,
            range: 0.0,
            description: "Links all allied insects into a hive consciousness, granting +30% to all stats for 15 seconds.".into(),
            evolution_path: None,
        },
    ]
}

fn demon_spells() -> Vec<Spell> {
    vec![
        Spell {
            id: "demon_fireball".into(),
            name: "Fireball".into(),
            mana_cost: 8.0,
            cast_time_secs: 1.0,
            cooldown_secs: 0.0,
            damage: 40.0,
            damage_type: "fire".into(),
            aoe_radius: 0.0,
            range: 12.0,
            description: "Hurls a ball of demonic fire at the target.".into(),
            evolution_path: Some("demon_flaming_ball".into()),
        },
        Spell {
            id: "demon_shadow_bolt".into(),
            name: "Shadow Bolt".into(),
            mana_cost: 10.0,
            cast_time_secs: 1.5,
            cooldown_secs: 0.0,
            damage: 55.0,
            damage_type: "shadow".into(),
            aoe_radius: 0.0,
            range: 14.0,
            description: "Launches a bolt of shadow energy that strikes for heavy shadow damage.".into(),
            evolution_path: None,
        },
        Spell {
            id: "demon_hellfire".into(),
            name: "Hellfire".into(),
            mana_cost: 30.0,
            cast_time_secs: 2.5,
            cooldown_secs: 20.0,
            damage: 80.0,
            damage_type: "fire".into(),
            aoe_radius: 7.0,
            range: 0.0,
            description: "Engulfs the area in hellfire, dealing massive AoE fire damage. The caster also takes 10% of the damage.".into(),
            evolution_path: None,
        },
        Spell {
            id: "demon_doom".into(),
            name: "Doom".into(),
            mana_cost: 50.0,
            cast_time_secs: 5.0,
            cooldown_secs: 60.0,
            damage: 300.0,
            damage_type: "shadow".into(),
            aoe_radius: 0.0,
            range: 10.0,
            description: "Places a curse of Doom on the target. After 15 seconds, the target suffers massive shadow damage.".into(),
            evolution_path: None,
        },
    ]
}

fn undead_spells() -> Vec<Spell> {
    vec![
        Spell {
            id: "undead_frost_bolt".into(),
            name: "Frost Bolt".into(),
            mana_cost: 8.0,
            cast_time_secs: 1.0,
            cooldown_secs: 0.0,
            damage: 35.0,
            damage_type: "frost".into(),
            aoe_radius: 0.0,
            range: 12.0,
            description: "Fires a bolt of frost that damages and slows the target by 30%.".into(),
            evolution_path: Some("undead_glacial_spike".into()),
        },
        Spell {
            id: "undead_death_coil".into(),
            name: "Death Coil".into(),
            mana_cost: 12.0,
            cast_time_secs: 0.0, // instant
            cooldown_secs: 6.0,
            damage: 45.0,
            damage_type: "shadow".into(),
            aoe_radius: 0.0,
            range: 10.0,
            description: "Sends a coil of death energy. Damages living targets or heals undead allies.".into(),
            evolution_path: None,
        },
        Spell {
            id: "undead_raise_dead".into(),
            name: "Raise Dead".into(),
            mana_cost: 20.0,
            cast_time_secs: 2.0,
            cooldown_secs: 30.0,
            damage: 0.0,
            damage_type: "shadow".into(),
            aoe_radius: 4.0,
            range: 6.0,
            description: "Raises 2 skeleton warriors from nearby corpses to fight for you.".into(),
            evolution_path: None,
        },
        Spell {
            id: "undead_plague".into(),
            name: "Plague".into(),
            mana_cost: 35.0,
            cast_time_secs: 3.0,
            cooldown_secs: 25.0,
            damage: 20.0,
            damage_type: "shadow".into(),
            aoe_radius: 6.0,
            range: 10.0,
            description: "Spreads a virulent plague in an area. Deals shadow DOT that spreads to nearby enemies when a victim dies.".into(),
            evolution_path: None,
        },
    ]
}

fn human_spells() -> Vec<Spell> {
    vec![
        Spell {
            id: "human_holy_light".into(),
            name: "Holy Light".into(),
            mana_cost: 10.0,
            cast_time_secs: 1.5,
            cooldown_secs: 0.0,
            damage: 50.0, // heals allies or damages undead
            damage_type: "holy".into(),
            aoe_radius: 0.0,
            range: 12.0,
            description: "Channels holy energy. Heals a friendly target or deals holy damage to undead.".into(),
            evolution_path: Some("human_divine_radiance".into()),
        },
        Spell {
            id: "human_blizzard".into(),
            name: "Blizzard".into(),
            mana_cost: 25.0,
            cast_time_secs: 2.0,
            cooldown_secs: 15.0,
            damage: 30.0,
            damage_type: "frost".into(),
            aoe_radius: 8.0,
            range: 14.0,
            description: "Calls down a blizzard in the target area, dealing frost damage over time and slowing enemies.".into(),
            evolution_path: None,
        },
        Spell {
            id: "human_thunder_clap".into(),
            name: "Thunder Clap".into(),
            mana_cost: 15.0,
            cast_time_secs: 0.0, // instant
            cooldown_secs: 8.0,
            damage: 35.0,
            damage_type: "physical".into(),
            aoe_radius: 5.0,
            range: 0.0,
            description: "Slams the ground, sending out a shockwave that damages and slows nearby enemies.".into(),
            evolution_path: None,
        },
        Spell {
            id: "human_resurrection".into(),
            name: "Resurrection".into(),
            mana_cost: 60.0,
            cast_time_secs: 5.0,
            cooldown_secs: 120.0,
            damage: 0.0,
            damage_type: "holy".into(),
            aoe_radius: 0.0,
            range: 8.0,
            description: "Revives a fallen allied unit at 50% HP. Long cast time and extreme mana cost.".into(),
            evolution_path: None,
        },
    ]
}

/// Demon Fireball evolution chain:
/// Fireball -> Flaming Ball -> Fire Beam -> Inferno
pub fn demon_fireball_evolution() -> Vec<Spell> {
    vec![
        Spell {
            id: "demon_fireball".into(),
            name: "Fireball".into(),
            mana_cost: 8.0,
            cast_time_secs: 1.0,
            cooldown_secs: 0.0,
            damage: 40.0,
            damage_type: "fire".into(),
            aoe_radius: 0.0,
            range: 12.0,
            description: "Basic demonic fireball.".into(),
            evolution_path: Some("demon_flaming_ball".into()),
        },
        Spell {
            id: "demon_flaming_ball".into(),
            name: "Flaming Ball".into(),
            mana_cost: 14.0,
            cast_time_secs: 1.2,
            cooldown_secs: 0.0,
            damage: 55.0,
            damage_type: "fire".into(),
            aoe_radius: 3.0,
            range: 12.0,
            description: "Evolved fireball that explodes in a small AoE on impact.".into(),
            evolution_path: Some("demon_fire_beam".into()),
        },
        Spell {
            id: "demon_fire_beam".into(),
            name: "Fire Beam".into(),
            mana_cost: 22.0,
            cast_time_secs: 0.0, // channeled, not cast-time
            cooldown_secs: 8.0,
            damage: 20.0, // per tick
            damage_type: "fire".into(),
            aoe_radius: 0.0,
            range: 14.0,
            description: "Channelled beam of fire. Deals damage every 0.5 seconds while maintained.".into(),
            evolution_path: Some("demon_inferno".into()),
        },
        Spell {
            id: "demon_inferno".into(),
            name: "Inferno".into(),
            mana_cost: 45.0,
            cast_time_secs: 3.0,
            cooldown_secs: 45.0,
            damage: 150.0,
            damage_type: "fire".into(),
            aoe_radius: 10.0,
            range: 14.0,
            description: "Ultimate fire spell. Rains infernal fire across a massive area.".into(),
            evolution_path: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Default Mana Pools by Unit Archetype
// ---------------------------------------------------------------------------

/// Return the default mana pool for a unit archetype.
///
/// Recognised types: "caster", "healer", "hybrid", "support", "tank", "melee".
/// Non-caster types get a minimal pool (or none).
pub fn default_mana_pool(unit_type: &str) -> ManaPool {
    match unit_type.to_lowercase().as_str() {
        "caster" => ManaPool::new(100.0, 2.0),
        "healer" => ManaPool::new(120.0, 2.5),
        "hybrid" => ManaPool::new(60.0, 1.2),
        "support" => ManaPool::new(80.0, 1.8),
        "tank" => ManaPool::new(30.0, 0.5),
        "melee" => ManaPool::new(20.0, 0.3),
        _ => ManaPool::new(50.0, 1.0),
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// Get the stat bonuses and ability unlock for a given tier value (1-15).
#[tauri::command]
pub async fn swarm_get_tier_info(tier: u8) -> AppResult<TierBonus> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_progression", "game_progression", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_progression", "game_progression");
    crate::synapse_fabric::synapse_session_push("swarm_progression", "game_progression", "swarm_get_tier_info called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_progression", "info", "swarm_progression active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_progression", "level_up", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"tier": tier}));
    let t = UnitTier::from_value(tier).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_TIER",
            format!("Tier value {tier} is out of range (valid: 1-15)"),
        )
    })?;
    Ok(tier_bonus(t))
}

/// Get the promotion requirements for a target tier value (2-15).
/// Tier 1 has no requirements (it is the starting tier).
#[tauri::command]
pub async fn swarm_get_tier_requirements(target: u8) -> AppResult<TierRequirement> {
    let t = UnitTier::from_value(target).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_TIER",
            format!("Tier value {target} is out of range (valid: 1-15)"),
        )
    })?;
    tier_requirements(t).ok_or_else(|| {
        ImpForgeError::validation(
            "NO_REQUIREMENTS",
            "Tier 1 is the starting tier and has no promotion requirements.",
        )
    })
}

/// Get the themed tier names for a faction.
///
/// Valid factions: "insect", "demon", "undead", "human".
/// Returns a 15-element Vec (one name per tier, Star1 through E5).
#[tauri::command]
pub async fn swarm_tier_names(faction: String) -> AppResult<Vec<String>> {
    let names = faction_tier_names(&faction);
    Ok(names)
}

/// Get the default mana pool for a unit archetype.
///
/// Valid types: "caster", "healer", "hybrid", "support", "tank", "melee".
#[tauri::command]
pub async fn swarm_get_mana_pool(unit_type: String) -> AppResult<ManaPool> {
    Ok(default_mana_pool(&unit_type))
}

/// Get all spells for a faction.
///
/// Valid factions: "insect", "demon", "undead", "human".
#[tauri::command]
pub async fn swarm_get_spells(faction: String) -> AppResult<Vec<Spell>> {
    let spells = faction_spells(&faction);
    if spells.is_empty() {
        return Err(ImpForgeError::validation(
            "UNKNOWN_FACTION",
            format!("Unknown faction '{faction}'. Valid: insect, demon, undead, human."),
        ));
    }
    Ok(spells)
}

/// Attempt to cast a spell, deducting mana from the unit's pool.
///
/// Returns a JSON object with the cast result, remaining mana, and spell details.
#[tauri::command]
pub async fn swarm_cast_spell(
    unit_id: String,
    spell_id: String,
    current_mana: f64,
    max_mana: f64,
) -> AppResult<serde_json::Value> {
    // Look up spell across all factions
    let spell = all_spells()
        .into_iter()
        .find(|s| s.id == spell_id)
        .ok_or_else(|| {
            ImpForgeError::validation(
                "UNKNOWN_SPELL",
                format!("Spell '{spell_id}' not found in any faction roster."),
            )
        })?;

    if current_mana < spell.mana_cost {
        return Ok(serde_json::json!({
            "success": false,
            "reason": "insufficient_mana",
            "unit_id": unit_id,
            "spell": spell.name,
            "mana_cost": spell.mana_cost,
            "current_mana": current_mana,
            "deficit": spell.mana_cost - current_mana,
        }));
    }

    let remaining = current_mana - spell.mana_cost;

    Ok(serde_json::json!({
        "success": true,
        "unit_id": unit_id,
        "spell": spell.name,
        "spell_id": spell.id,
        "mana_cost": spell.mana_cost,
        "remaining_mana": remaining,
        "max_mana": max_mana,
        "mana_fraction": if max_mana > 0.0 { remaining / max_mana } else { 0.0 },
        "cast_time_secs": spell.cast_time_secs,
        "damage": spell.damage,
        "damage_type": spell.damage_type,
        "aoe_radius": spell.aoe_radius,
    }))
}

/// Simulate WoW-style cast pushback.
///
/// Accepts a `CastingState` and returns the updated state after taking a hit.
#[tauri::command]
pub async fn swarm_simulate_pushback(cast_state: CastingState) -> AppResult<CastingState> {
    let mut state = cast_state;
    state.apply_pushback();
    Ok(state)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Collect every spell from every faction into one list.
fn all_spells() -> Vec<Spell> {
    let mut spells = Vec::with_capacity(16);
    spells.extend(insect_spells());
    spells.extend(demon_spells());
    spells.extend(undead_spells());
    spells.extend(human_spells());
    spells
}

// ---------------------------------------------------------------------------
// Additional Tauri Commands — wiring internal helpers
// ---------------------------------------------------------------------------

/// Get mana pool status after ticking and spending.
#[tauri::command]
pub async fn swarm_mana_pool_tick(
    maximum: f64,
    regen_per_sec: f64,
    intelligence: f64,
    dt: f64,
    spend_cost: f64,
) -> AppResult<serde_json::Value> {
    let mut pool = ManaPool::new(maximum, regen_per_sec);
    pool.current = maximum * 0.5; // start at half
    pool.tick(dt, intelligence);
    let eff_regen = pool.effective_regen(intelligence);
    let spent = pool.spend(spend_cost);
    Ok(serde_json::json!({
        "current": pool.current,
        "maximum": pool.maximum,
        "effective_regen": eff_regen,
        "fraction": pool.fraction(),
        "spend_success": spent,
    }))
}

/// Begin casting a spell and optionally advance/check state.
#[tauri::command]
pub async fn swarm_casting_state_info(
    spell_id: String,
    total_cast_time: f64,
    advance_dt: f64,
) -> AppResult<serde_json::Value> {
    let mut state = CastingState::begin(&spell_id);
    let complete = state.advance(advance_dt, total_cast_time);
    let casting = state.is_casting();
    Ok(serde_json::json!({
        "casting_spell": state.casting_spell,
        "progress": state.cast_progress,
        "is_casting": casting,
        "complete": complete,
    }))
}

/// Get an idle casting state.
#[tauri::command]
pub async fn swarm_casting_idle() -> AppResult<CastingState> {
    Ok(CastingState::idle())
}

/// List all unit tiers with labels and values.
#[tauri::command]
pub async fn swarm_unit_tiers() -> AppResult<Vec<serde_json::Value>> {
    Ok(UnitTier::all()
        .iter()
        .map(|t| {
            serde_json::json!({
                "value": t.as_u8(),
                "label": t.label(),
            })
        })
        .collect())
}

/// Get the demon fireball spell evolution chain.
#[tauri::command]
pub async fn swarm_demon_fireball_chain() -> AppResult<Vec<Spell>> {
    Ok(demon_fireball_evolution())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;


    // -- ManaPool --

    #[test]
    fn test_mana_pool_new() {
        let pool = ManaPool::new(100.0, 2.0);
        assert_eq!(pool.current, 100.0);
        assert_eq!(pool.maximum, 100.0);
        assert_eq!(pool.regen_per_sec, 2.0);
        assert_eq!(pool.regen_bonus, 0.0);
    }

    #[test]
    fn test_mana_pool_spend_success() {
        let mut pool = ManaPool::new(100.0, 2.0);
        assert!(pool.spend(30.0));
        assert!((pool.current - 70.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mana_pool_spend_insufficient() {
        let mut pool = ManaPool::new(100.0, 2.0);
        pool.current = 5.0;
        assert!(!pool.spend(10.0));
        assert!((pool.current - 5.0).abs() < f64::EPSILON); // unchanged
    }

    #[test]
    fn test_mana_pool_effective_regen() {
        let mut pool = ManaPool::new(100.0, 2.0);
        pool.regen_bonus = 1.0;
        // regen = 2.0 + (10.0 * 0.5) + 1.0 = 8.0
        let regen = pool.effective_regen(10.0);
        assert!((regen - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mana_pool_tick_clamps() {
        let mut pool = ManaPool::new(100.0, 50.0);
        pool.tick(10.0, 0.0); // 100 + 50*10 = 600, clamp to 100
        assert!((pool.current - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mana_pool_tick_regenerates() {
        let mut pool = ManaPool::new(100.0, 2.0);
        pool.current = 50.0;
        pool.tick(5.0, 0.0); // 50 + 2*5 = 60
        assert!((pool.current - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mana_pool_fraction() {
        let mut pool = ManaPool::new(200.0, 1.0);
        pool.current = 50.0;
        assert!((pool.fraction() - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mana_pool_fraction_zero_max() {
        let pool = ManaPool {
            current: 10.0,
            maximum: 0.0,
            regen_per_sec: 0.0,
            regen_bonus: 0.0,
        };
        assert!((pool.fraction() - 0.0).abs() < f64::EPSILON);
    }

    // -- CastingState --

    #[test]
    fn test_casting_state_begin() {
        let state = CastingState::begin("demon_fireball");
        assert_eq!(state.casting_spell, Some("demon_fireball".to_string()));
        assert_eq!(state.cast_progress, 0.0);
        assert_eq!(state.pushback_count, 0);
    }

    #[test]
    fn test_casting_state_idle() {
        let state = CastingState::idle();
        assert!(!state.is_casting());
        assert!(state.casting_spell.is_none());
    }

    #[test]
    fn test_pushback_reduces_progress() {
        let mut state = CastingState::begin("spell");
        state.cast_progress = 0.5;
        state.apply_pushback();
        assert!((state.cast_progress - 0.4).abs() < f64::EPSILON);
        assert_eq!(state.pushback_count, 1);
    }

    #[test]
    fn test_pushback_clamps_to_zero() {
        let mut state = CastingState::begin("spell");
        state.cast_progress = 0.05;
        state.apply_pushback();
        assert!((state.cast_progress - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pushback_max_immunity() {
        let mut state = CastingState::begin("spell");
        state.cast_progress = 0.8;
        state.max_pushbacks = 2;
        state.apply_pushback(); // 0.8 -> 0.7, count=1
        state.apply_pushback(); // 0.7 -> 0.6, count=2
        state.apply_pushback(); // immune, stays 0.6
        assert_eq!(state.pushback_count, 2);
        assert!((state.cast_progress - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pushback_on_idle_is_noop() {
        let mut state = CastingState::idle();
        state.apply_pushback();
        assert_eq!(state.pushback_count, 0);
    }

    #[test]
    fn test_advance_completes() {
        let mut state = CastingState::begin("spell");
        let done = state.advance(1.0, 1.0); // 1s of 1s cast = 100%
        assert!(done);
        assert!(state.cast_progress >= 1.0);
    }

    #[test]
    fn test_advance_partial() {
        let mut state = CastingState::begin("spell");
        let done = state.advance(0.5, 2.0); // 0.5s of 2s = 25%
        assert!(!done);
        assert!((state.cast_progress - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_advance_idle_noop() {
        let mut state = CastingState::idle();
        let done = state.advance(1.0, 1.0);
        assert!(!done);
    }

    // -- UnitTier --

    #[test]
    fn test_unit_tier_from_value_valid() {
        assert_eq!(UnitTier::from_value(1), Some(UnitTier::Star1));
        assert_eq!(UnitTier::from_value(10), Some(UnitTier::Star10));
        assert_eq!(UnitTier::from_value(15), Some(UnitTier::E5));
    }

    #[test]
    fn test_unit_tier_from_value_invalid() {
        assert_eq!(UnitTier::from_value(0), None);
        assert_eq!(UnitTier::from_value(16), None);
    }

    #[test]
    fn test_unit_tier_label() {
        assert_eq!(UnitTier::Star5.label(), "5-Star");
        assert_eq!(UnitTier::E3.label(), "E3");
    }

    #[test]
    fn test_unit_tier_ordering() {
        assert!(UnitTier::Star1 < UnitTier::Star5);
        assert!(UnitTier::Star10 < UnitTier::E1);
        assert!(UnitTier::E4 < UnitTier::E5);
    }

    #[test]
    fn test_unit_tier_all_count() {
        assert_eq!(UnitTier::all().len(), 15);
    }

    #[test]
    fn test_unit_tier_roundtrip() {
        for t in UnitTier::all() {
            let v = t.as_u8();
            assert_eq!(UnitTier::from_value(v), Some(*t));
        }
    }

    // -- TierRequirement --

    #[test]
    fn test_tier_req_star1_is_none() {
        assert!(tier_requirements(UnitTier::Star1).is_none());
    }

    #[test]
    fn test_tier_req_star2() {
        let req = tier_requirements(UnitTier::Star2).expect("should have reqs");
        assert_eq!(req.copies_needed, 1);
        assert_eq!(req.fodder_needed, 0);
    }

    #[test]
    fn test_tier_req_star4() {
        let req = tier_requirements(UnitTier::Star4).expect("should have reqs");
        assert_eq!(req.copies_needed, 2);
        assert_eq!(req.fodder_needed, 1);
        assert_eq!(req.fodder_tier, UnitTier::Star3);
    }

    #[test]
    fn test_tier_req_star6() {
        let req = tier_requirements(UnitTier::Star6).expect("should have reqs");
        assert_eq!(req.copies_needed, 2);
        assert_eq!(req.fodder_needed, 1);
        assert_eq!(req.fodder_tier, UnitTier::Star5);
    }

    #[test]
    fn test_tier_req_star10_has_sacrifice() {
        let req = tier_requirements(UnitTier::Star10).expect("should have reqs");
        assert_eq!(req.sacrifice_needed, 1);
        assert_eq!(req.sacrifice_tier, UnitTier::Star9);
    }

    #[test]
    fn test_tier_req_e1() {
        let req = tier_requirements(UnitTier::E1).expect("should have reqs");
        assert_eq!(req.sacrifice_needed, 1);
        assert_eq!(req.sacrifice_tier, UnitTier::Star9);
        assert_eq!(req.copies_needed, 0);
        assert_eq!(req.fodder_needed, 0);
    }

    #[test]
    fn test_tier_req_e5() {
        let req = tier_requirements(UnitTier::E5).expect("should have reqs");
        assert_eq!(req.sacrifice_needed, 1);
        assert_eq!(req.sacrifice_tier, UnitTier::Star10);
    }

    // -- TierBonus --

    #[test]
    fn test_tier_bonus_star1_base() {
        let b = tier_bonus(UnitTier::Star1);
        assert!((b.hp_multiplier - 1.0).abs() < f64::EPSILON);
        assert!(b.ability_unlock.is_none());
    }

    #[test]
    fn test_tier_bonus_star5_passive() {
        let b = tier_bonus(UnitTier::Star5);
        assert!((b.hp_multiplier - 1.7).abs() < f64::EPSILON);
        assert_eq!(b.ability_unlock, Some("Passive Ability".to_string()));
    }

    #[test]
    fn test_tier_bonus_star6_active() {
        let b = tier_bonus(UnitTier::Star6);
        assert!((b.hp_multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(b.ability_unlock, Some("Active Ability".to_string()));
    }

    #[test]
    fn test_tier_bonus_star9_ultimate() {
        let b = tier_bonus(UnitTier::Star9);
        assert!((b.hp_multiplier - 3.1).abs() < f64::EPSILON);
        assert_eq!(b.ability_unlock, Some("Ultimate Ability".to_string()));
    }

    #[test]
    fn test_tier_bonus_e1_ascension() {
        let b = tier_bonus(UnitTier::E1);
        assert!((b.hp_multiplier - 4.5).abs() < f64::EPSILON);
        assert_eq!(b.ability_unlock, Some("Ascension Passive".to_string()));
    }

    #[test]
    fn test_tier_bonus_e5_transcendence() {
        let b = tier_bonus(UnitTier::E5);
        assert!((b.hp_multiplier - 10.0).abs() < f64::EPSILON);
        assert_eq!(b.ability_unlock, Some("Transcendence Form".to_string()));
    }

    #[test]
    fn test_tier_bonus_monotonic_increase() {
        let tiers = UnitTier::all();
        for pair in tiers.windows(2) {
            let lower = tier_bonus(pair[0]);
            let higher = tier_bonus(pair[1]);
            assert!(
                higher.hp_multiplier > lower.hp_multiplier,
                "{:?} mult {} should be > {:?} mult {}",
                pair[1],
                higher.hp_multiplier,
                pair[0],
                lower.hp_multiplier,
            );
        }
    }

    // -- Faction Names --

    #[test]
    fn test_faction_tier_names_insect() {
        let names = faction_tier_names("insect");
        assert_eq!(names.len(), 15);
        assert_eq!(names[0], "Larva");
        assert_eq!(names[4], "Queen");
        assert_eq!(names[5], "Metamorphosis I");
        assert_eq!(names[14], "Enlightened V");
    }

    #[test]
    fn test_faction_tier_names_demon() {
        let names = faction_tier_names("demon");
        assert_eq!(names[0], "Lesser");
        assert_eq!(names[4], "Overlord");
        assert_eq!(names[5], "Ascension I");
    }

    #[test]
    fn test_faction_tier_names_undead() {
        let names = faction_tier_names("undead");
        assert_eq!(names[0], "Remnant");
        assert_eq!(names[4], "Death Knight");
        assert_eq!(names[5], "Transcendence I");
    }

    #[test]
    fn test_faction_tier_names_human() {
        let names = faction_tier_names("human");
        assert_eq!(names[0], "Recruit");
        assert_eq!(names[4], "Legend");
    }

    #[test]
    fn test_faction_tier_names_unknown_fallback() {
        let names = faction_tier_names("aliens");
        assert_eq!(names.len(), 15);
        assert_eq!(names[0], "Tier I");
        assert_eq!(names[5], "Advanced I");
    }

    #[test]
    fn test_faction_tier_names_case_insensitive() {
        let a = faction_tier_names("DEMON");
        let b = faction_tier_names("demon");
        assert_eq!(a, b);
    }

    // -- Spells --

    #[test]
    fn test_insect_spells_count() {
        assert_eq!(insect_spells().len(), 4);
    }

    #[test]
    fn test_demon_spells_count() {
        assert_eq!(demon_spells().len(), 4);
    }

    #[test]
    fn test_undead_spells_count() {
        assert_eq!(undead_spells().len(), 4);
    }

    #[test]
    fn test_human_spells_count() {
        assert_eq!(human_spells().len(), 4);
    }

    #[test]
    fn test_all_spells_count() {
        assert_eq!(all_spells().len(), 16);
    }

    #[test]
    fn test_spell_ids_unique() {
        let spells = all_spells();
        let mut ids: Vec<&str> = spells.iter().map(|s| s.id.as_str()).collect();
        let original_len = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), original_len, "spell IDs must be unique");
    }

    #[test]
    fn test_all_spells_have_positive_mana_cost() {
        for spell in all_spells() {
            assert!(
                spell.mana_cost > 0.0,
                "Spell '{}' should have positive mana cost, got {}",
                spell.id,
                spell.mana_cost,
            );
        }
    }

    #[test]
    fn test_all_spells_nonnegative_cast_time() {
        for spell in all_spells() {
            assert!(
                spell.cast_time_secs >= 0.0,
                "Spell '{}' has negative cast time",
                spell.id,
            );
        }
    }

    #[test]
    fn test_faction_spells_unknown_returns_empty() {
        assert!(faction_spells("martian").is_empty());
    }

    #[test]
    fn test_demon_fireball_has_evolution() {
        let spells = demon_spells();
        let fireball = spells.iter().find(|s| s.id == "demon_fireball").expect("fireball");
        assert_eq!(fireball.evolution_path, Some("demon_flaming_ball".into()));
    }

    #[test]
    fn test_demon_fireball_evolution_chain() {
        let chain = demon_fireball_evolution();
        assert_eq!(chain.len(), 4);
        assert_eq!(chain[0].id, "demon_fireball");
        assert_eq!(chain[1].id, "demon_flaming_ball");
        assert_eq!(chain[2].id, "demon_fire_beam");
        assert_eq!(chain[3].id, "demon_inferno");
        // Last in chain has no evolution
        assert!(chain[3].evolution_path.is_none());
    }

    #[test]
    fn test_evolution_chain_increasing_mana_cost() {
        let chain = demon_fireball_evolution();
        for pair in chain.windows(2) {
            assert!(
                pair[1].mana_cost > pair[0].mana_cost,
                "Evolution {} ({}) should cost more mana than {} ({})",
                pair[1].id, pair[1].mana_cost,
                pair[0].id, pair[0].mana_cost,
            );
        }
    }

    // -- Default Mana Pool --

    #[test]
    fn test_default_mana_pool_caster() {
        let pool = default_mana_pool("caster");
        assert!((pool.maximum - 100.0).abs() < f64::EPSILON);
        assert!((pool.regen_per_sec - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_mana_pool_healer_higher() {
        let caster = default_mana_pool("caster");
        let healer = default_mana_pool("healer");
        assert!(healer.maximum > caster.maximum);
        assert!(healer.regen_per_sec > caster.regen_per_sec);
    }

    #[test]
    fn test_default_mana_pool_tank_low() {
        let pool = default_mana_pool("tank");
        assert!(pool.maximum < 50.0);
    }

    #[test]
    fn test_default_mana_pool_unknown_fallback() {
        let pool = default_mana_pool("dragon");
        assert!((pool.maximum - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_mana_pool_case_insensitive() {
        let a = default_mana_pool("CASTER");
        let b = default_mana_pool("caster");
        assert!((a.maximum - b.maximum).abs() < f64::EPSILON);
    }

    // -- Integration: cast spell with mana check --

    #[test]
    fn test_cast_spell_deducts_mana() {
        let mut pool = ManaPool::new(100.0, 2.0);
        let spell = &demon_spells()[0]; // Fireball, 8 mana
        assert!(pool.spend(spell.mana_cost));
        assert!((pool.current - 92.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cast_spell_fails_low_mana() {
        let mut pool = ManaPool::new(100.0, 2.0);
        pool.current = 3.0;
        let spell = &demon_spells()[0]; // Fireball, 8 mana
        assert!(!pool.spend(spell.mana_cost));
        assert!((pool.current - 3.0).abs() < f64::EPSILON); // unchanged
    }

    #[test]
    fn test_pushback_during_cast_then_complete() {
        let mut state = CastingState::begin("demon_fireball");
        // Advance 70%
        state.advance(0.7, 1.0);
        assert!((state.cast_progress - 0.7).abs() < f64::EPSILON);
        // Take a hit: pushback -10%
        state.apply_pushback();
        assert!((state.cast_progress - 0.6).abs() < f64::EPSILON);
        // Advance remaining 40% (0.4s of 1.0s)
        let done = state.advance(0.4, 1.0);
        assert!(done);
    }

    #[test]
    fn test_full_pushback_scenario() {
        // 5 pushbacks on a long cast, then verify immunity
        let mut state = CastingState::begin("demon_doom");
        state.cast_progress = 0.9;
        for _ in 0..5 {
            state.apply_pushback();
        }
        assert_eq!(state.pushback_count, 5);
        let progress_after_5 = state.cast_progress;
        // 6th pushback should be immune
        state.apply_pushback();
        assert_eq!(state.pushback_count, 5); // still 5
        assert!((state.cast_progress - progress_after_5).abs() < f64::EPSILON);
    }
}
