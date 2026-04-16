// SPDX-License-Identifier: Elastic-2.0
//! Forge Swarm types -- Faction, UnitType, SwarmUnit, EvolutionPath.

use serde::{Deserialize, Serialize};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("forge_quest::swarm_types", "Swarm Types");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Faction {
    Insects,
    Demons,
    Undead,
    Humans,
}

impl Faction {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Insects => "insects",
            Self::Demons => "demons",
            Self::Undead => "undead",
            Self::Humans => "humans",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "demons" => Self::Demons,
            "undead" => Self::Undead,
            "humans" => Self::Humans,
            _ => Self::Insects,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UnitType {
    // Tier 1 (from Larva)
    ForgeDrone,    // Resource gatherer -- earns Essence from user actions
    ImpScout,      // Fast task runner -- quick AI queries, small tasks

    // Tier 2 (evolved from Tier 1)
    Viper,         // Multi-purpose -- complex tasks, analysis
    ShadowWeaver,  // Security/stealth -- self-healing, credential guard
    Skyweaver,     // Browser/web -- web scraping, research
    Overseer,      // Monitoring -- health checks, performance watch

    // Tier 3 (evolved from Tier 2)
    Titan,         // Heavy-duty -- MoA ensemble, complex reasoning
    SwarmMother,   // Spawner -- creates new Larva automatically
    Ravager,       // Elite fighter -- boss battles, hard quests

    // Tier 4 (unique, max 1)
    Matriarch,     // Queen -- controls entire swarm, +20% all stats

    // Tier 2 (expanded)
    SporeCrawler,  // Mobile defense structure -- can root/unroot
    Infestor,      // Mind control -- takes over enemy units
    NydusWorm,     // Tunnel transport between bases
    HiveGuard,     // Ranged bio-artillery

    // Tier 3 (expanded)
    Gargoyle,      // Flying acid-spitting scout
    Carnifex,      // Heavy siege breaker
    RipperSwarm,   // Biomass harvesters (eat planets)
    Haruspex,      // Devours enemies, heals self
    Broodling,     // Short-lived swarm units (auto-spawn from Swarm Mother)

    // Tier 4 (expanded, unique)
    Dominatrix,    // Swarm amplifier -- +20% all nearby units

    // --- Demon Faction (Tier 1-4) ---
    DemonImp,      // T1 Worker -- collects infernal essence
    Hellspawn,     // T1 Scout -- patrols the burning wastes
    Succubus,      // T1 Ranged -- charm-based ranged attacks
    Kultist,       // T1 Support -- buffs nearby demons
    FlameImp,      // T1 Ranged -- hurls fireballs
    Infernal,      // T2 Melee -- brute-force frontliner
    WarpFiend,     // T2 Caster -- warps reality around targets
    PitLord,       // T2 Tank -- massive HP, crowd control
    ChaosKnight,   // T2 Cavalry -- fast mounted demon
    Demonologist,  // T2 Caster -- summons lesser demons
    DoomGuard,     // T3 Elite -- elite demon warrior
    Balrog,        // T3 Flying -- winged fire demon
    ShadowDemon,   // T3 Assassin -- stealth one-shot kills
    Archfiend,     // T3 Support -- buffs entire demon army
    HellfireGolem, // T3 Siege -- lobs hellfire at structures
    VoidWalker,    // T4 Mythic -- phases between dimensions
    DemonPrince,   // T4 Commander -- leads demon armies
    AbyssLord,     // T4 Legendary -- risen from the deepest abyss
    Baalzephon,    // T4 Titan -- colossal arch-demon
    DarkArchon,    // T4 Hero -- master of forbidden magic

    // --- Undead Faction (Tier 1-4) ---
    Ghoul,            // T1 Worker -- digs graves, collects corpses
    SkeletonWarrior,  // T1 Melee -- basic skeletal foot-soldier
    ZombieHorde,      // T1 Swarm -- shambling mass of undead
    PlagueDoctor,     // T1 Support -- spreads disease, heals undead
    GraveDigger,      // T1 Scout -- unearths hidden resources
    Banshee,          // T2 Caster -- wailing spirit, AoE fear
    DeathKnight,      // T2 Cavalry -- mounted undead warrior
    Wraith,           // T2 Assassin -- phasing spirit, ignores armor
    CryptGuard,       // T2 Defender -- armored tomb protector
    SoulReaper,       // T2 Ranged -- scythe projectiles
    BoneGolem,        // T3 Tank -- massive construct of fused bones
    Necromancer,      // T3 Support -- raises fallen enemies
    Revenant,         // T3 Elite -- vengeful risen warrior
    Abomination,      // T3 Siege -- stitched horror, siege engine
    VampireLord,      // T3 Flying -- bat-form, life drain
    LichKing,         // T4 Commander -- supreme undead sorcerer
    DreadLord,        // T4 Legendary -- terror incarnate
    BoneHydra,        // T4 Mythic -- multi-headed skeletal beast
    PhantomLegion,    // T4 Swarm Elite -- spectral army
    DeathEmperor,     // T4 Titan -- ruler of all undead

    // --- Human Faction (Tier 1-4) ---
    Peasant,          // T1 Worker -- gathers gold and lumber
    Footman,          // T1 Melee -- basic infantry soldier
    Rifleman,         // T1 Ranged -- trained marksman with rifle
    Priest,           // T1 Support -- heals and dispels
    Militia,          // T1 Swarm -- civilian volunteers, temporary
    Knight,           // T2 Cavalry -- mounted heavy lancer
    Sorceress,        // T2 Caster -- arcane spellcaster
    CaptainOfTheGuard,// T2 Melee -- armored squad leader
    Crossbowman,      // T2 Ranged -- heavy crossbow specialist
    BattleMage,       // T2 Caster -- combat mage with area spells
    Paladin,          // T3 Holy -- divine warrior with healing
    GryphonRider,     // T3 Flying -- aerial cavalry on gryphon
    SiegeEngineer,    // T3 Siege -- operates siege weapons
    SpellBreaker,     // T3 Anti-magic -- nullifies enemy spells
    MortarTeam,       // T3 Siege -- long-range bombardment crew
    KingChampion,     // T4 Commander -- supreme military leader
    Archmage,         // T4 Hero -- most powerful human mage
    GrandMarshal,     // T4 Legendary -- undefeated field marshal
    DragonKnight,     // T4 Mythic -- rides an armored dragon
    HighInquisitor,   // T4 Titan -- purges all unholy forces
}
impl UnitType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::ForgeDrone => "forge_drone",
            Self::ImpScout => "imp_scout",
            Self::Viper => "viper",
            Self::ShadowWeaver => "shadow_weaver",
            Self::Skyweaver => "skyweaver",
            Self::Overseer => "overseer",
            Self::Titan => "titan",
            Self::SwarmMother => "swarm_mother",
            Self::Ravager => "ravager",
            Self::Matriarch => "matriarch",
            Self::SporeCrawler => "spore_crawler",
            Self::Infestor => "infestor",
            Self::NydusWorm => "nydus_worm",
            Self::HiveGuard => "hive_guard",
            Self::Gargoyle => "gargoyle",
            Self::Carnifex => "carnifex",
            Self::RipperSwarm => "ripper_swarm",
            Self::Haruspex => "haruspex",
            Self::Broodling => "broodling",
            Self::Dominatrix => "dominatrix",
            // Demon faction
            Self::DemonImp => "demon_imp",
            Self::Hellspawn => "hellspawn",
            Self::Succubus => "succubus",
            Self::Kultist => "kultist",
            Self::FlameImp => "flame_imp",
            Self::Infernal => "infernal",
            Self::WarpFiend => "warp_fiend",
            Self::PitLord => "pit_lord",
            Self::ChaosKnight => "chaos_knight",
            Self::Demonologist => "demonologist",
            Self::DoomGuard => "doom_guard",
            Self::Balrog => "balrog",
            Self::ShadowDemon => "shadow_demon",
            Self::Archfiend => "archfiend",
            Self::HellfireGolem => "hellfire_golem",
            Self::VoidWalker => "void_walker",
            Self::DemonPrince => "demon_prince",
            Self::AbyssLord => "abyss_lord",
            Self::Baalzephon => "baalzephon",
            Self::DarkArchon => "dark_archon",
            // Undead faction
            Self::Ghoul => "ghoul",
            Self::SkeletonWarrior => "skeleton_warrior",
            Self::ZombieHorde => "zombie_horde",
            Self::PlagueDoctor => "plague_doctor",
            Self::GraveDigger => "grave_digger",
            Self::Banshee => "banshee",
            Self::DeathKnight => "death_knight",
            Self::Wraith => "wraith",
            Self::CryptGuard => "crypt_guard",
            Self::SoulReaper => "soul_reaper",
            Self::BoneGolem => "bone_golem",
            Self::Necromancer => "necromancer",
            Self::Revenant => "revenant",
            Self::Abomination => "abomination",
            Self::VampireLord => "vampire_lord",
            Self::LichKing => "lich_king",
            Self::DreadLord => "dread_lord",
            Self::BoneHydra => "bone_hydra",
            Self::PhantomLegion => "phantom_legion",
            Self::DeathEmperor => "death_emperor",
            // Human faction
            Self::Peasant => "peasant",
            Self::Footman => "footman",
            Self::Rifleman => "rifleman",
            Self::Priest => "priest",
            Self::Militia => "militia",
            Self::Knight => "knight",
            Self::Sorceress => "sorceress",
            Self::CaptainOfTheGuard => "captain_of_the_guard",
            Self::Crossbowman => "crossbowman",
            Self::BattleMage => "battle_mage",
            Self::Paladin => "paladin",
            Self::GryphonRider => "gryphon_rider",
            Self::SiegeEngineer => "siege_engineer",
            Self::SpellBreaker => "spell_breaker",
            Self::MortarTeam => "mortar_team",
            Self::KingChampion => "king_champion",
            Self::Archmage => "archmage",
            Self::GrandMarshal => "grand_marshal",
            Self::DragonKnight => "dragon_knight",
            Self::HighInquisitor => "high_inquisitor",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "forge_drone" => Self::ForgeDrone,
            "imp_scout" => Self::ImpScout,
            "viper" => Self::Viper,
            "shadow_weaver" => Self::ShadowWeaver,
            "skyweaver" => Self::Skyweaver,
            "overseer" => Self::Overseer,
            "titan" => Self::Titan,
            "swarm_mother" => Self::SwarmMother,
            "ravager" => Self::Ravager,
            "matriarch" => Self::Matriarch,
            "spore_crawler" => Self::SporeCrawler,
            "infestor" => Self::Infestor,
            "nydus_worm" => Self::NydusWorm,
            "hive_guard" => Self::HiveGuard,
            "gargoyle" => Self::Gargoyle,
            "carnifex" => Self::Carnifex,
            "ripper_swarm" => Self::RipperSwarm,
            "haruspex" => Self::Haruspex,
            "broodling" => Self::Broodling,
            "dominatrix" => Self::Dominatrix,
            // Demon faction
            "demon_imp" => Self::DemonImp,
            "hellspawn" => Self::Hellspawn,
            "succubus" => Self::Succubus,
            "kultist" => Self::Kultist,
            "flame_imp" => Self::FlameImp,
            "infernal" => Self::Infernal,
            "warp_fiend" => Self::WarpFiend,
            "pit_lord" => Self::PitLord,
            "chaos_knight" => Self::ChaosKnight,
            "demonologist" => Self::Demonologist,
            "doom_guard" => Self::DoomGuard,
            "balrog" => Self::Balrog,
            "shadow_demon" => Self::ShadowDemon,
            "archfiend" => Self::Archfiend,
            "hellfire_golem" => Self::HellfireGolem,
            "void_walker" => Self::VoidWalker,
            "demon_prince" => Self::DemonPrince,
            "abyss_lord" => Self::AbyssLord,
            "baalzephon" => Self::Baalzephon,
            "dark_archon" => Self::DarkArchon,
            // Undead faction
            "ghoul" => Self::Ghoul,
            "skeleton_warrior" => Self::SkeletonWarrior,
            "zombie_horde" => Self::ZombieHorde,
            "plague_doctor" => Self::PlagueDoctor,
            "grave_digger" => Self::GraveDigger,
            "banshee" => Self::Banshee,
            "death_knight" => Self::DeathKnight,
            "wraith" => Self::Wraith,
            "crypt_guard" => Self::CryptGuard,
            "soul_reaper" => Self::SoulReaper,
            "bone_golem" => Self::BoneGolem,
            "necromancer" => Self::Necromancer,
            "revenant" => Self::Revenant,
            "abomination" => Self::Abomination,
            "vampire_lord" => Self::VampireLord,
            "lich_king" => Self::LichKing,
            "dread_lord" => Self::DreadLord,
            "bone_hydra" => Self::BoneHydra,
            "phantom_legion" => Self::PhantomLegion,
            "death_emperor" => Self::DeathEmperor,
            // Human faction
            "peasant" => Self::Peasant,
            "footman" => Self::Footman,
            "rifleman" => Self::Rifleman,
            "priest" => Self::Priest,
            "militia" => Self::Militia,
            "knight" => Self::Knight,
            "sorceress" => Self::Sorceress,
            "captain_of_the_guard" => Self::CaptainOfTheGuard,
            "crossbowman" => Self::Crossbowman,
            "battle_mage" => Self::BattleMage,
            "paladin" => Self::Paladin,
            "gryphon_rider" => Self::GryphonRider,
            "siege_engineer" => Self::SiegeEngineer,
            "spell_breaker" => Self::SpellBreaker,
            "mortar_team" => Self::MortarTeam,
            "king_champion" => Self::KingChampion,
            "archmage" => Self::Archmage,
            "grand_marshal" => Self::GrandMarshal,
            "dragon_knight" => Self::DragonKnight,
            "high_inquisitor" => Self::HighInquisitor,
            _ => Self::ForgeDrone,
        }
    }

    pub(crate) fn tier(&self) -> u32 {
        match self {
            Self::ForgeDrone | Self::ImpScout => 1,
            Self::Viper | Self::ShadowWeaver | Self::Skyweaver | Self::Overseer => 2,
            Self::Titan | Self::SwarmMother | Self::Ravager
            | Self::Gargoyle | Self::Carnifex | Self::RipperSwarm
            | Self::Haruspex | Self::Broodling => 3,
            Self::Matriarch | Self::Dominatrix => 4,
            Self::SporeCrawler | Self::Infestor
            | Self::NydusWorm | Self::HiveGuard => 2,
            // Demon faction
            Self::DemonImp | Self::Hellspawn | Self::Succubus
            | Self::Kultist | Self::FlameImp => 1,
            Self::Infernal | Self::WarpFiend | Self::PitLord
            | Self::ChaosKnight | Self::Demonologist => 2,
            Self::DoomGuard | Self::Balrog | Self::ShadowDemon
            | Self::Archfiend | Self::HellfireGolem => 3,
            Self::VoidWalker | Self::DemonPrince | Self::AbyssLord
            | Self::Baalzephon | Self::DarkArchon => 4,
            // Undead faction
            Self::Ghoul | Self::SkeletonWarrior | Self::ZombieHorde
            | Self::PlagueDoctor | Self::GraveDigger => 1,
            Self::Banshee | Self::DeathKnight | Self::Wraith
            | Self::CryptGuard | Self::SoulReaper => 2,
            Self::BoneGolem | Self::Necromancer | Self::Revenant
            | Self::Abomination | Self::VampireLord => 3,
            Self::LichKing | Self::DreadLord | Self::BoneHydra
            | Self::PhantomLegion | Self::DeathEmperor => 4,
            // Human faction
            Self::Peasant | Self::Footman | Self::Rifleman
            | Self::Priest | Self::Militia => 1,
            Self::Knight | Self::Sorceress | Self::CaptainOfTheGuard
            | Self::Crossbowman | Self::BattleMage => 2,
            Self::Paladin | Self::GryphonRider | Self::SiegeEngineer
            | Self::SpellBreaker | Self::MortarTeam => 3,
            Self::KingChampion | Self::Archmage | Self::GrandMarshal
            | Self::DragonKnight | Self::HighInquisitor => 4,
        }
    }

    pub(crate) fn emoji(&self) -> &'static str {
        match self {
            Self::ForgeDrone => "drone",
            Self::ImpScout => "scout",
            Self::Viper => "viper",
            Self::ShadowWeaver => "shadow",
            Self::Skyweaver => "sky",
            Self::Overseer => "eye",
            Self::Titan => "titan",
            Self::SwarmMother => "mother",
            Self::Ravager => "ravager",
            Self::Matriarch => "queen",
            Self::SporeCrawler => "crab",
            Self::Infestor => "brain",
            Self::NydusWorm => "worm",
            Self::HiveGuard => "bow",
            Self::Gargoyle => "bat",
            Self::Carnifex => "rhino",
            Self::RipperSwarm => "ant",
            Self::Haruspex => "croc",
            Self::Broodling => "chick",
            Self::Dominatrix => "crown",
            // Demon faction
            Self::DemonImp => "imp",
            Self::Hellspawn => "flame",
            Self::Succubus => "heart",
            Self::Kultist => "hooded",
            Self::FlameImp => "fire",
            Self::Infernal => "demon",
            Self::WarpFiend => "vortex",
            Self::PitLord => "horns",
            Self::ChaosKnight => "horse",
            Self::Demonologist => "book",
            Self::DoomGuard => "shield",
            Self::Balrog => "wings",
            Self::ShadowDemon => "dagger",
            Self::Archfiend => "star",
            Self::HellfireGolem => "rock",
            Self::VoidWalker => "ghost",
            Self::DemonPrince => "scepter",
            Self::AbyssLord => "abyss",
            Self::Baalzephon => "colossus",
            Self::DarkArchon => "orb",
            // Undead faction
            Self::Ghoul => "claw",
            Self::SkeletonWarrior => "bone",
            Self::ZombieHorde => "horde",
            Self::PlagueDoctor => "mask",
            Self::GraveDigger => "shovel",
            Self::Banshee => "scream",
            Self::DeathKnight => "skull",
            Self::Wraith => "wisp",
            Self::CryptGuard => "tomb",
            Self::SoulReaper => "scythe",
            Self::BoneGolem => "golem",
            Self::Necromancer => "staff",
            Self::Revenant => "blade",
            Self::Abomination => "stitch",
            Self::VampireLord => "fang",
            Self::LichKing => "lich",
            Self::DreadLord => "terror",
            Self::BoneHydra => "hydra",
            Self::PhantomLegion => "legion",
            Self::DeathEmperor => "throne",
            // Human faction
            Self::Peasant => "hammer",
            Self::Footman => "sword",
            Self::Rifleman => "rifle",
            Self::Priest => "cross",
            Self::Militia => "pitchfork",
            Self::Knight => "lance",
            Self::Sorceress => "wand",
            Self::CaptainOfTheGuard => "shield",
            Self::Crossbowman => "crossbow",
            Self::BattleMage => "fireball",
            Self::Paladin => "holy",
            Self::GryphonRider => "gryphon",
            Self::SiegeEngineer => "catapult",
            Self::SpellBreaker => "rune",
            Self::MortarTeam => "mortar",
            Self::KingChampion => "crown",
            Self::Archmage => "arcane",
            Self::GrandMarshal => "banner",
            Self::DragonKnight => "dragon",
            Self::HighInquisitor => "sunburst",
        }
    }

    pub(crate) fn base_stats(&self) -> (u32, u32, u32) {
        // (hp, attack, defense)
        match self {
            Self::ForgeDrone => (30, 5, 3),
            Self::ImpScout => (25, 8, 2),
            Self::Viper => (60, 18, 10),
            Self::ShadowWeaver => (50, 12, 18),
            Self::Skyweaver => (45, 15, 8),
            Self::Overseer => (55, 10, 15),
            Self::Titan => (120, 35, 25),
            Self::SwarmMother => (80, 15, 20),
            Self::Ravager => (100, 40, 15),
            Self::Matriarch => (200, 50, 40),
            Self::SporeCrawler => (70, 16, 22),
            Self::Infestor => (40, 8, 12),
            Self::NydusWorm => (90, 5, 30),
            Self::HiveGuard => (55, 22, 14),
            Self::Gargoyle => (65, 28, 10),
            Self::Carnifex => (140, 38, 30),
            Self::RipperSwarm => (35, 20, 5),
            Self::Haruspex => (110, 32, 18),
            Self::Broodling => (20, 12, 3),
            Self::Dominatrix => (180, 30, 35),
            // Demon faction -- T1
            Self::DemonImp => (28, 6, 4),
            Self::Hellspawn => (22, 9, 3),
            Self::Succubus => (26, 10, 5),
            Self::Kultist => (32, 4, 6),
            Self::FlameImp => (24, 11, 2),
            // Demon faction -- T2
            Self::Infernal => (65, 20, 14),
            Self::WarpFiend => (48, 16, 10),
            Self::PitLord => (85, 12, 24),
            Self::ChaosKnight => (58, 22, 12),
            Self::Demonologist => (42, 14, 11),
            // Demon faction -- T3
            Self::DoomGuard => (115, 36, 22),
            Self::Balrog => (105, 42, 16),
            Self::ShadowDemon => (70, 45, 8),
            Self::Archfiend => (90, 20, 28),
            Self::HellfireGolem => (150, 30, 35),
            // Demon faction -- T4
            Self::VoidWalker => (160, 38, 32),
            Self::DemonPrince => (190, 48, 38),
            Self::AbyssLord => (210, 55, 42),
            Self::Baalzephon => (250, 60, 50),
            Self::DarkArchon => (175, 52, 30),
            // Undead faction -- T1
            Self::Ghoul => (30, 7, 3),
            Self::SkeletonWarrior => (28, 8, 5),
            Self::ZombieHorde => (35, 5, 4),
            Self::PlagueDoctor => (26, 4, 6),
            Self::GraveDigger => (24, 6, 3),
            // Undead faction -- T2
            Self::Banshee => (44, 15, 8),
            Self::DeathKnight => (68, 22, 18),
            Self::Wraith => (38, 18, 6),
            Self::CryptGuard => (75, 14, 25),
            Self::SoulReaper => (50, 20, 10),
            // Undead faction -- T3
            Self::BoneGolem => (145, 25, 38),
            Self::Necromancer => (72, 18, 16),
            Self::Revenant => (110, 35, 20),
            Self::Abomination => (160, 32, 30),
            Self::VampireLord => (95, 38, 14),
            // Undead faction -- T4
            Self::LichKing => (185, 50, 36),
            Self::DreadLord => (200, 55, 40),
            Self::BoneHydra => (230, 45, 45),
            Self::PhantomLegion => (140, 42, 22),
            Self::DeathEmperor => (260, 58, 52),
            // Human faction -- T1
            Self::Peasant => (25, 4, 3),
            Self::Footman => (35, 8, 7),
            Self::Rifleman => (22, 10, 3),
            Self::Priest => (20, 3, 5),
            Self::Militia => (18, 6, 2),
            // Human faction -- T2
            Self::Knight => (70, 22, 18),
            Self::Sorceress => (45, 16, 8),
            Self::CaptainOfTheGuard => (65, 18, 20),
            Self::Crossbowman => (48, 20, 10),
            Self::BattleMage => (50, 18, 12),
            // Human faction -- T3
            Self::Paladin => (130, 32, 30),
            Self::GryphonRider => (100, 35, 16),
            Self::SiegeEngineer => (75, 28, 14),
            Self::SpellBreaker => (85, 25, 24),
            Self::MortarTeam => (60, 34, 8),
            // Human faction -- T4
            Self::KingChampion => (195, 52, 42),
            Self::Archmage => (170, 55, 30),
            Self::GrandMarshal => (220, 50, 48),
            Self::DragonKnight => (240, 58, 44),
            Self::HighInquisitor => (260, 54, 50),
        }
    }

    pub(crate) fn special_ability(&self) -> &'static str {
        match self {
            Self::ForgeDrone => "Gather: +10% Essence from productivity actions",
            Self::ImpScout => "Swift: Completes missions 20% faster",
            Self::Viper => "Analyze: +15% XP from complex tasks",
            Self::ShadowWeaver => "Cloak: 25% chance to avoid mission failure",
            Self::Skyweaver => "Soar: Can run web missions solo",
            Self::Overseer => "Watch: Reveals hidden mission bonuses",
            Self::Titan => "Crush: +30% damage in boss missions",
            Self::SwarmMother => "Spawn: Produces 1 free Larva every 60 min",
            Self::Ravager => "Frenzy: Double attack below 30% HP",
            Self::Matriarch => "Reign: +20% all stats for entire swarm",
            Self::SporeCrawler => "Root: Can root/unroot for mobile defense",
            Self::Infestor => "Dominate: Takes over enemy units temporarily",
            Self::NydusWorm => "Tunnel: Instant transport between bases",
            Self::HiveGuard => "Barrage: Ranged bio-artillery bombardment",
            Self::Gargoyle => "Acid Spit: Flying ranged AoE attack",
            Self::Carnifex => "Siege Break: +50% damage to buildings",
            Self::RipperSwarm => "Devour: Harvests biomass from fallen enemies",
            Self::Haruspex => "Consume: Devours enemies, heals self for 30% damage dealt",
            Self::Broodling => "Ephemeral: Auto-spawns from Swarm Mother, dies after 5 rounds",
            Self::Dominatrix => "Amplify: +20% all stats for nearby units",
            // Demon faction -- T1
            Self::DemonImp => "Scavenge: +12% Essence from defeated enemies",
            Self::Hellspawn => "Blaze Trail: +25% movement speed, reveals hidden paths",
            Self::Succubus => "Charm: 20% chance to skip enemy turn",
            Self::Kultist => "Dark Rite: +10% attack to all nearby demons",
            Self::FlameImp => "Fireball: Ranged AoE dealing 15% ATK to 3 targets",
            // Demon faction -- T2
            Self::Infernal => "Hellforge: +25% melee damage, ignores 10% armor",
            Self::WarpFiend => "Reality Tear: Teleports behind target, +30% crit chance",
            Self::PitLord => "Inferno Aura: Deals 5% max HP burn to adjacent enemies per round",
            Self::ChaosKnight => "Charge: First strike deals double damage",
            Self::Demonologist => "Summon Imp: Spawns a temporary DemonImp ally each battle",
            // Demon faction -- T3
            Self::DoomGuard => "Doom Strike: Attacks mark target, +40% damage on marked foes",
            Self::Balrog => "Flame Wing: Flying AoE fire, +35% damage to grounded units",
            Self::ShadowDemon => "Assassinate: One-shot kills targets below 20% HP",
            Self::Archfiend => "Infernal Command: +15% all stats for entire demon army",
            Self::HellfireGolem => "Hellfire Siege: +60% damage to buildings and structures",
            // Demon faction -- T4
            Self::VoidWalker => "Phase Shift: 30% chance to negate incoming damage entirely",
            Self::DemonPrince => "Legion Command: +25% all stats for demon faction",
            Self::AbyssLord => "Abyss Gate: Summons 2 random T2 demons at battle start",
            Self::Baalzephon => "Cataclysm: Deals 50% ATK to ALL enemies once per battle",
            Self::DarkArchon => "Forbidden Magic: Doubles spell damage, costs 10% own HP per cast",
            // Undead faction -- T1
            Self::Ghoul => "Corpse Harvest: +10% resources from fallen enemies",
            Self::SkeletonWarrior => "Reassemble: Revives once per battle at 30% HP",
            Self::ZombieHorde => "Overwhelm: +5% ATK per allied zombie in battle",
            Self::PlagueDoctor => "Pestilence: Poisons target, -3% HP per round for 5 rounds",
            Self::GraveDigger => "Unearth: +20% chance to find bonus materials after battles",
            // Undead faction -- T2
            Self::Banshee => "Wail: AoE fear, 15% chance to stun each enemy for 1 round",
            Self::DeathKnight => "Death Charge: Mounted strike, +30% first-hit damage",
            Self::Wraith => "Incorporeal: Ignores 50% of target armor",
            Self::CryptGuard => "Tomb Shield: Absorbs first 20% of incoming damage for allies",
            Self::SoulReaper => "Soul Harvest: Each kill grants +5% ATK for rest of battle",
            // Undead faction -- T3
            Self::BoneGolem => "Bone Wall: Reduces all incoming damage by 25%, taunts enemies",
            Self::Necromancer => "Raise Dead: Revives 1 fallen ally per battle at 40% HP",
            Self::Revenant => "Vengeance: +50% damage when below 25% HP",
            Self::Abomination => "Putrid Explosion: On death, deals 30% max HP to nearby enemies",
            Self::VampireLord => "Life Drain: Heals for 35% of damage dealt, can fly",
            // Undead faction -- T4
            Self::LichKing => "Undead Mastery: +20% all stats for entire undead army",
            Self::DreadLord => "Terror Aura: -15% ATK and DEF on all nearby enemies",
            Self::BoneHydra => "Regrow Heads: Regenerates 10% max HP per round, multi-attack",
            Self::PhantomLegion => "Ghost Army: Summons 3 spectral warriors at battle start",
            Self::DeathEmperor => "Death Dominion: All fallen enemies rise as temporary allies",
            // Human faction -- T1
            Self::Peasant => "Harvest: +12% Gold from productivity actions",
            Self::Footman => "Defend: +15% DEF when adjacent to another Footman",
            Self::Rifleman => "Long Shot: +20% range, ignores 10% armor at distance",
            Self::Priest => "Inner Fire: Heals 15% HP to lowest-health ally each round",
            Self::Militia => "Call to Arms: Temporary unit, +30% ATK for 3 rounds then disbands",
            // Human faction -- T2
            Self::Knight => "Charge: First strike deals double damage, +25% speed",
            Self::Sorceress => "Slow: Reduces enemy speed by 30% for 2 rounds",
            Self::CaptainOfTheGuard => "Rally: +15% ATK and DEF to all nearby infantry",
            Self::Crossbowman => "Piercing Bolt: Ignores 40% armor on critical hits",
            Self::BattleMage => "Flame Strike: AoE spell dealing 20% ATK to 3 targets",
            // Human faction -- T3
            Self::Paladin => "Holy Light: Heals 25% max HP to target ally, damages undead",
            Self::GryphonRider => "Storm Hammer: Flying AoE stun, +30% damage to grounded units",
            Self::SiegeEngineer => "Siege Mode: +60% damage to buildings, -50% movement speed",
            Self::SpellBreaker => "Spell Steal: Removes enemy buffs, gains +10% stats per buff stolen",
            Self::MortarTeam => "Barrage: Long-range AoE, +40% damage vs structures",
            // Human faction -- T4
            Self::KingChampion => "Royal Command: +20% all stats for entire human army",
            Self::Archmage => "Arcane Brilliance: Triples spell damage, +25% mana regeneration",
            Self::GrandMarshal => "Unbreakable Line: +30% DEF for all human units, taunt all enemies",
            Self::DragonKnight => "Dragon Fire: AoE 50% ATK to all enemies once per battle, flying",
            Self::HighInquisitor => "Purge: Deals 3x damage to Demons and Undead, immune to dark magic",
        }
    }

    /// Returns which faction this unit belongs to.
    pub(crate) fn faction(&self) -> Faction {
        match self {
            // Insect faction (original units)
            Self::ForgeDrone | Self::ImpScout
            | Self::Viper | Self::ShadowWeaver | Self::Skyweaver | Self::Overseer
            | Self::Titan | Self::SwarmMother | Self::Ravager
            | Self::Matriarch
            | Self::SporeCrawler | Self::Infestor | Self::NydusWorm | Self::HiveGuard
            | Self::Gargoyle | Self::Carnifex | Self::RipperSwarm | Self::Haruspex | Self::Broodling
            | Self::Dominatrix => Faction::Insects,
            // Demon faction
            Self::DemonImp | Self::Hellspawn | Self::Succubus | Self::Kultist | Self::FlameImp
            | Self::Infernal | Self::WarpFiend | Self::PitLord | Self::ChaosKnight | Self::Demonologist
            | Self::DoomGuard | Self::Balrog | Self::ShadowDemon | Self::Archfiend | Self::HellfireGolem
            | Self::VoidWalker | Self::DemonPrince | Self::AbyssLord | Self::Baalzephon | Self::DarkArchon => Faction::Demons,
            // Undead faction
            Self::Ghoul | Self::SkeletonWarrior | Self::ZombieHorde | Self::PlagueDoctor | Self::GraveDigger
            | Self::Banshee | Self::DeathKnight | Self::Wraith | Self::CryptGuard | Self::SoulReaper
            | Self::BoneGolem | Self::Necromancer | Self::Revenant | Self::Abomination | Self::VampireLord
            | Self::LichKing | Self::DreadLord | Self::BoneHydra | Self::PhantomLegion | Self::DeathEmperor => Faction::Undead,
            // Human faction
            Self::Peasant | Self::Footman | Self::Rifleman | Self::Priest | Self::Militia
            | Self::Knight | Self::Sorceress | Self::CaptainOfTheGuard | Self::Crossbowman | Self::BattleMage
            | Self::Paladin | Self::GryphonRider | Self::SiegeEngineer | Self::SpellBreaker | Self::MortarTeam
            | Self::KingChampion | Self::Archmage | Self::GrandMarshal | Self::DragonKnight | Self::HighInquisitor => Faction::Humans,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmUnit {
    pub id: String,
    pub unit_type: UnitType,
    pub name: String,
    pub level: u32,
    pub hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub special_ability: String,
    pub assigned_task: Option<String>,
    pub efficiency: f32, // 0.0-2.0, improves with use
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionPath {
    pub from: String,
    pub to: String,
    pub essence_cost: u64,
    pub level_requirement: u32,
    pub materials: Vec<(String, u32)>,
}
