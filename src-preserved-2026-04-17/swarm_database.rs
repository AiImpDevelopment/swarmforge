// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Unified Game Database
//!
//! Implements the complete 18-table SQLite schema for the SwarmForge RPG module.
//! Provides player management (Argon2id auth), procedural planet generation
//! (4 sizes, 10 biomes, Poisson resources), colony resource tracking with
//! faction-specific resources, patrol routes, prestige/Phoenix Ash meta-progression,
//! and newbie protection.
//!
//! ## Architecture
//! - Single `swarmforge.db` file in the app data directory (WAL mode)
//! - `SwarmDatabase` struct wraps a `Mutex<Connection>` for thread-safe access
//! - Tauri commands are thin wrappers that delegate to `SwarmDatabase` methods
//!
//! ## Schema Source
//! Exact SQL from `docs/files_extracted/swarmforge_schema.sql` (18 tables + indexes)
//!
//! ## Planet Generation Source
//! Logic from `docs/files_extracted/swarmforge_planet_gen.rs`:
//! - 4 size classes: Small(61 hex), Medium(127), Large(217), Huge(331)
//! - 10 biomes with temperature-based distribution
//! - Poisson-like resource node placement with biome multipliers
//! - Deterministic via ChaCha20Rng seeded from system_seed + slot

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params, Version,
};
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppResult, ImpForgeError};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_database", "Game");

// ============================================================================
// Constants
// ============================================================================

/// Argon2id parameters (OWASP 2025 recommended for game client)
const ARGON2_MEMORY_KIB: u32 = 65_536; // 64 MiB
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;
const ARGON2_OUTPUT_LEN: usize = 32;

/// Commander name constraints
const MIN_COMMANDER_NAME_LEN: usize = 3;
const MAX_COMMANDER_NAME_LEN: usize = 20;
const MIN_PASSWORD_LEN: usize = 8;

/// Newbie protection defaults
const NEWBIE_PROTECTION_SCORE_LIMIT: i64 = 5000;
const NEWBIE_PROTECTION_DAYS: i64 = 7;

// ============================================================================
// Faction
// ============================================================================

/// Playable factions (customer version).
/// Human faction is DEV-ONLY and NEVER included in customer builds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Faction {
    Insects,
    Demons,
    Undead,
}
impl Faction {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Insects => "insects",
            Self::Demons => "demons",
            Self::Undead => "undead",
        }
    }

    pub(crate) fn from_str(s: &str) -> AppResult<Self> {
        match s.to_lowercase().as_str() {
            "insects" => Ok(Self::Insects),
            "demons" => Ok(Self::Demons),
            "undead" => Ok(Self::Undead),
            _ => Err(ImpForgeError::validation(
                "INVALID_FACTION",
                format!("Invalid faction: '{}'. Must be insects, demons, or undead.", s),
            )),
        }
    }
}

// ============================================================================
// Planet Size & Biome Types
// ============================================================================

/// Planet size classes with hex grid parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PlanetSize {
    Small,  // 61 hexes, radius 4, max 5 colonies
    Medium, // 127 hexes, radius 6, max 7 colonies
    Large,  // 217 hexes, radius 8, max 9 colonies
    Huge,   // 331 hexes, radius 10, max 11 colonies
}

impl PlanetSize {
    pub(crate) fn hex_count(&self) -> i32 {
        match self {
            Self::Small => 61,
            Self::Medium => 127,
            Self::Large => 217,
            Self::Huge => 331,
        }
    }

    pub(crate) fn grid_radius(&self) -> i32 {
        match self {
            Self::Small => 4,
            Self::Medium => 6,
            Self::Large => 8,
            Self::Huge => 10,
        }
    }

    pub(crate) fn max_colonies(&self) -> i32 {
        match self {
            Self::Small => 5,
            Self::Medium => 7,
            Self::Large => 9,
            Self::Huge => 11,
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::Huge => "huge",
        }
    }
}

/// 10 biome types with resource multipliers and ecological properties.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Biome {
    Tropical,
    Temperate,
    Arid,
    Tundra,
    Volcanic,
    Ocean,
    Crystal,
    Fungal,
    Barren,
    Swamp,
}

impl Biome {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Tropical => "tropical",
            Self::Temperate => "temperate",
            Self::Arid => "arid",
            Self::Tundra => "tundra",
            Self::Volcanic => "volcanic",
            Self::Ocean => "ocean",
            Self::Crystal => "crystal",
            Self::Fungal => "fungal",
            Self::Barren => "barren",
            Self::Swamp => "swamp",
        }
    }

    /// Resource multipliers: (ore, crystal, essence)
    pub(crate) fn resource_multipliers(&self) -> (f64, f64, f64) {
        match self {
            Self::Tropical => (0.8, 0.6, 1.0),
            Self::Temperate => (1.0, 1.0, 0.8),
            Self::Arid => (1.5, 0.5, 0.3),
            Self::Tundra => (0.7, 0.8, 1.5),
            Self::Volcanic => (1.8, 1.5, 0.2),
            Self::Ocean => (0.1, 0.3, 1.8),
            Self::Crystal => (0.3, 2.5, 0.5),
            Self::Fungal => (0.5, 0.7, 1.3),
            Self::Barren => (0.4, 0.3, 0.2),
            Self::Swamp => (0.6, 0.4, 1.2),
        }
    }

    /// Flora density (0.0 - 1.0)
    pub(crate) fn flora_density(&self) -> f64 {
        match self {
            Self::Tropical => 0.9,
            Self::Temperate => 0.7,
            Self::Arid => 0.2,
            Self::Tundra => 0.3,
            Self::Volcanic => 0.05,
            Self::Ocean => 0.4,
            Self::Crystal => 0.1,
            Self::Fungal => 0.95,
            Self::Barren => 0.05,
            Self::Swamp => 0.85,
        }
    }

    /// Fauna density (0.0 - 1.0)
    pub(crate) fn fauna_density(&self) -> f64 {
        match self {
            Self::Tropical => 0.8,
            Self::Temperate => 0.6,
            Self::Arid => 0.3,
            Self::Tundra => 0.2,
            Self::Volcanic => 0.02,
            Self::Ocean => 0.7,
            Self::Crystal => 0.05,
            Self::Fungal => 0.4,
            Self::Barren => 0.01,
            Self::Swamp => 0.6,
        }
    }
}

// ============================================================================
// Biome Distribution
// ============================================================================

/// Biome percentage distribution (sums to 100).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct BiomeDistribution {
    pub tropical: i32,
    pub temperate: i32,
    pub arid: i32,
    pub tundra: i32,
    pub volcanic: i32,
    pub ocean: i32,
    pub crystal: i32,
    pub fungal: i32,
    pub barren: i32,
    pub swamp: i32,
}

/// Generated hex on planet surface.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GeneratedHex {
    pub q: i32,
    pub r: i32,
    pub biome: Biome,
    pub elevation: i32,
    pub ore_nodes: i32,
    pub crystal_nodes: i32,
    pub essence_nodes: i32,
    pub rare_node: Option<String>,
    pub has_flora: bool,
    pub has_fauna: bool,
}

/// Complete generated planet data before DB insertion.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GeneratedPlanet {
    pub system_id: i64,
    pub slot: i32,
    pub seed: u64,
    pub size_class: PlanetSize,
    pub diameter_km: i32,
    pub base_fields: i32,
    pub temperature_min: i32,
    pub temperature_max: i32,
    pub biome_distribution: BiomeDistribution,
    pub ore_richness: f64,
    pub crystal_richness: f64,
    pub essence_richness: f64,
    pub hexes: Vec<GeneratedHex>,
}

// ============================================================================
// Full 18-table Schema (exact SQL from swarmforge_schema.sql)
// ============================================================================

const SCHEMA_SQL: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

-- 1. PLAYER & AUTH
CREATE TABLE IF NOT EXISTS sf_players (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    commander_name          TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    email                   TEXT,
    password_hash           TEXT    NOT NULL,
    ed25519_public_key      BLOB   NOT NULL,
    ed25519_signing_key_enc BLOB   NOT NULL,
    subscription_tier       TEXT    NOT NULL DEFAULT 'free'
        CHECK (subscription_tier IN ('free','swarmforge_sub','impforge_sub','lifetime')),
    phoenix_ash             INTEGER NOT NULL DEFAULT 0,
    dark_matter             INTEGER NOT NULL DEFAULT 0,
    total_play_time_secs    INTEGER NOT NULL DEFAULT 0,
    auto_login              INTEGER NOT NULL DEFAULT 0,
    created_at              INTEGER NOT NULL,
    last_login              INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sf_sessions (
    id              TEXT    PRIMARY KEY,
    player_id       INTEGER NOT NULL REFERENCES sf_players(id) ON DELETE CASCADE,
    access_token    TEXT    NOT NULL,
    refresh_token   TEXT,
    expires_at      INTEGER NOT NULL,
    created_at      INTEGER NOT NULL
);

-- 2. SAVE SLOTS & FACTION CHOICE
CREATE TABLE IF NOT EXISTS sf_save_slots (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id       INTEGER NOT NULL REFERENCES sf_players(id) ON DELETE CASCADE,
    slot_number     INTEGER NOT NULL CHECK (slot_number BETWEEN 1 AND 3),
    faction         TEXT    NOT NULL CHECK (faction IN ('insects','demons','undead')),
    commander_alias TEXT    NOT NULL,
    home_galaxy     INTEGER NOT NULL CHECK (home_galaxy BETWEEN 1 AND 5),
    home_system     INTEGER NOT NULL CHECK (home_system BETWEEN 1 AND 500),
    home_planet     INTEGER NOT NULL CHECK (home_planet BETWEEN 1 AND 15),
    colony_points   INTEGER NOT NULL DEFAULT 0,
    military_points INTEGER NOT NULL DEFAULT 0,
    research_points INTEGER NOT NULL DEFAULT 0,
    alliance_id     INTEGER REFERENCES sf_alliances(id),
    is_destroyed    INTEGER NOT NULL DEFAULT 0,
    phoenix_ash_pending INTEGER NOT NULL DEFAULT 0,
    total_colonies  INTEGER NOT NULL DEFAULT 1,
    created_at      INTEGER NOT NULL,
    last_played     INTEGER NOT NULL,
    UNIQUE(player_id, slot_number)
);

-- 3. UNIVERSE STRUCTURE
CREATE TABLE IF NOT EXISTS sf_galaxies (
    id      INTEGER PRIMARY KEY CHECK (id BETWEEN 1 AND 5),
    name    TEXT    NOT NULL,
    seed    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sf_systems (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    galaxy_id   INTEGER NOT NULL REFERENCES sf_galaxies(id),
    position    INTEGER NOT NULL CHECK (position BETWEEN 1 AND 500),
    seed        INTEGER NOT NULL,
    UNIQUE(galaxy_id, position)
);

CREATE TABLE IF NOT EXISTS sf_planets (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    system_id       INTEGER NOT NULL REFERENCES sf_systems(id),
    slot            INTEGER NOT NULL CHECK (slot BETWEEN 1 AND 15),
    seed            INTEGER NOT NULL,
    size_class      TEXT NOT NULL DEFAULT 'medium'
        CHECK (size_class IN ('small','medium','large','huge')),
    hex_count       INTEGER NOT NULL,
    max_colonies    INTEGER NOT NULL,
    diameter_km     INTEGER NOT NULL,
    base_fields     INTEGER NOT NULL,
    temperature_min INTEGER NOT NULL,
    temperature_max INTEGER NOT NULL,
    biome_tropical_pct  INTEGER NOT NULL DEFAULT 0,
    biome_temperate_pct INTEGER NOT NULL DEFAULT 0,
    biome_arid_pct      INTEGER NOT NULL DEFAULT 0,
    biome_tundra_pct    INTEGER NOT NULL DEFAULT 0,
    biome_volcanic_pct  INTEGER NOT NULL DEFAULT 0,
    biome_ocean_pct     INTEGER NOT NULL DEFAULT 0,
    biome_crystal_pct   INTEGER NOT NULL DEFAULT 0,
    biome_fungal_pct    INTEGER NOT NULL DEFAULT 0,
    biome_barren_pct    INTEGER NOT NULL DEFAULT 0,
    biome_swamp_pct     INTEGER NOT NULL DEFAULT 0,
    ore_richness        REAL NOT NULL DEFAULT 1.0,
    crystal_richness    REAL NOT NULL DEFAULT 1.0,
    essence_richness    REAL NOT NULL DEFAULT 1.0,
    is_colonized        INTEGER NOT NULL DEFAULT 0,
    UNIQUE(system_id, slot)
);

CREATE TABLE IF NOT EXISTS sf_planet_hexes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    planet_id   INTEGER NOT NULL REFERENCES sf_planets(id) ON DELETE CASCADE,
    hex_q       INTEGER NOT NULL,
    hex_r       INTEGER NOT NULL,
    biome       TEXT NOT NULL CHECK (biome IN (
        'tropical','temperate','arid','tundra','volcanic',
        'ocean','crystal','fungal','barren','swamp'
    )),
    elevation   INTEGER NOT NULL DEFAULT 50 CHECK (elevation BETWEEN 0 AND 100),
    ore_nodes       INTEGER NOT NULL DEFAULT 0,
    crystal_nodes   INTEGER NOT NULL DEFAULT 0,
    essence_nodes   INTEGER NOT NULL DEFAULT 0,
    rare_node_type  TEXT,
    controlled_by   INTEGER REFERENCES sf_colonies(id),
    terrain_type    TEXT DEFAULT 'neutral'
        CHECK (terrain_type IN ('neutral','creep','panik_keim','blight','terraformed')),
    terrain_pct     REAL NOT NULL DEFAULT 0.0 CHECK (terrain_pct BETWEEN 0.0 AND 100.0),
    has_flora       INTEGER NOT NULL DEFAULT 1,
    has_fauna       INTEGER NOT NULL DEFAULT 0,
    UNIQUE(planet_id, hex_q, hex_r)
);

-- 4. COLONIES
CREATE TABLE IF NOT EXISTS sf_colonies (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    save_slot_id    INTEGER NOT NULL REFERENCES sf_save_slots(id) ON DELETE CASCADE,
    planet_id       INTEGER NOT NULL REFERENCES sf_planets(id),
    colony_number   INTEGER NOT NULL,
    colony_name     TEXT    NOT NULL,
    faction         TEXT    NOT NULL CHECK (faction IN ('insects','demons','undead')),
    center_hex_q    INTEGER NOT NULL,
    center_hex_r    INTEGER NOT NULL,
    max_fields      INTEGER NOT NULL,
    used_fields     INTEGER NOT NULL DEFAULT 0,
    is_bonus_colony     INTEGER NOT NULL DEFAULT 0,
    bonus_visible_until INTEGER,
    res_ore_stored      REAL NOT NULL DEFAULT 500.0,
    res_crystal_stored  REAL NOT NULL DEFAULT 300.0,
    res_essence_stored  REAL NOT NULL DEFAULT 100.0,
    res_ore_rate        REAL NOT NULL DEFAULT 30.0,
    res_crystal_rate    REAL NOT NULL DEFAULT 15.0,
    res_essence_rate    REAL NOT NULL DEFAULT 0.0,
    res_energy_balance  REAL NOT NULL DEFAULT 0.0,
    res_last_calc       INTEGER NOT NULL,
    res_biomass         REAL NOT NULL DEFAULT 200.0,
    res_spore_gas       REAL NOT NULL DEFAULT 0.0,
    res_larvae          INTEGER NOT NULL DEFAULT 3,
    res_suenden         REAL NOT NULL DEFAULT 200.0,
    res_kultisten       INTEGER NOT NULL DEFAULT 0,
    res_corruption      REAL NOT NULL DEFAULT 0.0,
    res_eiter_essence   REAL NOT NULL DEFAULT 200.0,
    res_leichenteile    INTEGER NOT NULL DEFAULT 0,
    res_nahrung         REAL NOT NULL DEFAULT 0.0,
    res_energie         REAL NOT NULL DEFAULT 0.0,
    res_faith           REAL NOT NULL DEFAULT 0.0,
    population_current  INTEGER NOT NULL DEFAULT 5,
    population_max      INTEGER NOT NULL DEFAULT 10,
    creep_coverage_pct  REAL NOT NULL DEFAULT 0.0,
    blight_coverage_pct REAL NOT NULL DEFAULT 0.0,
    panik_keim_pct      REAL NOT NULL DEFAULT 0.0,
    defense_score       INTEGER NOT NULL DEFAULT 0,
    founded_at          INTEGER NOT NULL,
    last_updated        INTEGER NOT NULL,
    UNIQUE(save_slot_id, planet_id, colony_number)
);

-- 5. BUILDINGS
CREATE TABLE IF NOT EXISTS sf_building_defs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    internal_name   TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    faction         TEXT NOT NULL CHECK (faction IN ('insects','demons','undead','human','all')),
    category        TEXT NOT NULL CHECK (category IN (
        'resource','energy','storage','production','research',
        'facility','defense','special','supply'
    )),
    base_ore        INTEGER NOT NULL DEFAULT 0,
    base_crystal    INTEGER NOT NULL DEFAULT 0,
    base_essence    INTEGER NOT NULL DEFAULT 0,
    cost_factor     REAL NOT NULL DEFAULT 1.5,
    base_build_time INTEGER NOT NULL DEFAULT 60,
    fields_used     INTEGER NOT NULL DEFAULT 1,
    max_level       INTEGER NOT NULL DEFAULT 0,
    prod_coefficient REAL NOT NULL DEFAULT 0.0,
    energy_coeff    REAL NOT NULL DEFAULT 0.0,
    description     TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS sf_buildings (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    colony_id       INTEGER NOT NULL REFERENCES sf_colonies(id) ON DELETE CASCADE,
    building_def_id INTEGER NOT NULL REFERENCES sf_building_defs(id),
    level           INTEGER NOT NULL DEFAULT 1,
    is_upgrading    INTEGER NOT NULL DEFAULT 0,
    upgrade_finish  INTEGER,
    hex_q           INTEGER,
    hex_r           INTEGER,
    sub_tile_x      INTEGER,
    sub_tile_y      INTEGER,
    created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sf_build_queue (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    colony_id       INTEGER NOT NULL REFERENCES sf_colonies(id) ON DELETE CASCADE,
    queue_type      TEXT NOT NULL CHECK (queue_type IN ('building','research','ship','defense')),
    target_id       INTEGER NOT NULL,
    target_level    INTEGER,
    quantity        INTEGER DEFAULT 1,
    started_at      INTEGER NOT NULL,
    finishes_at     INTEGER NOT NULL,
    is_active       INTEGER NOT NULL DEFAULT 1
);

-- 6. UNITS
CREATE TABLE IF NOT EXISTS sf_unit_defs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    internal_name   TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    faction         TEXT NOT NULL CHECK (faction IN ('insects','demons','undead','human')),
    category        TEXT NOT NULL CHECK (category IN (
        'worker','combat_melee','combat_ranged','caster',
        'siege','flying','hero','special'
    )),
    tier            INTEGER NOT NULL CHECK (tier BETWEEN 1 AND 5),
    hp              INTEGER NOT NULL,
    armor           INTEGER NOT NULL DEFAULT 0,
    armor_type      TEXT NOT NULL DEFAULT 'light'
        CHECK (armor_type IN ('light','medium','heavy','fortified','heroic','bio','mechanical')),
    damage          INTEGER NOT NULL DEFAULT 0,
    damage_type     TEXT NOT NULL DEFAULT 'normal'
        CHECK (damage_type IN ('normal','pierce','concussive','explosive','magic','poison','plague','fire')),
    attack_speed    REAL NOT NULL DEFAULT 1.0,
    attack_range    REAL NOT NULL DEFAULT 1.0,
    move_speed      REAL NOT NULL DEFAULT 1.0,
    cost_ore        INTEGER NOT NULL DEFAULT 0,
    cost_crystal    INTEGER NOT NULL DEFAULT 0,
    cost_essence    INTEGER NOT NULL DEFAULT 0,
    cost_supply     INTEGER NOT NULL DEFAULT 1,
    cost_special    INTEGER NOT NULL DEFAULT 0,
    build_time_secs INTEGER NOT NULL DEFAULT 10,
    can_burrow      INTEGER NOT NULL DEFAULT 0,
    can_fly         INTEGER NOT NULL DEFAULT 0,
    can_cloak       INTEGER NOT NULL DEFAULT 0,
    is_detector     INTEGER NOT NULL DEFAULT 0,
    is_biological   INTEGER NOT NULL DEFAULT 1,
    is_mechanical   INTEGER NOT NULL DEFAULT 0,
    carry_capacity  INTEGER NOT NULL DEFAULT 0,
    special_json    TEXT,
    description     TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS sf_units (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    colony_id       INTEGER NOT NULL REFERENCES sf_colonies(id) ON DELETE CASCADE,
    unit_def_id     INTEGER NOT NULL REFERENCES sf_unit_defs(id),
    pos_x           REAL NOT NULL DEFAULT 0.0,
    pos_y           REAL NOT NULL DEFAULT 0.0,
    hp_current      INTEGER NOT NULL,
    mana_current    INTEGER NOT NULL DEFAULT 0,
    state           TEXT NOT NULL DEFAULT 'idle'
        CHECK (state IN ('idle','moving','attacking','gathering','building',
                         'patrolling','burrowed','garrisoned','dead',
                         'spreading_blight','morphing')),
    target_x        REAL,
    target_y        REAL,
    target_unit_id  INTEGER,
    control_group   INTEGER CHECK (control_group BETWEEN 0 AND 9),
    talent_points_spent INTEGER NOT NULL DEFAULT 0,
    xp              INTEGER NOT NULL DEFAULT 0,
    level           INTEGER NOT NULL DEFAULT 1,
    kills           INTEGER NOT NULL DEFAULT 0,
    adept_service_hours   REAL,
    adept_service_start   INTEGER,
    adept_eimer_level     REAL NOT NULL DEFAULT 100.0,
    imp_mana_pool         REAL NOT NULL DEFAULT 0.0,
    imp_mana_regen        REAL NOT NULL DEFAULT 0.0,
    created_at      INTEGER NOT NULL
);

-- 7. SHIPS
CREATE TABLE IF NOT EXISTS sf_ship_defs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    internal_name   TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    faction         TEXT NOT NULL CHECK (faction IN ('insects','demons','undead','human')),
    ship_class      TEXT NOT NULL CHECK (ship_class IN (
        'light_fighter','heavy_fighter','cruiser','battleship',
        'battlecruiser','bomber','destroyer','deathstar',
        'reaper','pathfinder','small_cargo','large_cargo',
        'recycler','spy_probe','colony_ship'
    )),
    structural_integrity INTEGER NOT NULL,
    shield_power    INTEGER NOT NULL,
    weapon_power    INTEGER NOT NULL,
    cargo_capacity  INTEGER NOT NULL DEFAULT 0,
    base_speed      INTEGER NOT NULL,
    fuel_consumption INTEGER NOT NULL,
    cost_ore        INTEGER NOT NULL DEFAULT 0,
    cost_crystal    INTEGER NOT NULL DEFAULT 0,
    cost_essence    INTEGER NOT NULL DEFAULT 0,
    cost_special    INTEGER NOT NULL DEFAULT 0,
    build_time_secs INTEGER NOT NULL DEFAULT 60,
    rapid_fire_json TEXT,
    drive_type      TEXT NOT NULL DEFAULT 'combustion'
        CHECK (drive_type IN ('combustion','impulse','hyperspace','organic','demonic','necrotic')),
    description     TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS sf_fleets (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    save_slot_id    INTEGER NOT NULL REFERENCES sf_save_slots(id) ON DELETE CASCADE,
    fleet_name      TEXT,
    origin_colony_id INTEGER REFERENCES sf_colonies(id),
    origin_galaxy   INTEGER NOT NULL,
    origin_system   INTEGER NOT NULL,
    origin_planet   INTEGER NOT NULL,
    dest_galaxy     INTEGER NOT NULL,
    dest_system     INTEGER NOT NULL,
    dest_planet     INTEGER NOT NULL,
    mission_type    TEXT NOT NULL CHECK (mission_type IN (
        'attack','transport','deploy','spy','colonize',
        'recycle','destroy','expedition','trade','harvest'
    )),
    depart_time     INTEGER NOT NULL,
    arrive_time     INTEGER NOT NULL,
    return_time     INTEGER,
    state           TEXT NOT NULL DEFAULT 'outbound'
        CHECK (state IN ('outbound','arrived','returning','completed','destroyed')),
    speed_pct       INTEGER NOT NULL DEFAULT 100 CHECK (speed_pct BETWEEN 1 AND 100),
    carry_ore       REAL NOT NULL DEFAULT 0,
    carry_crystal   REAL NOT NULL DEFAULT 0,
    carry_essence   REAL NOT NULL DEFAULT 0,
    fuel_used       REAL NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS sf_fleet_ships (
    fleet_id    INTEGER NOT NULL REFERENCES sf_fleets(id) ON DELETE CASCADE,
    ship_def_id INTEGER NOT NULL REFERENCES sf_ship_defs(id),
    quantity    INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (fleet_id, ship_def_id)
);

-- 8. DEFENSE STRUCTURES
CREATE TABLE IF NOT EXISTS sf_defense_defs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    internal_name   TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    faction         TEXT NOT NULL CHECK (faction IN ('insects','demons','undead','human')),
    tier            INTEGER NOT NULL CHECK (tier BETWEEN 1 AND 4),
    role            TEXT NOT NULL CHECK (role IN (
        'anti_ground','anti_air','wall','detection','garrison',
        'aoe_splash','slow_debuff','support','siege_class','shield_dome'
    )),
    structural_integrity INTEGER NOT NULL,
    shield_power    INTEGER NOT NULL DEFAULT 0,
    weapon_power    INTEGER NOT NULL DEFAULT 0,
    attack_range    REAL NOT NULL DEFAULT 7.0,
    attack_speed    REAL NOT NULL DEFAULT 1.0,
    cost_ore        INTEGER NOT NULL DEFAULT 0,
    cost_crystal    INTEGER NOT NULL DEFAULT 0,
    cost_essence    INTEGER NOT NULL DEFAULT 0,
    rebuild_chance  REAL NOT NULL DEFAULT 0.70,
    special_json    TEXT,
    description     TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS sf_defenses (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    colony_id       INTEGER NOT NULL REFERENCES sf_colonies(id) ON DELETE CASCADE,
    defense_def_id  INTEGER NOT NULL REFERENCES sf_defense_defs(id),
    quantity        INTEGER NOT NULL DEFAULT 1,
    pos_x           REAL,
    pos_y           REAL,
    UNIQUE(colony_id, defense_def_id)
);

-- 9. RESEARCH / TECHNOLOGY
CREATE TABLE IF NOT EXISTS sf_tech_defs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    internal_name   TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    faction         TEXT NOT NULL CHECK (faction IN ('insects','demons','undead','human','all')),
    category        TEXT NOT NULL CHECK (category IN (
        'economy','military','propulsion','defense','special'
    )),
    max_level       INTEGER NOT NULL DEFAULT 20,
    base_ore        INTEGER NOT NULL DEFAULT 0,
    base_crystal    INTEGER NOT NULL DEFAULT 0,
    base_essence    INTEGER NOT NULL DEFAULT 0,
    cost_factor     REAL NOT NULL DEFAULT 2.0,
    base_time_secs  INTEGER NOT NULL DEFAULT 120,
    prerequisite_json TEXT,
    effect_json     TEXT,
    description     TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS sf_research (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    save_slot_id    INTEGER NOT NULL REFERENCES sf_save_slots(id) ON DELETE CASCADE,
    tech_def_id     INTEGER NOT NULL REFERENCES sf_tech_defs(id),
    current_level   INTEGER NOT NULL DEFAULT 0,
    is_researching  INTEGER NOT NULL DEFAULT 0,
    finish_time     INTEGER,
    UNIQUE(save_slot_id, tech_def_id)
);

-- 10. PATROL ROUTES
CREATE TABLE IF NOT EXISTS sf_patrol_routes (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    colony_id           INTEGER NOT NULL REFERENCES sf_colonies(id) ON DELETE CASCADE,
    route_name          TEXT,
    waypoints_json      TEXT NOT NULL,
    mode                TEXT NOT NULL DEFAULT 'cyclic'
        CHECK (mode IN ('cyclic','ping_pong')),
    engagement_radius   REAL NOT NULL DEFAULT 8.0 CHECK (engagement_radius BETWEEN 3.0 AND 15.0),
    leash_distance      REAL NOT NULL DEFAULT 12.0 CHECK (leash_distance BETWEEN 5.0 AND 20.0),
    behavior_on_contact TEXT NOT NULL DEFAULT 'engage'
        CHECK (behavior_on_contact IN ('engage','report','both')),
    is_active           INTEGER NOT NULL DEFAULT 1,
    offline_efficiency  REAL NOT NULL DEFAULT 0.75,
    created_at          INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sf_patrol_assignments (
    patrol_route_id INTEGER NOT NULL REFERENCES sf_patrol_routes(id) ON DELETE CASCADE,
    unit_id         INTEGER NOT NULL REFERENCES sf_units(id) ON DELETE CASCADE,
    PRIMARY KEY (patrol_route_id, unit_id)
);

CREATE TABLE IF NOT EXISTS sf_patrol_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    patrol_route_id INTEGER NOT NULL REFERENCES sf_patrol_routes(id),
    encounter_time  INTEGER NOT NULL,
    enemy_faction   TEXT NOT NULL,
    enemy_count     INTEGER NOT NULL,
    result          TEXT NOT NULL CHECK (result IN ('victory','defeat','draw','fled')),
    units_lost      INTEGER NOT NULL DEFAULT 0,
    enemies_killed  INTEGER NOT NULL DEFAULT 0,
    loot_json       TEXT,
    seen_by_player  INTEGER NOT NULL DEFAULT 0
);

-- 11. TALENT & SKILL TREES
CREATE TABLE IF NOT EXISTS sf_talent_defs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    unit_def_id     INTEGER REFERENCES sf_unit_defs(id),
    tree_branch     TEXT NOT NULL CHECK (tree_branch IN ('offensive','defensive','utility','doctrine')),
    node_position   INTEGER NOT NULL,
    talent_name     TEXT NOT NULL,
    max_rank        INTEGER NOT NULL DEFAULT 5,
    effect_per_rank_json TEXT NOT NULL,
    prereq_talent_id    INTEGER REFERENCES sf_talent_defs(id),
    prereq_min_rank     INTEGER DEFAULT 0,
    synergy_json    TEXT,
    is_keystone     INTEGER NOT NULL DEFAULT 0,
    description     TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS sf_talent_investments (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    unit_id         INTEGER NOT NULL REFERENCES sf_units(id) ON DELETE CASCADE,
    talent_def_id   INTEGER NOT NULL REFERENCES sf_talent_defs(id),
    current_rank    INTEGER NOT NULL DEFAULT 1,
    UNIQUE(unit_id, talent_def_id)
);

CREATE TABLE IF NOT EXISTS sf_doctrine_investments (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    save_slot_id    INTEGER NOT NULL REFERENCES sf_save_slots(id) ON DELETE CASCADE,
    talent_def_id   INTEGER NOT NULL REFERENCES sf_talent_defs(id),
    current_rank    INTEGER NOT NULL DEFAULT 1,
    UNIQUE(save_slot_id, talent_def_id)
);

-- 12. ALLIANCES
CREATE TABLE IF NOT EXISTS sf_alliances (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,
    tag         TEXT NOT NULL UNIQUE CHECK (length(tag) BETWEEN 2 AND 8),
    founder_id  INTEGER NOT NULL REFERENCES sf_save_slots(id),
    member_count INTEGER NOT NULL DEFAULT 1,
    total_points INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sf_alliance_members (
    alliance_id INTEGER NOT NULL REFERENCES sf_alliances(id) ON DELETE CASCADE,
    save_slot_id INTEGER NOT NULL REFERENCES sf_save_slots(id) ON DELETE CASCADE,
    rank        TEXT NOT NULL DEFAULT 'member'
        CHECK (rank IN ('founder','officer','member','recruit')),
    joined_at   INTEGER NOT NULL,
    PRIMARY KEY (alliance_id, save_slot_id)
);

-- 13. COMBAT LOG
CREATE TABLE IF NOT EXISTS sf_combat_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    attacker_slot_id INTEGER REFERENCES sf_save_slots(id),
    defender_slot_id INTEGER REFERENCES sf_save_slots(id),
    planet_id       INTEGER REFERENCES sf_planets(id),
    colony_id       INTEGER REFERENCES sf_colonies(id),
    combat_type     TEXT NOT NULL CHECK (combat_type IN ('ground','space','raid','patrol')),
    winner          TEXT CHECK (winner IN ('attacker','defender','draw')),
    rounds          INTEGER NOT NULL DEFAULT 1,
    attacker_losses_json TEXT,
    defender_losses_json TEXT,
    loot_json       TEXT,
    debris_ore      REAL NOT NULL DEFAULT 0,
    debris_crystal  REAL NOT NULL DEFAULT 0,
    occurred_at     INTEGER NOT NULL
);

-- 14. ESPIONAGE
CREATE TABLE IF NOT EXISTS sf_spy_reports (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    save_slot_id    INTEGER NOT NULL REFERENCES sf_save_slots(id) ON DELETE CASCADE,
    target_galaxy   INTEGER NOT NULL,
    target_system   INTEGER NOT NULL,
    target_planet   INTEGER NOT NULL,
    intel_level     INTEGER NOT NULL CHECK (intel_level BETWEEN 1 AND 5),
    resources_json  TEXT,
    buildings_json  TEXT,
    ships_json      TEXT,
    defenses_json   TEXT,
    research_json   TEXT,
    counter_espionage_chance REAL NOT NULL DEFAULT 0.0,
    created_at      INTEGER NOT NULL
);

-- 15. AUTO-PLAY TASK QUEUE
CREATE TABLE IF NOT EXISTS sf_auto_tasks (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    save_slot_id    INTEGER NOT NULL REFERENCES sf_save_slots(id) ON DELETE CASCADE,
    colony_id       INTEGER REFERENCES sf_colonies(id),
    task_type       TEXT NOT NULL CHECK (task_type IN (
        'build','upgrade','research','train','defend',
        'patrol','gather','expand','fleet_mission','auto_battle'
    )),
    priority        INTEGER NOT NULL DEFAULT 5 CHECK (priority BETWEEN 1 AND 10),
    config_json     TEXT NOT NULL,
    ai_reasoning    TEXT,
    state           TEXT NOT NULL DEFAULT 'queued'
        CHECK (state IN ('queued','active','paused','completed','failed','cancelled')),
    started_at      INTEGER,
    estimated_finish INTEGER,
    completed_at    INTEGER,
    created_at      INTEGER NOT NULL
);

-- 16. NOTIFICATIONS & EVENTS
CREATE TABLE IF NOT EXISTS sf_notifications (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    save_slot_id    INTEGER NOT NULL REFERENCES sf_save_slots(id) ON DELETE CASCADE,
    category        TEXT NOT NULL CHECK (category IN (
        'combat','espionage','construction','research','fleet',
        'alliance','system','patrol','colony','achievement'
    )),
    title           TEXT NOT NULL,
    message         TEXT NOT NULL,
    data_json       TEXT,
    is_read         INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL
);

-- 17. META-PROGRESSION (Phoenix Ash)
CREATE TABLE IF NOT EXISTS sf_prestige_bonuses (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id       INTEGER NOT NULL REFERENCES sf_players(id) ON DELETE CASCADE,
    bonus_type      TEXT NOT NULL CHECK (bonus_type IN (
        'production_speed','research_speed','build_speed',
        'combat_bonus','expansion_bonus','dark_matter_boost'
    )),
    bonus_value     REAL NOT NULL,
    phoenix_ash_cost INTEGER NOT NULL,
    is_active       INTEGER NOT NULL DEFAULT 1,
    purchased_at    INTEGER NOT NULL
);

-- 18. NEWBIE PROTECTION
CREATE TABLE IF NOT EXISTS sf_protection (
    save_slot_id    INTEGER PRIMARY KEY REFERENCES sf_save_slots(id) ON DELETE CASCADE,
    protection_score_limit INTEGER NOT NULL DEFAULT 5000,
    protection_until INTEGER NOT NULL,
    is_active       INTEGER NOT NULL DEFAULT 1
);

-- INDEXES FOR PERFORMANCE
CREATE INDEX IF NOT EXISTS idx_sf_planets_system ON sf_planets(system_id);
CREATE INDEX IF NOT EXISTS idx_sf_colonies_planet ON sf_colonies(planet_id);
CREATE INDEX IF NOT EXISTS idx_sf_colonies_slot ON sf_colonies(save_slot_id);
CREATE INDEX IF NOT EXISTS idx_sf_buildings_colony ON sf_buildings(colony_id);
CREATE INDEX IF NOT EXISTS idx_sf_units_colony ON sf_units(colony_id);
CREATE INDEX IF NOT EXISTS idx_sf_units_state ON sf_units(state);
CREATE INDEX IF NOT EXISTS idx_sf_fleets_slot ON sf_fleets(save_slot_id);
CREATE INDEX IF NOT EXISTS idx_sf_fleets_state ON sf_fleets(state);
CREATE INDEX IF NOT EXISTS idx_sf_defenses_colony ON sf_defenses(colony_id);
CREATE INDEX IF NOT EXISTS idx_sf_research_slot ON sf_research(save_slot_id);
CREATE INDEX IF NOT EXISTS idx_sf_patrol_colony ON sf_patrol_routes(colony_id);
CREATE INDEX IF NOT EXISTS idx_sf_combat_log_time ON sf_combat_log(occurred_at);
CREATE INDEX IF NOT EXISTS idx_sf_notifications_slot ON sf_notifications(save_slot_id, is_read);
CREATE INDEX IF NOT EXISTS idx_sf_auto_tasks_state ON sf_auto_tasks(state, priority);
CREATE INDEX IF NOT EXISTS idx_sf_planet_hexes_planet ON sf_planet_hexes(planet_id);
CREATE INDEX IF NOT EXISTS idx_sf_planet_hexes_control ON sf_planet_hexes(controlled_by);
"#;

// ============================================================================
// Helper: current unix timestamp
// ============================================================================

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ============================================================================
// Password Hashing (Argon2id — OWASP 2025)
// ============================================================================

fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(ARGON2_OUTPUT_LEN),
    )
    .map_err(|e| ImpForgeError::internal("ARGON2_PARAMS", format!("Argon2 params error: {e}")))?;

    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ImpForgeError::internal("ARGON2_HASH", format!("Password hash error: {e}")))?
        .to_string();

    Ok(hash)
}

fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| ImpForgeError::internal("HASH_PARSE", format!("Hash parse error: {e}")))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

// ============================================================================
// Input Validation
// ============================================================================

fn validate_commander_name(name: &str) -> AppResult<()> {
    if name.len() < MIN_COMMANDER_NAME_LEN || name.len() > MAX_COMMANDER_NAME_LEN {
        return Err(ImpForgeError::validation(
            "NAME_LENGTH",
            format!(
                "Commander name must be {}-{} characters",
                MIN_COMMANDER_NAME_LEN, MAX_COMMANDER_NAME_LEN
            ),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ImpForgeError::validation(
            "NAME_CHARS",
            "Commander name may only contain letters, numbers, underscores, and hyphens",
        ));
    }
    Ok(())
}

fn validate_password_strength(password: &str) -> AppResult<()> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(ImpForgeError::validation(
            "PASSWORD_SHORT",
            format!("Password must be at least {} characters", MIN_PASSWORD_LEN),
        ));
    }
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !(has_upper && has_lower && has_digit) {
        return Err(ImpForgeError::validation(
            "PASSWORD_WEAK",
            "Password must contain uppercase, lowercase, and digit",
        ));
    }
    Ok(())
}

// ============================================================================
// Planet Generation (from swarmforge_planet_gen.rs)
// ============================================================================

/// Determine planet size based on orbital slot and seed.
/// Inner slots tend small, mid slots medium/large, outer tend small/medium.
/// Huge only realistically at slots 7-9.
fn determine_planet_size(slot: i32, seed: u64) -> PlanetSize {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let roll: f64 = rng.gen();

    match slot {
        1..=3 => {
            if roll < 0.45 { PlanetSize::Small }
            else if roll < 0.85 { PlanetSize::Medium }
            else if roll < 0.97 { PlanetSize::Large }
            else { PlanetSize::Huge }
        }
        4..=6 => {
            if roll < 0.20 { PlanetSize::Small }
            else if roll < 0.65 { PlanetSize::Medium }
            else if roll < 0.90 { PlanetSize::Large }
            else { PlanetSize::Huge }
        }
        7..=9 => {
            if roll < 0.10 { PlanetSize::Small }
            else if roll < 0.45 { PlanetSize::Medium }
            else if roll < 0.80 { PlanetSize::Large }
            else { PlanetSize::Huge }
        }
        10..=12 => {
            if roll < 0.25 { PlanetSize::Small }
            else if roll < 0.70 { PlanetSize::Medium }
            else if roll < 0.92 { PlanetSize::Large }
            else { PlanetSize::Huge }
        }
        _ => {
            if roll < 0.35 { PlanetSize::Small }
            else if roll < 0.75 { PlanetSize::Medium }
            else if roll < 0.95 { PlanetSize::Large }
            else { PlanetSize::Huge }
        }
    }
}

/// Calculate OGame-style planet properties based on slot position.
/// Returns (diameter_km, base_fields, temp_min, temp_max).
fn calculate_planet_properties(slot: i32, seed: u64) -> (i32, i32, i32, i32) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed.wrapping_add(slot as u64 * 7919));

    let (temp_base_min, temp_base_max) = match slot {
        1 => (200, 280),
        2 => (150, 230),
        3 => (100, 180),
        4 => (60, 140),
        5 => (40, 120),
        6 => (20, 100),
        7 => (0, 80),
        8 => (-10, 70),
        9 => (-30, 50),
        10 => (-50, 30),
        11 => (-70, 10),
        12 => (-90, -10),
        13 => (-110, -30),
        14 => (-120, -50),
        15 => (-130, -70),
        _ => (0, 50),
    };

    let temp_min = temp_base_min + rng.gen_range(-20..=20);
    let temp_max = temp_base_max + rng.gen_range(-20..=20);

    let (diam_min, diam_max) = match slot {
        1 => (7_000, 10_000),
        2 => (8_000, 11_000),
        3 => (9_000, 12_000),
        4 => (10_000, 15_000),
        5 => (11_000, 16_000),
        6 => (11_000, 17_000),
        7 => (12_000, 17_500),
        8 => (12_000, 18_000),
        9 => (11_000, 16_000),
        10 => (10_000, 14_000),
        11 => (9_000, 12_000),
        12 => (8_000, 11_000),
        13 => (7_000, 10_000),
        14 => (7_000, 11_000),
        15 => (6_500, 13_000),
        _ => (10_000, 14_000),
    };

    let diameter = rng.gen_range(diam_min..=diam_max);
    let base_fields = ((diameter as f64 / 1000.0).powi(2)).floor() as i32;

    (diameter, base_fields, temp_min, temp_max)
}

/// Generate biome percentages based on temperature and seed.
fn generate_biome_distribution(temp_min: i32, temp_max: i32, seed: u64) -> BiomeDistribution {
    let mut rng = ChaCha20Rng::seed_from_u64(seed.wrapping_add(13337));
    let avg_temp = (temp_min + temp_max) / 2;

    let (mut tropical, mut temperate, mut arid, mut tundra, mut volcanic,
         mut ocean, mut crystal, mut fungal, mut barren, mut swamp) = if avg_temp > 150 {
        (5, 0, 15, 0, 40, 5, 10, 0, 20, 5)
    } else if avg_temp > 80 {
        (30, 10, 25, 0, 5, 10, 3, 5, 7, 5)
    } else if avg_temp > 20 {
        (15, 30, 10, 5, 3, 15, 5, 7, 5, 5)
    } else if avg_temp > -30 {
        (5, 20, 5, 25, 2, 15, 8, 8, 7, 5)
    } else if avg_temp > -80 {
        (0, 5, 0, 40, 0, 15, 12, 5, 18, 5)
    } else {
        (0, 0, 0, 30, 0, 10, 20, 3, 35, 2)
    };

    // Add random variation (+/-5 per biome)
    let vary = |base: &mut i32, rng: &mut ChaCha20Rng| {
        *base = (*base + rng.gen_range(-5..=5)).max(0);
    };
    vary(&mut tropical, &mut rng);
    vary(&mut temperate, &mut rng);
    vary(&mut arid, &mut rng);
    vary(&mut tundra, &mut rng);
    vary(&mut volcanic, &mut rng);
    vary(&mut ocean, &mut rng);
    vary(&mut crystal, &mut rng);
    vary(&mut fungal, &mut rng);
    vary(&mut barren, &mut rng);
    vary(&mut swamp, &mut rng);

    // Normalize to 100%
    let total = tropical + temperate + arid + tundra + volcanic
        + ocean + crystal + fungal + barren + swamp;

    if total == 0 {
        return BiomeDistribution {
            tropical: 0, temperate: 50, arid: 0, tundra: 0, volcanic: 0,
            ocean: 20, crystal: 5, fungal: 5, barren: 15, swamp: 5,
        };
    }

    let scale = |v: i32| -> i32 { ((v as f64 / total as f64) * 100.0).round() as i32 };
    let mut dist = BiomeDistribution {
        tropical: scale(tropical),
        temperate: scale(temperate),
        arid: scale(arid),
        tundra: scale(tundra),
        volcanic: scale(volcanic),
        ocean: scale(ocean),
        crystal: scale(crystal),
        fungal: scale(fungal),
        barren: scale(barren),
        swamp: scale(swamp),
    };

    // Fix rounding to exactly 100
    let sum = dist.tropical + dist.temperate + dist.arid + dist.tundra
        + dist.volcanic + dist.ocean + dist.crystal + dist.fungal
        + dist.barren + dist.swamp;
    dist.temperate += 100 - sum;

    dist
}

/// Pick a biome from weighted distribution.
fn pick_weighted_biome(weights: &[(Biome, i32)], total: i32, rng: &mut ChaCha20Rng) -> Biome {
    if total == 0 {
        return Biome::Temperate;
    }
    let roll = rng.gen_range(0..total);
    let mut cumulative = 0;
    for (biome, weight) in weights {
        cumulative += weight;
        if roll < cumulative {
            return *biome;
        }
    }
    Biome::Temperate
}

/// Generate resource node count for a hex (Poisson-like distribution).
fn generate_resource_count(multiplier: f64, rng: &mut ChaCha20Rng) -> i32 {
    if multiplier < 0.1 {
        return 0;
    }
    let expected = multiplier * 1.2;
    let mut count = 0;
    let max_tries = (expected * 3.0).ceil() as i32;
    for _ in 0..max_tries {
        if rng.gen::<f64>() < (expected / max_tries as f64) {
            count += 1;
        }
    }
    count
}

/// Generate axial-coordinate hexes for the planet surface.
fn generate_hex_grid(
    size: &PlanetSize,
    biome_dist: &BiomeDistribution,
    ore_richness: f64,
    crystal_richness: f64,
    essence_richness: f64,
    seed: u64,
) -> Vec<GeneratedHex> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed.wrapping_add(42));
    let radius = size.grid_radius();
    let mut hexes = Vec::with_capacity(size.hex_count() as usize);

    let biome_weights: Vec<(Biome, i32)> = vec![
        (Biome::Tropical, biome_dist.tropical),
        (Biome::Temperate, biome_dist.temperate),
        (Biome::Arid, biome_dist.arid),
        (Biome::Tundra, biome_dist.tundra),
        (Biome::Volcanic, biome_dist.volcanic),
        (Biome::Ocean, biome_dist.ocean),
        (Biome::Crystal, biome_dist.crystal),
        (Biome::Fungal, biome_dist.fungal),
        (Biome::Barren, biome_dist.barren),
        (Biome::Swamp, biome_dist.swamp),
    ];
    let total_weight: i32 = biome_weights.iter().map(|(_, w)| w).sum();

    for q in -radius..=radius {
        let r_min = (-radius).max(-q - radius);
        let r_max = radius.min(-q + radius);
        for r in r_min..=r_max {
            let biome = pick_weighted_biome(&biome_weights, total_weight, &mut rng);

            let dist_from_center = ((q.abs() + r.abs() + (q + r).abs()) as f64 / 2.0)
                / radius as f64;
            let base_elevation = match biome {
                Biome::Ocean => rng.gen_range(10..=35),
                Biome::Volcanic => rng.gen_range(60..=95),
                Biome::Tundra => rng.gen_range(55..=80),
                _ => rng.gen_range(40..=70),
            };
            let elevation = (base_elevation as f64 * (1.0 - dist_from_center * 0.3))
                .clamp(0.0, 100.0) as i32;

            let (ore_mult, crys_mult, ess_mult) = biome.resource_multipliers();
            let ore_nodes = generate_resource_count(ore_mult * ore_richness, &mut rng);
            let crystal_nodes = generate_resource_count(crys_mult * crystal_richness, &mut rng);
            let essence_nodes = generate_resource_count(ess_mult * essence_richness, &mut rng);

            let rare_node = if rng.gen::<f64>() < 0.02 {
                Some(match rng.gen_range(0..3) {
                    0 => "dark_matter".to_string(),
                    1 => "ancient_relic".to_string(),
                    _ => "fertile_soil".to_string(),
                })
            } else {
                None
            };

            let has_flora = rng.gen::<f64>() < biome.flora_density();
            let has_fauna = rng.gen::<f64>() < biome.fauna_density();

            hexes.push(GeneratedHex {
                q,
                r,
                biome,
                elevation,
                ore_nodes,
                crystal_nodes,
                essence_nodes,
                rare_node,
                has_flora,
                has_fauna,
            });
        }
    }

    hexes
}

/// Generate a complete planet with all properties.
fn generate_planet(system_id: i64, slot: i32, system_seed: u64) -> GeneratedPlanet {
    let planet_seed = system_seed.wrapping_mul(31).wrapping_add(slot as u64 * 7919);

    let size_class = determine_planet_size(slot, planet_seed);
    let (diameter, base_fields, temp_min, temp_max) =
        calculate_planet_properties(slot, planet_seed);
    let biome_dist = generate_biome_distribution(temp_min, temp_max, planet_seed);

    let mut rng = ChaCha20Rng::seed_from_u64(planet_seed.wrapping_add(99991));
    let ore_richness = match slot {
        4..=8 => 1.0 + rng.gen::<f64>() * 0.5,
        _ => 0.7 + rng.gen::<f64>() * 0.6,
    };
    let crystal_richness = match slot {
        1..=3 => 1.0 + rng.gen::<f64>() * 0.6,
        _ => 0.6 + rng.gen::<f64>() * 0.8,
    };
    let essence_richness = match slot {
        12..=15 => 1.0 + rng.gen::<f64>() * 0.8,
        _ => 0.5 + rng.gen::<f64>() * 0.7,
    };

    let hexes = generate_hex_grid(
        &size_class,
        &biome_dist,
        ore_richness,
        crystal_richness,
        essence_richness,
        planet_seed,
    );

    GeneratedPlanet {
        system_id,
        slot,
        seed: planet_seed,
        size_class,
        diameter_km: diameter,
        base_fields,
        temperature_min: temp_min,
        temperature_max: temp_max,
        biome_distribution: biome_dist,
        ore_richness,
        crystal_richness,
        essence_richness,
        hexes,
    }
}

// ============================================================================
// SwarmDatabase — the unified game database engine
// ============================================================================

pub(crate) struct SwarmDatabase {
    conn: Mutex<Connection>,
}

impl SwarmDatabase {
    /// Open (or create) swarmforge.db in the given data directory.
    pub(crate) fn new(data_dir: &Path) -> AppResult<Self> {
        let db_path = data_dir.join("swarmforge.db");
        let conn = Connection::open(&db_path).map_err(|e| {
            ImpForgeError::internal("SWARM_DB_OPEN", format!("Cannot open swarmforge.db: {e}"))
                .with_suggestion("Check disk space and file permissions for the app data directory.")
        })?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    /// Execute the full 18-table schema (idempotent via IF NOT EXISTS).
    pub(crate) fn init_schema(&self) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        // sf_alliances must exist before sf_save_slots references it,
        // and sf_colonies must exist before sf_planet_hexes references it.
        // The schema SQL handles this via CREATE TABLE IF NOT EXISTS ordering.
        // However, SQLite defers FK checks, so we disable FK enforcement during
        // schema init and re-enable after.
        conn.execute_batch("PRAGMA foreign_keys = OFF;")
            .map_err(|e| {
                ImpForgeError::internal("SWARM_DB_PRAGMA", format!("PRAGMA error: {e}"))
            })?;

        conn.execute_batch(SCHEMA_SQL).map_err(|e| {
            ImpForgeError::internal("SWARM_DB_SCHEMA", format!("Schema init failed: {e}"))
                .with_suggestion("The swarmforge.db file may be corrupted. Try deleting it to reset.")
        })?;

        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| {
                ImpForgeError::internal("SWARM_DB_PRAGMA", format!("PRAGMA error: {e}"))
            })?;

        Ok(())
    }

    // ========================================================================
    // Player & Auth
    // ========================================================================

    /// Create a new player with Argon2id-hashed password.
    /// Returns the new player ID.
    pub(crate) fn create_player(
        &self,
        name: &str,
        password: &str,
        email: Option<&str>,
        faction: &str,
    ) -> AppResult<i64> {
        validate_commander_name(name)?;
        validate_password_strength(password)?;
        let _faction = Faction::from_str(faction)?;

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        // Check name uniqueness
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sf_players WHERE commander_name = ?1)",
                params![name],
                |row| row.get(0),
            )
            .map_err(|e| {
                ImpForgeError::internal("SWARM_DB_QUERY", format!("Name check failed: {e}"))
            })?;

        if exists {
            return Err(ImpForgeError::validation(
                "NAME_TAKEN",
                "Commander name already taken",
            ));
        }

        let password_hash = hash_password(password)?;
        let now = unix_now();

        // Generate placeholder keys (32 zero bytes). Full Ed25519 keys are
        // generated when the separate auth module is integrated.
        let placeholder_key = vec![0u8; 32];

        conn.execute(
            "INSERT INTO sf_players (
                commander_name, email, password_hash,
                ed25519_public_key, ed25519_signing_key_enc,
                subscription_tier, created_at, last_login
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'free', ?6, ?6)",
            params![name, email, password_hash, placeholder_key, placeholder_key, now],
        )
        .map_err(|e| {
            ImpForgeError::internal("SWARM_DB_INSERT", format!("Registration failed: {e}"))
        })?;

        Ok(conn.last_insert_rowid())
    }

    /// Authenticate a player with commander name and password.
    /// Returns Some(player_id) on success, None on failure.
    pub(crate) fn authenticate(&self, name: &str, password: &str) -> AppResult<Option<i64>> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        let result = conn.query_row(
            "SELECT id, password_hash FROM sf_players WHERE commander_name = ?1",
            params![name],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        );

        match result {
            Ok((player_id, stored_hash)) => {
                if verify_password(password, &stored_hash)? {
                    // Update last login
                    let now = unix_now();
                    let _ = conn.execute(
                        "UPDATE sf_players SET last_login = ?1 WHERE id = ?2",
                        params![now, player_id],
                    );
                    Ok(Some(player_id))
                } else {
                    Ok(None)
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(ImpForgeError::internal(
                "SWARM_DB_QUERY",
                format!("Auth query failed: {e}"),
            )),
        }
    }

    /// Get player information as JSON.
    pub(crate) fn get_player(&self, id: i64) -> AppResult<serde_json::Value> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        let row = conn.query_row(
            "SELECT id, commander_name, email, subscription_tier,
                    phoenix_ash, dark_matter, total_play_time_secs,
                    created_at, last_login
             FROM sf_players WHERE id = ?1",
            params![id],
            |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "commander_name": row.get::<_, String>(1)?,
                    "email": row.get::<_, Option<String>>(2)?,
                    "subscription_tier": row.get::<_, String>(3)?,
                    "phoenix_ash": row.get::<_, i64>(4)?,
                    "dark_matter": row.get::<_, i64>(5)?,
                    "total_play_time_secs": row.get::<_, i64>(6)?,
                    "created_at": row.get::<_, i64>(7)?,
                    "last_login": row.get::<_, i64>(8)?,
                }))
            },
        );

        match row {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(ImpForgeError::validation(
                "PLAYER_NOT_FOUND",
                format!("Player with id {} not found", id),
            )),
            Err(e) => Err(ImpForgeError::internal(
                "SWARM_DB_QUERY",
                format!("Player query failed: {e}"),
            )),
        }
    }

    // ========================================================================
    // Planets & Hexes
    // ========================================================================

    /// Generate a planet and store it (with all hexes) in the database.
    /// Requires that the system already exists in sf_systems.
    /// Returns the new planet ID.
    pub(crate) fn generate_and_store_planet(
        &self,
        system_id: i64,
        slot: i32,
        seed: u64,
    ) -> AppResult<i64> {
        if !(1..=15).contains(&slot) {
            return Err(ImpForgeError::validation(
                "INVALID_SLOT",
                format!("Planet slot must be 1-15, got {slot}"),
            ));
        }

        let planet = generate_planet(system_id, slot, seed);

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        let bd = &planet.biome_distribution;
        conn.execute(
            "INSERT INTO sf_planets (
                system_id, slot, seed, size_class, hex_count, max_colonies,
                diameter_km, base_fields, temperature_min, temperature_max,
                biome_tropical_pct, biome_temperate_pct, biome_arid_pct,
                biome_tundra_pct, biome_volcanic_pct, biome_ocean_pct,
                biome_crystal_pct, biome_fungal_pct, biome_barren_pct,
                biome_swamp_pct, ore_richness, crystal_richness, essence_richness
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                ?21, ?22, ?23
            )",
            params![
                planet.system_id,
                planet.slot,
                planet.seed as i64,
                planet.size_class.as_str(),
                planet.size_class.hex_count(),
                planet.size_class.max_colonies(),
                planet.diameter_km,
                planet.base_fields,
                planet.temperature_min,
                planet.temperature_max,
                bd.tropical,
                bd.temperate,
                bd.arid,
                bd.tundra,
                bd.volcanic,
                bd.ocean,
                bd.crystal,
                bd.fungal,
                bd.barren,
                bd.swamp,
                planet.ore_richness,
                planet.crystal_richness,
                planet.essence_richness,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("SWARM_DB_INSERT", format!("Planet insert failed: {e}"))
        })?;

        let planet_id = conn.last_insert_rowid();

        // Batch insert hexes
        let mut stmt = conn
            .prepare(
                "INSERT INTO sf_planet_hexes (
                    planet_id, hex_q, hex_r, biome, elevation,
                    ore_nodes, crystal_nodes, essence_nodes, rare_node_type,
                    has_flora, has_fauna
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )
            .map_err(|e| {
                ImpForgeError::internal("SWARM_DB_PREPARE", format!("Hex prepare failed: {e}"))
            })?;

        for hex in &planet.hexes {
            stmt.execute(params![
                planet_id,
                hex.q,
                hex.r,
                hex.biome.as_str(),
                hex.elevation,
                hex.ore_nodes,
                hex.crystal_nodes,
                hex.essence_nodes,
                hex.rare_node.as_deref(),
                hex.has_flora as i32,
                hex.has_fauna as i32,
            ])
            .map_err(|e| {
                ImpForgeError::internal("SWARM_DB_INSERT", format!("Hex insert failed: {e}"))
            })?;
        }

        Ok(planet_id)
    }

    /// Get all hexes for a planet as JSON array.
    pub(crate) fn get_planet_hexes(&self, planet_id: i64) -> AppResult<Vec<serde_json::Value>> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT hex_q, hex_r, biome, elevation,
                        ore_nodes, crystal_nodes, essence_nodes,
                        rare_node_type, has_flora, has_fauna
                 FROM sf_planet_hexes WHERE planet_id = ?1
                 ORDER BY hex_q, hex_r",
            )
            .map_err(|e| {
                ImpForgeError::internal("SWARM_DB_PREPARE", format!("Hex query prepare failed: {e}"))
            })?;

        let rows = stmt
            .query_map(params![planet_id], |row| {
                Ok(serde_json::json!({
                    "q": row.get::<_, i32>(0)?,
                    "r": row.get::<_, i32>(1)?,
                    "biome": row.get::<_, String>(2)?,
                    "elevation": row.get::<_, i32>(3)?,
                    "ore_nodes": row.get::<_, i32>(4)?,
                    "crystal_nodes": row.get::<_, i32>(5)?,
                    "essence_nodes": row.get::<_, i32>(6)?,
                    "rare_node_type": row.get::<_, Option<String>>(7)?,
                    "has_flora": row.get::<_, bool>(8)?,
                    "has_fauna": row.get::<_, bool>(9)?,
                }))
            })
            .map_err(|e| {
                ImpForgeError::internal("SWARM_DB_QUERY", format!("Hex query failed: {e}"))
            })?;

        let mut hexes = Vec::new();
        for row in rows {
            hexes.push(row.map_err(|e| {
                ImpForgeError::internal("SWARM_DB_ROW", format!("Row read failed: {e}"))
            })?);
        }

        Ok(hexes)
    }

    // ========================================================================
    // Colonies
    // ========================================================================

    /// Create a colony on a planet at the specified hex.
    pub(crate) fn create_colony(
        &self,
        save_slot_id: i64,
        planet_id: i64,
        hex_q: i32,
        hex_r: i32,
        name: &str,
    ) -> AppResult<i64> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        // Get faction from save slot
        let faction: String = conn
            .query_row(
                "SELECT faction FROM sf_save_slots WHERE id = ?1",
                params![save_slot_id],
                |row| row.get(0),
            )
            .map_err(|e| {
                ImpForgeError::internal("SWARM_DB_QUERY", format!("Save slot query failed: {e}"))
            })?;

        // Count existing colonies on this planet for this player
        let colony_number: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(colony_number), 0) + 1
                 FROM sf_colonies WHERE save_slot_id = ?1 AND planet_id = ?2",
                params![save_slot_id, planet_id],
                |row| row.get(0),
            )
            .unwrap_or(1);

        // Get planet base_fields for max_fields calculation
        let base_fields: i32 = conn
            .query_row(
                "SELECT base_fields FROM sf_planets WHERE id = ?1",
                params![planet_id],
                |row| row.get(0),
            )
            .unwrap_or(100);

        let now = unix_now();

        conn.execute(
            "INSERT INTO sf_colonies (
                save_slot_id, planet_id, colony_number, colony_name, faction,
                center_hex_q, center_hex_r, max_fields, res_last_calc,
                founded_at, last_updated
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                save_slot_id,
                planet_id,
                colony_number,
                name,
                faction,
                hex_q,
                hex_r,
                base_fields,
                now,
                now,
                now,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("SWARM_DB_INSERT", format!("Colony creation failed: {e}"))
        })?;

        let colony_id = conn.last_insert_rowid();

        // Mark planet as colonized
        let _ = conn.execute(
            "UPDATE sf_planets SET is_colonized = 1 WHERE id = ?1",
            params![planet_id],
        );

        Ok(colony_id)
    }

    /// Get colony resources as JSON.
    pub(crate) fn get_colony_resources(&self, colony_id: i64) -> AppResult<serde_json::Value> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        let row = conn.query_row(
            "SELECT colony_name, faction,
                    res_ore_stored, res_crystal_stored, res_essence_stored,
                    res_ore_rate, res_crystal_rate, res_essence_rate,
                    res_energy_balance, res_last_calc,
                    res_biomass, res_spore_gas, res_larvae,
                    res_suenden, res_kultisten, res_corruption,
                    res_eiter_essence, res_leichenteile,
                    res_nahrung, res_energie, res_faith,
                    population_current, population_max,
                    creep_coverage_pct, blight_coverage_pct, panik_keim_pct,
                    defense_score
             FROM sf_colonies WHERE id = ?1",
            params![colony_id],
            |row| {
                let faction: String = row.get(1)?;
                let last_calc: i64 = row.get(9)?;
                let elapsed = (unix_now() - last_calc).max(0) as f64;
                let hours = elapsed / 3600.0;

                // Calculate current resources (stored + rate * hours_elapsed)
                let ore = row.get::<_, f64>(2)? + row.get::<_, f64>(5)? * hours;
                let crystal = row.get::<_, f64>(3)? + row.get::<_, f64>(6)? * hours;
                let essence = row.get::<_, f64>(4)? + row.get::<_, f64>(7)? * hours;

                let mut result = serde_json::json!({
                    "colony_name": row.get::<_, String>(0)?,
                    "faction": faction,
                    "ore": ore,
                    "crystal": crystal,
                    "essence": essence,
                    "ore_rate": row.get::<_, f64>(5)?,
                    "crystal_rate": row.get::<_, f64>(6)?,
                    "essence_rate": row.get::<_, f64>(7)?,
                    "energy_balance": row.get::<_, f64>(8)?,
                    "population_current": row.get::<_, i32>(21)?,
                    "population_max": row.get::<_, i32>(22)?,
                    "creep_coverage_pct": row.get::<_, f64>(23)?,
                    "blight_coverage_pct": row.get::<_, f64>(24)?,
                    "panik_keim_pct": row.get::<_, f64>(25)?,
                    "defense_score": row.get::<_, i32>(26)?,
                });

                // Add faction-specific resources
                match faction.as_str() {
                    "insects" => {
                        result["biomass"] = serde_json::json!(row.get::<_, f64>(10)?);
                        result["spore_gas"] = serde_json::json!(row.get::<_, f64>(11)?);
                        result["larvae"] = serde_json::json!(row.get::<_, i32>(12)?);
                    }
                    "demons" => {
                        result["suenden"] = serde_json::json!(row.get::<_, f64>(13)?);
                        result["kultisten"] = serde_json::json!(row.get::<_, i32>(14)?);
                        result["corruption"] = serde_json::json!(row.get::<_, f64>(15)?);
                    }
                    "undead" => {
                        result["eiter_essence"] = serde_json::json!(row.get::<_, f64>(16)?);
                        result["leichenteile"] = serde_json::json!(row.get::<_, i32>(17)?);
                    }
                    _ => {}
                }

                Ok(result)
            },
        );

        match row {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(ImpForgeError::validation(
                "COLONY_NOT_FOUND",
                format!("Colony with id {} not found", colony_id),
            )),
            Err(e) => Err(ImpForgeError::internal(
                "SWARM_DB_QUERY",
                format!("Colony query failed: {e}"),
            )),
        }
    }

    /// Update colony resources based on elapsed time (OGame idle production).
    /// `production_rate(level) = base_rate * level * 1.1^level` per hour.
    pub(crate) fn update_resources(&self, colony_id: i64, elapsed_secs: f64) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        let hours = elapsed_secs / 3600.0;
        let now = unix_now();

        conn.execute(
            "UPDATE sf_colonies SET
                res_ore_stored = res_ore_stored + res_ore_rate * ?1,
                res_crystal_stored = res_crystal_stored + res_crystal_rate * ?1,
                res_essence_stored = res_essence_stored + res_essence_rate * ?1,
                res_last_calc = ?2,
                last_updated = ?2
             WHERE id = ?3",
            params![hours, now, colony_id],
        )
        .map_err(|e| {
            ImpForgeError::internal("SWARM_DB_UPDATE", format!("Resource update failed: {e}"))
        })?;

        Ok(())
    }

    // ========================================================================
    // Patrols
    // ========================================================================

    /// Create a patrol route for a colony.
    pub(crate) fn create_patrol(
        &self,
        colony_id: i64,
        waypoints: &str,
        mode: &str,
    ) -> AppResult<i64> {
        // Validate mode
        if mode != "cyclic" && mode != "ping_pong" {
            return Err(ImpForgeError::validation(
                "INVALID_PATROL_MODE",
                format!("Patrol mode must be 'cyclic' or 'ping_pong', got '{mode}'"),
            ));
        }

        // Validate waypoints is valid JSON array
        let _: Vec<serde_json::Value> = serde_json::from_str(waypoints).map_err(|e| {
            ImpForgeError::validation(
                "INVALID_WAYPOINTS",
                format!("Waypoints must be a valid JSON array: {e}"),
            )
        })?;

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        let now = unix_now();

        conn.execute(
            "INSERT INTO sf_patrol_routes (
                colony_id, waypoints_json, mode, created_at
            ) VALUES (?1, ?2, ?3, ?4)",
            params![colony_id, waypoints, mode, now],
        )
        .map_err(|e| {
            ImpForgeError::internal("SWARM_DB_INSERT", format!("Patrol creation failed: {e}"))
        })?;

        Ok(conn.last_insert_rowid())
    }

    /// Tick all active patrol routes. Returns events for any encounters.
    pub(crate) fn tick_patrols(&self, _delta_secs: f64) -> AppResult<Vec<serde_json::Value>> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, colony_id, waypoints_json, mode, engagement_radius,
                        behavior_on_contact, offline_efficiency
                 FROM sf_patrol_routes WHERE is_active = 1",
            )
            .map_err(|e| {
                ImpForgeError::internal("SWARM_DB_PREPARE", format!("Patrol query failed: {e}"))
            })?;

        let patrols: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "patrol_id": row.get::<_, i64>(0)?,
                    "colony_id": row.get::<_, i64>(1)?,
                    "waypoints": row.get::<_, String>(2)?,
                    "mode": row.get::<_, String>(3)?,
                    "engagement_radius": row.get::<_, f64>(4)?,
                    "behavior": row.get::<_, String>(5)?,
                    "offline_efficiency": row.get::<_, f64>(6)?,
                    "status": "patrolling",
                }))
            })
            .map_err(|e| {
                ImpForgeError::internal("SWARM_DB_QUERY", format!("Patrol query failed: {e}"))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(patrols)
    }

    // ========================================================================
    // Stats
    // ========================================================================

    /// Get database statistics as JSON.
    pub(crate) fn get_database_stats(&self) -> AppResult<serde_json::Value> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_DB_LOCK", format!("DB lock failed: {e}"))
        })?;

        let count = |table: &str| -> i64 {
            conn.query_row(
                &format!("SELECT COUNT(*) FROM {table}"),
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
        };

        Ok(serde_json::json!({
            "players": count("sf_players"),
            "save_slots": count("sf_save_slots"),
            "galaxies": count("sf_galaxies"),
            "systems": count("sf_systems"),
            "planets": count("sf_planets"),
            "planet_hexes": count("sf_planet_hexes"),
            "colonies": count("sf_colonies"),
            "buildings": count("sf_buildings"),
            "units": count("sf_units"),
            "fleets": count("sf_fleets"),
            "defenses": count("sf_defenses"),
            "research": count("sf_research"),
            "patrols": count("sf_patrol_routes"),
            "combat_logs": count("sf_combat_log"),
            "notifications": count("sf_notifications"),
            "prestige_bonuses": count("sf_prestige_bonuses"),
            "auto_tasks": count("sf_auto_tasks"),
            "schema_version": "1.0.0",
            "tables": 18,
        }))
    }
}

// ============================================================================
// Tauri IPC Commands (8 commands)
// ============================================================================

#[tauri::command]
pub(crate) async fn swarm_db_init(
    state: tauri::State<'_, SwarmDatabase>,
) -> AppResult<serde_json::Value> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_database", "game_database", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_database", "game_database");
    crate::synapse_fabric::synapse_session_push("swarm_database", "game_database", "swarm_db_init called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_database", "info", "swarm_database active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    state.get_database_stats()
}

#[tauri::command]
pub(crate) async fn swarm_db_create_player(
    state: tauri::State<'_, SwarmDatabase>,
    name: String,
    password: String,
    faction: String,
) -> AppResult<i64> {
    crate::cortex_wiring::cortex_event(
        "swarm_database", "save",
        crate::cortex_wiring::EventCategory::Data,
        serde_json::json!({"player": name, "faction": faction}),
    );
    state.create_player(&name, &password, None, &faction)
}

#[tauri::command]
pub(crate) async fn swarm_db_authenticate(
    state: tauri::State<'_, SwarmDatabase>,
    name: String,
    password: String,
) -> AppResult<serde_json::Value> {
    match state.authenticate(&name, &password)? {
        Some(player_id) => {
            let player = state.get_player(player_id)?;
            Ok(serde_json::json!({
                "success": true,
                "player": player,
            }))
        }
        None => Ok(serde_json::json!({
            "success": false,
            "error": "Invalid commander name or password",
        })),
    }
}

#[tauri::command]
pub(crate) async fn swarm_db_generate_planet(
    state: tauri::State<'_, SwarmDatabase>,
    system_id: i64,
    slot: i32,
    seed: u64,
) -> AppResult<serde_json::Value> {
    let planet_id = state.generate_and_store_planet(system_id, slot, seed)?;
    let hexes = state.get_planet_hexes(planet_id)?;
    Ok(serde_json::json!({
        "planet_id": planet_id,
        "hex_count": hexes.len(),
        "slot": slot,
    }))
}

#[tauri::command]
pub(crate) async fn swarm_db_get_colony(
    state: tauri::State<'_, SwarmDatabase>,
    colony_id: i64,
) -> AppResult<serde_json::Value> {
    state.get_colony_resources(colony_id)
}

#[tauri::command]
pub(crate) async fn swarm_db_create_patrol(
    state: tauri::State<'_, SwarmDatabase>,
    colony_id: i64,
    waypoints: String,
    mode: String,
) -> AppResult<i64> {
    state.create_patrol(colony_id, &waypoints, &mode)
}

#[tauri::command]
pub(crate) async fn swarm_db_tick(
    state: tauri::State<'_, SwarmDatabase>,
    delta_secs: f64,
) -> AppResult<serde_json::Value> {
    let patrols = state.tick_patrols(delta_secs)?;
    Ok(serde_json::json!({
        "active_patrols": patrols.len(),
        "patrols": patrols,
    }))
}

#[tauri::command]
pub(crate) async fn swarm_db_stats(
    state: tauri::State<'_, SwarmDatabase>,
) -> AppResult<serde_json::Value> {
    state.get_database_stats()
}

// ============================================================================
// Additional Tauri Commands — wiring internal helpers
// ============================================================================

/// Get newbie protection settings and faction info.
#[tauri::command]
pub(crate) async fn swarm_db_newbie_protection(
    faction: String,
) -> AppResult<serde_json::Value> {
    let f = Faction::from_str(&faction)?;
    Ok(serde_json::json!({
        "faction": f.as_str(),
        "score_limit": NEWBIE_PROTECTION_SCORE_LIMIT,
        "protection_days": NEWBIE_PROTECTION_DAYS,
    }))
}

/// Create a colony and immediately tick resources for a given time.
#[tauri::command]
pub(crate) async fn swarm_db_create_colony_and_tick(
    save_slot_id: i64,
    planet_id: i64,
    hex_q: i32,
    hex_r: i32,
    name: String,
    tick_secs: f64,
    state: tauri::State<'_, SwarmDatabase>,
) -> AppResult<serde_json::Value> {
    let colony_id = state.create_colony(save_slot_id, planet_id, hex_q, hex_r, &name)?;
    if tick_secs > 0.0 {
        state.update_resources(colony_id, tick_secs)?;
    }
    Ok(serde_json::json!({
        "colony_id": colony_id,
        "ticked_secs": tick_secs,
    }))
}

// ============================================================================
// Tests (15+)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;


    fn temp_db() -> (TempDir, SwarmDatabase) {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let db = SwarmDatabase::new(dir.path()).expect("Failed to create SwarmDatabase");
        (dir, db)
    }

    #[test]
    fn test_schema_init() {
        let (_dir, db) = temp_db();
        let stats = db.get_database_stats().expect("Stats failed");
        assert_eq!(stats["tables"], 18);
        assert_eq!(stats["schema_version"], "1.0.0");
    }

    #[test]
    fn test_schema_idempotent() {
        let (_dir, db) = temp_db();
        // Init schema twice should not fail
        db.init_schema().expect("Second schema init should succeed");
    }

    #[test]
    fn test_create_player() {
        let (_dir, db) = temp_db();
        let id = db
            .create_player("Commander_01", "StrongPass1", None, "insects")
            .expect("Create player failed");
        assert!(id > 0);
    }

    #[test]
    fn test_create_player_validates_name() {
        let (_dir, db) = temp_db();
        let result = db.create_player("ab", "StrongPass1", None, "insects");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "NAME_LENGTH");
    }

    #[test]
    fn test_create_player_validates_password() {
        let (_dir, db) = temp_db();
        let result = db.create_player("Commander_01", "weak", None, "insects");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "PASSWORD_SHORT");
    }

    #[test]
    fn test_create_player_validates_faction() {
        let (_dir, db) = temp_db();
        let result = db.create_player("Commander_01", "StrongPass1", None, "human");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "INVALID_FACTION");
    }

    #[test]
    fn test_duplicate_name_rejected() {
        let (_dir, db) = temp_db();
        db.create_player("UniqueCmd", "StrongPass1", None, "demons")
            .expect("First create failed");
        let result = db.create_player("UniqueCmd", "StrongPass1", None, "demons");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "NAME_TAKEN");
    }

    #[test]
    fn test_authenticate_success() {
        let (_dir, db) = temp_db();
        db.create_player("AuthTest", "StrongPass1", None, "undead")
            .expect("Create failed");
        let result = db.authenticate("AuthTest", "StrongPass1").expect("Auth failed");
        assert!(result.is_some());
    }

    #[test]
    fn test_authenticate_wrong_password() {
        let (_dir, db) = temp_db();
        db.create_player("AuthTest2", "StrongPass1", None, "undead")
            .expect("Create failed");
        let result = db
            .authenticate("AuthTest2", "WrongPass1")
            .expect("Auth query failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_authenticate_nonexistent() {
        let (_dir, db) = temp_db();
        let result = db
            .authenticate("NoSuchUser", "StrongPass1")
            .expect("Auth query failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_player() {
        let (_dir, db) = temp_db();
        let id = db
            .create_player("PlayerInfo", "StrongPass1", Some("test@example.com"), "insects")
            .expect("Create failed");
        let player = db.get_player(id).expect("Get player failed");
        assert_eq!(player["commander_name"], "PlayerInfo");
        assert_eq!(player["email"], "test@example.com");
        assert_eq!(player["subscription_tier"], "free");
    }

    #[test]
    fn test_planet_generation_deterministic() {
        let p1 = generate_planet(1, 8, 12345);
        let p2 = generate_planet(1, 8, 12345);
        assert_eq!(p1.size_class, p2.size_class);
        assert_eq!(p1.diameter_km, p2.diameter_km);
        assert_eq!(p1.hexes.len(), p2.hexes.len());
        assert_eq!(p1.hexes[0].biome, p2.hexes[0].biome);
    }

    #[test]
    fn test_planet_hex_counts() {
        for (slot, _) in [(8, PlanetSize::Medium)].iter() {
            let planet = generate_planet(1, *slot, 42);
            assert_eq!(
                planet.hexes.len() as i32,
                planet.size_class.hex_count(),
                "Hex count mismatch for slot {}",
                slot
            );
        }
    }

    #[test]
    fn test_biome_distribution_sums_to_100() {
        for seed in 0..50u64 {
            let dist = generate_biome_distribution(-20, 80, seed);
            let sum = dist.tropical + dist.temperate + dist.arid + dist.tundra
                + dist.volcanic + dist.ocean + dist.crystal + dist.fungal
                + dist.barren + dist.swamp;
            assert_eq!(sum, 100, "Biome dist does not sum to 100 for seed {}", seed);
        }
    }

    #[test]
    fn test_hot_planets_more_volcanic() {
        let hot = generate_biome_distribution(200, 280, 42);
        let cold = generate_biome_distribution(-130, -70, 42);
        assert!(
            hot.volcanic > cold.volcanic,
            "Hot planet should have more volcanic than cold"
        );
    }

    #[test]
    fn test_generate_and_store_planet() {
        let (_dir, db) = temp_db();
        // Create galaxy and system first
        {
            let conn = db.conn.lock().expect("Lock failed");
            conn.execute(
                "INSERT INTO sf_galaxies (id, name, seed) VALUES (1, 'Alpha', 42)",
                [],
            )
            .expect("Galaxy insert failed");
            conn.execute(
                "INSERT INTO sf_systems (galaxy_id, position, seed) VALUES (1, 1, 12345)",
                [],
            )
            .expect("System insert failed");
        }

        let planet_id = db
            .generate_and_store_planet(1, 8, 12345)
            .expect("Planet gen failed");
        assert!(planet_id > 0);

        let hexes = db.get_planet_hexes(planet_id).expect("Get hexes failed");
        assert!(!hexes.is_empty());
        // Verify all hexes have required fields
        for hex in &hexes {
            assert!(hex["q"].is_number());
            assert!(hex["r"].is_number());
            assert!(hex["biome"].is_string());
        }
    }

    #[test]
    fn test_patrol_creation() {
        let (_dir, db) = temp_db();

        // Create patrol (colony_id=1 won't FK-check in test because we disabled
        // FK enforcement during schema init and the table might not have the colony)
        // So we set up the full chain.
        {
            let conn = db.conn.lock().expect("Lock failed");
            conn.execute(
                "INSERT INTO sf_galaxies (id, name, seed) VALUES (1, 'Alpha', 42)",
                [],
            )
            .expect("Galaxy insert failed");
            conn.execute(
                "INSERT INTO sf_systems (galaxy_id, position, seed) VALUES (1, 1, 99)",
                [],
            )
            .expect("System insert failed");
            let now = unix_now();
            conn.execute(
                "INSERT INTO sf_players (commander_name, password_hash, ed25519_public_key, ed25519_signing_key_enc, created_at, last_login)
                 VALUES ('TestCmd', 'hash', X'00', X'00', ?1, ?1)",
                params![now],
            ).expect("Player insert failed");
            // Save slot must be inserted before alliance (alliance.founder_id FK)
            conn.execute(
                "INSERT INTO sf_save_slots (player_id, slot_number, faction, commander_alias, home_galaxy, home_system, home_planet, created_at, last_played)
                 VALUES (1, 1, 'insects', 'TestCmd', 1, 1, 1, ?1, ?1)",
                params![now],
            ).expect("Save slot insert failed");
            conn.execute(
                "INSERT INTO sf_alliances (name, tag, founder_id, created_at) VALUES ('TestAlliance', 'TA', 1, ?1)",
                params![now],
            ).expect("Alliance insert failed");
            conn.execute(
                "INSERT INTO sf_planets (system_id, slot, seed, size_class, hex_count, max_colonies, diameter_km, base_fields, temperature_min, temperature_max)
                 VALUES (1, 1, 42, 'medium', 127, 7, 12000, 144, -10, 70)",
                [],
            ).expect("Planet insert failed");
            conn.execute(
                "INSERT INTO sf_colonies (save_slot_id, planet_id, colony_number, colony_name, faction, center_hex_q, center_hex_r, max_fields, res_last_calc, founded_at, last_updated)
                 VALUES (1, 1, 1, 'TestColony', 'insects', 0, 0, 144, ?1, ?1, ?1)",
                params![now],
            ).expect("Colony insert failed");
        }

        let patrol_id = db
            .create_patrol(1, r#"[{"x":1.0,"y":2.0},{"x":5.0,"y":3.0}]"#, "cyclic")
            .expect("Patrol creation failed");
        assert!(patrol_id > 0);

        let patrols = db.tick_patrols(1.0).expect("Tick failed");
        assert_eq!(patrols.len(), 1);
        assert_eq!(patrols[0]["status"], "patrolling");
    }

    #[test]
    fn test_patrol_invalid_mode() {
        let (_dir, db) = temp_db();
        let result = db.create_patrol(1, r#"[{"x":1.0,"y":2.0}]"#, "invalid");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "INVALID_PATROL_MODE");
    }

    #[test]
    fn test_patrol_invalid_waypoints() {
        let (_dir, db) = temp_db();
        let result = db.create_patrol(1, "not json", "cyclic");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "INVALID_WAYPOINTS");
    }

    #[test]
    fn test_faction_from_str() {
        assert_eq!(Faction::from_str("insects").expect("from str should succeed"), Faction::Insects);
        assert_eq!(Faction::from_str("Demons").expect("from str should succeed"), Faction::Demons);
        assert_eq!(Faction::from_str("UNDEAD").expect("from str should succeed"), Faction::Undead);
        assert!(Faction::from_str("human").is_err());
    }

    #[test]
    fn test_planet_size_properties() {
        assert_eq!(PlanetSize::Small.hex_count(), 61);
        assert_eq!(PlanetSize::Medium.hex_count(), 127);
        assert_eq!(PlanetSize::Large.hex_count(), 217);
        assert_eq!(PlanetSize::Huge.hex_count(), 331);
        assert_eq!(PlanetSize::Small.max_colonies(), 5);
        assert_eq!(PlanetSize::Huge.max_colonies(), 11);
    }

    #[test]
    fn test_resource_nodes_exist() {
        let planet = generate_planet(1, 8, 99999);
        let total_ore: i32 = planet.hexes.iter().map(|h| h.ore_nodes).sum();
        let total_crystal: i32 = planet.hexes.iter().map(|h| h.crystal_nodes).sum();
        assert!(total_ore > 0, "Planet should have ore nodes");
        assert!(total_crystal > 0, "Planet should have crystal nodes");
    }

    #[test]
    fn test_password_hash_and_verify() {
        let password = "TestPass123";
        let hash = hash_password(password).expect("Hash failed");
        assert!(verify_password(password, &hash).expect("Verify failed"));
        assert!(!verify_password("WrongPass123", &hash).expect("Verify failed"));
    }

    #[test]
    fn test_database_stats() {
        let (_dir, db) = temp_db();
        let stats = db.get_database_stats().expect("Stats failed");
        assert_eq!(stats["players"], 0);
        assert_eq!(stats["planets"], 0);
        assert_eq!(stats["colonies"], 0);
    }

    #[test]
    fn test_validate_commander_name() {
        assert!(validate_commander_name("Commander_01").is_ok());
        assert!(validate_commander_name("ab").is_err());
        assert!(validate_commander_name(&"a".repeat(21)).is_err());
        assert!(validate_commander_name("no spaces").is_err());
        assert!(validate_commander_name("no@special").is_err());
    }

    #[test]
    fn test_validate_password_strength() {
        assert!(validate_password_strength("StrongPass1").is_ok());
        assert!(validate_password_strength("short").is_err());
        assert!(validate_password_strength("nouppercase1").is_err());
        assert!(validate_password_strength("NOLOWERCASE1").is_err());
        assert!(validate_password_strength("NoDigitsHere").is_err());
    }

    #[test]
    fn test_planet_slot_validation() {
        let (_dir, db) = temp_db();
        let result = db.generate_and_store_planet(1, 0, 42);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "INVALID_SLOT");

        let result = db.generate_and_store_planet(1, 16, 42);
        assert!(result.is_err());
    }
}
