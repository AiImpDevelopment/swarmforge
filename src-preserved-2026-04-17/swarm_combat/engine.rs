// SPDX-License-Identifier: Elastic-2.0
//! SwarmForge Combat -- SimpleRng (deterministic) + SwarmCombatEngine.
//!
//! The engine owns SQLite persistence for fleet missions and exposes the
//! battle simulation + travel-time helpers used by the Tauri command layer.

use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;
use uuid::Uuid;

use crate::error::ImpForgeError;

use super::defense::DefenseType;
use super::types::*;

/// Health declaration for this sub-module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_combat::engine", "Game");

/// Simple deterministic PRNG (xorshift64) to avoid pulling in `rand`.
/// Seeded from the fleet composition hash so results are reproducible
/// for the same fleet matchup.
pub(crate) struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(1) } // avoid zero-state
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Return a random index in `0..len`.
    fn index(&mut self, len: usize) -> usize {
        (self.next_u64() % len as u64) as usize
    }
}

/// Create a seed from the fleet compositions.
fn fleet_seed(attacker: &[(String, u32)], defender: &[(String, u32)]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV offset basis
    for (st, c) in attacker.iter().chain(defender.iter()) {
        for b in st.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3); // FNV prime
        }
        h ^= *c as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Simulate a battle between two fleets.
///
/// Up to 6 rounds.  Each round:
/// 1. Shields regenerate to 100%.
/// 2. Every surviving unit picks a random enemy and deals damage.
/// 3. Dead units (hp <= 0) are removed.
///
/// After battle:
/// - 30% of destroyed ship costs become debris (floating in space).
/// - If attacker won, they loot 50% of destroyed defender cost.
pub fn simulate_battle(
    attacker_fleet: &[(String, u32)],
    defender_fleet: &[(String, u32)],
) -> BattleResult {
    let mut attackers = expand_fleet(attacker_fleet);
    let mut defenders = expand_fleet(defender_fleet);
    let mut rng = SimpleRng::new(fleet_seed(attacker_fleet, defender_fleet));

    let max_rounds: u32 = 6;
    let mut rounds_fought: u32 = 0;

    for _ in 0..max_rounds {
        if attackers.is_empty() || defenders.is_empty() {
            break;
        }
        rounds_fought += 1;

        // Regenerate shields each round (OGame mechanic)
        for u in attackers.iter_mut().chain(defenders.iter_mut()) {
            u.shields = u.shields_max;
        }

        // Collect damage: (target_side, target_index, damage_amount)
        let mut atk_damage: Vec<(usize, f64)> = Vec::new();
        let mut def_damage: Vec<(usize, f64)> = Vec::new();

        // Attackers fire at defenders
        for a in attackers.iter() {
            let target_idx = rng.index(defenders.len());
            let target = &defenders[target_idx];
            let dmg = calculate_damage(
                a.attack,
                target.shields + (target.hp * 0.1), // armour = shields + 10% hull
                a.damage_type,
                target.armor_type,
            );
            def_damage.push((target_idx, dmg));
        }

        // Defenders fire at attackers
        for d in defenders.iter() {
            let target_idx = rng.index(attackers.len());
            let target = &attackers[target_idx];
            let dmg = calculate_damage(
                d.attack,
                target.shields + (target.hp * 0.1),
                d.damage_type,
                target.armor_type,
            );
            atk_damage.push((target_idx, dmg));
        }

        // Apply damage to defenders
        for (idx, dmg) in &def_damage {
            let u = &mut defenders[*idx];
            let shield_absorb = dmg.min(u.shields);
            u.shields -= shield_absorb;
            let remaining = dmg - shield_absorb;
            u.hp -= remaining;
        }

        // Apply damage to attackers
        for (idx, dmg) in &atk_damage {
            let u = &mut attackers[*idx];
            let shield_absorb = dmg.min(u.shields);
            u.shields -= shield_absorb;
            let remaining = dmg - shield_absorb;
            u.hp -= remaining;
        }

        // Remove dead units
        attackers.retain(|u| u.hp > 0.0);
        defenders.retain(|u| u.hp > 0.0);
    }

    let attacker_won = !attackers.is_empty() && defenders.is_empty();

    let attacker_losses = count_losses(attacker_fleet, &attackers);
    let defender_losses = count_losses(defender_fleet, &defenders);

    // Debris = 30% of ALL destroyed ship costs
    let mut debris = destroyed_cost(&attacker_losses);
    let defender_destroyed_cost = destroyed_cost(&defender_losses);
    debris.add(&defender_destroyed_cost);
    debris.scale(0.3);

    // Loot = 50% of DEFENDER destroyed cost (only if attacker won)
    let mut loot = Resources::default();
    if attacker_won {
        loot = destroyed_cost(&defender_losses);
        loot.scale(0.5);
    }

    BattleResult {
        id: Uuid::new_v4().to_string(),
        rounds: rounds_fought,
        attacker_losses,
        defender_losses,
        attacker_won,
        loot,
        debris,
        timestamp: Utc::now().to_rfc3339(),
    }
}

// ---------------------------------------------------------------------------
// SwarmCombatEngine — SQLite persistence for fleet missions
// ---------------------------------------------------------------------------

pub struct SwarmCombatEngine {
    conn: Mutex<Connection>,
}

impl SwarmCombatEngine {
    /// Open (or create) the swarmforge database at `data_dir/swarmforge.db`.
    pub fn new(data_dir: &std::path::Path) -> Result<Self, ImpForgeError> {
        std::fs::create_dir_all(data_dir).map_err(|e| {
            ImpForgeError::filesystem("COMBAT_DIR", format!("Cannot create data dir: {e}"))
        })?;

        let db_path = data_dir.join("swarmforge.db");
        let conn = Connection::open(&db_path).map_err(|e| {
            ImpForgeError::internal("COMBAT_DB_OPEN", format!("SQLite open failed: {e}"))
        })?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;"
        ).map_err(|e| {
            ImpForgeError::internal("COMBAT_DB_PRAGMA", format!("Pragma failed: {e}"))
        })?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS fleet_missions (
                id              TEXT PRIMARY KEY,
                fleet_json      TEXT NOT NULL,
                origin_galaxy   INTEGER NOT NULL,
                origin_system   INTEGER NOT NULL,
                origin_planet   INTEGER NOT NULL,
                target_galaxy   INTEGER NOT NULL,
                target_system   INTEGER NOT NULL,
                target_planet   INTEGER NOT NULL,
                mission_type    TEXT NOT NULL,
                departure_time  TEXT NOT NULL,
                arrival_time    TEXT NOT NULL,
                return_time     TEXT,
                status          TEXT NOT NULL DEFAULT 'outbound',
                cargo_json      TEXT NOT NULL DEFAULT '{}',
                created_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS battle_reports (
                id              TEXT PRIMARY KEY,
                mission_id      TEXT,
                rounds          INTEGER NOT NULL,
                attacker_losses TEXT NOT NULL,
                defender_losses TEXT NOT NULL,
                attacker_won    INTEGER NOT NULL,
                loot_json       TEXT NOT NULL,
                debris_json     TEXT NOT NULL,
                timestamp       TEXT NOT NULL,
                created_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_missions_status ON fleet_missions(status);
            CREATE INDEX IF NOT EXISTS idx_reports_mission ON battle_reports(mission_id);

            CREATE TABLE IF NOT EXISTS colony_defenses (
                colony_id       TEXT NOT NULL,
                defense_type    TEXT NOT NULL,
                count           INTEGER NOT NULL DEFAULT 0,
                updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (colony_id, defense_type)
            );

            CREATE INDEX IF NOT EXISTS idx_defenses_colony ON colony_defenses(colony_id);"
        ).map_err(|e| {
            ImpForgeError::internal("COMBAT_DB_SCHEMA", format!("Schema creation failed: {e}"))
        })?;

        log::info!("SwarmCombat DB initialized at {}", db_path.display());
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// Dispatch a new fleet mission.
    pub fn dispatch_fleet(
        &self,
        origin: (u32, u32, u32),
        target: (u32, u32, u32),
        ships: Vec<(String, u32)>,
        mission_type: &str,
        cargo: Resources,
        speed_factor: f64,
    ) -> Result<FleetMission, ImpForgeError> {
        if ships.is_empty() || ships.iter().all(|(_, c)| *c == 0) {
            return Err(ImpForgeError::validation(
                "COMBAT_NO_SHIPS",
                "Fleet must contain at least one ship.",
            ));
        }
        if origin == target {
            return Err(ImpForgeError::validation(
                "COMBAT_SAME_COORDS",
                "Origin and target coordinates must differ.",
            ));
        }

        let mt = MissionType::from_str(mission_type);
        let travel_secs = calculate_travel_time(origin, target, &ships, speed_factor);

        let now = Utc::now();
        let departure = now.to_rfc3339();
        let arrival = (now + chrono::Duration::seconds(travel_secs as i64)).to_rfc3339();
        let return_time = if mt.returns() {
            Some((now + chrono::Duration::seconds(travel_secs as i64 * 2)).to_rfc3339())
        } else {
            None
        };

        let id = Uuid::new_v4().to_string();
        let fleet_json = serde_json::to_string(&ships).unwrap_or_default();
        let cargo_json = serde_json::to_string(&cargo).unwrap_or_default();

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("COMBAT_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.execute(
            "INSERT INTO fleet_missions
                (id, fleet_json, origin_galaxy, origin_system, origin_planet,
                 target_galaxy, target_system, target_planet, mission_type,
                 departure_time, arrival_time, return_time, status, cargo_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                id, fleet_json,
                origin.0, origin.1, origin.2,
                target.0, target.1, target.2,
                mt.as_str(),
                departure, arrival, return_time,
                FleetStatus::Outbound.as_str(),
                cargo_json,
            ],
        ).map_err(|e| {
            ImpForgeError::internal("COMBAT_INSERT", format!("Insert failed: {e}"))
        })?;

        Ok(FleetMission {
            id,
            fleet: ships,
            origin,
            target,
            mission_type: mt,
            departure_time: departure,
            arrival_time: arrival,
            return_time,
            status: FleetStatus::Outbound,
            cargo,
        })
    }

    /// Get a single fleet mission by ID.
    pub fn get_fleet_status(&self, mission_id: &str) -> Result<FleetMission, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("COMBAT_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.query_row(
            "SELECT id, fleet_json, origin_galaxy, origin_system, origin_planet,
                    target_galaxy, target_system, target_planet, mission_type,
                    departure_time, arrival_time, return_time, status, cargo_json
             FROM fleet_missions WHERE id = ?1",
            params![mission_id],
            |row| {
                Ok(Self::row_to_mission(row))
            },
        )
        .map_err(|e| {
            ImpForgeError::validation("COMBAT_NOT_FOUND", format!("Mission not found: {e}"))
        })
    }

    /// List all active (non-completed) fleet missions.
    pub fn list_fleets(&self) -> Result<Vec<FleetMission>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("COMBAT_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, fleet_json, origin_galaxy, origin_system, origin_planet,
                    target_galaxy, target_system, target_planet, mission_type,
                    departure_time, arrival_time, return_time, status, cargo_json
             FROM fleet_missions
             WHERE status != 'completed'
             ORDER BY departure_time DESC"
        ).map_err(|e| {
            ImpForgeError::internal("COMBAT_QUERY", format!("Prepare failed: {e}"))
        })?;

        let missions = stmt
            .query_map([], |row| Ok(Self::row_to_mission(row)))
            .map_err(|e| {
                ImpForgeError::internal("COMBAT_QUERY", format!("Query failed: {e}"))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(missions)
    }

    /// Recall (cancel) a fleet that is still outbound.
    pub fn recall_fleet(&self, mission_id: &str) -> Result<FleetMission, ImpForgeError> {
        let mut mission = self.get_fleet_status(mission_id)?;

        if mission.status != FleetStatus::Outbound {
            return Err(ImpForgeError::validation(
                "COMBAT_CANNOT_RECALL",
                format!("Cannot recall fleet in status '{}'.", mission.status.as_str()),
            ));
        }

        mission.status = FleetStatus::Returning;
        mission.return_time = Some(Utc::now().to_rfc3339());

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("COMBAT_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.execute(
            "UPDATE fleet_missions SET status = ?1, return_time = ?2 WHERE id = ?3",
            params![
                FleetStatus::Returning.as_str(),
                &mission.return_time,
                mission_id,
            ],
        ).map_err(|e| {
            ImpForgeError::internal("COMBAT_UPDATE", format!("Update failed: {e}"))
        })?;

        Ok(mission)
    }

    /// Save a battle report.
    pub fn save_battle_report(
        &self,
        report: &BattleResult,
        mission_id: Option<&str>,
    ) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("COMBAT_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let atk_json = serde_json::to_string(&report.attacker_losses).unwrap_or_default();
        let def_json = serde_json::to_string(&report.defender_losses).unwrap_or_default();
        let loot_json = serde_json::to_string(&report.loot).unwrap_or_default();
        let debris_json = serde_json::to_string(&report.debris).unwrap_or_default();

        conn.execute(
            "INSERT INTO battle_reports
                (id, mission_id, rounds, attacker_losses, defender_losses,
                 attacker_won, loot_json, debris_json, timestamp)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                report.id,
                mission_id,
                report.rounds,
                atk_json,
                def_json,
                report.attacker_won as i32,
                loot_json,
                debris_json,
                report.timestamp,
            ],
        ).map_err(|e| {
            ImpForgeError::internal("COMBAT_REPORT_INSERT", format!("Insert failed: {e}"))
        })?;

        Ok(())
    }

    /// Get a battle report by ID.
    pub fn get_battle_report(&self, battle_id: &str) -> Result<BattleResult, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("COMBAT_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.query_row(
            "SELECT id, rounds, attacker_losses, defender_losses,
                    attacker_won, loot_json, debris_json, timestamp
             FROM battle_reports WHERE id = ?1",
            params![battle_id],
            |row| {
                let id: String = row.get(0)?;
                let rounds: u32 = row.get(1)?;
                let atk_json: String = row.get(2)?;
                let def_json: String = row.get(3)?;
                let won: i32 = row.get(4)?;
                let loot_json: String = row.get(5)?;
                let debris_json: String = row.get(6)?;
                let timestamp: String = row.get(7)?;

                Ok(BattleResult {
                    id,
                    rounds,
                    attacker_losses: serde_json::from_str(&atk_json).unwrap_or_default(),
                    defender_losses: serde_json::from_str(&def_json).unwrap_or_default(),
                    attacker_won: won != 0,
                    loot: serde_json::from_str(&loot_json).unwrap_or_default(),
                    debris: serde_json::from_str(&debris_json).unwrap_or_default(),
                    timestamp,
                })
            },
        )
        .map_err(|e| {
            ImpForgeError::validation("COMBAT_REPORT_NOT_FOUND", format!("Report not found: {e}"))
        })
    }

    // -- helpers --

    fn row_to_mission(row: &rusqlite::Row<'_>) -> FleetMission {
        let id: String = row.get(0).unwrap_or_default();
        let fleet_json: String = row.get(1).unwrap_or_default();
        let og: u32 = row.get(2).unwrap_or(1);
        let os: u32 = row.get(3).unwrap_or(1);
        let op: u32 = row.get(4).unwrap_or(1);
        let tg: u32 = row.get(5).unwrap_or(1);
        let ts: u32 = row.get(6).unwrap_or(1);
        let tp: u32 = row.get(7).unwrap_or(1);
        let mt_str: String = row.get(8).unwrap_or_default();
        let departure: String = row.get(9).unwrap_or_default();
        let arrival: String = row.get(10).unwrap_or_default();
        let return_time: Option<String> = row.get(11).unwrap_or(None);
        let status_str: String = row.get(12).unwrap_or_default();
        let cargo_json: String = row.get(13).unwrap_or_default();

        FleetMission {
            id,
            fleet: serde_json::from_str(&fleet_json).unwrap_or_default(),
            origin: (og, os, op),
            target: (tg, ts, tp),
            mission_type: MissionType::from_str(&mt_str),
            departure_time: departure,
            arrival_time: arrival,
            return_time,
            status: FleetStatus::from_str(&status_str),
            cargo: serde_json::from_str(&cargo_json).unwrap_or_default(),
        }
    }

    /// Build (or add to) defenses on a colony.
    ///
    /// Uses `INSERT ... ON CONFLICT UPDATE` to upsert the count.
    pub fn build_defense(
        &self,
        colony_id: &str,
        def_type: &DefenseType,
        count: u32,
    ) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("COMBAT_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.execute(
            "INSERT INTO colony_defenses (colony_id, defense_type, count, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(colony_id, defense_type) DO UPDATE
             SET count = count + ?3, updated_at = datetime('now')",
            params![colony_id, def_type.as_str(), count],
        ).map_err(|e| {
            ImpForgeError::internal("DEFENSE_BUILD", format!("Build defense failed: {e}"))
        })?;

        Ok(())
    }

    /// Get all defenses for a colony.
    pub fn get_colony_defenses(
        &self,
        colony_id: &str,
    ) -> Result<Vec<(String, u32)>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("COMBAT_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let mut stmt = conn.prepare(
            "SELECT defense_type, count FROM colony_defenses
             WHERE colony_id = ?1 AND count > 0
             ORDER BY defense_type"
        ).map_err(|e| {
            ImpForgeError::internal("DEFENSE_QUERY", format!("Prepare failed: {e}"))
        })?;

        let defenses = stmt
            .query_map(params![colony_id], |row| {
                let dt: String = row.get(0)?;
                let count: u32 = row.get(1)?;
                Ok((dt, count))
            })
            .map_err(|e| {
                ImpForgeError::internal("DEFENSE_QUERY", format!("Query failed: {e}"))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(defenses)
    }
}

