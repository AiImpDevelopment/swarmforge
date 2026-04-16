// SPDX-License-Identifier: Elastic-2.0
//! OGame-style Colony types -- buildings, resources, research, fleet, shop, missions.

use serde::{Deserialize, Serialize};

use super::swarm_types::{SwarmUnit, EvolutionPath};

// `Mutation`, `UnitMutations`, `Faction`, `UnitType` are only referenced by
// the test suite via super-glob re-exports; importing them here would be
// cargo-warning noise. Tests reach them through `super::*;`.

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("forge_quest::colony_types", "Colony Types");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BuildingType {
    Nest,              // Increases max unit cap (5 per level, starts at 10)
    EvolutionChamber,  // Unlocks higher tier evolutions
    EssencePool,       // Stores more Essence (1000 per level)
    NeuralWeb,         // ForgeMemory boost (+10% search quality per level)
    Armory,            // +5% unit attack per level
    Sanctuary,         // +5% unit defense per level
    Arcanum,           // +10% AI quality per level
    WarCouncil,        // Unlocks swarm analytics + auto-assign
}

impl BuildingType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Nest => "nest",
            Self::EvolutionChamber => "evolution_chamber",
            Self::EssencePool => "essence_pool",
            Self::NeuralWeb => "neural_web",
            Self::Armory => "armory",
            Self::Sanctuary => "sanctuary",
            Self::Arcanum => "arcanum",
            Self::WarCouncil => "war_council",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "nest" => Self::Nest,
            "evolution_chamber" => Self::EvolutionChamber,
            "essence_pool" => Self::EssencePool,
            "neural_web" => Self::NeuralWeb,
            "armory" => Self::Armory,
            "sanctuary" => Self::Sanctuary,
            "arcanum" => Self::Arcanum,
            "war_council" => Self::WarCouncil,
            _ => Self::Nest,
        }
    }

    pub(crate) fn max_level(&self) -> u32 {
        match self {
            Self::Nest => 20,
            Self::EvolutionChamber => 4,
            Self::EssencePool => 10,
            Self::NeuralWeb => 10,
            Self::Armory => 10,
            Self::Sanctuary => 10,
            Self::Arcanum => 10,
            Self::WarCouncil => 5,
        }
    }

    pub(crate) fn base_upgrade_cost(&self) -> u64 {
        match self {
            Self::Nest => 100,
            Self::EvolutionChamber => 300,
            Self::EssencePool => 150,
            Self::NeuralWeb => 200,
            Self::Armory => 200,
            Self::Sanctuary => 200,
            Self::Arcanum => 250,
            Self::WarCouncil => 400,
        }
    }

    pub(crate) fn bonus_description(&self, level: u32) -> String {
        match self {
            Self::Nest => format!("Max units: {}", 10 + level * 5),
            Self::EvolutionChamber => format!("Unlocks Tier {} evolutions", level + 1),
            Self::EssencePool => format!("Essence cap: {}", 1000 + level * 1000),
            Self::NeuralWeb => format!("+{}% ForgeMemory search quality", level * 10),
            Self::Armory => format!("+{}% unit attack", level * 5),
            Self::Sanctuary => format!("+{}% unit defense", level * 5),
            Self::Arcanum => format!("+{}% AI quality", level * 10),
            Self::WarCouncil => format!("Analytics tier {}/5", level),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Building {
    pub id: String,
    pub building_type: BuildingType,
    pub level: u32,
    pub max_level: u32,
    pub bonus: String,
    pub upgrade_cost: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SwarmResources {
    pub essence: u64,
    pub minerals: u64,
    pub vespene: u64,    // "Arcane Gas"
    pub biomass: u64,
    pub dark_matter: u64,
}

// ---------------------------------------------------------------------------
// OGame-style Colony System — Resources, Buildings, Research, Fleet, Creep
// ---------------------------------------------------------------------------

/// Planet resources with OGame-style production rates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetResources {
    pub biomass: f64,
    pub minerals: f64,
    pub crystal: f64,
    pub spore_gas: f64,
    pub energy: i64,
    pub dark_matter: u64,
    // Production rates per hour
    pub biomass_per_hour: f64,
    pub minerals_per_hour: f64,
    pub crystal_per_hour: f64,
    pub spore_gas_per_hour: f64,
    pub energy_production: i64,
    pub energy_consumption: i64,
}

impl Default for PlanetResources {
    fn default() -> Self {
        Self {
            biomass: 500.0,
            minerals: 500.0,
            crystal: 0.0,
            spore_gas: 0.0,
            energy: 0,
            dark_matter: 0,
            biomass_per_hour: 20.0,
            minerals_per_hour: 10.0,
            crystal_per_hour: 0.0,
            spore_gas_per_hour: 0.0,
            energy_production: 0,
            energy_consumption: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanetBuildingType {
    // Resource production
    BiomassConverter,
    MineralDrill,
    CrystalSynthesizer,
    SporeExtractor,
    EnergyNest,
    // Infrastructure
    CreepGenerator,
    BroodNest,
    EvolutionLab,
    // Military
    Blighthaven,
    SporeDefense,
    // Storage
    BiomassStorage,
    MineralSilo,

    // Expanded buildings
    SpawnPool,         // Faster larva production
    HydraliskDen,      // Unlocks Tier 2 units
    UltraliskCavern,   // Unlocks Tier 3 units
    NydusNetwork,      // Instant unit transport
    SporeLauncher,     // Orbital defense cannon
    BioReactor,        // Advanced energy from biomass
    GeneticArchive,    // Stores DNA sequences permanently
    CreepTumor,        // Extends creep range rapidly
    PsionicLink,       // Connects to other player colonies
    ObservationSpire,  // Detects enemy fleets early

    // --- Human Faction Buildings ---
    TownHall,          // Produces Peasants, Tier 1 headquarters
    Keep,              // Upgraded Town Hall, Tier 2
    Castle,            // Upgraded Keep, Tier 3
    HumanBarracks,     // Infantry production
    LumberMill,        // Lumber processing + masonry upgrades
    HumanBlacksmith,   // Weapon and armor upgrades
    ArcaneSanctum,     // Magic unit production
    Workshop,          // Mechanical unit production
    GryphonAviary,     // Air unit production
    AltarOfKings,      // Hero revival altar
    ScoutTower,        // Basic vision tower
    GuardTower,        // Pierce damage defense tower
    CannonTower,       // Siege damage defense tower
    ArcaneTower,       // Magic damage + feedback
    Farm,              // Food supply (+6 per level)
    Marketplace,       // Trade resources between types
    Church,            // Healing aura for nearby units
    Academy,           // Advanced training facility
    SiegeWorks,        // Siege engine production
    MageTower,         // Arcane research center
    Harbor,            // Ship production for human fleet
    FortressWall,      // Defensive fortification

    // --- Demon Faction Buildings (22) ---
    InfernalPit,       // Main hall, produces Imps
    HellfireForge,     // Military production
    SoulWell,          // Resource extraction (Demon Souls)
    BrimstoneRefinery, // Refines Brimstone
    DarkAltar,         // Hero summoning
    DemonGate,         // Portal for reinforcements
    TortureChamber,    // Research building
    LavaFoundry,       // Siege weapons
    ImpBarracks,       // Basic military
    SuccubusDen,       // Specialist units
    HellhoundKennel,   // Fast attack units
    InfernalTower,     // Defensive tower, fire damage
    ChaosSpire,        // Magic tower, chaos bolt
    WrathEngine,       // Heavy siege
    BloodPool,         // Healing facility
    SummoningCircle,   // Elite unit production
    HellfireWall,      // Wall defense + fire aura
    ShadowMarket,      // Trading post
    DoomSpire,         // Advanced research
    CorruptionNode,    // Terrain spread (Hellfire Corruption)
    AbyssalShipyard,   // Ship production
    ThroneOfAgony,     // Faction capital upgrade

    // --- Undead Faction Buildings (22) ---
    Necropolis,        // Main hall, produces Ghouls
    UndeadCrypt,       // Basic military production
    Graveyard,         // Corpse collection, enables undead raising
    PlagueCauldron,    // Eiter Essence production
    AltarOfDarkness,   // Hero summoning
    UndeadSlaughterhouse, // Advanced military
    TempleOfTheDamned, // Caster training
    BoneForge,         // Bone constructs/siege
    Ziggurat,          // Basic tower + supply
    SpiritTower,       // Upgrade Ziggurat, frost damage tower
    NerubianTower,     // Upgrade Ziggurat, web + slow
    TombOfRelics,      // Artifact shop
    UndeadBoneyard,    // Air unit production (Frost Wyrms)
    SacrificialPit,    // Tier upgrades
    NecrosisSpreader,  // Terrain spread (Necrosis)
    Ossuary,           // Bone storage/research
    PlagueLab,         // Disease research
    BoneWall,          // Wall defense + damage reflect
    SoulCage,          // Captures enemy souls for power
    SpectralMarket,    // Trading post
    GhostShipyard,     // Ship production
    CitadelOfUndeath,  // Faction capital upgrade
}

impl PlanetBuildingType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::BiomassConverter => "biomass_converter",
            Self::MineralDrill => "mineral_drill",
            Self::CrystalSynthesizer => "crystal_synthesizer",
            Self::SporeExtractor => "spore_extractor",
            Self::EnergyNest => "energy_nest",
            Self::CreepGenerator => "creep_generator",
            Self::BroodNest => "brood_nest",
            Self::EvolutionLab => "evolution_lab",
            Self::Blighthaven => "blighthaven",
            Self::SporeDefense => "spore_defense",
            Self::BiomassStorage => "biomass_storage",
            Self::MineralSilo => "mineral_silo",
            Self::SpawnPool => "spawn_pool",
            Self::HydraliskDen => "hydralisk_den",
            Self::UltraliskCavern => "ultralisk_cavern",
            Self::NydusNetwork => "nydus_network",
            Self::SporeLauncher => "spore_launcher",
            Self::BioReactor => "bio_reactor",
            Self::GeneticArchive => "genetic_archive",
            Self::CreepTumor => "creep_tumor",
            Self::PsionicLink => "psionic_link",
            Self::ObservationSpire => "observation_spire",
            // Human faction
            Self::TownHall => "town_hall",
            Self::Keep => "keep",
            Self::Castle => "castle",
            Self::HumanBarracks => "human_barracks",
            Self::LumberMill => "lumber_mill",
            Self::HumanBlacksmith => "human_blacksmith",
            Self::ArcaneSanctum => "arcane_sanctum",
            Self::Workshop => "workshop",
            Self::GryphonAviary => "gryphon_aviary",
            Self::AltarOfKings => "altar_of_kings",
            Self::ScoutTower => "scout_tower",
            Self::GuardTower => "guard_tower",
            Self::CannonTower => "cannon_tower",
            Self::ArcaneTower => "arcane_tower",
            Self::Farm => "farm",
            Self::Marketplace => "marketplace",
            Self::Church => "church",
            Self::Academy => "academy",
            Self::SiegeWorks => "siege_works",
            Self::MageTower => "mage_tower",
            Self::Harbor => "harbor",
            Self::FortressWall => "fortress_wall",
            // Demon buildings
            Self::InfernalPit => "infernal_pit",
            Self::HellfireForge => "hellfire_forge",
            Self::SoulWell => "soul_well",
            Self::BrimstoneRefinery => "brimstone_refinery",
            Self::DarkAltar => "dark_altar",
            Self::DemonGate => "demon_gate",
            Self::TortureChamber => "torture_chamber",
            Self::LavaFoundry => "lava_foundry",
            Self::ImpBarracks => "imp_barracks",
            Self::SuccubusDen => "succubus_den",
            Self::HellhoundKennel => "hellhound_kennel",
            Self::InfernalTower => "infernal_tower",
            Self::ChaosSpire => "chaos_spire",
            Self::WrathEngine => "wrath_engine",
            Self::BloodPool => "blood_pool",
            Self::SummoningCircle => "summoning_circle",
            Self::HellfireWall => "hellfire_wall",
            Self::ShadowMarket => "shadow_market",
            Self::DoomSpire => "doom_spire",
            Self::CorruptionNode => "corruption_node",
            Self::AbyssalShipyard => "abyssal_shipyard",
            Self::ThroneOfAgony => "throne_of_agony",
            // Undead buildings
            Self::Necropolis => "necropolis",
            Self::UndeadCrypt => "undead_crypt",
            Self::Graveyard => "graveyard",
            Self::PlagueCauldron => "plague_cauldron",
            Self::AltarOfDarkness => "altar_of_darkness",
            Self::UndeadSlaughterhouse => "undead_slaughterhouse",
            Self::TempleOfTheDamned => "temple_of_the_damned",
            Self::BoneForge => "bone_forge",
            Self::Ziggurat => "ziggurat",
            Self::SpiritTower => "spirit_tower",
            Self::NerubianTower => "nerubian_tower",
            Self::TombOfRelics => "tomb_of_relics",
            Self::UndeadBoneyard => "undead_boneyard",
            Self::SacrificialPit => "sacrificial_pit",
            Self::NecrosisSpreader => "necrosis_spreader",
            Self::Ossuary => "ossuary",
            Self::PlagueLab => "plague_lab",
            Self::BoneWall => "bone_wall",
            Self::SoulCage => "soul_cage",
            Self::SpectralMarket => "spectral_market",
            Self::GhostShipyard => "ghost_shipyard",
            Self::CitadelOfUndeath => "citadel_of_undeath",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "biomass_converter" => Self::BiomassConverter,
            "mineral_drill" => Self::MineralDrill,
            "crystal_synthesizer" => Self::CrystalSynthesizer,
            "spore_extractor" => Self::SporeExtractor,
            "energy_nest" => Self::EnergyNest,
            "creep_generator" => Self::CreepGenerator,
            "brood_nest" => Self::BroodNest,
            "evolution_lab" => Self::EvolutionLab,
            "blighthaven" => Self::Blighthaven,
            "spore_defense" => Self::SporeDefense,
            "biomass_storage" => Self::BiomassStorage,
            "mineral_silo" => Self::MineralSilo,
            "spawn_pool" => Self::SpawnPool,
            "hydralisk_den" => Self::HydraliskDen,
            "ultralisk_cavern" => Self::UltraliskCavern,
            "nydus_network" => Self::NydusNetwork,
            "spore_launcher" => Self::SporeLauncher,
            "bio_reactor" => Self::BioReactor,
            "genetic_archive" => Self::GeneticArchive,
            "creep_tumor" => Self::CreepTumor,
            "psionic_link" => Self::PsionicLink,
            "observation_spire" => Self::ObservationSpire,
            // Human faction
            "town_hall" => Self::TownHall,
            "keep" => Self::Keep,
            "castle" => Self::Castle,
            "human_barracks" => Self::HumanBarracks,
            "lumber_mill" => Self::LumberMill,
            "human_blacksmith" => Self::HumanBlacksmith,
            "arcane_sanctum" => Self::ArcaneSanctum,
            "workshop" => Self::Workshop,
            "gryphon_aviary" => Self::GryphonAviary,
            "altar_of_kings" => Self::AltarOfKings,
            "scout_tower" => Self::ScoutTower,
            "guard_tower" => Self::GuardTower,
            "cannon_tower" => Self::CannonTower,
            "arcane_tower" => Self::ArcaneTower,
            "farm" => Self::Farm,
            "marketplace" => Self::Marketplace,
            "church" => Self::Church,
            "academy" => Self::Academy,
            "siege_works" => Self::SiegeWorks,
            "mage_tower" => Self::MageTower,
            "harbor" => Self::Harbor,
            "fortress_wall" => Self::FortressWall,
            // Demon buildings
            "infernal_pit" => Self::InfernalPit,
            "hellfire_forge" => Self::HellfireForge,
            "soul_well" => Self::SoulWell,
            "brimstone_refinery" => Self::BrimstoneRefinery,
            "dark_altar" => Self::DarkAltar,
            "demon_gate" => Self::DemonGate,
            "torture_chamber" => Self::TortureChamber,
            "lava_foundry" => Self::LavaFoundry,
            "imp_barracks" => Self::ImpBarracks,
            "succubus_den" => Self::SuccubusDen,
            "hellhound_kennel" => Self::HellhoundKennel,
            "infernal_tower" => Self::InfernalTower,
            "chaos_spire" => Self::ChaosSpire,
            "wrath_engine" => Self::WrathEngine,
            "blood_pool" => Self::BloodPool,
            "summoning_circle" => Self::SummoningCircle,
            "hellfire_wall" => Self::HellfireWall,
            "shadow_market" => Self::ShadowMarket,
            "doom_spire" => Self::DoomSpire,
            "corruption_node" => Self::CorruptionNode,
            "abyssal_shipyard" => Self::AbyssalShipyard,
            "throne_of_agony" => Self::ThroneOfAgony,
            // Undead buildings
            "necropolis" => Self::Necropolis,
            "undead_crypt" => Self::UndeadCrypt,
            "graveyard" => Self::Graveyard,
            "plague_cauldron" => Self::PlagueCauldron,
            "altar_of_darkness" => Self::AltarOfDarkness,
            "undead_slaughterhouse" => Self::UndeadSlaughterhouse,
            "temple_of_the_damned" => Self::TempleOfTheDamned,
            "bone_forge" => Self::BoneForge,
            "ziggurat" => Self::Ziggurat,
            "spirit_tower" => Self::SpiritTower,
            "nerubian_tower" => Self::NerubianTower,
            "tomb_of_relics" => Self::TombOfRelics,
            "undead_boneyard" => Self::UndeadBoneyard,
            "sacrificial_pit" => Self::SacrificialPit,
            "necrosis_spreader" => Self::NecrosisSpreader,
            "ossuary" => Self::Ossuary,
            "plague_lab" => Self::PlagueLab,
            "bone_wall" => Self::BoneWall,
            "soul_cage" => Self::SoulCage,
            "spectral_market" => Self::SpectralMarket,
            "ghost_shipyard" => Self::GhostShipyard,
            "citadel_of_undeath" => Self::CitadelOfUndeath,
            _ => Self::BiomassConverter,
        }
    }

    pub(crate) fn display_name(&self) -> &'static str {
        match self {
            Self::BiomassConverter => "Biomass Converter",
            Self::MineralDrill => "Mineral Drill",
            Self::CrystalSynthesizer => "Crystal Synthesizer",
            Self::SporeExtractor => "Spore Extractor",
            Self::EnergyNest => "Energy Nest",
            Self::CreepGenerator => "Creep Generator",
            Self::BroodNest => "Brood Nest",
            Self::EvolutionLab => "Evolution Lab",
            Self::Blighthaven => "Blighthaven",
            Self::SporeDefense => "Spore Defense",
            Self::BiomassStorage => "Biomass Storage",
            Self::MineralSilo => "Mineral Silo",
            Self::SpawnPool => "Spawn Pool",
            Self::HydraliskDen => "Hydralisk Den",
            Self::UltraliskCavern => "Ultralisk Cavern",
            Self::NydusNetwork => "Nydus Network",
            Self::SporeLauncher => "Spore Launcher",
            Self::BioReactor => "Bio-Reactor",
            Self::GeneticArchive => "Genetic Archive",
            Self::CreepTumor => "Creep Tumor",
            Self::PsionicLink => "Psionic Link",
            Self::ObservationSpire => "Observation Spire",
            // Human faction
            Self::TownHall => "Town Hall",
            Self::Keep => "Keep",
            Self::Castle => "Castle",
            Self::HumanBarracks => "Barracks",
            Self::LumberMill => "Lumber Mill",
            Self::HumanBlacksmith => "Blacksmith",
            Self::ArcaneSanctum => "Arcane Sanctum",
            Self::Workshop => "Workshop",
            Self::GryphonAviary => "Gryphon Aviary",
            Self::AltarOfKings => "Altar of Kings",
            Self::ScoutTower => "Scout Tower",
            Self::GuardTower => "Guard Tower",
            Self::CannonTower => "Cannon Tower",
            Self::ArcaneTower => "Arcane Tower",
            Self::Farm => "Farm",
            Self::Marketplace => "Marketplace",
            Self::Church => "Church",
            Self::Academy => "Academy",
            Self::SiegeWorks => "Siege Works",
            Self::MageTower => "Mage Tower",
            Self::Harbor => "Harbor",
            Self::FortressWall => "Fortress Wall",
            // Demon buildings
            Self::InfernalPit => "Infernal Pit",
            Self::HellfireForge => "Hellfire Forge",
            Self::SoulWell => "Soul Well",
            Self::BrimstoneRefinery => "Brimstone Refinery",
            Self::DarkAltar => "Dark Altar",
            Self::DemonGate => "Demon Gate",
            Self::TortureChamber => "Torture Chamber",
            Self::LavaFoundry => "Lava Foundry",
            Self::ImpBarracks => "Imp Barracks",
            Self::SuccubusDen => "Succubus Den",
            Self::HellhoundKennel => "Hellhound Kennel",
            Self::InfernalTower => "Infernal Tower",
            Self::ChaosSpire => "Chaos Spire",
            Self::WrathEngine => "Wrath Engine",
            Self::BloodPool => "Blood Pool",
            Self::SummoningCircle => "Summoning Circle",
            Self::HellfireWall => "Hellfire Wall",
            Self::ShadowMarket => "Shadow Market",
            Self::DoomSpire => "Doom Spire",
            Self::CorruptionNode => "Corruption Node",
            Self::AbyssalShipyard => "Abyssal Shipyard",
            Self::ThroneOfAgony => "Throne of Agony",
            // Undead buildings
            Self::Necropolis => "Necropolis",
            Self::UndeadCrypt => "Crypt",
            Self::Graveyard => "Graveyard",
            Self::PlagueCauldron => "Plague Cauldron",
            Self::AltarOfDarkness => "Altar of Darkness",
            Self::UndeadSlaughterhouse => "Slaughterhouse",
            Self::TempleOfTheDamned => "Temple of the Damned",
            Self::BoneForge => "Bone Forge",
            Self::Ziggurat => "Ziggurat",
            Self::SpiritTower => "Spirit Tower",
            Self::NerubianTower => "Nerubian Tower",
            Self::TombOfRelics => "Tomb of Relics",
            Self::UndeadBoneyard => "Boneyard",
            Self::SacrificialPit => "Sacrificial Pit",
            Self::NecrosisSpreader => "Necrosis Spreader",
            Self::Ossuary => "Ossuary",
            Self::PlagueLab => "Plague Lab",
            Self::BoneWall => "Bone Wall",
            Self::SoulCage => "Soul Cage",
            Self::SpectralMarket => "Spectral Market",
            Self::GhostShipyard => "Ghost Shipyard",
            Self::CitadelOfUndeath => "Citadel of Undeath",
        }
    }

    pub(crate) fn description(&self) -> &'static str {
        match self {
            Self::BiomassConverter => "Digests flora and fauna into biomass for the swarm.",
            Self::MineralDrill => "Extracts minerals from deep underground deposits.",
            Self::CrystalSynthesizer => "Grows rare crystals through bio-alchemical synthesis.",
            Self::SporeExtractor => "Harvests spore gas from underground vents.",
            Self::EnergyNest => "Bio-luminescent energy generation for the colony.",
            Self::CreepGenerator => "Spreads living creep across the planet surface.",
            Self::BroodNest => "Increases maximum unit capacity for the swarm.",
            Self::EvolutionLab => "Required for researching new technologies.",
            Self::Blighthaven => "Orbital shipyard for constructing the bio-fleet.",
            Self::SporeDefense => "Planetary defense turrets against enemy raids.",
            Self::BiomassStorage => "Increases maximum biomass storage capacity.",
            Self::MineralSilo => "Increases maximum mineral storage capacity.",
            Self::SpawnPool => "Accelerates larva production for faster unit spawning.",
            Self::HydraliskDen => "Unlocks Tier 2 unit evolutions for the swarm.",
            Self::UltraliskCavern => "Unlocks Tier 3 heavy assault units.",
            Self::NydusNetwork => "Enables instant unit transport between bases.",
            Self::SporeLauncher => "Orbital defense cannon that bombards enemy fleets.",
            Self::BioReactor => "Generates advanced energy from processed biomass.",
            Self::GeneticArchive => "Permanently stores discovered DNA sequences.",
            Self::CreepTumor => "Rapidly extends creep coverage across the planet.",
            Self::PsionicLink => "Enables psionic connections with allied colonies.",
            Self::ObservationSpire => "Early-warning system detecting incoming threats.",
            // Human faction
            Self::TownHall => "Central command building. Produces Peasants and stores resources.",
            Self::Keep => "Upgraded Town Hall with stronger defenses and Tier 2 access.",
            Self::Castle => "Ultimate fortification. Unlocks Tier 3 units and technologies.",
            Self::HumanBarracks => "Trains infantry units: Footmen, Riflemen, and Knights.",
            Self::LumberMill => "Processes lumber and researches masonry upgrades.",
            Self::HumanBlacksmith => "Forges weapons and armor upgrades for all units.",
            Self::ArcaneSanctum => "Trains magical units: Sorceresses and Spell Breakers.",
            Self::Workshop => "Builds mechanical siege units and flying machines.",
            Self::GryphonAviary => "Breeds and trains Gryphon Riders for aerial combat.",
            Self::AltarOfKings => "Sacred altar where fallen heroes can be revived.",
            Self::ScoutTower => "Basic watchtower providing vision over surrounding area.",
            Self::GuardTower => "Armed tower dealing pierce damage to approaching enemies.",
            Self::CannonTower => "Heavy tower with cannon dealing siege damage.",
            Self::ArcaneTower => "Magical tower that drains enemy mana with feedback.",
            Self::Farm => "Provides food supply for sustaining the army (+6 per level).",
            Self::Marketplace => "Enables resource trading between gold, lumber, and crystal.",
            Self::Church => "Sacred building with healing aura for nearby wounded units.",
            Self::Academy => "Advanced military academy for elite unit training.",
            Self::SiegeWorks => "Produces heavy siege engines: rams, trebuchets, cannons.",
            Self::MageTower => "Research center for advanced arcane technologies.",
            Self::Harbor => "Constructs ships for the human fleet.",
            Self::FortressWall => "Thick stone fortification protecting the settlement.",
            // Demon buildings
            Self::InfernalPit => "The beating heart of a demon colony. Produces Imps.",
            Self::HellfireForge => "Forges infernal weapons and armor for the demon army.",
            Self::SoulWell => "Extracts tortured souls as a resource for dark rituals.",
            Self::BrimstoneRefinery => "Refines raw brimstone into usable infernal materials.",
            Self::DarkAltar => "Summons demon heroes from the Abyss.",
            Self::DemonGate => "Opens portals to the demon realm for reinforcements.",
            Self::TortureChamber => "Research facility powered by suffering and agony.",
            Self::LavaFoundry => "Constructs devastating siege weapons from molten rock.",
            Self::ImpBarracks => "Training grounds for basic demon infantry.",
            Self::SuccubusDen => "Trains specialist seduction and infiltration units.",
            Self::HellhoundKennel => "Breeds fast attack hellhound packs.",
            Self::InfernalTower => "Defensive tower that hurls balls of hellfire.",
            Self::ChaosSpire => "Magic defense tower that fires chaos bolts.",
            Self::WrathEngine => "Massive siege engine of demonic fury.",
            Self::BloodPool => "Healing pool filled with regenerative demon blood.",
            Self::SummoningCircle => "Arcane circle for summoning elite demon units.",
            Self::HellfireWall => "Defensive wall wreathed in eternal hellfire.",
            Self::ShadowMarket => "Black market for trading infernal goods.",
            Self::DoomSpire => "Advanced research spire probing forbidden knowledge.",
            Self::CorruptionNode => "Spreads Hellfire Corruption across the planet surface.",
            Self::AbyssalShipyard => "Constructs the demon fleet from abyssal materials.",
            Self::ThroneOfAgony => "The ultimate seat of demonic power. Capital upgrade.",
            // Undead buildings
            Self::Necropolis => "Central citadel of the undead. Produces Ghouls.",
            Self::UndeadCrypt => "Houses the restless dead for basic military training.",
            Self::Graveyard => "Collects corpses to fuel the undead war machine.",
            Self::PlagueCauldron => "Brews Eiter Essence from plague and decay.",
            Self::AltarOfDarkness => "Summons undead heroes from beyond the grave.",
            Self::UndeadSlaughterhouse => "Processes bodies into advanced undead warriors.",
            Self::TempleOfTheDamned => "Trains necromancers and dark casters.",
            Self::BoneForge => "Constructs siege weapons and bone golems.",
            Self::Ziggurat => "Basic defensive tower that also provides supply.",
            Self::SpiritTower => "Upgraded Ziggurat that deals frost damage.",
            Self::NerubianTower => "Upgraded Ziggurat that webs and slows enemies.",
            Self::TombOfRelics => "Stores and sells ancient undead artifacts.",
            Self::UndeadBoneyard => "Produces air units including Frost Wyrms.",
            Self::SacrificialPit => "Enables tier upgrades through dark sacrifice.",
            Self::NecrosisSpreader => "Spreads Necrosis corruption across the planet.",
            Self::Ossuary => "Stores bones for research and construction.",
            Self::PlagueLab => "Researches devastating plagues and diseases.",
            Self::BoneWall => "Defensive wall that reflects damage back to attackers.",
            Self::SoulCage => "Captures enemy souls to fuel dark power.",
            Self::SpectralMarket => "Ghostly marketplace for spectral trade.",
            Self::GhostShipyard => "Constructs the undead ghost fleet.",
            Self::CitadelOfUndeath => "The ultimate bastion of undeath. Capital upgrade.",
        }
    }

    /// Base cost (biomass, minerals, crystal, spore_gas) and cost factor
    pub(crate) fn base_costs(&self) -> (f64, f64, f64, f64, f64) {
        // (biomass, minerals, crystal, spore_gas, factor)
        match self {
            Self::BiomassConverter =>    (60.0, 15.0, 0.0, 0.0, 1.5),
            Self::MineralDrill =>        (48.0, 24.0, 0.0, 0.0, 1.6),
            Self::CrystalSynthesizer =>  (225.0, 75.0, 0.0, 0.0, 1.5),
            Self::SporeExtractor =>      (225.0, 75.0, 0.0, 0.0, 1.5),
            Self::EnergyNest =>          (75.0, 30.0, 0.0, 0.0, 1.5),
            Self::CreepGenerator =>      (200.0, 100.0, 50.0, 0.0, 1.8),
            Self::BroodNest =>           (400.0, 120.0, 200.0, 0.0, 2.0),
            Self::EvolutionLab =>        (200.0, 400.0, 200.0, 0.0, 2.0),
            Self::Blighthaven =>         (400.0, 200.0, 100.0, 0.0, 2.0),
            Self::SporeDefense =>        (300.0, 200.0, 100.0, 50.0, 1.8),
            Self::BiomassStorage =>      (100.0, 0.0, 0.0, 0.0, 2.0),
            Self::MineralSilo =>         (100.0, 50.0, 0.0, 0.0, 2.0),
            Self::SpawnPool =>           (150.0, 75.0, 0.0, 0.0, 1.5),
            Self::HydraliskDen =>        (250.0, 150.0, 50.0, 0.0, 1.8),
            Self::UltraliskCavern =>     (500.0, 300.0, 150.0, 100.0, 2.0),
            Self::NydusNetwork =>        (300.0, 200.0, 100.0, 50.0, 1.8),
            Self::SporeLauncher =>       (400.0, 250.0, 150.0, 75.0, 1.8),
            Self::BioReactor =>          (200.0, 100.0, 50.0, 0.0, 1.6),
            Self::GeneticArchive =>      (350.0, 200.0, 200.0, 0.0, 2.0),
            Self::CreepTumor =>          (100.0, 50.0, 25.0, 0.0, 1.5),
            Self::PsionicLink =>         (600.0, 400.0, 300.0, 200.0, 2.0),
            Self::ObservationSpire =>    (300.0, 250.0, 150.0, 50.0, 1.8),
            // Human faction (biomass=gold, minerals=lumber, crystal, spore_gas, factor)
            Self::TownHall =>            (400.0, 200.0, 0.0, 0.0, 1.6),
            Self::Keep =>                (320.0, 210.0, 50.0, 0.0, 1.8),
            Self::Castle =>              (360.0, 210.0, 100.0, 0.0, 2.0),
            Self::HumanBarracks =>       (160.0, 60.0, 0.0, 0.0, 1.5),
            Self::LumberMill =>          (120.0, 0.0, 0.0, 0.0, 1.5),
            Self::HumanBlacksmith =>     (140.0, 60.0, 0.0, 0.0, 1.6),
            Self::ArcaneSanctum =>       (150.0, 140.0, 50.0, 0.0, 1.8),
            Self::Workshop =>            (140.0, 140.0, 0.0, 0.0, 1.6),
            Self::GryphonAviary =>       (140.0, 150.0, 50.0, 0.0, 1.8),
            Self::AltarOfKings =>        (180.0, 50.0, 100.0, 0.0, 2.0),
            Self::ScoutTower =>          (30.0, 20.0, 0.0, 0.0, 1.4),
            Self::GuardTower =>          (70.0, 50.0, 0.0, 0.0, 1.5),
            Self::CannonTower =>         (100.0, 100.0, 0.0, 0.0, 1.6),
            Self::ArcaneTower =>         (50.0, 100.0, 50.0, 0.0, 1.6),
            Self::Farm =>                (80.0, 20.0, 0.0, 0.0, 1.4),
            Self::Marketplace =>         (150.0, 50.0, 0.0, 0.0, 1.6),
            Self::Church =>              (200.0, 100.0, 50.0, 0.0, 1.8),
            Self::Academy =>             (180.0, 80.0, 0.0, 0.0, 1.6),
            Self::SiegeWorks =>          (200.0, 150.0, 0.0, 0.0, 1.8),
            Self::MageTower =>           (250.0, 200.0, 100.0, 0.0, 2.0),
            Self::Harbor =>              (300.0, 200.0, 50.0, 0.0, 2.0),
            Self::FortressWall =>        (100.0, 50.0, 0.0, 0.0, 1.5),
            // Demon buildings
            Self::InfernalPit =>         (400.0, 200.0, 0.0, 0.0, 2.0),
            Self::HellfireForge =>       (160.0, 80.0, 0.0, 0.0, 1.6),
            Self::SoulWell =>            (120.0, 60.0, 0.0, 0.0, 1.5),
            Self::BrimstoneRefinery =>   (150.0, 100.0, 0.0, 0.0, 1.5),
            Self::DarkAltar =>           (180.0, 50.0, 0.0, 0.0, 1.8),
            Self::DemonGate =>           (300.0, 200.0, 100.0, 0.0, 2.0),
            Self::TortureChamber =>      (200.0, 100.0, 50.0, 0.0, 2.0),
            Self::LavaFoundry =>         (250.0, 150.0, 50.0, 0.0, 1.8),
            Self::ImpBarracks =>         (100.0, 40.0, 0.0, 0.0, 1.5),
            Self::SuccubusDen =>         (200.0, 120.0, 50.0, 0.0, 1.8),
            Self::HellhoundKennel =>     (150.0, 80.0, 0.0, 0.0, 1.6),
            Self::InfernalTower =>       (80.0, 60.0, 0.0, 0.0, 1.5),
            Self::ChaosSpire =>          (120.0, 100.0, 50.0, 0.0, 1.8),
            Self::WrathEngine =>         (300.0, 250.0, 100.0, 50.0, 1.8),
            Self::BloodPool =>           (100.0, 50.0, 0.0, 0.0, 1.5),
            Self::SummoningCircle =>     (250.0, 200.0, 100.0, 50.0, 2.0),
            Self::HellfireWall =>        (80.0, 30.0, 0.0, 0.0, 1.5),
            Self::ShadowMarket =>        (150.0, 60.0, 0.0, 0.0, 1.6),
            Self::DoomSpire =>           (350.0, 250.0, 150.0, 100.0, 2.0),
            Self::CorruptionNode =>      (60.0, 30.0, 0.0, 0.0, 1.5),
            Self::AbyssalShipyard =>     (400.0, 200.0, 100.0, 0.0, 2.0),
            Self::ThroneOfAgony =>       (500.0, 300.0, 200.0, 100.0, 2.0),
            // Undead buildings
            Self::Necropolis =>          (400.0, 200.0, 0.0, 0.0, 2.0),
            Self::UndeadCrypt =>         (160.0, 80.0, 0.0, 0.0, 1.6),
            Self::Graveyard =>           (100.0, 0.0, 0.0, 0.0, 1.5),
            Self::PlagueCauldron =>      (150.0, 100.0, 0.0, 0.0, 1.5),
            Self::AltarOfDarkness =>     (180.0, 50.0, 0.0, 0.0, 1.8),
            Self::UndeadSlaughterhouse =>(200.0, 100.0, 50.0, 0.0, 1.8),
            Self::TempleOfTheDamned =>   (200.0, 120.0, 50.0, 0.0, 1.8),
            Self::BoneForge =>           (250.0, 150.0, 50.0, 0.0, 1.8),
            Self::Ziggurat =>            (50.0, 20.0, 0.0, 0.0, 1.5),
            Self::SpiritTower =>         (80.0, 40.0, 20.0, 0.0, 1.6),
            Self::NerubianTower =>       (80.0, 40.0, 20.0, 0.0, 1.6),
            Self::TombOfRelics =>        (130.0, 60.0, 0.0, 0.0, 1.6),
            Self::UndeadBoneyard =>      (250.0, 200.0, 100.0, 50.0, 2.0),
            Self::SacrificialPit =>      (150.0, 120.0, 50.0, 0.0, 1.8),
            Self::NecrosisSpreader =>    (60.0, 30.0, 0.0, 0.0, 1.5),
            Self::Ossuary =>             (200.0, 100.0, 50.0, 0.0, 1.8),
            Self::PlagueLab =>           (300.0, 200.0, 100.0, 50.0, 2.0),
            Self::BoneWall =>            (80.0, 30.0, 0.0, 0.0, 1.5),
            Self::SoulCage =>            (200.0, 150.0, 50.0, 0.0, 1.8),
            Self::SpectralMarket =>      (150.0, 60.0, 0.0, 0.0, 1.6),
            Self::GhostShipyard =>       (300.0, 200.0, 100.0, 0.0, 2.0),
            Self::CitadelOfUndeath =>    (500.0, 300.0, 200.0, 100.0, 2.0),
        }
    }

    /// Energy delta per level: positive = production, negative = consumption
    pub(crate) fn energy_per_level(&self) -> i64 {
        match self {
            Self::EnergyNest => 22,   // Produces energy
            Self::BiomassConverter => -10,
            Self::MineralDrill => -10,
            Self::CrystalSynthesizer => -20,
            Self::SporeExtractor => -30,
            Self::CreepGenerator => -15,
            Self::Blighthaven => -25,
            Self::BioReactor => 30,   // Produces energy
            Self::SpawnPool => -8,
            Self::HydraliskDen => -12,
            Self::UltraliskCavern => -20,
            Self::NydusNetwork => -18,
            Self::SporeLauncher => -22,
            Self::GeneticArchive => -10,
            Self::CreepTumor => -5,
            Self::PsionicLink => -35,
            Self::ObservationSpire => -15,
            // Human faction
            Self::Farm => 5,             // Produces energy (windmills)
            Self::TownHall => -8,
            Self::Keep => -12,
            Self::Castle => -18,
            Self::HumanBarracks => -6,
            Self::LumberMill => -5,
            Self::HumanBlacksmith => -8,
            Self::ArcaneSanctum => -15,
            Self::Workshop => -12,
            Self::GryphonAviary => -10,
            Self::AltarOfKings => -20,
            Self::ScoutTower => -2,
            Self::GuardTower => -5,
            Self::CannonTower => -8,
            Self::ArcaneTower => -10,
            Self::Marketplace => -5,
            Self::Church => -8,
            Self::Academy => -10,
            Self::SiegeWorks => -12,
            Self::MageTower => -18,
            Self::Harbor => -15,
            Self::FortressWall => -3,
            // Demon buildings
            Self::InfernalPit => -15,
            Self::HellfireForge => -12,
            Self::SoulWell => 20,
            Self::BrimstoneRefinery => -10,
            Self::DarkAltar => -8,
            Self::DemonGate => -25,
            Self::TortureChamber => -10,
            Self::LavaFoundry => -18,
            Self::ImpBarracks => -5,
            Self::SuccubusDen => -8,
            Self::HellhoundKennel => -6,
            Self::InfernalTower => -5,
            Self::ChaosSpire => -10,
            Self::WrathEngine => -20,
            Self::BloodPool => -5,
            Self::SummoningCircle => -15,
            Self::HellfireWall => 0,
            Self::ShadowMarket => -5,
            Self::DoomSpire => -18,
            Self::CorruptionNode => -3,
            Self::AbyssalShipyard => -25,
            Self::ThroneOfAgony => -30,
            // Undead buildings
            Self::Necropolis => -15,
            Self::UndeadCrypt => -8,
            Self::Graveyard => 0,
            Self::PlagueCauldron => -10,
            Self::AltarOfDarkness => -8,
            Self::UndeadSlaughterhouse => -12,
            Self::TempleOfTheDamned => -10,
            Self::BoneForge => -15,
            Self::Ziggurat => 0,
            Self::SpiritTower => -5,
            Self::NerubianTower => -5,
            Self::TombOfRelics => -3,
            Self::UndeadBoneyard => -20,
            Self::SacrificialPit => 15,
            Self::NecrosisSpreader => -3,
            Self::Ossuary => -5,
            Self::PlagueLab => -12,
            Self::BoneWall => 0,
            Self::SoulCage => 18,
            Self::SpectralMarket => -5,
            Self::GhostShipyard => -25,
            Self::CitadelOfUndeath => -30,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetBuilding {
    pub building_type: PlanetBuildingType,
    pub level: u32,
    pub upgrading: bool,
    pub upgrade_finish: Option<String>,
    pub display_name: String,
    pub description: String,
    pub cost_biomass: f64,
    pub cost_minerals: f64,
    pub cost_crystal: f64,
    pub cost_spore_gas: f64,
    pub build_time_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TechType {
    Genetics,
    ArmorPlating,
    WeaponSystems,
    PropulsionDrive,
    SwarmIntelligence,
    Regeneration,
    MutationTech,
    CreepBiology,
    SpaceFaring,
    DarkMatterResearch,

    // Expanded technologies
    BioPlasma,          // Unlocks plasma weapons for ships
    AdaptiveArmor,      // Armor regenerates during battle
    NeuralNetwork,      // Units share XP within range
    TunnelDigestion,    // Underground resource extraction
    WarpDrive,          // Faster fleet travel (2x)
    PsionicScream,      // AoE stun ability
    Symbiosis,          // Units heal each other passively
    MassEvolution,      // Evolve 5 units simultaneously
    OrbitalBombardment, // Attack planets from orbit
    HiveMindLink,       // All units +10% stats when Matriarch alive

    // --- Human Faction Technologies ---
    IronForging,        // +1 melee damage
    SteelForging,       // +2 melee damage
    MithrilForging,     // +3 melee damage
    IronPlating,        // +1 armor
    SteelPlating,       // +2 armor
    MithrilPlating,     // +3 armor
    LongRifles,         // +1 range damage
    Rifling,            // +2 range damage
    Masonry,            // +20% building HP
    AdvancedMasonry,    // +40% building HP
    HumanFortification, // +60% building HP
    AnimalHusbandry,    // +20% cavalry speed
    CloudTechnology,    // Flying unit +25% HP
    ArcaneTraining,     // +15% spell damage
    HolyLightTech,      // Unlock Paladin healing
    BlizzardResearch,   // Unlock AoE blizzard spell
    Telescope,          // +20% scout vision range
    CombustionEngine,   // Siege +30% speed
    Logistics,          // +10% army supply capacity
    Diplomacy,          // -10% trade tax

    // --- Demon Faction Technologies (20) ---
    HellfireWeaponsI,    // +1 fire damage
    HellfireWeaponsII,   // +2 fire damage
    HellfireWeaponsIII,  // +3 fire damage
    DemonHideI,          // +1 armor
    DemonHideII,         // +2 armor
    DemonHideIII,        // +3 armor
    InfernalSpeed,       // +15% movement speed
    SoulAbsorption,      // Lifesteal 5% on hit
    ChaosMagic,          // +20% spell damage
    HellfireMastery,     // Fire attacks gain splash
    DemonWings,          // Unlock flying for select units
    TortureExpertise,    // +25% research speed
    PortalNetwork,       // +30% reinforcement speed
    BrimstoneExtraction, // +20% resource production
    CorruptionSpread,    // +50% terrain expansion
    FearAura,            // Enemy units near demons have -10% attack
    FelEngineering,      // Siege weapons +40% damage
    BloodPact,           // Hero abilities -20% cooldown
    AbyssalSummoning,    // Unlock elite demon units
    ApocalypseProtocol,  // Unlock Abyssal Maw (Deathstar)

    // --- Undead Faction Technologies (20) ---
    BoneWeaponsI,        // +1 melee damage
    BoneWeaponsII,       // +2 melee damage
    BoneWeaponsIII,      // +3 melee damage
    UnholyArmorI,        // +1 armor
    UnholyArmorII,       // +2 armor
    UnholyArmorIII,      // +3 armor
    GhoulFrenzy,         // +25% Ghoul attack speed
    DiseaseCloud,        // Ranged units apply poison DOT
    NecromancyTech,      // 30% chance to raise killed enemies
    FrostMagic,          // +20% frost spell damage
    SkeletalMastery,     // +2 skeleton limit per Necromancer
    PlagueResearch,      // +25% Eiter Essence production
    SpectralBinding,     // Ghost units +30% HP
    CorpseExplosion,     // Dead enemies deal AoE damage
    DarkRitual,          // Sacrifice units for mana
    NecrosisExpansion,   // +50% terrain spread
    BoneArmor,           // Buildings +20% HP
    SoulHarvest,         // +10% resource from kills
    LichAscension,       // Unlock Lich hero ultimate
    WorldEaterProtocol,  // Unlock World Eater (Deathstar)
}

impl TechType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Genetics => "genetics",
            Self::ArmorPlating => "armor_plating",
            Self::WeaponSystems => "weapon_systems",
            Self::PropulsionDrive => "propulsion_drive",
            Self::SwarmIntelligence => "swarm_intelligence",
            Self::Regeneration => "regeneration",
            Self::MutationTech => "mutation_tech",
            Self::CreepBiology => "creep_biology",
            Self::SpaceFaring => "space_faring",
            Self::DarkMatterResearch => "dark_matter_research",
            Self::BioPlasma => "bio_plasma",
            Self::AdaptiveArmor => "adaptive_armor",
            Self::NeuralNetwork => "neural_network",
            Self::TunnelDigestion => "tunnel_digestion",
            Self::WarpDrive => "warp_drive",
            Self::PsionicScream => "psionic_scream",
            Self::Symbiosis => "symbiosis",
            Self::MassEvolution => "mass_evolution",
            Self::OrbitalBombardment => "orbital_bombardment",
            Self::HiveMindLink => "hive_mind_link",
            // Human faction
            Self::IronForging => "iron_forging",
            Self::SteelForging => "steel_forging",
            Self::MithrilForging => "mithril_forging",
            Self::IronPlating => "iron_plating",
            Self::SteelPlating => "steel_plating",
            Self::MithrilPlating => "mithril_plating",
            Self::LongRifles => "long_rifles",
            Self::Rifling => "rifling",
            Self::Masonry => "masonry",
            Self::AdvancedMasonry => "advanced_masonry",
            Self::HumanFortification => "human_fortification",
            Self::AnimalHusbandry => "animal_husbandry",
            Self::CloudTechnology => "cloud_technology",
            Self::ArcaneTraining => "arcane_training",
            Self::HolyLightTech => "holy_light_tech",
            Self::BlizzardResearch => "blizzard_research",
            Self::Telescope => "telescope",
            Self::CombustionEngine => "combustion_engine",
            Self::Logistics => "logistics",
            Self::Diplomacy => "diplomacy",
            // Demon technologies
            Self::HellfireWeaponsI => "hellfire_weapons_i",
            Self::HellfireWeaponsII => "hellfire_weapons_ii",
            Self::HellfireWeaponsIII => "hellfire_weapons_iii",
            Self::DemonHideI => "demon_hide_i",
            Self::DemonHideII => "demon_hide_ii",
            Self::DemonHideIII => "demon_hide_iii",
            Self::InfernalSpeed => "infernal_speed",
            Self::SoulAbsorption => "soul_absorption",
            Self::ChaosMagic => "chaos_magic",
            Self::HellfireMastery => "hellfire_mastery",
            Self::DemonWings => "demon_wings",
            Self::TortureExpertise => "torture_expertise",
            Self::PortalNetwork => "portal_network",
            Self::BrimstoneExtraction => "brimstone_extraction",
            Self::CorruptionSpread => "corruption_spread",
            Self::FearAura => "fear_aura",
            Self::FelEngineering => "fel_engineering",
            Self::BloodPact => "blood_pact",
            Self::AbyssalSummoning => "abyssal_summoning",
            Self::ApocalypseProtocol => "apocalypse_protocol",
            // Undead technologies
            Self::BoneWeaponsI => "bone_weapons_i",
            Self::BoneWeaponsII => "bone_weapons_ii",
            Self::BoneWeaponsIII => "bone_weapons_iii",
            Self::UnholyArmorI => "unholy_armor_i",
            Self::UnholyArmorII => "unholy_armor_ii",
            Self::UnholyArmorIII => "unholy_armor_iii",
            Self::GhoulFrenzy => "ghoul_frenzy",
            Self::DiseaseCloud => "disease_cloud",
            Self::NecromancyTech => "necromancy_tech",
            Self::FrostMagic => "frost_magic",
            Self::SkeletalMastery => "skeletal_mastery",
            Self::PlagueResearch => "plague_research",
            Self::SpectralBinding => "spectral_binding",
            Self::CorpseExplosion => "corpse_explosion",
            Self::DarkRitual => "dark_ritual",
            Self::NecrosisExpansion => "necrosis_expansion",
            Self::BoneArmor => "bone_armor",
            Self::SoulHarvest => "soul_harvest",
            Self::LichAscension => "lich_ascension",
            Self::WorldEaterProtocol => "world_eater_protocol",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "genetics" => Self::Genetics,
            "armor_plating" => Self::ArmorPlating,
            "weapon_systems" => Self::WeaponSystems,
            "propulsion_drive" => Self::PropulsionDrive,
            "swarm_intelligence" => Self::SwarmIntelligence,
            "regeneration" => Self::Regeneration,
            "mutation_tech" => Self::MutationTech,
            "creep_biology" => Self::CreepBiology,
            "space_faring" => Self::SpaceFaring,
            "dark_matter_research" => Self::DarkMatterResearch,
            "bio_plasma" => Self::BioPlasma,
            "adaptive_armor" => Self::AdaptiveArmor,
            "neural_network" => Self::NeuralNetwork,
            "tunnel_digestion" => Self::TunnelDigestion,
            "warp_drive" => Self::WarpDrive,
            "psionic_scream" => Self::PsionicScream,
            "symbiosis" => Self::Symbiosis,
            "mass_evolution" => Self::MassEvolution,
            "orbital_bombardment" => Self::OrbitalBombardment,
            "hive_mind_link" => Self::HiveMindLink,
            // Human faction
            "iron_forging" => Self::IronForging,
            "steel_forging" => Self::SteelForging,
            "mithril_forging" => Self::MithrilForging,
            "iron_plating" => Self::IronPlating,
            "steel_plating" => Self::SteelPlating,
            "mithril_plating" => Self::MithrilPlating,
            "long_rifles" => Self::LongRifles,
            "rifling" => Self::Rifling,
            "masonry" => Self::Masonry,
            "advanced_masonry" => Self::AdvancedMasonry,
            "human_fortification" => Self::HumanFortification,
            "animal_husbandry" => Self::AnimalHusbandry,
            "cloud_technology" => Self::CloudTechnology,
            "arcane_training" => Self::ArcaneTraining,
            "holy_light_tech" => Self::HolyLightTech,
            "blizzard_research" => Self::BlizzardResearch,
            "telescope" => Self::Telescope,
            "combustion_engine" => Self::CombustionEngine,
            "logistics" => Self::Logistics,
            "diplomacy" => Self::Diplomacy,
            // Demon technologies
            "hellfire_weapons_i" => Self::HellfireWeaponsI,
            "hellfire_weapons_ii" => Self::HellfireWeaponsII,
            "hellfire_weapons_iii" => Self::HellfireWeaponsIII,
            "demon_hide_i" => Self::DemonHideI,
            "demon_hide_ii" => Self::DemonHideII,
            "demon_hide_iii" => Self::DemonHideIII,
            "infernal_speed" => Self::InfernalSpeed,
            "soul_absorption" => Self::SoulAbsorption,
            "chaos_magic" => Self::ChaosMagic,
            "hellfire_mastery" => Self::HellfireMastery,
            "demon_wings" => Self::DemonWings,
            "torture_expertise" => Self::TortureExpertise,
            "portal_network" => Self::PortalNetwork,
            "brimstone_extraction" => Self::BrimstoneExtraction,
            "corruption_spread" => Self::CorruptionSpread,
            "fear_aura" => Self::FearAura,
            "fel_engineering" => Self::FelEngineering,
            "blood_pact" => Self::BloodPact,
            "abyssal_summoning" => Self::AbyssalSummoning,
            "apocalypse_protocol" => Self::ApocalypseProtocol,
            // Undead technologies
            "bone_weapons_i" => Self::BoneWeaponsI,
            "bone_weapons_ii" => Self::BoneWeaponsII,
            "bone_weapons_iii" => Self::BoneWeaponsIII,
            "unholy_armor_i" => Self::UnholyArmorI,
            "unholy_armor_ii" => Self::UnholyArmorII,
            "unholy_armor_iii" => Self::UnholyArmorIII,
            "ghoul_frenzy" => Self::GhoulFrenzy,
            "disease_cloud" => Self::DiseaseCloud,
            "necromancy_tech" => Self::NecromancyTech,
            "frost_magic" => Self::FrostMagic,
            "skeletal_mastery" => Self::SkeletalMastery,
            "plague_research" => Self::PlagueResearch,
            "spectral_binding" => Self::SpectralBinding,
            "corpse_explosion" => Self::CorpseExplosion,
            "dark_ritual" => Self::DarkRitual,
            "necrosis_expansion" => Self::NecrosisExpansion,
            "bone_armor" => Self::BoneArmor,
            "soul_harvest" => Self::SoulHarvest,
            "lich_ascension" => Self::LichAscension,
            "world_eater_protocol" => Self::WorldEaterProtocol,
            _ => Self::Genetics,
        }
    }

    pub(crate) fn display_name(&self) -> &'static str {
        match self {
            Self::Genetics => "Genetics",
            Self::ArmorPlating => "Armor Plating",
            Self::WeaponSystems => "Weapon Systems",
            Self::PropulsionDrive => "Propulsion Drive",
            Self::SwarmIntelligence => "Swarm Intelligence",
            Self::Regeneration => "Regeneration",
            Self::MutationTech => "Mutation Tech",
            Self::CreepBiology => "Creep Biology",
            Self::SpaceFaring => "Space Faring",
            Self::DarkMatterResearch => "Dark Matter Research",
            Self::BioPlasma => "Bio-Plasma",
            Self::AdaptiveArmor => "Adaptive Armor",
            Self::NeuralNetwork => "Neural Network",
            Self::TunnelDigestion => "Tunnel Digestion",
            Self::WarpDrive => "Warp Drive",
            Self::PsionicScream => "Psionic Scream",
            Self::Symbiosis => "Symbiosis",
            Self::MassEvolution => "Mass Evolution",
            Self::OrbitalBombardment => "Orbital Bombardment",
            Self::HiveMindLink => "Hive Mind Link",
            // Human faction
            Self::IronForging => "Iron Forging",
            Self::SteelForging => "Steel Forging",
            Self::MithrilForging => "Mithril Forging",
            Self::IronPlating => "Iron Plating",
            Self::SteelPlating => "Steel Plating",
            Self::MithrilPlating => "Mithril Plating",
            Self::LongRifles => "Long Rifles",
            Self::Rifling => "Rifling",
            Self::Masonry => "Masonry",
            Self::AdvancedMasonry => "Advanced Masonry",
            Self::HumanFortification => "Fortification",
            Self::AnimalHusbandry => "Animal Husbandry",
            Self::CloudTechnology => "Cloud Technology",
            Self::ArcaneTraining => "Arcane Training",
            Self::HolyLightTech => "Holy Light",
            Self::BlizzardResearch => "Blizzard Research",
            Self::Telescope => "Telescope",
            Self::CombustionEngine => "Combustion Engine",
            Self::Logistics => "Logistics",
            Self::Diplomacy => "Diplomacy",
            // Demon technologies
            Self::HellfireWeaponsI => "Hellfire Weapons I",
            Self::HellfireWeaponsII => "Hellfire Weapons II",
            Self::HellfireWeaponsIII => "Hellfire Weapons III",
            Self::DemonHideI => "Demon Hide I",
            Self::DemonHideII => "Demon Hide II",
            Self::DemonHideIII => "Demon Hide III",
            Self::InfernalSpeed => "Infernal Speed",
            Self::SoulAbsorption => "Soul Absorption",
            Self::ChaosMagic => "Chaos Magic",
            Self::HellfireMastery => "Hellfire Mastery",
            Self::DemonWings => "Demon Wings",
            Self::TortureExpertise => "Torture Expertise",
            Self::PortalNetwork => "Portal Network",
            Self::BrimstoneExtraction => "Brimstone Extraction",
            Self::CorruptionSpread => "Corruption Spread",
            Self::FearAura => "Fear Aura",
            Self::FelEngineering => "Fel Engineering",
            Self::BloodPact => "Blood Pact",
            Self::AbyssalSummoning => "Abyssal Summoning",
            Self::ApocalypseProtocol => "Apocalypse Protocol",
            // Undead technologies
            Self::BoneWeaponsI => "Bone Weapons I",
            Self::BoneWeaponsII => "Bone Weapons II",
            Self::BoneWeaponsIII => "Bone Weapons III",
            Self::UnholyArmorI => "Unholy Armor I",
            Self::UnholyArmorII => "Unholy Armor II",
            Self::UnholyArmorIII => "Unholy Armor III",
            Self::GhoulFrenzy => "Ghoul Frenzy",
            Self::DiseaseCloud => "Disease Cloud",
            Self::NecromancyTech => "Necromancy",
            Self::FrostMagic => "Frost Magic",
            Self::SkeletalMastery => "Skeletal Mastery",
            Self::PlagueResearch => "Plague Research",
            Self::SpectralBinding => "Spectral Binding",
            Self::CorpseExplosion => "Corpse Explosion",
            Self::DarkRitual => "Dark Ritual",
            Self::NecrosisExpansion => "Necrosis Expansion",
            Self::BoneArmor => "Bone Armor",
            Self::SoulHarvest => "Soul Harvest",
            Self::LichAscension => "Lich Ascension",
            Self::WorldEaterProtocol => "World Eater Protocol",
        }
    }

    pub(crate) fn description(&self) -> &'static str {
        match self {
            Self::Genetics => "Unlocks higher tier biological units.",
            Self::ArmorPlating => "+10% unit defense per level.",
            Self::WeaponSystems => "+10% unit attack per level.",
            Self::PropulsionDrive => "Faster fleet travel speed.",
            Self::SwarmIntelligence => "+5% all production per level.",
            Self::Regeneration => "Units heal over time after battle.",
            Self::MutationTech => "Unlocks biological mutations for units.",
            Self::CreepBiology => "Faster creep spread across the planet.",
            Self::SpaceFaring => "Required for Blighthaven construction.",
            Self::DarkMatterResearch => "Unlocks dark matter shop items.",
            Self::BioPlasma => "Unlocks plasma weapons for fleet ships.",
            Self::AdaptiveArmor => "Armor regenerates slowly during battle.",
            Self::NeuralNetwork => "Units share XP gains within range.",
            Self::TunnelDigestion => "Underground resource extraction from deep deposits.",
            Self::WarpDrive => "2x fleet travel speed across systems.",
            Self::PsionicScream => "Unlocks AoE psionic stun ability.",
            Self::Symbiosis => "Units passively heal each other in proximity.",
            Self::MassEvolution => "Evolve up to 5 units simultaneously.",
            Self::OrbitalBombardment => "Fleet can attack planet buildings from orbit.",
            Self::HiveMindLink => "All units gain +10% stats when Matriarch is alive.",
            // Human faction
            Self::IronForging => "+1 melee damage for all infantry units.",
            Self::SteelForging => "+2 melee damage for all infantry units.",
            Self::MithrilForging => "+3 melee damage for all infantry units.",
            Self::IronPlating => "+1 armor for all human units.",
            Self::SteelPlating => "+2 armor for all human units.",
            Self::MithrilPlating => "+3 armor for all human units.",
            Self::LongRifles => "+1 ranged damage for Riflemen and Crossbowmen.",
            Self::Rifling => "+2 ranged damage for Riflemen and Crossbowmen.",
            Self::Masonry => "+20% HP for all human buildings.",
            Self::AdvancedMasonry => "+40% HP for all human buildings.",
            Self::HumanFortification => "+60% HP for all human buildings.",
            Self::AnimalHusbandry => "+20% movement speed for cavalry units.",
            Self::CloudTechnology => "+25% HP for all flying units.",
            Self::ArcaneTraining => "+15% spell damage for caster units.",
            Self::HolyLightTech => "Unlocks Paladin healing ability.",
            Self::BlizzardResearch => "Unlocks AoE Blizzard spell for Archmage.",
            Self::Telescope => "+20% vision range for Scout Towers.",
            Self::CombustionEngine => "+30% movement speed for siege units.",
            Self::Logistics => "+10% army supply capacity.",
            Self::Diplomacy => "-10% trade tax at Marketplace.",
            // Demon technologies
            Self::HellfireWeaponsI => "+1 fire damage to all demon melee attacks.",
            Self::HellfireWeaponsII => "+2 fire damage to all demon melee attacks.",
            Self::HellfireWeaponsIII => "+3 fire damage to all demon melee attacks.",
            Self::DemonHideI => "+1 armor to all demon units.",
            Self::DemonHideII => "+2 armor to all demon units.",
            Self::DemonHideIII => "+3 armor to all demon units.",
            Self::InfernalSpeed => "+15% movement speed for all demon units.",
            Self::SoulAbsorption => "Demon units lifesteal 5% of damage dealt.",
            Self::ChaosMagic => "+20% spell damage for demon casters.",
            Self::HellfireMastery => "Fire attacks gain splash damage in a small area.",
            Self::DemonWings => "Unlocks flying capability for select demon units.",
            Self::TortureExpertise => "+25% research speed in Torture Chamber.",
            Self::PortalNetwork => "+30% reinforcement arrival speed via Demon Gate.",
            Self::BrimstoneExtraction => "+20% brimstone resource production.",
            Self::CorruptionSpread => "+50% Hellfire Corruption terrain expansion speed.",
            Self::FearAura => "Enemy units near demons suffer -10% attack power.",
            Self::FelEngineering => "Demon siege weapons deal +40% damage.",
            Self::BloodPact => "Hero abilities have -20% cooldown time.",
            Self::AbyssalSummoning => "Unlocks elite demon units from the Abyss.",
            Self::ApocalypseProtocol => "Unlocks the Abyssal Maw dreadnought.",
            // Undead technologies
            Self::BoneWeaponsI => "+1 melee damage to all undead units.",
            Self::BoneWeaponsII => "+2 melee damage to all undead units.",
            Self::BoneWeaponsIII => "+3 melee damage to all undead units.",
            Self::UnholyArmorI => "+1 armor to all undead units.",
            Self::UnholyArmorII => "+2 armor to all undead units.",
            Self::UnholyArmorIII => "+3 armor to all undead units.",
            Self::GhoulFrenzy => "+25% attack speed for Ghoul units.",
            Self::DiseaseCloud => "Ranged undead units apply poison damage over time.",
            Self::NecromancyTech => "30% chance to raise slain enemies as skeletons.",
            Self::FrostMagic => "+20% frost spell damage for undead casters.",
            Self::SkeletalMastery => "+2 skeleton summon limit per Necromancer.",
            Self::PlagueResearch => "+25% Eiter Essence production rate.",
            Self::SpectralBinding => "Ghost and spectral units gain +30% HP.",
            Self::CorpseExplosion => "Dead enemy units explode for area damage.",
            Self::DarkRitual => "Sacrifice own units to restore mana.",
            Self::NecrosisExpansion => "+50% Necrosis terrain spread speed.",
            Self::BoneArmor => "All undead buildings gain +20% HP.",
            Self::SoulHarvest => "+10% resource gain from enemy unit kills.",
            Self::LichAscension => "Unlocks the Lich hero ultimate ability.",
            Self::WorldEaterProtocol => "Unlocks the World Eater dreadnought.",
        }
    }

    pub(crate) fn base_costs(&self) -> (f64, f64, f64, f64, f64) {
        // (biomass, minerals, crystal, spore_gas, factor)
        match self {
            Self::Genetics =>          (200.0, 100.0, 100.0, 0.0, 2.0),
            Self::ArmorPlating =>      (100.0, 200.0, 0.0, 0.0, 2.0),
            Self::WeaponSystems =>     (200.0, 100.0, 0.0, 0.0, 2.0),
            Self::PropulsionDrive =>   (100.0, 200.0, 100.0, 0.0, 2.0),
            Self::SwarmIntelligence => (300.0, 200.0, 100.0, 0.0, 2.0),
            Self::Regeneration =>      (200.0, 100.0, 200.0, 0.0, 2.0),
            Self::MutationTech =>      (400.0, 200.0, 200.0, 100.0, 2.0),
            Self::CreepBiology =>      (150.0, 50.0, 50.0, 0.0, 2.0),
            Self::SpaceFaring =>       (500.0, 400.0, 300.0, 200.0, 2.0),
            Self::DarkMatterResearch =>(800.0, 400.0, 400.0, 200.0, 2.0),
            Self::BioPlasma =>         (400.0, 200.0, 300.0, 100.0, 2.0),
            Self::AdaptiveArmor =>     (300.0, 300.0, 150.0, 0.0, 2.0),
            Self::NeuralNetwork =>     (350.0, 150.0, 200.0, 100.0, 2.0),
            Self::TunnelDigestion =>   (200.0, 300.0, 50.0, 0.0, 2.0),
            Self::WarpDrive =>         (500.0, 300.0, 300.0, 200.0, 2.0),
            Self::PsionicScream =>     (400.0, 100.0, 300.0, 200.0, 2.0),
            Self::Symbiosis =>         (250.0, 100.0, 200.0, 50.0, 2.0),
            Self::MassEvolution =>     (600.0, 300.0, 300.0, 150.0, 2.0),
            Self::OrbitalBombardment =>(700.0, 500.0, 400.0, 300.0, 2.0),
            Self::HiveMindLink =>      (900.0, 500.0, 500.0, 300.0, 2.0),
            // Human faction (biomass=gold, minerals=lumber, crystal, spore_gas, factor)
            Self::IronForging =>       (100.0, 50.0, 0.0, 0.0, 2.0),
            Self::SteelForging =>      (200.0, 100.0, 0.0, 0.0, 2.0),
            Self::MithrilForging =>    (400.0, 200.0, 100.0, 0.0, 2.0),
            Self::IronPlating =>       (100.0, 75.0, 0.0, 0.0, 2.0),
            Self::SteelPlating =>      (200.0, 150.0, 0.0, 0.0, 2.0),
            Self::MithrilPlating =>    (400.0, 300.0, 100.0, 0.0, 2.0),
            Self::LongRifles =>        (150.0, 50.0, 0.0, 0.0, 2.0),
            Self::Rifling =>           (300.0, 100.0, 50.0, 0.0, 2.0),
            Self::Masonry =>           (150.0, 75.0, 0.0, 0.0, 2.0),
            Self::AdvancedMasonry =>   (300.0, 150.0, 0.0, 0.0, 2.0),
            Self::HumanFortification =>(500.0, 250.0, 50.0, 0.0, 2.0),
            Self::AnimalHusbandry =>   (200.0, 100.0, 0.0, 0.0, 2.0),
            Self::CloudTechnology =>   (250.0, 150.0, 50.0, 0.0, 2.0),
            Self::ArcaneTraining =>    (200.0, 200.0, 100.0, 0.0, 2.0),
            Self::HolyLightTech =>     (300.0, 150.0, 100.0, 0.0, 2.0),
            Self::BlizzardResearch =>  (400.0, 200.0, 150.0, 0.0, 2.0),
            Self::Telescope =>         (100.0, 50.0, 0.0, 0.0, 2.0),
            Self::CombustionEngine =>  (350.0, 200.0, 50.0, 0.0, 2.0),
            Self::Logistics =>         (200.0, 100.0, 0.0, 0.0, 2.0),
            Self::Diplomacy =>         (150.0, 75.0, 0.0, 0.0, 2.0),
            // Demon technologies
            Self::HellfireWeaponsI =>    (150.0, 100.0, 0.0, 0.0, 2.0),
            Self::HellfireWeaponsII =>   (300.0, 200.0, 50.0, 0.0, 2.0),
            Self::HellfireWeaponsIII =>  (600.0, 400.0, 150.0, 50.0, 2.0),
            Self::DemonHideI =>         (100.0, 150.0, 0.0, 0.0, 2.0),
            Self::DemonHideII =>        (200.0, 300.0, 50.0, 0.0, 2.0),
            Self::DemonHideIII =>       (400.0, 600.0, 150.0, 50.0, 2.0),
            Self::InfernalSpeed =>      (250.0, 150.0, 100.0, 0.0, 2.0),
            Self::SoulAbsorption =>     (300.0, 200.0, 150.0, 50.0, 2.0),
            Self::ChaosMagic =>         (400.0, 150.0, 250.0, 100.0, 2.0),
            Self::HellfireMastery =>    (500.0, 300.0, 200.0, 100.0, 2.0),
            Self::DemonWings =>         (350.0, 200.0, 200.0, 100.0, 2.0),
            Self::TortureExpertise =>   (200.0, 100.0, 100.0, 0.0, 2.0),
            Self::PortalNetwork =>      (400.0, 300.0, 200.0, 100.0, 2.0),
            Self::BrimstoneExtraction =>(250.0, 200.0, 50.0, 0.0, 2.0),
            Self::CorruptionSpread =>   (200.0, 100.0, 100.0, 50.0, 2.0),
            Self::FearAura =>           (350.0, 200.0, 200.0, 100.0, 2.0),
            Self::FelEngineering =>     (500.0, 400.0, 200.0, 100.0, 2.0),
            Self::BloodPact =>          (400.0, 200.0, 300.0, 150.0, 2.0),
            Self::AbyssalSummoning =>   (700.0, 500.0, 400.0, 200.0, 2.0),
            Self::ApocalypseProtocol => (900.0, 600.0, 500.0, 300.0, 2.0),
            // Undead technologies
            Self::BoneWeaponsI =>       (150.0, 100.0, 0.0, 0.0, 2.0),
            Self::BoneWeaponsII =>      (300.0, 200.0, 50.0, 0.0, 2.0),
            Self::BoneWeaponsIII =>     (600.0, 400.0, 150.0, 50.0, 2.0),
            Self::UnholyArmorI =>       (100.0, 150.0, 0.0, 0.0, 2.0),
            Self::UnholyArmorII =>      (200.0, 300.0, 50.0, 0.0, 2.0),
            Self::UnholyArmorIII =>     (400.0, 600.0, 150.0, 50.0, 2.0),
            Self::GhoulFrenzy =>        (200.0, 100.0, 50.0, 0.0, 2.0),
            Self::DiseaseCloud =>       (300.0, 150.0, 150.0, 50.0, 2.0),
            Self::NecromancyTech =>     (400.0, 200.0, 200.0, 100.0, 2.0),
            Self::FrostMagic =>         (350.0, 150.0, 250.0, 100.0, 2.0),
            Self::SkeletalMastery =>    (250.0, 150.0, 100.0, 50.0, 2.0),
            Self::PlagueResearch =>     (200.0, 150.0, 50.0, 0.0, 2.0),
            Self::SpectralBinding =>    (350.0, 200.0, 200.0, 100.0, 2.0),
            Self::CorpseExplosion =>    (400.0, 250.0, 200.0, 100.0, 2.0),
            Self::DarkRitual =>         (300.0, 100.0, 250.0, 100.0, 2.0),
            Self::NecrosisExpansion =>  (200.0, 100.0, 100.0, 50.0, 2.0),
            Self::BoneArmor =>          (300.0, 300.0, 100.0, 0.0, 2.0),
            Self::SoulHarvest =>        (350.0, 200.0, 200.0, 100.0, 2.0),
            Self::LichAscension =>      (700.0, 400.0, 400.0, 200.0, 2.0),
            Self::WorldEaterProtocol => (900.0, 600.0, 500.0, 300.0, 2.0),
        }
    }

    /// Required Evolution Lab level to start this research
    pub(crate) fn required_lab_level(&self) -> u32 {
        match self {
            Self::Genetics => 1,
            Self::ArmorPlating => 1,
            Self::WeaponSystems => 1,
            Self::PropulsionDrive => 2,
            Self::SwarmIntelligence => 3,
            Self::Regeneration => 3,
            Self::MutationTech => 5,
            Self::CreepBiology => 2,
            Self::SpaceFaring => 4,
            Self::DarkMatterResearch => 7,
            Self::BioPlasma => 4,
            Self::AdaptiveArmor => 3,
            Self::NeuralNetwork => 4,
            Self::TunnelDigestion => 2,
            Self::WarpDrive => 5,
            Self::PsionicScream => 5,
            Self::Symbiosis => 3,
            Self::MassEvolution => 6,
            Self::OrbitalBombardment => 8,
            Self::HiveMindLink => 9,
            // Human faction (requires Academy level instead of Evolution Lab)
            Self::IronForging => 1,
            Self::SteelForging => 3,
            Self::MithrilForging => 5,
            Self::IronPlating => 1,
            Self::SteelPlating => 3,
            Self::MithrilPlating => 5,
            Self::LongRifles => 2,
            Self::Rifling => 4,
            Self::Masonry => 1,
            Self::AdvancedMasonry => 3,
            Self::HumanFortification => 5,
            Self::AnimalHusbandry => 2,
            Self::CloudTechnology => 4,
            Self::ArcaneTraining => 3,
            Self::HolyLightTech => 4,
            Self::BlizzardResearch => 6,
            Self::Telescope => 1,
            Self::CombustionEngine => 5,
            Self::Logistics => 2,
            Self::Diplomacy => 2,
            // Demon technologies
            Self::HellfireWeaponsI => 1,
            Self::HellfireWeaponsII => 3,
            Self::HellfireWeaponsIII => 6,
            Self::DemonHideI => 1,
            Self::DemonHideII => 3,
            Self::DemonHideIII => 6,
            Self::InfernalSpeed => 2,
            Self::SoulAbsorption => 4,
            Self::ChaosMagic => 3,
            Self::HellfireMastery => 5,
            Self::DemonWings => 4,
            Self::TortureExpertise => 2,
            Self::PortalNetwork => 5,
            Self::BrimstoneExtraction => 2,
            Self::CorruptionSpread => 3,
            Self::FearAura => 4,
            Self::FelEngineering => 6,
            Self::BloodPact => 5,
            Self::AbyssalSummoning => 8,
            Self::ApocalypseProtocol => 10,
            // Undead technologies
            Self::BoneWeaponsI => 1,
            Self::BoneWeaponsII => 3,
            Self::BoneWeaponsIII => 6,
            Self::UnholyArmorI => 1,
            Self::UnholyArmorII => 3,
            Self::UnholyArmorIII => 6,
            Self::GhoulFrenzy => 2,
            Self::DiseaseCloud => 3,
            Self::NecromancyTech => 4,
            Self::FrostMagic => 3,
            Self::SkeletalMastery => 3,
            Self::PlagueResearch => 2,
            Self::SpectralBinding => 4,
            Self::CorpseExplosion => 5,
            Self::DarkRitual => 4,
            Self::NecrosisExpansion => 3,
            Self::BoneArmor => 3,
            Self::SoulHarvest => 5,
            Self::LichAscension => 8,
            Self::WorldEaterProtocol => 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Research {
    pub tech_type: TechType,
    pub level: u32,
    pub researching: bool,
    pub research_finish: Option<String>,
    pub display_name: String,
    pub description: String,
    pub cost_biomass: f64,
    pub cost_minerals: f64,
    pub cost_crystal: f64,
    pub cost_spore_gas: f64,
    pub research_time_seconds: u64,
    pub required_lab_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ShipType {
    BioFighter,
    SporeInterceptor,
    KrakenFrigate,
    Leviathan,
    BioTransporter,
    ColonyPod,
    Devourer,
    WorldEater,

    // Expanded fleet
    LeechHauler,     // Giant transport -- orbit-to-surface fuel proboscis
    SporeCarrier,    // Spreads spores to new systems
    HiveShip,        // Mobile nest -- spawns units in orbit
    VoidKraken,      // Devours space stations
    MyceticSpore,    // Drop pod for planetary invasion
    NeuralParasite,  // Hijacks enemy ships
    Narwhal,         // Warp navigation vessel
    DroneShip,       // Autonomous drone mothership
    Razorfiend,      // Fast interceptor
    Hierophant,      // Bio-Titan -- largest warship

    // --- Human Faction Fleet ---
    ScoutFighter,    // Light fighter -- fast reconnaissance
    AssaultFighter,  // Heavy fighter -- armored attack craft
    StrikeCruiser,   // Cruiser -- multi-role warship
    HumanBattleship, // Battleship -- heavy capital ship
    BattleCruiser,   // Battlecruiser -- fast heavy ship
    StrategicBomber, // Bomber -- long-range bombardment
    FleetDestroyer,  // Destroyer -- anti-capital ship
    OrbitalCannon,   // Deathstar equivalent -- planetary weapon
    SalvageVessel,   // Reaper equivalent -- battlefield salvage
    SurveyShip,      // Pathfinder -- exploration vessel
    LightFreighter,  // Small cargo -- trade transport
    HeavyFreighter,  // Large cargo -- bulk transport
    SalvageTug,      // Recycler -- wreckage recovery
    SpyDrone,        // Spy probe -- stealth reconnaissance
    ColonyTransport, // Colony ship -- establishes outposts

    // --- Demon Faction Ships (15) ---
    FireImp,           // Light fighter -- swarming fire imps
    FiendRaider,       // Heavy fighter with infernal weapons
    HellChariot,       // Multi-role cruiser with flame arrays
    InfernalDreadnought, // Massive demon battleship
    BaalfireCruiser,   // Heavy cruiser with baalfire cannons
    HellfireRainer,    // Bombardment vessel raining hellfire
    PitLordVessel,     // Capital ship commanded by a Pit Lord
    AbyssalMaw,        // Planet-killer. The demon Deathstar
    SoulHarvester,     // Collects souls from destroyed ships
    ShadowStalker,     // Stealth ship for ambush operations
    ImpBarge,          // Small transport for imp deployment
    AbyssalBarge,      // Heavy transport for demon armies
    SlagDredger,       // Resource collection from slag fields
    EyeOfPerdition,    // Recon vessel with far-sight abilities
    HellgateOpener,    // Opens warp gates for fleet deployment

    // --- Undead Faction Ships (15) ---
    Specter,           // Light fighter -- ghostly raider
    BansheeShip,       // Heavy fighter with wailing attacks
    DeathFrigate,      // Multi-role frigate of the dead
    PhantomGalleon,    // Massive ghost battleship
    LichCruiser,       // Heavy cruiser commanded by a Lich
    PlagueBringer,     // Spreads plague across enemy fleets
    DreadRevenant,     // Capital ship crewed by revenants
    UndeadWorldEater,  // Planet-killer. The undead Deathstar
    CorpseCollector,   // Collects wreckage and bodies
    Haunt,             // Stealth ghost ship for haunting ops
    WraithSkiff,       // Small transport for spectral units
    BoneGalleon,       // Heavy transport made of bone
    BonePicker,        // Resource scavenger from battlefields
    Shade,             // Recon vessel that phases through matter
    CryptShip,         // Mobile crypt for fleet deployment
}
impl ShipType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::BioFighter => "bio_fighter",
            Self::SporeInterceptor => "spore_interceptor",
            Self::KrakenFrigate => "kraken_frigate",
            Self::Leviathan => "leviathan",
            Self::BioTransporter => "bio_transporter",
            Self::ColonyPod => "colony_pod",
            Self::Devourer => "devourer",
            Self::WorldEater => "world_eater",
            Self::LeechHauler => "leech_hauler",
            Self::SporeCarrier => "spore_carrier",
            Self::HiveShip => "hive_ship",
            Self::VoidKraken => "void_kraken",
            Self::MyceticSpore => "mycetic_spore",
            Self::NeuralParasite => "neural_parasite",
            Self::Narwhal => "narwhal",
            Self::DroneShip => "drone_ship",
            Self::Razorfiend => "razorfiend",
            Self::Hierophant => "hierophant",
            // Human faction
            Self::ScoutFighter => "scout_fighter",
            Self::AssaultFighter => "assault_fighter",
            Self::StrikeCruiser => "strike_cruiser",
            Self::HumanBattleship => "human_battleship",
            Self::BattleCruiser => "battle_cruiser",
            Self::StrategicBomber => "strategic_bomber",
            Self::FleetDestroyer => "fleet_destroyer",
            Self::OrbitalCannon => "orbital_cannon",
            Self::SalvageVessel => "salvage_vessel",
            Self::SurveyShip => "survey_ship",
            Self::LightFreighter => "light_freighter",
            Self::HeavyFreighter => "heavy_freighter",
            Self::SalvageTug => "salvage_tug",
            Self::SpyDrone => "spy_drone",
            Self::ColonyTransport => "colony_transport",
            // Demon ships
            Self::FireImp => "fire_imp",
            Self::FiendRaider => "fiend_raider",
            Self::HellChariot => "hell_chariot",
            Self::InfernalDreadnought => "infernal_dreadnought",
            Self::BaalfireCruiser => "baalfire_cruiser",
            Self::HellfireRainer => "hellfire_rainer",
            Self::PitLordVessel => "pit_lord_vessel",
            Self::AbyssalMaw => "abyssal_maw",
            Self::SoulHarvester => "soul_harvester",
            Self::ShadowStalker => "shadow_stalker",
            Self::ImpBarge => "imp_barge",
            Self::AbyssalBarge => "abyssal_barge",
            Self::SlagDredger => "slag_dredger",
            Self::EyeOfPerdition => "eye_of_perdition",
            Self::HellgateOpener => "hellgate_opener",
            // Undead ships
            Self::Specter => "specter",
            Self::BansheeShip => "banshee_ship",
            Self::DeathFrigate => "death_frigate",
            Self::PhantomGalleon => "phantom_galleon",
            Self::LichCruiser => "lich_cruiser",
            Self::PlagueBringer => "plague_bringer",
            Self::DreadRevenant => "dread_revenant",
            Self::UndeadWorldEater => "undead_world_eater",
            Self::CorpseCollector => "corpse_collector",
            Self::Haunt => "haunt",
            Self::WraithSkiff => "wraith_skiff",
            Self::BoneGalleon => "bone_galleon",
            Self::BonePicker => "bone_picker",
            Self::Shade => "shade",
            Self::CryptShip => "crypt_ship",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "bio_fighter" => Self::BioFighter,
            "spore_interceptor" => Self::SporeInterceptor,
            "kraken_frigate" => Self::KrakenFrigate,
            "leviathan" => Self::Leviathan,
            "bio_transporter" => Self::BioTransporter,
            "colony_pod" => Self::ColonyPod,
            "devourer" => Self::Devourer,
            "world_eater" => Self::WorldEater,
            "leech_hauler" => Self::LeechHauler,
            "spore_carrier" => Self::SporeCarrier,
            "hive_ship" => Self::HiveShip,
            "void_kraken" => Self::VoidKraken,
            "mycetic_spore" => Self::MyceticSpore,
            "neural_parasite" => Self::NeuralParasite,
            "narwhal" => Self::Narwhal,
            "drone_ship" => Self::DroneShip,
            "razorfiend" => Self::Razorfiend,
            "hierophant" => Self::Hierophant,
            // Human faction
            "scout_fighter" => Self::ScoutFighter,
            "assault_fighter" => Self::AssaultFighter,
            "strike_cruiser" => Self::StrikeCruiser,
            "human_battleship" => Self::HumanBattleship,
            "battle_cruiser" => Self::BattleCruiser,
            "strategic_bomber" => Self::StrategicBomber,
            "fleet_destroyer" => Self::FleetDestroyer,
            "orbital_cannon" => Self::OrbitalCannon,
            "salvage_vessel" => Self::SalvageVessel,
            "survey_ship" => Self::SurveyShip,
            "light_freighter" => Self::LightFreighter,
            "heavy_freighter" => Self::HeavyFreighter,
            "salvage_tug" => Self::SalvageTug,
            "spy_drone" => Self::SpyDrone,
            "colony_transport" => Self::ColonyTransport,
            // Demon ships
            "fire_imp" => Self::FireImp,
            "fiend_raider" => Self::FiendRaider,
            "hell_chariot" => Self::HellChariot,
            "infernal_dreadnought" => Self::InfernalDreadnought,
            "baalfire_cruiser" => Self::BaalfireCruiser,
            "hellfire_rainer" => Self::HellfireRainer,
            "pit_lord_vessel" => Self::PitLordVessel,
            "abyssal_maw" => Self::AbyssalMaw,
            "soul_harvester" => Self::SoulHarvester,
            "shadow_stalker" => Self::ShadowStalker,
            "imp_barge" => Self::ImpBarge,
            "abyssal_barge" => Self::AbyssalBarge,
            "slag_dredger" => Self::SlagDredger,
            "eye_of_perdition" => Self::EyeOfPerdition,
            "hellgate_opener" => Self::HellgateOpener,
            // Undead ships
            "specter" => Self::Specter,
            "banshee_ship" => Self::BansheeShip,
            "death_frigate" => Self::DeathFrigate,
            "phantom_galleon" => Self::PhantomGalleon,
            "lich_cruiser" => Self::LichCruiser,
            "plague_bringer" => Self::PlagueBringer,
            "dread_revenant" => Self::DreadRevenant,
            "undead_world_eater" => Self::UndeadWorldEater,
            "corpse_collector" => Self::CorpseCollector,
            "haunt" => Self::Haunt,
            "wraith_skiff" => Self::WraithSkiff,
            "bone_galleon" => Self::BoneGalleon,
            "bone_picker" => Self::BonePicker,
            "shade" => Self::Shade,
            "crypt_ship" => Self::CryptShip,
            _ => Self::BioFighter,
        }
    }

    pub(crate) fn display_name(&self) -> &'static str {
        match self {
            Self::BioFighter => "Bio Fighter",
            Self::SporeInterceptor => "Spore Interceptor",
            Self::KrakenFrigate => "Kraken Frigate",
            Self::Leviathan => "Leviathan",
            Self::BioTransporter => "Bio Transporter",
            Self::ColonyPod => "Colony Pod",
            Self::Devourer => "Devourer",
            Self::WorldEater => "World Eater",
            Self::LeechHauler => "Leech Hauler",
            Self::SporeCarrier => "Spore Carrier",
            Self::HiveShip => "Hive Ship",
            Self::VoidKraken => "Void Kraken",
            Self::MyceticSpore => "Mycetic Spore",
            Self::NeuralParasite => "Neural Parasite",
            Self::Narwhal => "Narwhal",
            Self::DroneShip => "Drone Ship",
            Self::Razorfiend => "Razorfiend",
            Self::Hierophant => "Hierophant",
            // Human faction
            Self::ScoutFighter => "Scout Fighter",
            Self::AssaultFighter => "Assault Fighter",
            Self::StrikeCruiser => "Strike Cruiser",
            Self::HumanBattleship => "Battleship",
            Self::BattleCruiser => "Battle Cruiser",
            Self::StrategicBomber => "Strategic Bomber",
            Self::FleetDestroyer => "Fleet Destroyer",
            Self::OrbitalCannon => "Orbital Cannon",
            Self::SalvageVessel => "Salvage Vessel",
            Self::SurveyShip => "Survey Ship",
            Self::LightFreighter => "Light Freighter",
            Self::HeavyFreighter => "Heavy Freighter",
            Self::SalvageTug => "Salvage Tug",
            Self::SpyDrone => "Spy Drone",
            Self::ColonyTransport => "Colony Transport",
            // Demon ships
            Self::FireImp => "Fire Imp",
            Self::FiendRaider => "Fiend Raider",
            Self::HellChariot => "Hell Chariot",
            Self::InfernalDreadnought => "Infernal Dreadnought",
            Self::BaalfireCruiser => "Baalfire Cruiser",
            Self::HellfireRainer => "Hellfire Rainer",
            Self::PitLordVessel => "Pit Lord Vessel",
            Self::AbyssalMaw => "Abyssal Maw",
            Self::SoulHarvester => "Soul Harvester",
            Self::ShadowStalker => "Shadow Stalker",
            Self::ImpBarge => "Imp Barge",
            Self::AbyssalBarge => "Abyssal Barge",
            Self::SlagDredger => "Slag Dredger",
            Self::EyeOfPerdition => "Eye of Perdition",
            Self::HellgateOpener => "Hellgate Opener",
            // Undead ships
            Self::Specter => "Specter",
            Self::BansheeShip => "Banshee",
            Self::DeathFrigate => "Death Frigate",
            Self::PhantomGalleon => "Phantom Galleon",
            Self::LichCruiser => "Lich Cruiser",
            Self::PlagueBringer => "Plague Bringer",
            Self::DreadRevenant => "Dread Revenant",
            Self::UndeadWorldEater => "World Eater",
            Self::CorpseCollector => "Corpse Collector",
            Self::Haunt => "Haunt",
            Self::WraithSkiff => "Wraith Skiff",
            Self::BoneGalleon => "Bone Galleon",
            Self::BonePicker => "Bone Picker",
            Self::Shade => "Shade",
            Self::CryptShip => "Crypt Ship",
        }
    }

    pub(crate) fn description(&self) -> &'static str {
        match self {
            Self::BioFighter => "Light fighter -- cheap, fast, disposable.",
            Self::SporeInterceptor => "Heavy fighter with spore weapons.",
            Self::KrakenFrigate => "Multi-role cruiser with tentacle arrays.",
            Self::Leviathan => "Massive battleship of the bio-fleet.",
            Self::BioTransporter => "Cargo ship for resource transport.",
            Self::ColonyPod => "Colonizes new planets for expansion.",
            Self::Devourer => "Destroyer-class vessel that eats enemy ships.",
            Self::WorldEater => "Planet-killer. The ultimate weapon. Requires Lv.20 research.",
            Self::LeechHauler => "Giant transport with orbit-to-surface fuel proboscis.",
            Self::SporeCarrier => "Spreads bio-spores to seed new star systems.",
            Self::HiveShip => "Mobile nest that spawns swarm units in orbit.",
            Self::VoidKraken => "Enormous beast that devours space stations.",
            Self::MyceticSpore => "Orbital drop pod for rapid planetary invasion.",
            Self::NeuralParasite => "Infiltrator vessel that hijacks enemy ships.",
            Self::Narwhal => "Warp-capable navigation vessel for deep space travel.",
            Self::DroneShip => "Autonomous mothership controlling swarms of drones.",
            Self::Razorfiend => "Lightning-fast interceptor with bio-blade wings.",
            Self::Hierophant => "Bio-Titan capital ship. The largest warship in the fleet.",
            // Human faction
            Self::ScoutFighter => "Fast reconnaissance fighter. Light armor, high speed.",
            Self::AssaultFighter => "Armored attack craft with heavy forward weapons.",
            Self::StrikeCruiser => "Multi-role warship balancing firepower and speed.",
            Self::HumanBattleship => "Heavy capital ship with devastating broadside cannons.",
            Self::BattleCruiser => "Fast heavy warship combining cruiser speed with battleship guns.",
            Self::StrategicBomber => "Long-range bombardment vessel for strategic strikes.",
            Self::FleetDestroyer => "Anti-capital ship designed to hunt larger vessels.",
            Self::OrbitalCannon => "Orbital weapons platform. The ultimate human superweapon.",
            Self::SalvageVessel => "Battlefield salvage ship that recovers wreckage and materials.",
            Self::SurveyShip => "Exploration vessel equipped for deep-space surveys.",
            Self::LightFreighter => "Small cargo transport for quick trade runs.",
            Self::HeavyFreighter => "Bulk cargo hauler for massive resource shipments.",
            Self::SalvageTug => "Heavy-duty tug that recovers debris fields.",
            Self::SpyDrone => "Stealth reconnaissance drone. Nearly undetectable.",
            Self::ColonyTransport => "Colony ship carrying settlers to establish new outposts.",
            // Demon ships
            Self::FireImp => "Swarming fire imp fighters. Cheap and disposable.",
            Self::FiendRaider => "Heavy fighter with infernal weapon arrays.",
            Self::HellChariot => "Multi-role cruiser with flame cannons.",
            Self::InfernalDreadnought => "Massive demon battleship wreathed in fire.",
            Self::BaalfireCruiser => "Heavy cruiser armed with baalfire cannons.",
            Self::HellfireRainer => "Bombardment vessel that rains hellfire from orbit.",
            Self::PitLordVessel => "Capital ship commanded by a fearsome Pit Lord.",
            Self::AbyssalMaw => "Planet-killer. The demon Deathstar. Requires Lv.20 research.",
            Self::SoulHarvester => "Collects souls from the wreckage of destroyed ships.",
            Self::ShadowStalker => "Stealth vessel for ambush and reconnaissance.",
            Self::ImpBarge => "Small transport for rapid imp deployment.",
            Self::AbyssalBarge => "Heavy transport for moving demon armies.",
            Self::SlagDredger => "Resource collector that dredges slag fields.",
            Self::EyeOfPerdition => "Reconnaissance vessel with demonic far-sight.",
            Self::HellgateOpener => "Opens warp gates for rapid fleet deployment.",
            // Undead ships
            Self::Specter => "Ghostly light fighter that phases through shields.",
            Self::BansheeShip => "Heavy fighter with soul-rending wail attacks.",
            Self::DeathFrigate => "Multi-role frigate crewed by the restless dead.",
            Self::PhantomGalleon => "Massive ghost battleship from beyond the veil.",
            Self::LichCruiser => "Heavy cruiser commanded by an undead Lich.",
            Self::PlagueBringer => "Spreads virulent plague across enemy fleets.",
            Self::DreadRevenant => "Capital ship crewed by vengeful revenants.",
            Self::UndeadWorldEater => "Planet-killer. The undead Deathstar. Requires Lv.20 research.",
            Self::CorpseCollector => "Collects wreckage and bodies from battlefields.",
            Self::Haunt => "Stealth ghost ship for haunting operations.",
            Self::WraithSkiff => "Small transport for spectral unit deployment.",
            Self::BoneGalleon => "Heavy bone transport for undead armies.",
            Self::BonePicker => "Resource scavenger that picks clean battlefields.",
            Self::Shade => "Recon vessel that phases through solid matter.",
            Self::CryptShip => "Mobile crypt enabling fleet-wide deployment.",
        }
    }

    /// (biomass, minerals, crystal, spore_gas) per unit
    pub(crate) fn unit_cost(&self) -> (f64, f64, f64, f64) {
        match self {
            Self::BioFighter =>       (3000.0, 1000.0, 0.0, 0.0),
            Self::SporeInterceptor => (6000.0, 4000.0, 0.0, 0.0),
            Self::KrakenFrigate =>    (20000.0, 7000.0, 2000.0, 0.0),
            Self::Leviathan =>        (45000.0, 15000.0, 0.0, 0.0),
            Self::BioTransporter =>   (2000.0, 2000.0, 0.0, 0.0),
            Self::ColonyPod =>        (10000.0, 10000.0, 10000.0, 10000.0),
            Self::Devourer =>         (60000.0, 50000.0, 15000.0, 0.0),
            Self::WorldEater =>       (5000000.0, 4000000.0, 1000000.0, 1000000.0),
            Self::LeechHauler =>      (8000.0, 5000.0, 1000.0, 0.0),
            Self::SporeCarrier =>     (15000.0, 8000.0, 5000.0, 3000.0),
            Self::HiveShip =>         (50000.0, 30000.0, 20000.0, 10000.0),
            Self::VoidKraken =>       (100000.0, 80000.0, 40000.0, 20000.0),
            Self::MyceticSpore =>     (1500.0, 500.0, 0.0, 0.0),
            Self::NeuralParasite =>   (12000.0, 6000.0, 8000.0, 4000.0),
            Self::Narwhal =>          (25000.0, 15000.0, 10000.0, 5000.0),
            Self::DroneShip =>        (35000.0, 20000.0, 15000.0, 5000.0),
            Self::Razorfiend =>       (4000.0, 2000.0, 500.0, 0.0),
            Self::Hierophant =>       (2000000.0, 1500000.0, 500000.0, 500000.0),
            // Human faction (gold, lumber, crystal, gas)
            Self::ScoutFighter =>     (3000.0, 1000.0, 0.0, 0.0),
            Self::AssaultFighter =>   (6000.0, 4000.0, 0.0, 0.0),
            Self::StrikeCruiser =>    (20000.0, 7000.0, 2000.0, 0.0),
            Self::HumanBattleship => (45000.0, 15000.0, 5000.0, 0.0),
            Self::BattleCruiser =>    (30000.0, 15000.0, 10000.0, 0.0),
            Self::StrategicBomber =>  (50000.0, 25000.0, 15000.0, 0.0),
            Self::FleetDestroyer =>   (60000.0, 50000.0, 15000.0, 0.0),
            Self::OrbitalCannon =>    (5000000.0, 4000000.0, 1000000.0, 1000000.0),
            Self::SalvageVessel =>    (10000.0, 6000.0, 2000.0, 0.0),
            Self::SurveyShip =>       (8000.0, 15000.0, 5000.0, 0.0),
            Self::LightFreighter =>   (2000.0, 2000.0, 0.0, 0.0),
            Self::HeavyFreighter =>   (6000.0, 6000.0, 0.0, 0.0),
            Self::SalvageTug =>       (10000.0, 6000.0, 2000.0, 0.0),
            Self::SpyDrone =>         (0.0, 1000.0, 0.0, 0.0),
            Self::ColonyTransport =>  (10000.0, 10000.0, 10000.0, 10000.0),
            // Demon ships
            Self::FireImp =>              (2500.0, 1200.0, 0.0, 0.0),
            Self::FiendRaider =>          (5500.0, 4500.0, 0.0, 0.0),
            Self::HellChariot =>          (18000.0, 8000.0, 2500.0, 0.0),
            Self::InfernalDreadnought =>  (50000.0, 14000.0, 0.0, 0.0),
            Self::BaalfireCruiser =>      (22000.0, 9000.0, 3000.0, 0.0),
            Self::HellfireRainer =>       (55000.0, 45000.0, 18000.0, 0.0),
            Self::PitLordVessel =>        (48000.0, 28000.0, 22000.0, 12000.0),
            Self::AbyssalMaw =>           (5500000.0, 3800000.0, 1100000.0, 1100000.0),
            Self::SoulHarvester =>        (10000.0, 5000.0, 3000.0, 2000.0),
            Self::ShadowStalker =>        (14000.0, 7000.0, 9000.0, 5000.0),
            Self::ImpBarge =>             (1800.0, 2200.0, 0.0, 0.0),
            Self::AbyssalBarge =>         (9000.0, 6000.0, 1200.0, 0.0),
            Self::SlagDredger =>          (7000.0, 4000.0, 800.0, 0.0),
            Self::EyeOfPerdition =>       (4500.0, 2500.0, 600.0, 0.0),
            Self::HellgateOpener =>       (30000.0, 18000.0, 12000.0, 6000.0),
            // Undead ships
            Self::Specter =>              (2800.0, 1100.0, 0.0, 0.0),
            Self::BansheeShip =>          (6500.0, 3800.0, 0.0, 0.0),
            Self::DeathFrigate =>         (21000.0, 6500.0, 2200.0, 0.0),
            Self::PhantomGalleon =>       (42000.0, 16000.0, 0.0, 0.0),
            Self::LichCruiser =>          (24000.0, 8000.0, 2800.0, 0.0),
            Self::PlagueBringer =>        (52000.0, 48000.0, 16000.0, 0.0),
            Self::DreadRevenant =>        (55000.0, 32000.0, 18000.0, 9000.0),
            Self::UndeadWorldEater =>     (4800000.0, 4200000.0, 950000.0, 950000.0),
            Self::CorpseCollector =>      (8500.0, 5500.0, 1200.0, 0.0),
            Self::Haunt =>                (11000.0, 6500.0, 7500.0, 4500.0),
            Self::WraithSkiff =>          (2200.0, 1800.0, 0.0, 0.0),
            Self::BoneGalleon =>          (8000.0, 5500.0, 1000.0, 0.0),
            Self::BonePicker =>           (6500.0, 3500.0, 700.0, 0.0),
            Self::Shade =>                (3800.0, 2200.0, 500.0, 0.0),
            Self::CryptShip =>            (28000.0, 16000.0, 11000.0, 5500.0),
        }
    }

    /// (attack, shields, hp)
    pub(crate) fn combat_stats(&self) -> (u32, u32, u32) {
        match self {
            Self::BioFighter =>       (50, 10, 400),
            Self::SporeInterceptor => (150, 25, 1000),
            Self::KrakenFrigate =>    (400, 50, 2700),
            Self::Leviathan =>        (1000, 200, 6000),
            Self::BioTransporter =>   (5, 10, 1200),
            Self::ColonyPod =>        (50, 100, 3000),
            Self::Devourer =>         (2000, 500, 11000),
            Self::WorldEater =>       (200000, 50000, 900000),
            Self::LeechHauler =>      (10, 20, 2500),
            Self::SporeCarrier =>     (100, 80, 4000),
            Self::HiveShip =>         (800, 400, 15000),
            Self::VoidKraken =>       (5000, 2000, 50000),
            Self::MyceticSpore =>     (20, 5, 200),
            Self::NeuralParasite =>   (300, 150, 3500),
            Self::Narwhal =>          (200, 300, 8000),
            Self::DroneShip =>        (600, 250, 10000),
            Self::Razorfiend =>       (120, 30, 800),
            Self::Hierophant =>       (80000, 30000, 500000),
            // Human faction (attack, shields, hp)
            Self::ScoutFighter =>     (50, 10, 400),
            Self::AssaultFighter =>   (150, 25, 1000),
            Self::StrikeCruiser =>    (400, 50, 2700),
            Self::HumanBattleship =>  (1000, 200, 6000),
            Self::BattleCruiser =>    (700, 400, 7000),
            Self::StrategicBomber =>  (1000, 500, 7500),
            Self::FleetDestroyer =>   (2000, 500, 11000),
            Self::OrbitalCannon =>    (200000, 50000, 900000),
            Self::SalvageVessel =>    (1, 10, 1000),
            Self::SurveyShip =>       (200, 100, 2300),
            Self::LightFreighter =>   (5, 10, 400),
            Self::HeavyFreighter =>   (5, 25, 1200),
            Self::SalvageTug =>       (1, 10, 1600),
            Self::SpyDrone =>         (0, 0, 100),
            Self::ColonyTransport =>  (50, 100, 3000),
            // Demon ships
            Self::FireImp =>              (55, 8, 380),
            Self::FiendRaider =>          (165, 22, 950),
            Self::HellChariot =>          (420, 45, 2600),
            Self::InfernalDreadnought =>  (1100, 180, 5800),
            Self::BaalfireCruiser =>      (500, 60, 3200),
            Self::HellfireRainer =>       (2200, 450, 10500),
            Self::PitLordVessel =>        (850, 380, 14000),
            Self::AbyssalMaw =>           (220000, 48000, 880000),
            Self::SoulHarvester =>        (120, 90, 4200),
            Self::ShadowStalker =>        (320, 140, 3200),
            Self::ImpBarge =>             (5, 8, 1100),
            Self::AbyssalBarge =>         (12, 22, 2800),
            Self::SlagDredger =>          (30, 15, 2000),
            Self::EyeOfPerdition =>       (80, 25, 700),
            Self::HellgateOpener =>       (250, 320, 9000),
            // Undead ships
            Self::Specter =>              (45, 12, 420),
            Self::BansheeShip =>          (140, 30, 1050),
            Self::DeathFrigate =>         (380, 55, 2850),
            Self::PhantomGalleon =>       (950, 220, 6200),
            Self::LichCruiser =>          (480, 65, 3000),
            Self::PlagueBringer =>        (1900, 520, 11500),
            Self::DreadRevenant =>        (900, 420, 16000),
            Self::UndeadWorldEater =>     (190000, 52000, 920000),
            Self::CorpseCollector =>      (90, 60, 3800),
            Self::Haunt =>                (280, 160, 3600),
            Self::WraithSkiff =>          (5, 10, 1300),
            Self::BoneGalleon =>          (10, 18, 2600),
            Self::BonePicker =>           (25, 12, 1800),
            Self::Shade =>                (60, 20, 650),
            Self::CryptShip =>            (220, 280, 8500),
        }
    }

    /// Required Blighthaven level to build
    pub(crate) fn required_shipyard_level(&self) -> u32 {
        match self {
            Self::BioFighter => 1,
            Self::SporeInterceptor => 3,
            Self::KrakenFrigate => 5,
            Self::Leviathan => 7,
            Self::BioTransporter => 2,
            Self::ColonyPod => 4,
            Self::Devourer => 9,
            Self::WorldEater => 12,
            Self::LeechHauler => 3,
            Self::SporeCarrier => 5,
            Self::HiveShip => 8,
            Self::VoidKraken => 10,
            Self::MyceticSpore => 1,
            Self::NeuralParasite => 6,
            Self::Narwhal => 6,
            Self::DroneShip => 7,
            Self::Razorfiend => 2,
            Self::Hierophant => 11,
            // Human faction (requires Harbor level)
            Self::ScoutFighter => 1,
            Self::AssaultFighter => 3,
            Self::StrikeCruiser => 5,
            Self::HumanBattleship => 7,
            Self::BattleCruiser => 8,
            Self::StrategicBomber => 8,
            Self::FleetDestroyer => 9,
            Self::OrbitalCannon => 12,
            Self::SalvageVessel => 4,
            Self::SurveyShip => 5,
            Self::LightFreighter => 2,
            Self::HeavyFreighter => 4,
            Self::SalvageTug => 4,
            Self::SpyDrone => 1,
            Self::ColonyTransport => 6,
            // Demon ships
            Self::FireImp => 1,
            Self::FiendRaider => 3,
            Self::HellChariot => 5,
            Self::InfernalDreadnought => 7,
            Self::BaalfireCruiser => 6,
            Self::HellfireRainer => 9,
            Self::PitLordVessel => 8,
            Self::AbyssalMaw => 12,
            Self::SoulHarvester => 4,
            Self::ShadowStalker => 6,
            Self::ImpBarge => 2,
            Self::AbyssalBarge => 3,
            Self::SlagDredger => 3,
            Self::EyeOfPerdition => 2,
            Self::HellgateOpener => 7,
            // Undead ships
            Self::Specter => 1,
            Self::BansheeShip => 3,
            Self::DeathFrigate => 5,
            Self::PhantomGalleon => 7,
            Self::LichCruiser => 6,
            Self::PlagueBringer => 9,
            Self::DreadRevenant => 8,
            Self::UndeadWorldEater => 12,
            Self::CorpseCollector => 4,
            Self::Haunt => 6,
            Self::WraithSkiff => 2,
            Self::BoneGalleon => 3,
            Self::BonePicker => 3,
            Self::Shade => 2,
            Self::CryptShip => 7,
        }
    }

    /// Build time per unit in seconds
    pub(crate) fn build_time_per_unit(&self) -> u64 {
        match self {
            Self::BioFighter => 30,
            Self::SporeInterceptor => 60,
            Self::KrakenFrigate => 180,
            Self::Leviathan => 600,
            Self::BioTransporter => 45,
            Self::ColonyPod => 300,
            Self::Devourer => 1200,
            Self::WorldEater => 7200,
            Self::LeechHauler => 90,
            Self::SporeCarrier => 240,
            Self::HiveShip => 900,
            Self::VoidKraken => 3600,
            Self::MyceticSpore => 15,
            Self::NeuralParasite => 360,
            Self::Narwhal => 480,
            Self::DroneShip => 600,
            Self::Razorfiend => 45,
            Self::Hierophant => 5400,
            // Human faction
            Self::ScoutFighter => 30,
            Self::AssaultFighter => 60,
            Self::StrikeCruiser => 180,
            Self::HumanBattleship => 600,
            Self::BattleCruiser => 480,
            Self::StrategicBomber => 900,
            Self::FleetDestroyer => 1200,
            Self::OrbitalCannon => 7200,
            Self::SalvageVessel => 120,
            Self::SurveyShip => 240,
            Self::LightFreighter => 45,
            Self::HeavyFreighter => 90,
            Self::SalvageTug => 150,
            Self::SpyDrone => 15,
            Self::ColonyTransport => 300,
            // Demon ships
            Self::FireImp => 28,
            Self::FiendRaider => 55,
            Self::HellChariot => 170,
            Self::InfernalDreadnought => 650,
            Self::BaalfireCruiser => 200,
            Self::HellfireRainer => 1300,
            Self::PitLordVessel => 950,
            Self::AbyssalMaw => 7500,
            Self::SoulHarvester => 120,
            Self::ShadowStalker => 380,
            Self::ImpBarge => 40,
            Self::AbyssalBarge => 100,
            Self::SlagDredger => 80,
            Self::EyeOfPerdition => 50,
            Self::HellgateOpener => 520,
            // Undead ships
            Self::Specter => 32,
            Self::BansheeShip => 65,
            Self::DeathFrigate => 190,
            Self::PhantomGalleon => 580,
            Self::LichCruiser => 220,
            Self::PlagueBringer => 1250,
            Self::DreadRevenant => 1000,
            Self::UndeadWorldEater => 7000,
            Self::CorpseCollector => 100,
            Self::Haunt => 350,
            Self::WraithSkiff => 42,
            Self::BoneGalleon => 95,
            Self::BonePicker => 75,
            Self::Shade => 48,
            Self::CryptShip => 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ship {
    pub ship_type: ShipType,
    pub count: u32,
    pub display_name: String,
    pub description: String,
    pub attack: u32,
    pub shields: u32,
    pub hp: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreepStatus {
    pub coverage_percent: f32,
    pub spread_rate_per_hour: f32,
    pub flora_corrupted: f32,
    pub fauna_consumed: f32,
    pub biomass_bonus: f32,
}

impl Default for CreepStatus {
    fn default() -> Self {
        Self {
            coverage_percent: 0.0,
            spread_rate_per_hour: 0.0,
            flora_corrupted: 0.0,
            fauna_consumed: 0.0,
            biomass_bonus: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ShopEffect {
    ProductionBoost(f32),
    ResearchSpeed(f32),
    BuildSpeed(f32),
    FleetSpeed(f32),
    ExtraQueue,
    CreepBoost(f32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cost_dark_matter: u64,
    pub effect: ShopEffect,
    pub duration_hours: Option<u32>,
}

// ---------------------------------------------------------------------------
// Dark Matter Earnings -- productivity-to-game currency
// ---------------------------------------------------------------------------
//
// Dark Matter is ONLY earned by using ImpForge productively.  It is NOT
// purchasable with real money.  This is the core productivity-to-game bridge.

/// Sources of Dark Matter earned through ImpForge productivity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DmSource {
    /// 2 DM per document saved
    DocumentWritten,
    /// 3 DM per git commit
    CodeCommitted,
    /// 2 DM per spreadsheet created
    SpreadsheetCreated,
    /// 1 DM per email sent
    EmailSent,
    /// 1 DM per task completed
    TaskCompleted,
    /// 1 DM per 5 minutes of active use
    ActiveUsage,
    /// 2 DM per test suite pass
    TestsPassed,
    /// 2 DM per successful build
    BuildSucceeded,
    /// 10 DM per achievement unlocked
    MilestoneReached,
    /// 5 DM per daily login streak
    DailyLogin,
    /// 25 DM per weekly challenge completed
    WeeklyChallenge,
}

impl DmSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::DocumentWritten => "document_written",
            Self::CodeCommitted => "code_committed",
            Self::SpreadsheetCreated => "spreadsheet_created",
            Self::EmailSent => "email_sent",
            Self::TaskCompleted => "task_completed",
            Self::ActiveUsage => "active_usage",
            Self::TestsPassed => "tests_passed",
            Self::BuildSucceeded => "build_succeeded",
            Self::MilestoneReached => "milestone_reached",
            Self::DailyLogin => "daily_login",
            Self::WeeklyChallenge => "weekly_challenge",
        }
    }

    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "document_written" => Some(Self::DocumentWritten),
            "code_committed" => Some(Self::CodeCommitted),
            "spreadsheet_created" => Some(Self::SpreadsheetCreated),
            "email_sent" => Some(Self::EmailSent),
            "task_completed" => Some(Self::TaskCompleted),
            "active_usage" => Some(Self::ActiveUsage),
            "tests_passed" => Some(Self::TestsPassed),
            "build_succeeded" => Some(Self::BuildSucceeded),
            "milestone_reached" => Some(Self::MilestoneReached),
            "daily_login" => Some(Self::DailyLogin),
            "weekly_challenge" => Some(Self::WeeklyChallenge),
            _ => None,
        }
    }

    /// How much Dark Matter this activity awards.
    pub(crate) fn dm_amount(&self) -> u32 {
        match self {
            Self::DocumentWritten => 2,
            Self::CodeCommitted => 3,
            Self::SpreadsheetCreated => 2,
            Self::EmailSent => 1,
            Self::TaskCompleted => 1,
            Self::ActiveUsage => 1,
            Self::TestsPassed => 2,
            Self::BuildSucceeded => 2,
            Self::MilestoneReached => 10,
            Self::DailyLogin => 5,
            Self::WeeklyChallenge => 25,
        }
    }
}

/// A record of Dark Matter earned from a specific activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DarkMatterEarnings {
    pub source: DmSource,
    pub amount: u32,
    pub timestamp: String,
}

/// Combined planet state for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Planet {
    pub name: String,
    pub resources: PlanetResources,
    pub buildings: Vec<PlanetBuilding>,
    pub research: Vec<Research>,
    pub fleet: Vec<Ship>,
    pub creep: CreepStatus,
    pub storage_biomass_cap: f64,
    pub storage_minerals_cap: f64,
    pub storage_crystal_cap: f64,
    pub storage_spore_gas_cap: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetSlot {
    pub position: u32,
    pub occupied: bool,
    pub planet_name: Option<String>,
    pub player_name: Option<String>,
    pub planet_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedTimer {
    pub timer_type: String,
    pub item_name: String,
    pub completed_at: String,
}

// ---------------------------------------------------------------------------
// End of OGame types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MissionStatus {
    Available,
    InProgress,
    Completed,
    Failed,
}

impl MissionStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "available" => Self::Available,
            "in_progress" => Self::InProgress,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Available,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmMission {
    pub id: String,
    pub name: String,
    pub description: String,
    pub required_unit_types: Vec<String>,
    pub required_unit_count: u32,
    pub assigned_units: Vec<String>,
    pub duration_minutes: u32,
    pub reward: SwarmResources,
    pub reward_items: Vec<String>,
    pub status: MissionStatus,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionReward {
    pub resources: SwarmResources,
    pub items: Vec<String>,
    pub xp_earned: u64,
    pub mission_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmState {
    pub units: Vec<SwarmUnit>,
    pub buildings: Vec<Building>,
    pub resources: SwarmResources,
    pub max_units: u32,
    pub max_essence: u64,
    pub evolution_paths: Vec<EvolutionPath>,
}
