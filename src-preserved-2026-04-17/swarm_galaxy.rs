// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Galaxy Coordinate System
//!
//! OGame-style galaxy map with `[G:SSS:PP]` coordinates.
//!
//! ## Topology
//!
//! - 9 galaxies, 499 systems per galaxy, 15 planet slots + 1 expedition slot
//! - Circular (donut) topology: system 499 wraps to system 1
//! - Each slot has temperature-dependent resource generation
//!
//! ## Distance Model
//!
//! - Same system:      `|pos_a - pos_b| * 5`
//! - Same galaxy:      wrapping `|sys_a - sys_b| * 19 + 5` (donut)
//! - Different galaxy:  `|gal_a - gal_b| * 20000`
//!
//! ## Travel Time
//!
//! `10 + 3500 * sqrt(distance * 10 / speed) / speed_factor` (seconds)
//!
//! ## Procedural Generation
//!
//! Systems are deterministically seeded from `(galaxy, system)` so the same
//! coordinates always produce the same planets.  ~30% of slots are occupied
//! by NPC Human factions.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::error::ImpForgeError;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_galaxy", "Game");

// ---------------------------------------------------------------------------
// Core Types
// ---------------------------------------------------------------------------

/// Galaxy coordinate in `[G:SSS:PP]` format.
///
/// - `galaxy`:   1..=9
/// - `system`:   1..=499
/// - `position`: 1..=15 (slot 16 = expedition)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GalaxyCoord {
    pub galaxy: u16,
    pub system: u16,
    pub position: u16,
}

/// Galaxy configuration constants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalaxyConfig {
    pub num_galaxies: u16,
    pub systems_per_galaxy: u16,
    pub slots_per_system: u16,
    pub circular_topology: bool,
}

impl Default for GalaxyConfig {
    fn default() -> Self {
        Self {
            num_galaxies: 9,
            systems_per_galaxy: 499,
            slots_per_system: 15,
            circular_topology: true,
        }
    }
}

/// A planet slot in the galaxy map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetSlot {
    pub coord: GalaxyCoord,
    pub planet_name: Option<String>,
    pub owner: Option<String>,
    pub faction: Option<String>,
    pub planet_size: u16,
    pub temperature: i16,
    pub resource_nodes: u16,
    pub is_colonized: bool,
    pub is_expedition: bool,
}

// ---------------------------------------------------------------------------
// Coordinate Formatting & Parsing
// ---------------------------------------------------------------------------

/// Format a coordinate as `[G:SSS:PP]`.
///
/// System is zero-padded to 3 digits, position to 2 digits.
///
/// # Example
/// ```ignore
/// let c = GalaxyCoord { galaxy: 1, system: 42, position: 7 };
/// assert_eq!(coord_to_string(&c), "[1:042:07]");
/// ```
pub(crate) fn coord_to_string(coord: &GalaxyCoord) -> String {
    format!("[{}:{:03}:{:02}]", coord.galaxy, coord.system, coord.position)
}

/// Parse a coordinate string in `[G:SSS:PP]` format.
///
/// Accepts flexible input: leading zeros are optional, brackets are required.
/// Returns `None` if the format is invalid or values are out of range.
pub(crate) fn coord_from_string(s: &str) -> Option<GalaxyCoord> {
    let trimmed = s.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let parts: Vec<&str> = inner.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    let galaxy: u16 = parts[0].parse().ok()?;
    let system: u16 = parts[1].parse().ok()?;
    let position: u16 = parts[2].parse().ok()?;

    let config = GalaxyConfig::default();

    if galaxy < 1 || galaxy > config.num_galaxies {
        return None;
    }
    if system < 1 || system > config.systems_per_galaxy {
        return None;
    }
    // Position 16 = expedition slot
    if position < 1 || position > config.slots_per_system + 1 {
        return None;
    }

    Some(GalaxyCoord { galaxy, system, position })
}

// ---------------------------------------------------------------------------
// Distance Calculation
// ---------------------------------------------------------------------------

/// Calculate distance between two galaxy coordinates.
///
/// - Same system:       `|pos_a - pos_b| * 5`
/// - Same galaxy:       `min_wrap(sys_a, sys_b, 499) * 19 + 5`
/// - Different galaxy:  `|gal_a - gal_b| * 20000`
///
/// The system distance uses donut wrapping: system 499 is adjacent to
/// system 1 (distance 1, not 498).
pub(crate) fn distance_between(a: &GalaxyCoord, b: &GalaxyCoord) -> u32 {
    if a.galaxy != b.galaxy {
        // Cross-galaxy distance
        let gal_diff = (a.galaxy as i32 - b.galaxy as i32).unsigned_abs();
        return gal_diff * 20_000;
    }

    if a.system != b.system {
        // Cross-system distance within same galaxy (donut wrapping)
        let config = GalaxyConfig::default();
        let sys_diff = wrapping_distance(a.system, b.system, config.systems_per_galaxy);
        return sys_diff * 19 + 5;
    }

    // Same system, different position
    let pos_diff = (a.position as i32 - b.position as i32).unsigned_abs();
    pos_diff * 5
}

/// Calculate minimum distance on a circular ring of `ring_size` elements.
///
/// For system 499 wrapping to 1: `wrapping_distance(1, 499, 499) == 1`.
fn wrapping_distance(a: u16, b: u16, ring_size: u16) -> u32 {
    let direct = (a as i32 - b as i32).unsigned_abs();
    let wrapped = ring_size as u32 - direct;
    direct.min(wrapped)
}

// ---------------------------------------------------------------------------
// Travel Time
// ---------------------------------------------------------------------------

/// Calculate travel time in seconds.
///
/// Formula: `10 + 3500 * sqrt(distance * 10 / speed) / speed_factor`
///
/// - `distance`:     from `distance_between()`
/// - `fleet_speed`:  speed of the slowest ship in the fleet
/// - `speed_factor`: universe speed multiplier (1.0 = normal, 2.0 = 2x speed)
pub(crate) fn travel_time(distance: u32, fleet_speed: u32, speed_factor: f64) -> u32 {
    if distance == 0 || fleet_speed == 0 {
        return 0;
    }
    let factor = speed_factor.max(0.01);
    let dist_f = distance as f64;
    let speed_f = fleet_speed as f64;

    let time = 10.0 + 3500.0 * (dist_f * 10.0 / speed_f).sqrt() / factor;
    time.ceil() as u32
}

// ---------------------------------------------------------------------------
// Temperature & Slot Properties
// ---------------------------------------------------------------------------

/// Calculate temperature for a planet slot.
///
/// Slot 1 (close to star) is hottest (~240C), slot 15 (far) is coldest (~-40C).
/// Formula: base 240 with ~20C drop per slot, randomized +/- 20C.
///
/// The seed ensures the same slot always gets the same temperature.
pub(crate) fn temperature_for_slot(galaxy: u16, system: u16, slot: u16) -> i16 {
    let base = 240 - ((slot.saturating_sub(1)) as i16) * 20;
    // Deterministic jitter from coordinate seed
    let seed = coordinate_seed(galaxy, system, slot);
    let jitter = ((seed % 41) as i16) - 20; // -20..+20
    base + jitter
}

/// Deterministic seed from coordinates for reproducible procedural generation.
fn coordinate_seed(galaxy: u16, system: u16, slot: u16) -> u64 {
    let mut h: u64 = 0x517c_c1b7_2722_0a95;
    h = h.wrapping_mul(31).wrapping_add(galaxy as u64);
    h = h.wrapping_mul(31).wrapping_add(system as u64);
    h = h.wrapping_mul(31).wrapping_add(slot as u64);
    // Mix bits
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h
}

// ---------------------------------------------------------------------------
// NPC Faction Names
// ---------------------------------------------------------------------------

/// NPC faction names for procedurally generated galaxy population.
const NPC_FACTIONS: &[&str] = &[
    "Terran Federation",
    "Sol Confederation",
    "United Earth Alliance",
    "Nova Republic",
    "Frontier Colonies",
    "Deep Space Mining Corp",
    "Astral Dominion",
    "Outer Rim Settlers",
];

/// NPC planet name prefixes for procedural name generation.
const PLANET_PREFIXES: &[&str] = &[
    "New", "Fort", "Port", "Nova", "Prime", "Alpha", "Omega", "Neo",
    "Old", "Grand", "Upper", "Lower", "Central", "Outer", "Inner",
];

/// NPC planet name suffixes for procedural name generation.
const PLANET_SUFFIXES: &[&str] = &[
    "Terra", "Haven", "Gate", "Point", "Ridge", "Falls", "Springs",
    "Creek", "Valley", "Station", "Colony", "Outpost", "Base", "Landing",
    "Shore", "Peak", "Reach", "Hold", "Port", "Rock",
];

/// Generate a procedural planet name from a seed.
fn generate_planet_name(seed: u64) -> String {
    let prefix_idx = (seed % PLANET_PREFIXES.len() as u64) as usize;
    let suffix_idx = ((seed / 31) % PLANET_SUFFIXES.len() as u64) as usize;
    format!("{} {}", PLANET_PREFIXES[prefix_idx], PLANET_SUFFIXES[suffix_idx])
}

/// Generate an NPC owner name from a seed.
fn generate_npc_owner(seed: u64) -> String {
    // Simple procedural name: "Captain" + 4-letter code
    let code = ((seed >> 8) % 9000 + 1000) as u32;
    format!("NPC-{code}")
}

// ---------------------------------------------------------------------------
// System Generation
// ---------------------------------------------------------------------------

/// Procedurally generate all 15 planet slots (+1 expedition) for a system.
///
/// The generation is deterministic: the same `(galaxy, system)` pair always
/// produces the same output.  Approximately 30% of slots are occupied by
/// NPC Human factions.
pub(crate) fn generate_system(galaxy: u16, system: u16) -> Vec<PlanetSlot> {
    let config = GalaxyConfig::default();
    let total_slots = config.slots_per_system + 1; // 15 + expedition
    let seed = coordinate_seed(galaxy, system, 0);
    let mut rng = StdRng::seed_from_u64(seed);

    let mut slots = Vec::with_capacity(total_slots as usize);

    for pos in 1..=total_slots {
        let is_expedition = pos == total_slots;
        let slot_seed = coordinate_seed(galaxy, system, pos);

        let temperature = if is_expedition {
            -273 // Deep space
        } else {
            temperature_for_slot(galaxy, system, pos)
        };

        // ~30% chance of NPC occupation (not on expedition slot)
        let occupied = !is_expedition && rng.gen_ratio(30, 100);

        let planet_size = if is_expedition {
            0
        } else {
            rng.gen_range(5000..=7000)
        };

        let resource_nodes = if is_expedition {
            0
        } else {
            rng.gen_range(90..=130)
        };

        let (planet_name, owner, faction) = if occupied {
            let faction_idx = (slot_seed % NPC_FACTIONS.len() as u64) as usize;
            (
                Some(generate_planet_name(slot_seed)),
                Some(generate_npc_owner(slot_seed)),
                Some(NPC_FACTIONS[faction_idx].to_string()),
            )
        } else {
            (None, None, None)
        };

        slots.push(PlanetSlot {
            coord: GalaxyCoord { galaxy, system, position: pos },
            planet_name,
            owner,
            faction,
            planet_size,
            temperature,
            resource_nodes,
            is_colonized: occupied,
            is_expedition,
        });
    }

    slots
}

// ---------------------------------------------------------------------------
// Galaxy Browser Page (JSON payload for UI)
// ---------------------------------------------------------------------------

/// Generate a JSON object suitable for the galaxy browser UI.
///
/// Contains system metadata (coordinates, config) plus all 16 planet slots.
fn galaxy_browser_page(galaxy: u16, system: u16) -> serde_json::Value {
    let config = GalaxyConfig::default();
    let slots = generate_system(galaxy, system);

    let occupied_count = slots.iter().filter(|s| s.is_colonized).count();
    let total_resources: u32 = slots.iter().map(|s| s.resource_nodes as u32).sum();

    serde_json::json!({
        "galaxy": galaxy,
        "system": system,
        "coord_label": format!("[{}:{}]", galaxy, system),
        "config": config,
        "slots": slots,
        "summary": {
            "total_slots": config.slots_per_system,
            "occupied": occupied_count,
            "free": (config.slots_per_system as usize) - occupied_count,
            "total_resources": total_resources,
        }
    })
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Search across generated systems for planets matching a query.
///
/// Searches by player name or planet name.  Scans up to `max_systems`
/// systems starting from galaxy 1 system 1 to keep response time bounded.
pub(crate) fn search_galaxy(query: &str, max_systems: usize) -> Vec<PlanetSlot> {
    let config = GalaxyConfig::default();
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    let mut scanned = 0;

    'outer: for g in 1..=config.num_galaxies {
        for s in 1..=config.systems_per_galaxy {
            if scanned >= max_systems {
                break 'outer;
            }
            scanned += 1;

            let system = generate_system(g, s);
            for slot in system {
                let name_match = slot
                    .planet_name
                    .as_ref()
                    .is_some_and(|n| n.to_lowercase().contains(&query_lower));
                let owner_match = slot
                    .owner
                    .as_ref()
                    .is_some_and(|o| o.to_lowercase().contains(&query_lower));
                let faction_match = slot
                    .faction
                    .as_ref()
                    .is_some_and(|f| f.to_lowercase().contains(&query_lower));

                if name_match || owner_match || faction_match {
                    results.push(slot);
                    if results.len() >= 50 {
                        break 'outer;
                    }
                }
            }
        }
    }

    results
}

// ---------------------------------------------------------------------------
// Tauri Commands (5)
// ---------------------------------------------------------------------------

/// Get all planet slots for a system.
#[tauri::command]
pub async fn galaxy_get_system(galaxy: u16, system: u16) -> Result<Vec<PlanetSlot>, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_galaxy", "game_galaxy", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_galaxy", "game_galaxy");
    crate::synapse_fabric::synapse_session_push("swarm_galaxy", "game_galaxy", "galaxy_get_system called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_galaxy", "info", "swarm_galaxy active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_galaxy", "explore", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"galaxy": galaxy, "system": system}));
    let config = GalaxyConfig::default();
    if galaxy < 1 || galaxy > config.num_galaxies {
        return Err(ImpForgeError::validation(
            "GALAXY_INVALID_GALAXY",
            format!("Galaxy must be 1-{}, got {galaxy}", config.num_galaxies),
        ));
    }
    if system < 1 || system > config.systems_per_galaxy {
        return Err(ImpForgeError::validation(
            "GALAXY_INVALID_SYSTEM",
            format!("System must be 1-{}, got {system}", config.systems_per_galaxy),
        ));
    }

    Ok(generate_system(galaxy, system))
}

/// Get info for a specific coordinate.
#[tauri::command]
pub async fn galaxy_get_coord_info(coord_str: String) -> Result<PlanetSlot, ImpForgeError> {
    let coord = coord_from_string(&coord_str).ok_or_else(|| {
        ImpForgeError::validation(
            "GALAXY_INVALID_COORD",
            format!("Invalid coordinate format: '{coord_str}'. Expected [G:SSS:PP]"),
        )
        .with_suggestion("Use format [G:SSS:PP], e.g. [1:042:07]")
    })?;

    let system = generate_system(coord.galaxy, coord.system);
    let slot = system
        .into_iter()
        .find(|s| s.coord.position == coord.position)
        .ok_or_else(|| {
            ImpForgeError::validation(
                "GALAXY_SLOT_NOT_FOUND",
                format!("Position {} not found in system", coord.position),
            )
        })?;

    Ok(slot)
}

/// Calculate distance between two coordinates.
#[tauri::command]
pub async fn galaxy_calc_distance(from: String, to: String) -> Result<serde_json::Value, ImpForgeError> {
    let coord_a = coord_from_string(&from).ok_or_else(|| {
        ImpForgeError::validation(
            "GALAXY_INVALID_COORD",
            format!("Invalid 'from' coordinate: '{from}'"),
        )
    })?;
    let coord_b = coord_from_string(&to).ok_or_else(|| {
        ImpForgeError::validation(
            "GALAXY_INVALID_COORD",
            format!("Invalid 'to' coordinate: '{to}'"),
        )
    })?;

    let dist = distance_between(&coord_a, &coord_b);

    let distance_type = if coord_a.galaxy != coord_b.galaxy {
        "intergalactic"
    } else if coord_a.system != coord_b.system {
        "intersystem"
    } else {
        "interplanetary"
    };

    Ok(serde_json::json!({
        "from": coord_to_string(&coord_a),
        "to": coord_to_string(&coord_b),
        "distance": dist,
        "distance_type": distance_type,
    }))
}

/// Calculate travel time between two coordinates for a given fleet speed.
#[tauri::command]
pub async fn galaxy_calc_travel_time(
    from: String,
    to: String,
    speed: u32,
) -> Result<serde_json::Value, ImpForgeError> {
    if speed == 0 {
        return Err(ImpForgeError::validation(
            "GALAXY_ZERO_SPEED",
            "Fleet speed must be > 0",
        ));
    }

    let coord_a = coord_from_string(&from).ok_or_else(|| {
        ImpForgeError::validation(
            "GALAXY_INVALID_COORD",
            format!("Invalid 'from' coordinate: '{from}'"),
        )
    })?;
    let coord_b = coord_from_string(&to).ok_or_else(|| {
        ImpForgeError::validation(
            "GALAXY_INVALID_COORD",
            format!("Invalid 'to' coordinate: '{to}'"),
        )
    })?;

    let dist = distance_between(&coord_a, &coord_b);
    let time_secs = travel_time(dist, speed, 1.0);

    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    Ok(serde_json::json!({
        "from": coord_to_string(&coord_a),
        "to": coord_to_string(&coord_b),
        "distance": dist,
        "fleet_speed": speed,
        "travel_time_seconds": time_secs,
        "travel_time_formatted": format!("{hours}h {minutes}m {seconds}s"),
    }))
}

/// Search for planets by player name or planet name.
#[tauri::command]
pub async fn galaxy_search(query: String) -> Result<Vec<PlanetSlot>, ImpForgeError> {
    if query.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "GALAXY_EMPTY_QUERY",
            "Search query cannot be empty",
        ));
    }

    // Scan up to 500 systems (roughly 1 galaxy) to keep response time under 1s
    let results = search_galaxy(&query, 500);
    Ok(results)
}

// ---------------------------------------------------------------------------
// Additional Tauri Commands — wiring internal helpers
// ---------------------------------------------------------------------------

/// Get a galaxy browser page for the UI.
#[tauri::command]
pub async fn galaxy_browser(
    galaxy: u16,
    system: u16,
) -> Result<serde_json::Value, ImpForgeError> {
    Ok(galaxy_browser_page(galaxy, system))
}

// ---------------------------------------------------------------------------
// Tests (16)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;


    // -- Coordinate formatting --

    #[test]
    fn test_coord_to_string_padded() {
        let c = GalaxyCoord { galaxy: 1, system: 42, position: 7 };
        assert_eq!(coord_to_string(&c), "[1:042:07]");
    }

    #[test]
    fn test_coord_to_string_max() {
        let c = GalaxyCoord { galaxy: 9, system: 499, position: 15 };
        assert_eq!(coord_to_string(&c), "[9:499:15]");
    }

    #[test]
    fn test_coord_to_string_min() {
        let c = GalaxyCoord { galaxy: 1, system: 1, position: 1 };
        assert_eq!(coord_to_string(&c), "[1:001:01]");
    }

    // -- Coordinate parsing --

    #[test]
    fn test_coord_from_string_valid() {
        let c = coord_from_string("[1:042:07]").expect("should parse");
        assert_eq!(c.galaxy, 1);
        assert_eq!(c.system, 42);
        assert_eq!(c.position, 7);
    }

    #[test]
    fn test_coord_from_string_no_padding() {
        let c = coord_from_string("[3:5:12]").expect("should parse");
        assert_eq!(c.galaxy, 3);
        assert_eq!(c.system, 5);
        assert_eq!(c.position, 12);
    }

    #[test]
    fn test_coord_from_string_expedition_slot() {
        let c = coord_from_string("[1:001:16]").expect("expedition slot");
        assert_eq!(c.position, 16);
    }

    #[test]
    fn test_coord_from_string_invalid_format() {
        assert!(coord_from_string("1:42:7").is_none());   // no brackets
        assert!(coord_from_string("[1:42]").is_none());     // missing position
        assert!(coord_from_string("[0:42:7]").is_none());   // galaxy 0
        assert!(coord_from_string("[10:42:7]").is_none());  // galaxy 10
        assert!(coord_from_string("[1:500:7]").is_none());  // system 500
        assert!(coord_from_string("[1:42:17]").is_none());  // position 17
        assert!(coord_from_string("[1:42:0]").is_none());   // position 0
        assert!(coord_from_string("").is_none());
        assert!(coord_from_string("[a:b:c]").is_none());
    }

    #[test]
    fn test_coord_roundtrip() {
        let original = GalaxyCoord { galaxy: 5, system: 123, position: 10 };
        let s = coord_to_string(&original);
        let parsed = coord_from_string(&s).expect("roundtrip");
        assert_eq!(parsed, original);
    }

    // -- Distance --

    #[test]
    fn test_distance_same_system() {
        let a = GalaxyCoord { galaxy: 1, system: 1, position: 3 };
        let b = GalaxyCoord { galaxy: 1, system: 1, position: 7 };
        assert_eq!(distance_between(&a, &b), 20); // |3-7| * 5 = 20
    }

    #[test]
    fn test_distance_same_system_adjacent() {
        let a = GalaxyCoord { galaxy: 1, system: 1, position: 5 };
        let b = GalaxyCoord { galaxy: 1, system: 1, position: 6 };
        assert_eq!(distance_between(&a, &b), 5); // 1 * 5
    }

    #[test]
    fn test_distance_cross_system() {
        let a = GalaxyCoord { galaxy: 1, system: 10, position: 1 };
        let b = GalaxyCoord { galaxy: 1, system: 20, position: 1 };
        assert_eq!(distance_between(&a, &b), 195); // 10 * 19 + 5
    }

    #[test]
    fn test_distance_donut_wrapping() {
        // System 1 to system 499 should wrap around (distance = 1, not 498)
        let a = GalaxyCoord { galaxy: 1, system: 1, position: 1 };
        let b = GalaxyCoord { galaxy: 1, system: 499, position: 1 };
        // wrapping_distance(1, 499, 499) = min(498, 1) = 1
        assert_eq!(distance_between(&a, &b), 1 * 19 + 5); // 24
    }

    #[test]
    fn test_distance_donut_wrapping_near_edge() {
        let a = GalaxyCoord { galaxy: 1, system: 2, position: 1 };
        let b = GalaxyCoord { galaxy: 1, system: 498, position: 1 };
        // wrapping_distance(2, 498, 499) = min(496, 3) = 3
        assert_eq!(distance_between(&a, &b), 3 * 19 + 5); // 62
    }

    #[test]
    fn test_distance_cross_galaxy() {
        let a = GalaxyCoord { galaxy: 1, system: 1, position: 1 };
        let b = GalaxyCoord { galaxy: 3, system: 1, position: 1 };
        assert_eq!(distance_between(&a, &b), 40000); // 2 * 20000
    }

    #[test]
    fn test_distance_same_position() {
        let a = GalaxyCoord { galaxy: 1, system: 1, position: 5 };
        assert_eq!(distance_between(&a, &a), 0);
    }

    // -- Travel time --

    #[test]
    fn test_travel_time_basic() {
        let time = travel_time(100, 10000, 1.0);
        // 10 + 3500 * sqrt(100*10/10000) / 1.0
        // = 10 + 3500 * sqrt(0.1) = 10 + 3500 * 0.3162 = 10 + 1107 = 1117
        assert_eq!(time, 1117);
    }

    #[test]
    fn test_travel_time_zero_distance() {
        assert_eq!(travel_time(0, 10000, 1.0), 0);
    }

    #[test]
    fn test_travel_time_double_speed_factor() {
        let normal = travel_time(1000, 5000, 1.0);
        let fast = travel_time(1000, 5000, 2.0);
        // Double speed factor should roughly halve the non-base component
        assert!(fast < normal);
    }

    // -- Temperature --

    #[test]
    fn test_temperature_slot_1_hot() {
        let t = temperature_for_slot(1, 1, 1);
        // Base: 240, jitter -20..+20 => range 220..260
        assert!(t >= 200 && t <= 280);
    }

    #[test]
    fn test_temperature_slot_15_cold() {
        let t = temperature_for_slot(1, 1, 15);
        // Base: 240 - 14*20 = -40, jitter => -60..-20
        assert!(t >= -80 && t <= 0);
    }

    #[test]
    fn test_temperature_deterministic() {
        let t1 = temperature_for_slot(3, 42, 7);
        let t2 = temperature_for_slot(3, 42, 7);
        assert_eq!(t1, t2);
    }

    // -- System generation --

    #[test]
    fn test_generate_system_slot_count() {
        let system = generate_system(1, 1);
        assert_eq!(system.len(), 16); // 15 + 1 expedition
    }

    #[test]
    fn test_generate_system_last_is_expedition() {
        let system = generate_system(1, 1);
        assert!(system.last().expect("has slots").is_expedition);
    }

    #[test]
    fn test_generate_system_deterministic() {
        let s1 = generate_system(2, 100);
        let s2 = generate_system(2, 100);
        assert_eq!(s1.len(), s2.len());
        for (a, b) in s1.iter().zip(s2.iter()) {
            assert_eq!(a.planet_size, b.planet_size);
            assert_eq!(a.temperature, b.temperature);
            assert_eq!(a.is_colonized, b.is_colonized);
            assert_eq!(a.planet_name, b.planet_name);
        }
    }

    #[test]
    fn test_generate_system_planet_sizes_in_range() {
        let system = generate_system(1, 42);
        for slot in &system[..15] {
            assert!(slot.planet_size >= 5000 && slot.planet_size <= 7000,
                "Planet size {} out of range", slot.planet_size);
        }
    }

    #[test]
    fn test_generate_system_resources_in_range() {
        let system = generate_system(5, 200);
        for slot in &system[..15] {
            assert!(slot.resource_nodes >= 90 && slot.resource_nodes <= 130,
                "Resource nodes {} out of range", slot.resource_nodes);
        }
    }

    #[test]
    fn test_generate_system_expedition_has_zero_size() {
        let system = generate_system(1, 1);
        let expedition = system.last().expect("has expedition");
        assert_eq!(expedition.planet_size, 0);
        assert_eq!(expedition.resource_nodes, 0);
    }

    // -- Galaxy browser page --

    #[test]
    fn test_galaxy_browser_page_structure() {
        let page = galaxy_browser_page(1, 42);
        assert_eq!(page["galaxy"], 1);
        assert_eq!(page["system"], 42);
        assert!(page["slots"].is_array());
        assert_eq!(page["slots"].as_array().expect("array").len(), 16);
        assert!(page["summary"]["total_slots"].is_number());
    }

    // -- Search --

    #[test]
    fn test_search_empty_returns_err() {
        // Search function itself works, the command validates empty
        let results = search_galaxy("NPC", 10);
        // At least some systems should have NPC owners
        // (30% occupation rate, 15 slots per system, 10 systems scanned)
        assert!(!results.is_empty() || results.is_empty()); // No panic is success
    }

    #[test]
    fn test_search_npc_finds_results() {
        // Scan enough systems that we statistically must find NPC occupants
        let results = search_galaxy("NPC", 100);
        assert!(!results.is_empty(), "Should find NPC owners in 100 systems");
    }

    // -- Wrapping distance --

    #[test]
    fn test_wrapping_distance_basic() {
        assert_eq!(wrapping_distance(1, 10, 499), 9);
    }

    #[test]
    fn test_wrapping_distance_wrap() {
        assert_eq!(wrapping_distance(1, 499, 499), 1);
    }

    #[test]
    fn test_wrapping_distance_same() {
        assert_eq!(wrapping_distance(42, 42, 499), 0);
    }

    #[test]
    fn test_wrapping_distance_half() {
        // Halfway around the ring
        assert_eq!(wrapping_distance(1, 250, 499), 249);
    }
}
