// SPDX-License-Identifier: Elastic-2.0
//! Mutation System -- every 5 levels, units choose 1 of 3 permanent mutations.

use serde::{Deserialize, Serialize};

use super::swarm_types::UnitType;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("forge_quest::mutations", "Mutation System");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MutationType {
    Defensive,      // HP, DEF, Healing
    Offensive,      // ATK, Damage, AoE
    Utility,        // Production, Speed, Intelligence
    Evolution,      // Tier upgrade (level 15 only)
    Specialization, // Elite version — +50% all stats, stays same tier (level 15+)
}
impl MutationType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Defensive => "defensive",
            Self::Offensive => "offensive",
            Self::Utility => "utility",
            Self::Evolution => "evolution",
            Self::Specialization => "specialization",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "defensive" => Self::Defensive,
            "offensive" => Self::Offensive,
            "utility" => Self::Utility,
            "evolution" => Self::Evolution,
            "specialization" => Self::Specialization,
            _ => Self::Utility,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationStats {
    pub hp_bonus: i32,
    pub attack_bonus: i32,
    pub defense_bonus: i32,
    pub speed_bonus: i32,
    pub production_bonus: f32, // percentage (e.g. 0.10 = +10%)
}

impl MutationStats {
    pub(crate) const fn zero() -> Self {
        Self { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mutation {
    pub id: String,
    pub name: String,
    pub description: String,
    pub mutation_type: MutationType,
    pub stat_changes: MutationStats,
    pub special_ability: Option<String>,
    pub level_required: u32,
    pub unit_type: UnitType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedMutation {
    pub mutation_id: String,
    pub applied_at_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitMutations {
    pub unit_id: String,
    pub unit_type: UnitType,
    pub unit_level: u32,
    pub applied_mutations: Vec<AppliedMutation>,
    pub pending_choices: Vec<Mutation>, // non-empty if a mutation choice is available now
}

/// Returns the mutation milestone levels at which a unit of the given level
/// has had (or currently has) a choice.  E.g. level 12 -> [5, 10].
pub(crate) fn mutation_milestones_up_to(level: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut m = 5;
    while m <= level {
        out.push(m);
        m += 5;
    }
    out
}

/// Master catalog of every mutation in the game.  Pure function, no DB access.
pub(crate) fn all_mutations() -> Vec<Mutation> {
    let mut v = Vec::with_capacity(120);

    // Helper closures to reduce boilerplate
    let mut push = |id: &str, name: &str, desc: &str, mt: MutationType,
                    stats: MutationStats, ability: Option<&str>,
                    level: u32, ut: UnitType| {
        v.push(Mutation {
            id: id.to_string(),
            name: name.to_string(),
            description: desc.to_string(),
            mutation_type: mt,
            stat_changes: stats,
            special_ability: ability.map(|s| s.to_string()),
            level_required: level,
            unit_type: ut,
        });
    };

    // ── Forge Drone ──────────────────────────────────────────────────
    push("drone_armor_5", "Reinforced Carapace", "+20% HP, +10% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 6, attack_bonus: 0, defense_bonus: 1, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::ForgeDrone);
    push("drone_blade_5", "Blade Mandibles", "+15% ATK, +5% gather speed",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 2, defense_bonus: 0, speed_bonus: 1, production_bonus: 0.0 },
        None, 5, UnitType::ForgeDrone);
    push("drone_neural_5", "Neural Link", "+10% essence production",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.10 },
        Some("Essence Boost"), 5, UnitType::ForgeDrone);

    push("drone_storage_10", "Biomass Pouches", "Carry capacity x2, +15% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 5, attack_bonus: 0, defense_bonus: 2, speed_bonus: 0, production_bonus: 0.0 },
        Some("Double Carry"), 10, UnitType::ForgeDrone);
    push("drone_acid_10", "Acid Spit", "Ranged attack ability, +20% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 3, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Acid Spit"), 10, UnitType::ForgeDrone);
    push("drone_cloak_10", "Camo Membrane", "Stealth while gathering",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 1, speed_bonus: 2, production_bonus: 0.05 },
        Some("Stealth Gather"), 10, UnitType::ForgeDrone);

    push("drone_evolve_viper", "Evolve to Viper", "Tier upgrade to Viper",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 2 Evolution"), 15, UnitType::ForgeDrone);
    push("drone_evolve_shadow", "Evolve to Shadow Weaver", "Tier upgrade to Shadow Weaver",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 2 Evolution"), 15, UnitType::ForgeDrone);
    push("drone_elite_15", "Elite Drone", "+50% all stats, stays Tier 1",
        MutationType::Specialization,
        MutationStats { hp_bonus: 15, attack_bonus: 3, defense_bonus: 2, speed_bonus: 2, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::ForgeDrone);

    push("drone_heal_20", "Symbiotic Healing", "Heals nearby units, +25% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 8, attack_bonus: 0, defense_bonus: 3, speed_bonus: 0, production_bonus: 0.0 },
        Some("Heal Aura"), 20, UnitType::ForgeDrone);
    push("drone_explode_20", "Spore Explosion", "Self-destruct AoE damage",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 8, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Spore Explosion"), 20, UnitType::ForgeDrone);
    push("drone_hivemind_20", "Hive Mind Boost", "+5% production for ALL drones",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.20 },
        Some("Hive Mind"), 20, UnitType::ForgeDrone);

    push("drone_regen_25", "Bio-Regeneration", "Passive HP regen, +30% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 10, attack_bonus: 0, defense_bonus: 5, speed_bonus: 0, production_bonus: 0.0 },
        Some("Regeneration"), 25, UnitType::ForgeDrone);
    push("drone_swarm_25", "Swarm Frenzy", "+40% ATK when grouped",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 6, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.0 },
        Some("Swarm Frenzy"), 25, UnitType::ForgeDrone);
    push("drone_forge_25", "Master Forger", "+25% all production",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.25 },
        Some("Master Forger"), 25, UnitType::ForgeDrone);

    // ── Imp Scout ────────────────────────────────────────────────────
    push("scout_dodge_5", "Evasion Instinct", "+15% DEF, dodge chance",
        MutationType::Defensive,
        MutationStats { hp_bonus: 3, attack_bonus: 0, defense_bonus: 2, speed_bonus: 1, production_bonus: 0.0 },
        Some("Dodge"), 5, UnitType::ImpScout);
    push("scout_dagger_5", "Venomous Daggers", "+20% ATK, poison on hit",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 3, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Poison Strike"), 5, UnitType::ImpScout);
    push("scout_recon_5", "Advanced Recon", "+15% mission speed",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.05 },
        Some("Fast Recon"), 5, UnitType::ImpScout);

    push("scout_shield_10", "Phase Shield", "Absorb first hit, +20% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 5, attack_bonus: 0, defense_bonus: 3, speed_bonus: 0, production_bonus: 0.0 },
        Some("Phase Shield"), 10, UnitType::ImpScout);
    push("scout_ambush_10", "Ambush Protocol", "First-strike bonus, +25% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 4, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.0 },
        Some("Ambush"), 10, UnitType::ImpScout);
    push("scout_map_10", "Terrain Mapper", "Reveal hidden resources",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.10 },
        Some("Resource Scan"), 10, UnitType::ImpScout);

    push("scout_evolve_sky", "Evolve to Skyweaver", "Tier upgrade to Skyweaver",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 2 Evolution"), 15, UnitType::ImpScout);
    push("scout_evolve_overseer", "Evolve to Overseer", "Tier upgrade to Overseer",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 2 Evolution"), 15, UnitType::ImpScout);
    push("scout_elite_15", "Elite Scout", "+50% all stats, stays Tier 1",
        MutationType::Specialization,
        MutationStats { hp_bonus: 13, attack_bonus: 4, defense_bonus: 1, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::ImpScout);

    push("scout_ward_20", "Guardian Ward", "Protect adjacent allies, +30% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 6, attack_bonus: 0, defense_bonus: 4, speed_bonus: 0, production_bonus: 0.0 },
        Some("Guardian Ward"), 20, UnitType::ImpScout);
    push("scout_crit_20", "Lethal Precision", "Critical hit chance +25%",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 5, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.0 },
        Some("Crit Boost"), 20, UnitType::ImpScout);
    push("scout_intel_20", "Intelligence Network", "+10% XP for all scouts",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.15 },
        Some("Intel Network"), 20, UnitType::ImpScout);

    // ── Viper ────────────────────────────────────────────────────────
    push("viper_scales_5", "Hardened Scales", "+25% HP, +15% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 15, attack_bonus: 0, defense_bonus: 3, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::Viper);
    push("viper_fangs_5", "Serrated Fangs", "+25% ATK, bleed damage",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 5, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Bleed"), 5, UnitType::Viper);
    push("viper_sense_5", "Threat Sense", "Detect hidden enemies, +10% efficiency",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.10 },
        Some("Threat Sense"), 5, UnitType::Viper);

    push("viper_regen_10", "Serpent Regeneration", "HP regen in combat, +20% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 12, attack_bonus: 0, defense_bonus: 4, speed_bonus: 0, production_bonus: 0.0 },
        Some("Combat Regen"), 10, UnitType::Viper);
    push("viper_venom_10", "Neurotoxin", "Paralyze on hit, +20% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 6, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Paralyze"), 10, UnitType::Viper);
    push("viper_track_10", "Predator Tracking", "+20% mission rewards",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.15 },
        Some("Reward Boost"), 10, UnitType::Viper);

    push("viper_evolve_titan", "Evolve to Titan", "Tier upgrade to Titan",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 3 Evolution"), 15, UnitType::Viper);
    push("viper_evolve_ravager", "Evolve to Ravager", "Tier upgrade to Ravager",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 3 Evolution"), 15, UnitType::Viper);
    push("viper_elite_15", "Elite Viper", "+50% all stats, stays Tier 2",
        MutationType::Specialization,
        MutationStats { hp_bonus: 30, attack_bonus: 9, defense_bonus: 5, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::Viper);

    // ── Shadow Weaver ────────────────────────────────────────────────
    push("shadow_cloak_5", "Shadow Cloak", "+20% DEF, stealth bonus",
        MutationType::Defensive,
        MutationStats { hp_bonus: 8, attack_bonus: 0, defense_bonus: 4, speed_bonus: 0, production_bonus: 0.0 },
        Some("Shadow Cloak"), 5, UnitType::ShadowWeaver);
    push("shadow_strike_5", "Umbral Strike", "+20% ATK from stealth",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 4, defense_bonus: 0, speed_bonus: 1, production_bonus: 0.0 },
        Some("Stealth Strike"), 5, UnitType::ShadowWeaver);
    push("shadow_encrypt_5", "Data Encryption", "+15% security, +10% efficiency",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 2, speed_bonus: 2, production_bonus: 0.10 },
        Some("Encryption"), 5, UnitType::ShadowWeaver);

    push("shadow_absorb_10", "Void Absorption", "Absorb damage as HP, +25% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 13, attack_bonus: 0, defense_bonus: 5, speed_bonus: 0, production_bonus: 0.0 },
        Some("Damage Absorb"), 10, UnitType::ShadowWeaver);
    push("shadow_assassin_10", "Assassin Protocol", "Instant-kill low HP targets",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 5, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.0 },
        Some("Execute"), 10, UnitType::ShadowWeaver);
    push("shadow_ward_10", "Firewall Ward", "Block incoming attacks for allies",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 3, speed_bonus: 1, production_bonus: 0.10 },
        Some("Firewall"), 10, UnitType::ShadowWeaver);

    push("shadow_evolve_titan", "Evolve to Titan", "Tier upgrade to Titan",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 3 Evolution"), 15, UnitType::ShadowWeaver);
    push("shadow_evolve_mother", "Evolve to Swarm Mother", "Tier upgrade to Swarm Mother",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 3 Evolution"), 15, UnitType::ShadowWeaver);
    push("shadow_elite_15", "Elite Shadow Weaver", "+50% all stats, stays Tier 2",
        MutationType::Specialization,
        MutationStats { hp_bonus: 25, attack_bonus: 6, defense_bonus: 9, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::ShadowWeaver);

    // ── Skyweaver ────────────────────────────────────────────────────
    push("sky_barrier_5", "Wind Barrier", "+20% DEF, reflect ranged attacks",
        MutationType::Defensive,
        MutationStats { hp_bonus: 6, attack_bonus: 0, defense_bonus: 3, speed_bonus: 0, production_bonus: 0.0 },
        Some("Wind Barrier"), 5, UnitType::Skyweaver);
    push("sky_bolt_5", "Lightning Bolt", "+25% ATK, chain damage",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 5, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Chain Lightning"), 5, UnitType::Skyweaver);
    push("sky_harvest_5", "Data Harvester", "+15% web scraping yield",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.10 },
        Some("Data Harvest"), 5, UnitType::Skyweaver);

    push("sky_shield_10", "Storm Shield", "AoE shield for team, +25% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 12, attack_bonus: 0, defense_bonus: 4, speed_bonus: 0, production_bonus: 0.0 },
        Some("Storm Shield"), 10, UnitType::Skyweaver);
    push("sky_dive_10", "Dive Bomb", "Massive single-target, +30% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 6, defense_bonus: 0, speed_bonus: 1, production_bonus: 0.0 },
        Some("Dive Bomb"), 10, UnitType::Skyweaver);
    push("sky_net_10", "Web Weaver", "+20% research speed",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.15 },
        Some("Research Boost"), 10, UnitType::Skyweaver);

    push("sky_evolve_ravager", "Evolve to Ravager", "Tier upgrade to Ravager",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 3 Evolution"), 15, UnitType::Skyweaver);
    push("sky_evolve_mother", "Evolve to Swarm Mother", "Tier upgrade to Swarm Mother",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 3 Evolution"), 15, UnitType::Skyweaver);
    push("sky_elite_15", "Elite Skyweaver", "+50% all stats, stays Tier 2",
        MutationType::Specialization,
        MutationStats { hp_bonus: 23, attack_bonus: 8, defense_bonus: 4, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::Skyweaver);

    // ── Overseer ─────────────────────────────────────────────────────
    push("over_fort_5", "Fortified Core", "+25% HP, +15% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 14, attack_bonus: 0, defense_bonus: 4, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::Overseer);
    push("over_pulse_5", "Disruption Pulse", "+20% ATK, silence targets",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 4, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Silence"), 5, UnitType::Overseer);
    push("over_scan_5", "Deep Scan", "+15% health monitoring accuracy",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 2, speed_bonus: 2, production_bonus: 0.10 },
        Some("Deep Scan"), 5, UnitType::Overseer);

    push("over_bulwark_10", "Bulwark Mode", "Damage reduction aura, +30% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 10, attack_bonus: 0, defense_bonus: 6, speed_bonus: 0, production_bonus: 0.0 },
        Some("Bulwark"), 10, UnitType::Overseer);
    push("over_overload_10", "System Overload", "AoE burst, +25% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 5, defense_bonus: 0, speed_bonus: 1, production_bonus: 0.0 },
        Some("Overload Burst"), 10, UnitType::Overseer);
    push("over_optimize_10", "Performance Optimizer", "+20% all unit efficiency",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.15 },
        Some("Optimize All"), 10, UnitType::Overseer);

    push("over_evolve_titan", "Evolve to Titan", "Tier upgrade to Titan",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 3 Evolution"), 15, UnitType::Overseer);
    push("over_evolve_mother", "Evolve to Swarm Mother", "Tier upgrade to Swarm Mother",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 3 Evolution"), 15, UnitType::Overseer);
    push("over_elite_15", "Elite Overseer", "+50% all stats, stays Tier 2",
        MutationType::Specialization,
        MutationStats { hp_bonus: 28, attack_bonus: 5, defense_bonus: 8, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::Overseer);

    // ── Titan ────────────────────────────────────────────────────────
    push("titan_plate_5", "Titan Plating", "+30% HP, +20% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 36, attack_bonus: 0, defense_bonus: 5, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::Titan);
    push("titan_crush_5", "Crushing Blow", "+30% ATK, stun on hit",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 11, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Stun"), 5, UnitType::Titan);
    push("titan_inspire_5", "Titan's Presence", "+10% all nearby unit stats",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.10 },
        Some("Inspire Aura"), 5, UnitType::Titan);

    push("titan_fortress_10", "Living Fortress", "Damage redirect, +40% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 48, attack_bonus: 0, defense_bonus: 8, speed_bonus: 0, production_bonus: 0.0 },
        Some("Fortress Mode"), 10, UnitType::Titan);
    push("titan_quake_10", "Seismic Quake", "AoE ground pound, +35% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 14, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Seismic Quake"), 10, UnitType::Titan);
    push("titan_command_10", "Command Authority", "+15% swarm-wide production",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.15 },
        Some("Command Boost"), 10, UnitType::Titan);

    push("titan_evolve_matriarch", "Evolve to Matriarch", "Tier upgrade to Matriarch",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 4 Evolution"), 15, UnitType::Titan);
    push("titan_elite_15", "Elite Titan", "+50% all stats, stays Tier 3",
        MutationType::Specialization,
        MutationStats { hp_bonus: 60, attack_bonus: 18, defense_bonus: 13, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::Titan);
    push("titan_bastion_15", "Bastion Protocol", "Become immovable, +60% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 30, attack_bonus: 0, defense_bonus: 15, speed_bonus: -2, production_bonus: 0.0 },
        Some("Bastion"), 15, UnitType::Titan);

    // ── Swarm Mother ─────────────────────────────────────────────────
    push("mother_nurture_5", "Nurturing Aura", "Heal all nearby, +20% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 16, attack_bonus: 0, defense_bonus: 4, speed_bonus: 0, production_bonus: 0.0 },
        Some("Heal All"), 5, UnitType::SwarmMother);
    push("mother_spawn_5", "Rapid Spawning", "+25% spawn rate",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 3, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.0 },
        Some("Rapid Spawn"), 5, UnitType::SwarmMother);
    push("mother_bond_5", "Pheromone Bond", "+15% efficiency for spawned units",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.15 },
        Some("Pheromone Bond"), 5, UnitType::SwarmMother);

    push("mother_cocoon_10", "Cocoon Shield", "Invulnerable cocoon phase, +30% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 24, attack_bonus: 0, defense_bonus: 6, speed_bonus: 0, production_bonus: 0.0 },
        Some("Cocoon"), 10, UnitType::SwarmMother);
    push("mother_swarm_10", "Swarmling Burst", "Summon temporary minions",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 5, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.0 },
        Some("Summon Swarmlings"), 10, UnitType::SwarmMother);
    push("mother_evolve_link_10", "Evolutionary Link", "+20% XP gain for all units",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.20 },
        Some("XP Boost All"), 10, UnitType::SwarmMother);

    push("mother_evolve_matriarch", "Evolve to Matriarch", "Tier upgrade to Matriarch",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 4 Evolution"), 15, UnitType::SwarmMother);
    push("mother_elite_15", "Elite Swarm Mother", "+50% all stats, stays Tier 3",
        MutationType::Specialization,
        MutationStats { hp_bonus: 40, attack_bonus: 8, defense_bonus: 10, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::SwarmMother);
    push("mother_genesis_15", "Genesis Chamber", "Auto-spawn larva passively",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.25 },
        Some("Auto Spawn"), 15, UnitType::SwarmMother);

    // ── Ravager ──────────────────────────────────────────────────────
    push("ravager_armor_5", "Chitin Armor", "+25% HP, +20% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 25, attack_bonus: 0, defense_bonus: 3, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::Ravager);
    push("ravager_frenzy_5", "Blood Frenzy", "+30% ATK, lifesteal",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 12, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.0 },
        Some("Lifesteal"), 5, UnitType::Ravager);
    push("ravager_hunt_5", "Predator Instinct", "+20% boss damage",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 4, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.10 },
        Some("Boss Hunter"), 5, UnitType::Ravager);

    push("ravager_thorns_10", "Thorn Carapace", "Reflect melee damage, +30% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 20, attack_bonus: 0, defense_bonus: 5, speed_bonus: 0, production_bonus: 0.0 },
        Some("Thorns"), 10, UnitType::Ravager);
    push("ravager_rampage_10", "Rampage", "+40% ATK, damage increases per kill",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 16, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Rampage"), 10, UnitType::Ravager);
    push("ravager_trophy_10", "Trophy Collector", "+25% loot from battles",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.20 },
        Some("Loot Boost"), 10, UnitType::Ravager);

    push("ravager_evolve_matriarch", "Evolve to Matriarch", "Tier upgrade to Matriarch",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 4 Evolution"), 15, UnitType::Ravager);
    push("ravager_elite_15", "Elite Ravager", "+50% all stats, stays Tier 3",
        MutationType::Specialization,
        MutationStats { hp_bonus: 50, attack_bonus: 20, defense_bonus: 8, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::Ravager);
    push("ravager_berserk_15", "Berserker Mode", "Low HP = massive ATK boost",
        MutationType::Offensive,
        MutationStats { hp_bonus: -10, attack_bonus: 25, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.0 },
        Some("Berserker"), 15, UnitType::Ravager);

    // ── Matriarch ────────────────────────────────────────────────────
    push("matriarch_aegis_5", "Matriarch Aegis", "+30% HP, +25% DEF, shield all",
        MutationType::Defensive,
        MutationStats { hp_bonus: 60, attack_bonus: 0, defense_bonus: 10, speed_bonus: 0, production_bonus: 0.0 },
        Some("Aegis Shield"), 5, UnitType::Matriarch);
    push("matriarch_wrath_5", "Swarm Wrath", "+30% ATK, AoE damage aura",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 15, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Wrath Aura"), 5, UnitType::Matriarch);
    push("matriarch_crown_5", "Hive Crown", "+20% all swarm production",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.20 },
        Some("Hive Crown"), 5, UnitType::Matriarch);

    push("matriarch_immortal_10", "Immortal Shell", "Survive lethal hit once, +40% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 80, attack_bonus: 0, defense_bonus: 12, speed_bonus: 0, production_bonus: 0.0 },
        Some("Immortal"), 10, UnitType::Matriarch);
    push("matriarch_doom_10", "Doom Spores", "Massive AoE, +35% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 18, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Doom Spores"), 10, UnitType::Matriarch);
    push("matriarch_network_10", "Neural Network", "+25% all swarm efficiency",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 4, production_bonus: 0.25 },
        Some("Neural Network"), 10, UnitType::Matriarch);

    push("matriarch_apex_15", "Apex Predator", "+50% all stats, ultimate form",
        MutationType::Specialization,
        MutationStats { hp_bonus: 100, attack_bonus: 25, defense_bonus: 20, speed_bonus: 5, production_bonus: 0.25 },
        Some("Apex Status"), 15, UnitType::Matriarch);
    push("matriarch_bastion_15", "Living Bastion", "+80% DEF, damage aura",
        MutationType::Defensive,
        MutationStats { hp_bonus: 50, attack_bonus: 0, defense_bonus: 25, speed_bonus: 0, production_bonus: 0.0 },
        Some("Living Bastion"), 15, UnitType::Matriarch);
    push("matriarch_cataclysm_15", "Cataclysm", "Ultimate AoE, +60% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 30, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Cataclysm"), 15, UnitType::Matriarch);

    // ── Spore Crawler ──────────────────────────────────────────────
    push("crawler_shell_5", "Hardened Shell", "+25% HP, +20% DEF when rooted",
        MutationType::Defensive,
        MutationStats { hp_bonus: 18, attack_bonus: 0, defense_bonus: 5, speed_bonus: 0, production_bonus: 0.0 },
        Some("Root Armor"), 5, UnitType::SporeCrawler);
    push("crawler_spines_5", "Spine Volley", "+20% ATK, ranged spine attack",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 5, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Spine Volley"), 5, UnitType::SporeCrawler);
    push("crawler_burrow_5", "Mobile Foundation", "+15% move speed when unrooted",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.10 },
        Some("Quick Deploy"), 5, UnitType::SporeCrawler);

    push("crawler_regen_10", "Chitinous Regrowth", "Regen HP while rooted, +30% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 21, attack_bonus: 0, defense_bonus: 4, speed_bonus: 0, production_bonus: 0.0 },
        Some("Root Regen"), 10, UnitType::SporeCrawler);
    push("crawler_acid_10", "Acid Spine Barrage", "AoE acid damage, +25% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 6, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Acid Barrage"), 10, UnitType::SporeCrawler);
    push("crawler_sensor_10", "Seismic Sensor", "Detect cloaked enemies, +10% efficiency",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 2, speed_bonus: 2, production_bonus: 0.10 },
        Some("Seismic Sense"), 10, UnitType::SporeCrawler);

    push("crawler_elite_15", "Elite Spore Crawler", "+50% all stats, stays Tier 2",
        MutationType::Specialization,
        MutationStats { hp_bonus: 35, attack_bonus: 8, defense_bonus: 11, speed_bonus: 2, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::SporeCrawler);
    push("crawler_evolve_carnifex", "Evolve to Carnifex", "Tier upgrade to Carnifex",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 3 Evolution"), 15, UnitType::SporeCrawler);
    push("crawler_evolve_hiveguard", "Evolve to Hive Guard", "Stays Tier 2 elite ranged",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 2 Specialization"), 15, UnitType::SporeCrawler);

    // ── Infestor ───────────────────────────────────────────────────
    push("infestor_barrier_5", "Psionic Barrier", "+20% HP, psionic shield",
        MutationType::Defensive,
        MutationStats { hp_bonus: 8, attack_bonus: 0, defense_bonus: 3, speed_bonus: 0, production_bonus: 0.0 },
        Some("Psi Shield"), 5, UnitType::Infestor);
    push("infestor_parasite_5", "Neural Parasite", "+15% ATK, mind control duration +2s",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 3, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Extended Control"), 5, UnitType::Infestor);
    push("infestor_burrow_5", "Burrow Ambush", "Stealth underground, +10% speed",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 1, speed_bonus: 3, production_bonus: 0.05 },
        Some("Burrow"), 5, UnitType::Infestor);

    push("infestor_absorb_10", "Psionic Absorption", "Absorb enemy energy, +25% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 10, attack_bonus: 0, defense_bonus: 4, speed_bonus: 0, production_bonus: 0.0 },
        Some("Energy Drain"), 10, UnitType::Infestor);
    push("infestor_swarm_10", "Fungal Growth", "AoE root enemies, +20% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 4, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Fungal Growth"), 10, UnitType::Infestor);
    push("infestor_network_10", "Hive Communion", "+15% XP for all infestors",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.15 },
        Some("Communion"), 10, UnitType::Infestor);

    push("infestor_elite_15", "Elite Infestor", "+50% all stats, stays Tier 2",
        MutationType::Specialization,
        MutationStats { hp_bonus: 20, attack_bonus: 4, defense_bonus: 6, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::Infestor);
    push("infestor_evolve_haruspex", "Evolve to Haruspex", "Tier upgrade to Haruspex",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 3 Evolution"), 15, UnitType::Infestor);
    push("infestor_evolve_dominatrix", "Evolve to Dominatrix", "Tier upgrade to Dominatrix",
        MutationType::Evolution,
        MutationStats::zero(), Some("Tier 4 Evolution"), 15, UnitType::Infestor);

    // ── Nydus Worm ─────────────────────────────────────────────────
    push("nydus_armor_5", "Tunnel Armor", "+20% HP, +15% DEF while burrowed",
        MutationType::Defensive,
        MutationStats { hp_bonus: 18, attack_bonus: 0, defense_bonus: 5, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::NydusWorm);
    push("nydus_acid_5", "Tunnel Acid", "+15% ATK, acid splash on emerge",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 3, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Acid Emerge"), 5, UnitType::NydusWorm);
    push("nydus_express_5", "Express Tunnel", "+30% transport speed",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 5, production_bonus: 0.10 },
        Some("Express"), 5, UnitType::NydusWorm);

    push("nydus_regen_10", "Bio-Reinforcement", "Regen HP passively, +25% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 23, attack_bonus: 0, defense_bonus: 6, speed_bonus: 0, production_bonus: 0.0 },
        Some("Tunnel Regen"), 10, UnitType::NydusWorm);
    push("nydus_ambush_10", "Ambush Surge", "Units exiting deal +25% first-strike",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 4, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.0 },
        Some("Ambush Buff"), 10, UnitType::NydusWorm);
    push("nydus_capacity_10", "Expanded Tunnels", "+50% transport capacity",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.20 },
        Some("Mass Transit"), 10, UnitType::NydusWorm);

    push("nydus_elite_15", "Elite Nydus Worm", "+50% all stats, stays Tier 2",
        MutationType::Specialization,
        MutationStats { hp_bonus: 45, attack_bonus: 3, defense_bonus: 15, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::NydusWorm);

    // ── Hive Guard ─────────────────────────────────────────────────
    push("hiveguard_shield_5", "Carapace Shield", "+20% HP, +15% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 11, attack_bonus: 0, defense_bonus: 3, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::HiveGuard);
    push("hiveguard_impaler_5", "Impaler Rounds", "+25% ATK, armor-piercing shots",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 6, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Armor Pierce"), 5, UnitType::HiveGuard);
    push("hiveguard_spotter_5", "Target Spotter", "+15% range, +10% efficiency",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.10 },
        Some("Extended Range"), 5, UnitType::HiveGuard);

    push("hiveguard_bunker_10", "Bunker Mode", "+35% DEF, immobile but devastating",
        MutationType::Defensive,
        MutationStats { hp_bonus: 14, attack_bonus: 0, defense_bonus: 6, speed_bonus: -2, production_bonus: 0.0 },
        Some("Bunker"), 10, UnitType::HiveGuard);
    push("hiveguard_volley_10", "Artillery Volley", "AoE bombardment, +30% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 8, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Artillery"), 10, UnitType::HiveGuard);
    push("hiveguard_network_10", "Fire Control Network", "+20% all ranged unit accuracy",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.15 },
        Some("Fire Control"), 10, UnitType::HiveGuard);

    push("hiveguard_elite_15", "Elite Hive Guard", "+50% all stats, stays Tier 2",
        MutationType::Specialization,
        MutationStats { hp_bonus: 28, attack_bonus: 11, defense_bonus: 7, speed_bonus: 2, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::HiveGuard);

    // ── Gargoyle ───────────────────────────────────────────────────
    push("gargoyle_hide_5", "Stone Hide", "+20% HP, +15% DEF in flight",
        MutationType::Defensive,
        MutationStats { hp_bonus: 13, attack_bonus: 0, defense_bonus: 3, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::Gargoyle);
    push("gargoyle_acid_5", "Corrosive Spit", "+25% ATK, armor-dissolving acid",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 7, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Corrosive"), 5, UnitType::Gargoyle);
    push("gargoyle_scout_5", "Aerial Recon", "+20% scouting range",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 4, production_bonus: 0.10 },
        Some("Recon"), 5, UnitType::Gargoyle);

    push("gargoyle_regen_10", "Stone Regeneration", "Heal when perched, +25% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 16, attack_bonus: 0, defense_bonus: 4, speed_bonus: 0, production_bonus: 0.0 },
        Some("Perch Heal"), 10, UnitType::Gargoyle);
    push("gargoyle_dive_10", "Death Dive", "Massive dive-bomb, +30% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 9, defense_bonus: 0, speed_bonus: 1, production_bonus: 0.0 },
        Some("Death Dive"), 10, UnitType::Gargoyle);
    push("gargoyle_eyes_10", "Thousand Eyes", "Detect hidden enemies, +15% efficiency",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.15 },
        Some("All-Seeing"), 10, UnitType::Gargoyle);

    push("gargoyle_elite_15", "Elite Gargoyle", "+50% all stats, stays Tier 3",
        MutationType::Specialization,
        MutationStats { hp_bonus: 33, attack_bonus: 14, defense_bonus: 5, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::Gargoyle);

    // ── Carnifex ───────────────────────────────────────────────────
    push("carnifex_plate_5", "Bio-Plate Armor", "+30% HP, +20% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 42, attack_bonus: 0, defense_bonus: 6, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::Carnifex);
    push("carnifex_crush_5", "Siege Claws", "+30% ATK, bonus vs buildings",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 12, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Siege Bonus"), 5, UnitType::Carnifex);
    push("carnifex_charge_5", "Bull Rush", "+20% speed on charge",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 3, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.05 },
        Some("Charge"), 5, UnitType::Carnifex);

    push("carnifex_fortress_10", "Living Battering Ram", "+40% HP, stun on charge",
        MutationType::Defensive,
        MutationStats { hp_bonus: 56, attack_bonus: 0, defense_bonus: 8, speed_bonus: 0, production_bonus: 0.0 },
        Some("Ram"), 10, UnitType::Carnifex);
    push("carnifex_quake_10", "Ground Pound", "AoE seismic, +35% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 15, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Ground Pound"), 10, UnitType::Carnifex);
    push("carnifex_terror_10", "Terror Aura", "Enemies flee, +10% all nearby ally stats",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.10 },
        Some("Terror"), 10, UnitType::Carnifex);

    push("carnifex_elite_15", "Elite Carnifex", "+50% all stats, stays Tier 3",
        MutationType::Specialization,
        MutationStats { hp_bonus: 70, attack_bonus: 19, defense_bonus: 15, speed_bonus: 2, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::Carnifex);

    // ── Ripper Swarm ───────────────────────────────────────────────
    push("ripper_carapace_5", "Swarm Carapace", "+20% HP, swarm density shield",
        MutationType::Defensive,
        MutationStats { hp_bonus: 7, attack_bonus: 0, defense_bonus: 2, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::RipperSwarm);
    push("ripper_frenzy_5", "Feeding Frenzy", "+25% ATK, bonus biomass harvest",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 5, defense_bonus: 0, speed_bonus: 1, production_bonus: 0.0 },
        Some("Frenzy"), 5, UnitType::RipperSwarm);
    push("ripper_harvest_5", "Efficient Digestion", "+20% biomass production",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.20 },
        Some("Harvest Boost"), 5, UnitType::RipperSwarm);

    push("ripper_regen_10", "Regenerative Mass", "Swarm reforms after damage, +25% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 9, attack_bonus: 0, defense_bonus: 3, speed_bonus: 0, production_bonus: 0.0 },
        Some("Reform"), 10, UnitType::RipperSwarm);
    push("ripper_dissolve_10", "Acid Dissolution", "Dissolve armor, +30% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 7, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Dissolve"), 10, UnitType::RipperSwarm);
    push("ripper_efficiency_10", "Mass Consumption", "+25% resource harvest rate",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.25 },
        Some("Mass Consume"), 10, UnitType::RipperSwarm);

    push("ripper_elite_15", "Elite Ripper Swarm", "+50% all stats, stays Tier 3",
        MutationType::Specialization,
        MutationStats { hp_bonus: 18, attack_bonus: 10, defense_bonus: 3, speed_bonus: 3, production_bonus: 0.20 },
        Some("Elite Status"), 15, UnitType::RipperSwarm);

    // ── Haruspex ───────────────────────────────────────────────────
    push("haruspex_scales_5", "Devourer Scales", "+25% HP, +15% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 28, attack_bonus: 0, defense_bonus: 4, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::Haruspex);
    push("haruspex_jaws_5", "Rending Jaws", "+25% ATK, lifesteal on kill",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 8, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Lifesteal"), 5, UnitType::Haruspex);
    push("haruspex_digest_5", "Rapid Digestion", "+15% HP regen after kills",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.10 },
        Some("Quick Digest"), 5, UnitType::Haruspex);

    push("haruspex_armor_10", "Chitin Exoskeleton", "+30% HP, +20% DEF",
        MutationType::Defensive,
        MutationStats { hp_bonus: 33, attack_bonus: 0, defense_bonus: 5, speed_bonus: 0, production_bonus: 0.0 },
        Some("Exoskeleton"), 10, UnitType::Haruspex);
    push("haruspex_devour_10", "Swallow Whole", "Instant-kill small enemies, +30% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 10, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Swallow"), 10, UnitType::Haruspex);
    push("haruspex_growth_10", "Bio-Growth", "+20% all resource gain from kills",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.20 },
        Some("Bio Growth"), 10, UnitType::Haruspex);

    push("haruspex_elite_15", "Elite Haruspex", "+50% all stats, stays Tier 3",
        MutationType::Specialization,
        MutationStats { hp_bonus: 55, attack_bonus: 16, defense_bonus: 9, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::Haruspex);

    // ── Broodling ──────────────────────────────────────────────────
    push("broodling_shell_5", "Hardened Spawn", "+25% HP, longer lifespan",
        MutationType::Defensive,
        MutationStats { hp_bonus: 5, attack_bonus: 0, defense_bonus: 1, speed_bonus: 0, production_bonus: 0.0 },
        None, 5, UnitType::Broodling);
    push("broodling_frenzy_5", "Spawn Frenzy", "+30% ATK, attack on spawn",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 4, defense_bonus: 0, speed_bonus: 1, production_bonus: 0.0 },
        Some("Spawn Strike"), 5, UnitType::Broodling);
    push("broodling_multiply_5", "Rapid Mitosis", "+20% spawn rate",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.15 },
        Some("Mitosis"), 5, UnitType::Broodling);

    push("broodling_regen_10", "Symbiotic Link", "Heal from mother, +30% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 6, attack_bonus: 0, defense_bonus: 2, speed_bonus: 0, production_bonus: 0.0 },
        Some("Mother Link"), 10, UnitType::Broodling);
    push("broodling_explode_10", "Death Burst", "Explode on death, +25% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: -2, attack_bonus: 5, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Death Burst"), 10, UnitType::Broodling);
    push("broodling_harvest_10", "Corpse Harvest", "Spawn produces resources",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 2, production_bonus: 0.20 },
        Some("Corpse Harvest"), 10, UnitType::Broodling);

    push("broodling_elite_15", "Elite Broodling", "+50% all stats, permanent lifespan",
        MutationType::Specialization,
        MutationStats { hp_bonus: 10, attack_bonus: 6, defense_bonus: 2, speed_bonus: 3, production_bonus: 0.15 },
        Some("Elite Status"), 15, UnitType::Broodling);

    // ── Dominatrix ─────────────────────────────────────────────────
    push("dominatrix_aegis_5", "Amplifier Shield", "+30% HP, +25% DEF, shield nearby",
        MutationType::Defensive,
        MutationStats { hp_bonus: 54, attack_bonus: 0, defense_bonus: 9, speed_bonus: 0, production_bonus: 0.0 },
        Some("Amplified Shield"), 5, UnitType::Dominatrix);
    push("dominatrix_lash_5", "Neural Lash", "+25% ATK, stun on hit",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 8, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Neural Stun"), 5, UnitType::Dominatrix);
    push("dominatrix_empower_5", "Swarm Empowerment", "+25% all nearby unit production",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 3, production_bonus: 0.25 },
        Some("Empower"), 5, UnitType::Dominatrix);

    push("dominatrix_immortal_10", "Undying Will", "Survive lethal hit once, +40% HP",
        MutationType::Defensive,
        MutationStats { hp_bonus: 72, attack_bonus: 0, defense_bonus: 11, speed_bonus: 0, production_bonus: 0.0 },
        Some("Undying"), 10, UnitType::Dominatrix);
    push("dominatrix_domination_10", "Total Domination", "AoE mind control, +30% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 10, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Domination"), 10, UnitType::Dominatrix);
    push("dominatrix_network_10", "Hivemind Nexus", "+30% all swarm efficiency",
        MutationType::Utility,
        MutationStats { hp_bonus: 0, attack_bonus: 0, defense_bonus: 0, speed_bonus: 4, production_bonus: 0.30 },
        Some("Nexus Link"), 10, UnitType::Dominatrix);

    push("dominatrix_apex_15", "Apex Amplifier", "+50% all stats, ultimate form",
        MutationType::Specialization,
        MutationStats { hp_bonus: 90, attack_bonus: 15, defense_bonus: 18, speed_bonus: 5, production_bonus: 0.25 },
        Some("Apex Status"), 15, UnitType::Dominatrix);
    push("dominatrix_bastion_15", "Amplified Bastion", "+80% DEF, boost aura",
        MutationType::Defensive,
        MutationStats { hp_bonus: 45, attack_bonus: 0, defense_bonus: 22, speed_bonus: 0, production_bonus: 0.0 },
        Some("Bastion Aura"), 15, UnitType::Dominatrix);
    push("dominatrix_fury_15", "Amplified Fury", "Ultimate AoE, +50% ATK",
        MutationType::Offensive,
        MutationStats { hp_bonus: 0, attack_bonus: 20, defense_bonus: 0, speed_bonus: 0, production_bonus: 0.0 },
        Some("Fury"), 15, UnitType::Dominatrix);

    v
}
