// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Advanced Systems — Session-9 Crown-Jewel Split.
//!
//! Three major subsystems that extend SwarmForge:
//!
//! ## Part 1: Human Faction Commander Dashboard
//! The developer (Karsten) plays the ENTIRE Human faction as a real player.
//! Same rules, same resources, same combat — NO cheating.  When offline,
//! NPC AI takes over with standing orders.
//!
//! ## Part 2: Standalone Release System
//! SwarmForge can launch as its own standalone app **or** embedded inside
//! ImpForge.  Detection is automatic via `IMPFORGE_EMBEDDED` env var.
//!
//! ## Part 3: Advanced OGame Mechanics
//! Fleet Save, Vacation Mode, Noob Protection, Debris Fields, Moon
//! Creation, Phalanx Sensor Scanning, and Jump Gate transfers.
//!
//! ## Session-9 split layout
//! | Sub-module   | Responsibility                                     |
//! |--------------|----------------------------------------------------|
//! | [`types`]    | Every data shape + the commander passphrase const  |
//! | [`engine`]   | `SwarmAdvancedEngine` + SQLite schema + impl        |
//! | [`commands`] | All 24 `#[tauri::command]` handlers                 |
//! | [`tests`]    | The full integration test suite (~50 tests)         |
//!
//! Storage: `~/.impforge/swarmforge.db` (SQLite, WAL mode).
//!
//! Note: SwarmForge is scheduled for extraction into its own public GitHub
//! repository (Session 10+).  This Crown-Jewel split produces a clean hand-
//! off state for that move.

pub(crate) mod commands;
pub(crate) mod engine;
pub(crate) mod types;

#[cfg(test)]
mod tests;

// Glob re-exports so `lib.rs` keeps its `swarm_advanced::<cmd>` registrations
// working unchanged — required because `#[tauri::command]` expands into
// `__cmd__*` helper modules alongside each handler.
pub use commands::*;
pub use engine::SwarmAdvancedEngine;

// Test-only re-exports for the `tests.rs` module (uses `super::*;` via
// `mod tests`).  Tests need access to types + engine + constants.
#[cfg(test)]
pub(crate) use types::*;

/// Health declaration (roll-up of all sub-modules).
const _MODULE_HEALTH: (&str, &str) = ("swarm_advanced", "Game");
