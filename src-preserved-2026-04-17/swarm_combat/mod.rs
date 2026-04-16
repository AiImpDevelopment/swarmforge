// SPDX-License-Identifier: Elastic-2.0
//! SwarmForge Combat — Session-9 Crown-Jewel Split.
//!
//! ## Subsystems
//!
//! - **LoL-style damage matrix:** 7x7 type/armour effectiveness with
//!   `Damage = ATK * mult * 100 / (100 + armour)`
//! - **Fleet movement (OGame-inspired):** seven mission types, travel time
//!   scaled by distance and slowest-ship speed
//! - **Battle simulation:** up to 6 rounds, shields-first-then-hull,
//!   30 %-destroyed-to-debris, 50 %-loot on victory
//! - **Terrain modifiers:** per-faction home terrain grants defensive
//!   bonuses
//! - **Defensive structures:** missile towers, shield generators, etc.
//!
//! ## Session-9 split layout
//! | Sub-module   | Responsibility                                              |
//! |--------------|-------------------------------------------------------------|
//! | [`types`]    | Damage/armour enums, matrix, Resources, MissionType,        |
//! |              | FleetStatus, FleetMission, BattleResult + their impls       |
//! | [`engine`]   | SimpleRng (deterministic) + `SwarmCombatEngine` + SQLite    |
//! | [`terrain`]  | `FactionTerrain`, `TerrainTile`, `TerrainEffect`             |
//! | [`defense`]  | `DefenseType`, `DefenseStats`, `DefenseCost`                 |
//! | [`commands`] | All 21 `#[tauri::command]` handlers                          |
//! | [`tests`]    | Full integration test suite                                  |
//!
//! Storage: `~/.impforge/swarmforge.db` (SQLite, WAL mode) — same DB as
//! `swarm_advanced` (tables are named distinctly).
//!
//! Note: SwarmForge is scheduled for extraction into its own public GitHub
//! repository.  This split produces a clean hand-off state for that move.

pub(crate) mod commands;
pub(crate) mod defense;
pub(crate) mod engine;
pub(crate) mod terrain;
pub(crate) mod types;

#[cfg(test)]
mod tests;

// Glob re-exports so `lib.rs` keeps its `swarm_combat::<cmd>` registrations
// working unchanged — required because `#[tauri::command]` expands into
// `__cmd__*` helper modules alongside each handler.
pub use commands::*;
pub use engine::SwarmCombatEngine;

// Test-only re-exports — `tests.rs` uses `super::*;` so it needs access to
// every type, every helper fn, every free fn.  Production code goes through
// the `commands` layer which already has explicit `super::<module>::*` imports.
#[cfg(test)]
pub(crate) use defense::*;
#[cfg(test)]
pub(crate) use engine::simulate_battle;
#[cfg(test)]
pub(crate) use terrain::*;
#[cfg(test)]
pub(crate) use types::*;

/// Health declaration (roll-up of all sub-modules).
const _MODULE_HEALTH: (&str, &str) = ("swarm_combat", "Game");
