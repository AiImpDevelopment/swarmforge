// SPDX-License-Identifier: Elastic-2.0
//! SwarmForge -- Idle RPG + OGame-style Colony Builder
//!
//! A medieval/sci-fi hybrid idle game where the user's REAL productivity powers
//! their character AND their planet colony. Writing documents = crafting weapons
//! AND producing biomass. Running workflows = fighting monsters AND building
//! fleet ships. AI queries = casting spells AND researching technology.
//!
//! ## Two Game Layers
//! 1. **RPG Layer** (ForgeQuest legacy): Character, equipment, skills, zones, quests
//! 2. **Colony Layer** (OGame-inspired): Planet resources, buildings, research, fleet,
//!    creep mechanic, dark matter shop -- all with exponential cost curves
//!
//! ## Architecture
//! - `ForgeQuestEngine` owns the SQLite connection (WAL mode) and all game logic
//! - Tauri commands are thin wrappers that delegate to the engine
//! - `quest_track_action` is the primary entry-point -- call it from any module
//!   when a trackable event occurs to grant XP, gold, materials, and auto-battles
//!
//! ## Sub-modules
//! - `types` — RPG layer: character, items, equipment, skills, zones, quests
//! - `evosys` — Novelty multiplier, governing attributes
//! - `swarm_types` — Swarm/Colony: factions, unit types, evolution paths
//! - `mutations` — Mutation system (every 5 levels, 1-of-3 permanent choices)
//! - `colony_types` — OGame: buildings, resources, research, fleet, shop, missions
//! - `engine` — ForgeQuestEngine (SQLite WAL, all game logic)
//! - `static_data` — Zones, recipes, action mapping, evolution paths, shop items
//! - `commands` — Tauri command wrappers

pub(crate) mod types;
pub(crate) mod evosys;
pub(crate) mod swarm_types;
pub(crate) mod mutations;
pub(crate) mod colony_types;
pub(crate) mod engine;
pub(crate) mod static_data;
pub(crate) mod commands;

// Production re-exports — lib.rs reaches `forge_quest::quest_*` commands
// and `forge_quest::ForgeQuestEngine` through these two glob imports.
pub(crate) use engine::*;
pub(crate) use commands::*;
pub(crate) use swarm_types::*;

// Test-only re-exports — the Session-8 test suite in `tests.rs` uses
// `super::*;` to reach helper types (e.g. `CharacterClass`, `PlanetBuildingType`,
// `xp_for_level`) without per-test `use` statements.  Gated on `cfg(test)`
// so the production binary does not re-export unused symbols.
#[cfg(test)]
pub(crate) use types::*;
#[cfg(test)]
pub(crate) use mutations::*;
#[cfg(test)]
pub(crate) use colony_types::*;
#[cfg(test)]
pub(crate) use static_data::*;

#[cfg(test)]
mod tests;
