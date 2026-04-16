// SPDX-License-Identifier: Elastic-2.0
//! SwarmForge Combat -- Terrain types (FactionTerrain, TerrainTile,
//! TerrainEffect) + their impls.  Terrain modifies defence and movement
//! depending on the defending faction's natural environment.

use serde::{Deserialize, Serialize};

/// Health declaration for this sub-module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_combat::terrain", "Game");

/// Faction-specific terrain types that spread outward from colonies.
///
/// Each faction spreads a unique terrain type outward from its colony.
/// Terrain grants bonuses to the owning faction and imposes penalties on
/// enemies.  Where two terrains meet a *Contested* zone forms; where three
/// or more meet a *Nexus* zone spawns neutral hostiles.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FactionTerrain {
    /// Insects: +25% move speed, +10% attack speed, grants vision
    ChitinousResin,
    /// Demons: +15% ability power, +mana regen, 1%HP/sec to non-demons
    HellfireCorruption,
    /// Undead: +2% HP/sec regen, 30% auto-raise corpses
    Necrosis,
    /// Humans: +10% armor, +5% resource production
    HumanSettlement,
    /// No faction bonus
    Neutral,
    /// Where 2 terrains meet: visual border war
    Contested,
    /// Where 3+ terrains meet: spawns neutral hostiles
    Nexus,
}

impl FactionTerrain {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChitinousResin => "chitinous_resin",
            Self::HellfireCorruption => "hellfire_corruption",
            Self::Necrosis => "necrosis",
            Self::HumanSettlement => "human_settlement",
            Self::Neutral => "neutral",
            Self::Contested => "contested",
            Self::Nexus => "nexus",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "chitinous_resin" => Self::ChitinousResin,
            "hellfire_corruption" => Self::HellfireCorruption,
            "necrosis" => Self::Necrosis,
            "human_settlement" => Self::HumanSettlement,
            "contested" => Self::Contested,
            "nexus" => Self::Nexus,
            _ => Self::Neutral,
        }
    }

    /// Which faction owns this terrain type (None for neutral/contested/nexus).
    pub fn owner_faction(&self) -> Option<&'static str> {
        match self {
            Self::ChitinousResin => Some("insects"),
            Self::HellfireCorruption => Some("demons"),
            Self::Necrosis => Some("undead"),
            Self::HumanSettlement => Some("humans"),
            _ => None,
        }
    }
}

/// A single tile on the terrain map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainTile {
    pub x: i32,
    pub y: i32,
    pub terrain_type: FactionTerrain,
    /// How established this terrain is (0.0 = just appeared, 1.0 = fully mature).
    pub strength: f64,
    /// Tiles per hour this terrain spreads outward.
    pub spread_rate: f64,
    /// Faction that controls this tile.
    pub owner_faction: String,
}

/// Stat bonuses/penalties applied by a terrain type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainEffect {
    pub move_speed_bonus: f64,
    pub attack_speed_bonus: f64,
    pub ability_power_bonus: f64,
    pub mana_regen_bonus: f64,
    pub hp_regen_per_sec: f64,
    pub damage_per_sec_to_enemies: f64,
    pub armor_bonus: f64,
    pub resource_bonus: f64,
    pub vision_granted: bool,
    pub auto_raise_chance: f64,
}

impl Default for TerrainEffect {
    fn default() -> Self {
        Self {
            move_speed_bonus: 0.0,
            attack_speed_bonus: 0.0,
            ability_power_bonus: 0.0,
            mana_regen_bonus: 0.0,
            hp_regen_per_sec: 0.0,
            damage_per_sec_to_enemies: 0.0,
            armor_bonus: 0.0,
            resource_bonus: 0.0,
            vision_granted: false,
            auto_raise_chance: 0.0,
        }
    }
}

/// Return the stat bonuses for a given terrain type.
pub fn terrain_get_effect(terrain: &FactionTerrain) -> TerrainEffect {
    match terrain {
        FactionTerrain::ChitinousResin => TerrainEffect {
            move_speed_bonus: 0.25,
            attack_speed_bonus: 0.10,
            vision_granted: true,
            ..Default::default()
        },
        FactionTerrain::HellfireCorruption => TerrainEffect {
            ability_power_bonus: 0.15,
            mana_regen_bonus: 5.0,
            damage_per_sec_to_enemies: 0.01, // 1% HP/sec
            ..Default::default()
        },
        FactionTerrain::Necrosis => TerrainEffect {
            hp_regen_per_sec: 0.02, // 2% HP/sec
            auto_raise_chance: 0.30,
            ..Default::default()
        },
        FactionTerrain::HumanSettlement => TerrainEffect {
            armor_bonus: 0.10,
            resource_bonus: 0.05,
            ..Default::default()
        },
        FactionTerrain::Contested => TerrainEffect {
            // Contested zones give reduced bonuses to both sides
            damage_per_sec_to_enemies: 0.005,
            ..Default::default()
        },
        FactionTerrain::Nexus => TerrainEffect {
            // Nexus zones are dangerous for everyone
            damage_per_sec_to_enemies: 0.02,
            ..Default::default()
        },
        FactionTerrain::Neutral => TerrainEffect::default(),
    }
}

/// Advance terrain spread by `delta_secs`.  Each tile grows in strength and
/// can cause adjacent neutral tiles to flip to the spreading faction.
pub fn terrain_spread_tick(tiles: &mut [TerrainTile], delta_secs: f64) {
    let delta_hours = delta_secs / 3600.0;

    // Phase 1: strengthen existing tiles
    for tile in tiles.iter_mut() {
        if tile.terrain_type != FactionTerrain::Neutral
            && tile.terrain_type != FactionTerrain::Contested
            && tile.terrain_type != FactionTerrain::Nexus
        {
            tile.strength = (tile.strength + delta_hours * 0.1).min(1.0);
        }
    }

    // Phase 2: collect spread candidates from mature tiles (strength > 0.5)
    let spread_sources: Vec<(i32, i32, FactionTerrain, String, f64)> = tiles
        .iter()
        .filter(|t| {
            t.strength > 0.5
                && t.terrain_type != FactionTerrain::Neutral
                && t.terrain_type != FactionTerrain::Contested
                && t.terrain_type != FactionTerrain::Nexus
        })
        .map(|t| {
            (
                t.x,
                t.y,
                t.terrain_type.clone(),
                t.owner_faction.clone(),
                t.spread_rate,
            )
        })
        .collect();

    for (sx, sy, terrain, faction, rate) in &spread_sources {
        let spread_chance = rate * delta_hours;
        if spread_chance <= 0.0 {
            continue;
        }

        // Check 4-connected neighbours
        let neighbours = [(sx - 1, *sy), (sx + 1, *sy), (*sx, sy - 1), (*sx, sy + 1)];

        for (nx, ny) in neighbours {
            if let Some(neighbour) = tiles.iter_mut().find(|t| t.x == nx && t.y == ny) {
                if neighbour.terrain_type == FactionTerrain::Neutral {
                    // Neutral tile gets claimed (deterministic: if rate*dt >= 1 tile/hr)
                    if spread_chance >= 1.0 {
                        neighbour.terrain_type = terrain.clone();
                        neighbour.owner_faction = faction.clone();
                        neighbour.strength = 0.1;
                        neighbour.spread_rate = *rate;
                    }
                }
            }
        }
    }
}

/// Find all tiles that are contested (adjacent to two or more different factions).
/// Returns the (x, y) coordinates of border-war zones.
pub fn terrain_check_contested(tiles: &[TerrainTile]) -> Vec<(i32, i32)> {
    let mut contested = Vec::new();

    for tile in tiles {
        let neighbours = [
            (tile.x - 1, tile.y),
            (tile.x + 1, tile.y),
            (tile.x, tile.y - 1),
            (tile.x, tile.y + 1),
        ];

        let mut adjacent_factions: Vec<&str> = Vec::new();

        for (nx, ny) in &neighbours {
            if let Some(n) = tiles.iter().find(|t| t.x == *nx && t.y == *ny) {
                if let Some(faction) = n.terrain_type.owner_faction() {
                    if !adjacent_factions.contains(&faction) {
                        adjacent_factions.push(faction);
                    }
                }
            }
        }

        // A tile is contested if it borders 2+ different factions
        if adjacent_factions.len() >= 2 {
            contested.push((tile.x, tile.y));
        }
    }

    contested
}

/// Calculate damage-per-second a unit takes from standing on enemy terrain.
/// Returns 0.0 if the unit belongs to the terrain's faction.
pub fn terrain_cross_faction_damage(unit_faction: &str, tile: &TerrainTile) -> f64 {
    let effect = terrain_get_effect(&tile.terrain_type);

    // No damage to the terrain's owner
    if let Some(owner) = tile.terrain_type.owner_faction() {
        if owner == unit_faction {
            return 0.0;
        }
    }

    // Scale damage by terrain strength
    effect.damage_per_sec_to_enemies * tile.strength
}

/// Generate a small test map for a colony: 7x7 grid with the colony's terrain
/// in the centre, spreading outward, and neutral tiles at the edges.
pub(crate) fn generate_test_terrain_map(colony_id: &str) -> Vec<TerrainTile> {
    // Determine faction from colony_id prefix or default to insects
    let (terrain, faction) = if colony_id.starts_with("demon") {
        (FactionTerrain::HellfireCorruption, "demons")
    } else if colony_id.starts_with("undead") {
        (FactionTerrain::Necrosis, "undead")
    } else if colony_id.starts_with("human") {
        (FactionTerrain::HumanSettlement, "humans")
    } else {
        (FactionTerrain::ChitinousResin, "insects")
    };

    let mut tiles = Vec::with_capacity(49);
    let centre = 3i32;

    for y in 0..7 {
        for x in 0..7 {
            let dist = ((x - centre).abs() + (y - centre).abs()) as f64;
            let (tt, strength, owner) = if dist <= 1.0 {
                // Core: fully established faction terrain
                (terrain.clone(), 1.0, faction.to_string())
            } else if dist <= 2.0 {
                // Inner ring: partially established
                (terrain.clone(), 0.7, faction.to_string())
            } else if dist <= 3.0 {
                // Outer ring: weakly established
                (terrain.clone(), 0.3, faction.to_string())
            } else {
                // Edge: neutral
                (FactionTerrain::Neutral, 0.0, "neutral".to_string())
            };

            tiles.push(TerrainTile {
                x,
                y,
                terrain_type: tt,
                strength,
                spread_rate: 0.5,
                owner_faction: owner,
            });
        }
    }

    tiles
}
