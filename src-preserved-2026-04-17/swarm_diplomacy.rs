// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Diplomacy — Multi-Colony, Espionage & Trade
//!
//! Three interconnected subsystems that extend the SwarmForge game layer:
//!
//! ## Multi-Colony Management
//!
//! Players unlock additional colony slots (base 1, +1 per 500 DM) and assign
//! specializations that grant production bonuses with tradeoffs.  Resources
//! can be transferred between colonies via cargo ships.
//!
//! ## Espionage (Planted Agents + HOI4 Cipher War)
//!
//! Long-term agents are planted in enemy colonies at three cover depths
//! (Shallow, Mid, Deep).  Deeper agents gather richer intel but face higher
//! detection risk per tick.  Intel confidence decays exponentially with a
//! 7-day half-life.
//!
//! Encryption/decryption follows the Hearts of Iron IV cipher model:
//! - Encryption strength: `12000 + 4250 * upgrade_level`
//! - Decryption power: `25 + 25 * upgrade_level` (max 150)
//! - Crack chance per tick: `decryption_power / encryption_strength`
//!
//! OGame probe formula: `Effective_Probes = Probes + (Your_Level - Enemy_Level)^2`
//!
//! ## Trade System (Marketplace)
//!
//! Order-book marketplace with buy/sell orders.  Pricing follows OGame base
//! ratios (3 Metal : 2 Crystal : 1 Deuterium) adjusted by supply/demand:
//!
//! `price = base_price * (1 + (demand - supply) / (demand + supply))`
//!
//! 5% trade tax (reduced by Diplomacy tech).
//!
//! ## Persistence
//!
//! All data stored in `~/.impforge/swarm_diplomacy.db` (SQLite, WAL mode).

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

use crate::error::ImpForgeError;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_diplomacy", "Game");

// ═══════════════════════════════════════════════════════════════════════════
//  SYSTEM 1 — Multi-Colony Management
// ═══════════════════════════════════════════════════════════════════════════

/// Colony specialization — bonuses come with tradeoffs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColonySpec {
    Balanced,   // No bonuses, no penalties
    Mining,     // +30% resource production, -15% military
    Military,   // +25% unit production speed, -10% resources
    Research,   // +20% research speed, -10% resources
    Trade,      // +25% trade income, -10% military
    Fortress,   // +40% defense, -20% resources
}

impl ColonySpec {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Mining => "mining",
            Self::Military => "military",
            Self::Research => "research",
            Self::Trade => "trade",
            Self::Fortress => "fortress",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "mining" => Self::Mining,
            "military" => Self::Military,
            "research" => Self::Research,
            "trade" => Self::Trade,
            "fortress" => Self::Fortress,
            _ => Self::Balanced,
        }
    }

    /// Resource production multiplier (1.0 = baseline).
    pub fn resource_multiplier(&self) -> f64 {
        match self {
            Self::Balanced => 1.0,
            Self::Mining => 1.3,
            Self::Military => 0.9,
            Self::Research => 0.9,
            Self::Trade => 1.0,
            Self::Fortress => 0.8,
        }
    }

    /// Military / unit production speed multiplier.
    pub fn military_multiplier(&self) -> f64 {
        match self {
            Self::Balanced => 1.0,
            Self::Mining => 0.85,
            Self::Military => 1.25,
            Self::Research => 1.0,
            Self::Trade => 0.9,
            Self::Fortress => 1.0,
        }
    }

    /// Defense rating multiplier.
    pub fn defense_multiplier(&self) -> f64 {
        match self {
            Self::Fortress => 1.4,
            _ => 1.0,
        }
    }
}

/// Per-resource production rates (units per hour).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRates {
    pub metal: f64,
    pub crystal: f64,
    pub deuterium: f64,
    pub dark_matter: f64,
}

impl Default for ResourceRates {
    fn default() -> Self {
        Self {
            metal: 30.0,
            crystal: 15.0,
            deuterium: 7.5,
            dark_matter: 0.0,
        }
    }
}

/// A player-owned colony in the galaxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Colony {
    pub id: String,
    pub name: String,
    pub coord: String,
    pub faction: String,
    pub specialization: ColonySpec,
    pub level: u32,
    pub population: u32,
    pub defense_rating: f64,
    pub resource_production: ResourceRates,
    pub is_capital: bool,
}

/// Colony slot availability and unlock cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonySlots {
    pub max_slots: u32,
    pub used_slots: u32,
    pub unlock_cost_dm: u32,
    pub prestige_bonus_slots: u32,
}

// ═══════════════════════════════════════════════════════════════════════════
//  SYSTEM 2 — Espionage (Planted Agents + HOI4 Cipher War)
// ═══════════════════════════════════════════════════════════════════════════

/// How deeply embedded an agent is — determines intel quality and risk.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverDepth {
    Shallow, // Day 1-3: resource snapshots only
    Mid,     // Day 3-7: troop movements, construction queues
    Deep,    // Day 7+:  real-time everything
}

impl CoverDepth {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Shallow => "shallow",
            Self::Mid => "mid",
            Self::Deep => "deep",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "mid" => Self::Mid,
            "deep" => Self::Deep,
            _ => Self::Shallow,
        }
    }

    /// Detection risk multiplier (higher = more dangerous).
    pub fn risk_factor(&self) -> f64 {
        match self {
            Self::Shallow => 1.0,
            Self::Mid => 2.0,
            Self::Deep => 3.0,
        }
    }
}

/// Current status of a planted spy agent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Active,      // Operating normally
    Detected,    // Blown cover, captured
    Extracted,   // Safely pulled out
    DoubleAgent, // Turned by enemy — feeds disinformation
}

impl AgentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Detected => "detected",
            Self::Extracted => "extracted",
            Self::DoubleAgent => "double_agent",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "detected" => Self::Detected,
            "extracted" => Self::Extracted,
            "double_agent" => Self::DoubleAgent,
            _ => Self::Active,
        }
    }
}

/// What kind of intelligence an agent has gathered.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntelCategory {
    Resources,         // Shallow+
    FleetMovements,    // Mid+
    ConstructionQueue, // Mid+
    ResearchProgress,  // Deep
    FleetComposition,  // Deep
    DefenseLayout,     // Deep
}

impl IntelCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Resources => "resources",
            Self::FleetMovements => "fleet_movements",
            Self::ConstructionQueue => "construction_queue",
            Self::ResearchProgress => "research_progress",
            Self::FleetComposition => "fleet_composition",
            Self::DefenseLayout => "defense_layout",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "fleet_movements" => Self::FleetMovements,
            "construction_queue" => Self::ConstructionQueue,
            "research_progress" => Self::ResearchProgress,
            "fleet_composition" => Self::FleetComposition,
            "defense_layout" => Self::DefenseLayout,
            _ => Self::Resources,
        }
    }

    /// Minimum cover depth required to gather this category.
    pub fn min_depth(&self) -> CoverDepth {
        match self {
            Self::Resources => CoverDepth::Shallow,
            Self::FleetMovements | Self::ConstructionQueue => CoverDepth::Mid,
            Self::ResearchProgress | Self::FleetComposition | Self::DefenseLayout => {
                CoverDepth::Deep
            }
        }
    }
}

/// A single intel report gathered by a spy agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelReport {
    pub id: String,
    pub agent_id: String,
    pub category: IntelCategory,
    pub data: serde_json::Value,
    pub confidence: f64,
    pub timestamp: String,
}

/// A spy agent planted in an enemy colony.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpyAgent {
    pub id: String,
    pub target_colony: String,
    pub cover_depth: CoverDepth,
    pub days_embedded: u32,
    pub detection_chance_per_tick: f64,
    pub intel_gathered: Vec<IntelReport>,
    pub status: AgentStatus,
}

/// HOI4-inspired encryption/decryption state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CipherState {
    pub colony_id: String,
    pub encryption_strength: f64,
    pub decryption_power: f64,
    pub cipher_level: u32,
    pub code_cracked: bool,
}

impl CipherState {
    /// Base encryption: 12000 + 4250 per upgrade level.
    pub fn calc_encryption(level: u32) -> f64 {
        12_000.0 + 4_250.0 * level as f64
    }

    /// Decryption power: 25 + 25 per upgrade (max 150).
    pub fn calc_decryption(level: u32) -> f64 {
        (25.0 + 25.0 * level as f64).min(150.0)
    }

    /// Per-tick probability of cracking the code.
    pub fn crack_chance(&self) -> f64 {
        if self.encryption_strength <= 0.0 {
            return 1.0;
        }
        (self.decryption_power / self.encryption_strength).min(1.0)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  SYSTEM 3 — Trade System (Marketplace)
// ═══════════════════════════════════════════════════════════════════════════

/// Buy or Sell.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Buy,
    Sell,
}

impl OrderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "sell" => Self::Sell,
            _ => Self::Buy,
        }
    }
}

/// Lifecycle of a trade order.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
}

impl OrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::PartiallyFilled => "partially_filled",
            Self::Filled => "filled",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "partially_filled" => Self::PartiallyFilled,
            "filled" => Self::Filled,
            "cancelled" => Self::Cancelled,
            "expired" => Self::Expired,
            _ => Self::Open,
        }
    }
}

/// Price direction indicator.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PriceTrend {
    Rising,
    Falling,
    Stable,
}

/// A single trade order on the marketplace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeOrder {
    pub id: String,
    pub order_type: OrderType,
    pub resource: String,
    pub amount: f64,
    pub price_per_unit: f64,
    pub colony_id: String,
    pub created_at: String,
    pub expires_at: String,
    pub filled: f64,
    pub status: OrderStatus,
}

/// Aggregated market price information for a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPrices {
    pub resource: String,
    pub current_price: f64,
    pub price_24h_ago: f64,
    pub volume_24h: f64,
    pub trend: PriceTrend,
}

// ═══════════════════════════════════════════════════════════════════════════
//  Formulas
// ═══════════════════════════════════════════════════════════════════════════

/// Detection chance per tick for a planted agent.
///
/// `0.01 * days_embedded * (1 + cover_depth_factor)`
/// where Shallow=1, Mid=2, Deep=3.
pub fn detection_chance(days_embedded: u32, cover_depth: CoverDepth) -> f64 {
    let factor = cover_depth.risk_factor();
    (0.01 * days_embedded as f64 * (1.0 + factor)).min(1.0)
}

/// Intel confidence decay over time.
///
/// `confidence = initial * e^(-t/7)` (7-day half-life).
pub fn intel_confidence_decay(initial: f64, days_elapsed: f64) -> f64 {
    (initial * (-days_elapsed / 7.0_f64).exp()).max(0.0)
}

/// OGame-style effective probe count.
///
/// `Effective = Probes + (Your_Level - Enemy_Level)^2`
pub fn effective_probes(probes: u32, your_level: u32, enemy_level: u32) -> u32 {
    let diff = your_level as i64 - enemy_level as i64;
    let bonus = (diff * diff) as u32;
    probes.saturating_add(bonus)
}

/// OGame base exchange ratios: 3 Metal = 2 Crystal = 1 Deuterium.
///
/// Returns the base price in a normalised unit for the named resource.
pub fn base_resource_price(resource: &str) -> f64 {
    match resource {
        "metal" => 1.0,
        "crystal" => 1.5,
        "deuterium" => 3.0,
        "dark_matter" => 10.0,
        _ => 1.0,
    }
}

/// Supply/demand dynamic pricing.
///
/// `price = base_price * (1 + (demand - supply) / (demand + supply))`
pub fn dynamic_price(base_price: f64, demand: f64, supply: f64) -> f64 {
    let total = demand + supply;
    if total <= 0.0 {
        return base_price;
    }
    base_price * (1.0 + (demand - supply) / total)
}

/// Trade tax (default 5%).
pub const TRADE_TAX_RATE: f64 = 0.05;

/// Calculate trade tax amount.
pub fn trade_tax(total_value: f64, diplomacy_tech_level: u32) -> f64 {
    // Each diplomacy tech level reduces tax by 0.5%, minimum 1%
    let rate = (TRADE_TAX_RATE - 0.005 * diplomacy_tech_level as f64).max(0.01);
    total_value * rate
}

// ═══════════════════════════════════════════════════════════════════════════
//  Engine — SQLite persistence
// ═══════════════════════════════════════════════════════════════════════════

/// Central engine owning the SQLite connection for all three subsystems.
pub struct SwarmDiplomacyEngine {
    conn: Mutex<Connection>,
}

impl SwarmDiplomacyEngine {
    /// Open (or create) the diplomacy database at `data_dir/swarm_diplomacy.db`.
    pub fn new(data_dir: &std::path::Path) -> Result<Self, ImpForgeError> {
        std::fs::create_dir_all(data_dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIPLOMACY_DIR",
                format!("Cannot create data dir: {e}"),
            )
        })?;

        let db_path = data_dir.join("swarm_diplomacy.db");
        let conn = Connection::open(&db_path).map_err(|e| {
            ImpForgeError::internal(
                "DIPLOMACY_DB_OPEN",
                format!("SQLite open failed: {e}"),
            )
        })?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;",
        )
        .map_err(|e| {
            ImpForgeError::internal(
                "DIPLOMACY_DB_PRAGMA",
                format!("Pragma failed: {e}"),
            )
        })?;

        Self::create_tables(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn create_tables(conn: &Connection) -> Result<(), ImpForgeError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS colonies (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                coord           TEXT NOT NULL,
                faction         TEXT NOT NULL DEFAULT 'player',
                specialization  TEXT NOT NULL DEFAULT 'balanced',
                level           INTEGER NOT NULL DEFAULT 1,
                population      INTEGER NOT NULL DEFAULT 100,
                defense_rating  REAL NOT NULL DEFAULT 10.0,
                metal_rate      REAL NOT NULL DEFAULT 30.0,
                crystal_rate    REAL NOT NULL DEFAULT 15.0,
                deuterium_rate  REAL NOT NULL DEFAULT 7.5,
                dm_rate         REAL NOT NULL DEFAULT 0.0,
                is_capital      INTEGER NOT NULL DEFAULT 0,
                created_at      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS colony_slots (
                id              INTEGER PRIMARY KEY CHECK (id = 1),
                max_slots       INTEGER NOT NULL DEFAULT 1,
                used_slots      INTEGER NOT NULL DEFAULT 0,
                unlock_cost_dm  INTEGER NOT NULL DEFAULT 500,
                prestige_bonus  INTEGER NOT NULL DEFAULT 0
            );

            INSERT OR IGNORE INTO colony_slots (id) VALUES (1);

            CREATE TABLE IF NOT EXISTS spy_agents (
                id              TEXT PRIMARY KEY,
                target_colony   TEXT NOT NULL,
                cover_depth     TEXT NOT NULL DEFAULT 'shallow',
                days_embedded   INTEGER NOT NULL DEFAULT 0,
                status          TEXT NOT NULL DEFAULT 'active',
                created_at      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS intel_reports (
                id              TEXT PRIMARY KEY,
                agent_id        TEXT NOT NULL REFERENCES spy_agents(id) ON DELETE CASCADE,
                category        TEXT NOT NULL,
                data_json       TEXT NOT NULL DEFAULT '{}',
                confidence      REAL NOT NULL DEFAULT 1.0,
                created_at      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS cipher_states (
                colony_id       TEXT PRIMARY KEY,
                encryption_lvl  INTEGER NOT NULL DEFAULT 0,
                decryption_lvl  INTEGER NOT NULL DEFAULT 0,
                code_cracked    INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS trade_orders (
                id              TEXT PRIMARY KEY,
                order_type      TEXT NOT NULL,
                resource        TEXT NOT NULL,
                amount          REAL NOT NULL,
                price_per_unit  REAL NOT NULL,
                colony_id       TEXT NOT NULL,
                created_at      TEXT NOT NULL,
                expires_at      TEXT NOT NULL,
                filled          REAL NOT NULL DEFAULT 0.0,
                status          TEXT NOT NULL DEFAULT 'open'
            );

            CREATE INDEX IF NOT EXISTS idx_trade_status ON trade_orders(status);
            CREATE INDEX IF NOT EXISTS idx_trade_resource ON trade_orders(resource);
            CREATE INDEX IF NOT EXISTS idx_spy_status ON spy_agents(status);
            CREATE INDEX IF NOT EXISTS idx_intel_agent ON intel_reports(agent_id);",
        )
        .map_err(|e| {
            ImpForgeError::internal(
                "DIPLOMACY_DB_SCHEMA",
                format!("Table creation failed: {e}"),
            )
        })?;
        Ok(())
    }

    // ───────────────────────────────────────────────────────────────────────
    //  Colony operations
    // ───────────────────────────────────────────────────────────────────────

    pub fn colony_create(
        &self,
        name: &str,
        coord: &str,
        faction: &str,
        spec: ColonySpec,
    ) -> Result<Colony, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        // Check slot availability
        let (max, used): (u32, u32) = conn
            .query_row(
                "SELECT max_slots + prestige_bonus, used_slots FROM colony_slots WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| {
                ImpForgeError::internal("COLONY_SLOTS_QUERY", format!("Slot query failed: {e}"))
            })?;

        if used >= max {
            return Err(ImpForgeError::validation(
                "NO_COLONY_SLOTS",
                format!("All {max} colony slots are occupied. Unlock more with Dark Matter."),
            ));
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let rates = ResourceRates::default();
        let is_capital = used == 0; // First colony is always capital

        conn.execute(
            "INSERT INTO colonies (id, name, coord, faction, specialization, level, population,
                defense_rating, metal_rate, crystal_rate, deuterium_rate, dm_rate, is_capital, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, 100, 10.0, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id,
                name,
                coord,
                faction,
                spec.as_str(),
                rates.metal * spec.resource_multiplier(),
                rates.crystal * spec.resource_multiplier(),
                rates.deuterium * spec.resource_multiplier(),
                rates.dark_matter,
                is_capital as i32,
                now,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("COLONY_INSERT", format!("Insert failed: {e}"))
        })?;

        conn.execute("UPDATE colony_slots SET used_slots = used_slots + 1 WHERE id = 1", [])
            .map_err(|e| {
                ImpForgeError::internal("COLONY_SLOT_INC", format!("Slot update failed: {e}"))
            })?;

        Ok(Colony {
            id,
            name: name.to_string(),
            coord: coord.to_string(),
            faction: faction.to_string(),
            specialization: spec,
            level: 1,
            population: 100,
            defense_rating: 10.0 * spec.defense_multiplier(),
            resource_production: ResourceRates {
                metal: rates.metal * spec.resource_multiplier(),
                crystal: rates.crystal * spec.resource_multiplier(),
                deuterium: rates.deuterium * spec.resource_multiplier(),
                dark_matter: rates.dark_matter,
            },
            is_capital,
        })
    }

    pub fn colony_list(&self) -> Result<Vec<Colony>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, coord, faction, specialization, level, population,
                        defense_rating, metal_rate, crystal_rate, deuterium_rate, dm_rate, is_capital
                 FROM colonies ORDER BY is_capital DESC, created_at ASC",
            )
            .map_err(|e| ImpForgeError::internal("COLONY_LIST", format!("Prepare failed: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                let spec_str: String = row.get(4)?;
                let spec = ColonySpec::from_str(&spec_str);
                Ok(Colony {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    coord: row.get(2)?,
                    faction: row.get(3)?,
                    specialization: spec,
                    level: row.get(5)?,
                    population: row.get(6)?,
                    defense_rating: row.get(7)?,
                    resource_production: ResourceRates {
                        metal: row.get(8)?,
                        crystal: row.get(9)?,
                        deuterium: row.get(10)?,
                        dark_matter: row.get(11)?,
                    },
                    is_capital: row.get::<_, i32>(12)? != 0,
                })
            })
            .map_err(|e| ImpForgeError::internal("COLONY_LIST", format!("Query failed: {e}")))?;

        let mut colonies = Vec::new();
        for row in rows {
            colonies.push(row.map_err(|e| {
                ImpForgeError::internal("COLONY_LIST_ROW", format!("Row error: {e}"))
            })?);
        }
        Ok(colonies)
    }

    pub fn colony_transfer_resources(
        &self,
        from_id: &str,
        to_id: &str,
        metal: f64,
        crystal: f64,
        deuterium: f64,
    ) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        // Verify both colonies exist
        let from_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM colonies WHERE id = ?1",
                params![from_id],
                |row| row.get(0),
            )
            .unwrap_or(false);
        let to_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM colonies WHERE id = ?1",
                params![to_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !from_exists {
            return Err(ImpForgeError::validation(
                "COLONY_NOT_FOUND",
                format!("Source colony {from_id} not found"),
            ));
        }
        if !to_exists {
            return Err(ImpForgeError::validation(
                "COLONY_NOT_FOUND",
                format!("Target colony {to_id} not found"),
            ));
        }
        if metal < 0.0 || crystal < 0.0 || deuterium < 0.0 {
            return Err(ImpForgeError::validation(
                "NEGATIVE_TRANSFER",
                "Transfer amounts must be non-negative",
            ));
        }

        // In a full implementation, this would check cargo ship capacity
        // and create a timed transfer mission.  For now, record the intent.
        log::info!(
            "Resource transfer: {} -> {} | M:{metal} C:{crystal} D:{deuterium}",
            from_id,
            to_id
        );

        Ok(())
    }

    pub fn colony_set_specialization(
        &self,
        colony_id: &str,
        spec: ColonySpec,
    ) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let base = ResourceRates::default();
        let updated = conn
            .execute(
                "UPDATE colonies SET specialization = ?1,
                    metal_rate = ?2, crystal_rate = ?3, deuterium_rate = ?4
                 WHERE id = ?5",
                params![
                    spec.as_str(),
                    base.metal * spec.resource_multiplier(),
                    base.crystal * spec.resource_multiplier(),
                    base.deuterium * spec.resource_multiplier(),
                    colony_id,
                ],
            )
            .map_err(|e| {
                ImpForgeError::internal("COLONY_SPEC_UPDATE", format!("Update failed: {e}"))
            })?;

        if updated == 0 {
            return Err(ImpForgeError::validation(
                "COLONY_NOT_FOUND",
                format!("Colony {colony_id} not found"),
            ));
        }
        Ok(())
    }

    pub fn colony_slots_available(&self) -> Result<ColonySlots, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        conn.query_row(
            "SELECT max_slots, used_slots, unlock_cost_dm, prestige_bonus FROM colony_slots WHERE id = 1",
            [],
            |row| {
                Ok(ColonySlots {
                    max_slots: row.get(0)?,
                    used_slots: row.get(1)?,
                    unlock_cost_dm: row.get(2)?,
                    prestige_bonus_slots: row.get(3)?,
                })
            },
        )
        .map_err(|e| {
            ImpForgeError::internal("COLONY_SLOTS_QUERY", format!("Query failed: {e}"))
        })
    }

    // ───────────────────────────────────────────────────────────────────────
    //  Espionage operations
    // ───────────────────────────────────────────────────────────────────────

    pub fn espionage_plant_agent(
        &self,
        target_colony: &str,
        cover_depth: CoverDepth,
    ) -> Result<SpyAgent, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO spy_agents (id, target_colony, cover_depth, days_embedded, status, created_at)
             VALUES (?1, ?2, ?3, 0, 'active', ?4)",
            params![id, target_colony, cover_depth.as_str(), now],
        )
        .map_err(|e| {
            ImpForgeError::internal("SPY_INSERT", format!("Insert failed: {e}"))
        })?;

        Ok(SpyAgent {
            id,
            target_colony: target_colony.to_string(),
            cover_depth,
            days_embedded: 0,
            detection_chance_per_tick: detection_chance(0, cover_depth),
            intel_gathered: Vec::new(),
            status: AgentStatus::Active,
        })
    }

    pub fn espionage_gather_intel(
        &self,
        agent_id: &str,
    ) -> Result<IntelReport, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        // Fetch the agent
        let (target, depth_str, days, status_str): (String, String, u32, String) = conn
            .query_row(
                "SELECT target_colony, cover_depth, days_embedded, status FROM spy_agents WHERE id = ?1",
                params![agent_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|e| {
                ImpForgeError::validation("AGENT_NOT_FOUND", format!("Agent not found: {e}"))
            })?;

        let status = AgentStatus::from_str(&status_str);
        if status != AgentStatus::Active {
            return Err(ImpForgeError::validation(
                "AGENT_INACTIVE",
                format!("Agent is {}, cannot gather intel", status.as_str()),
            ));
        }

        let depth = CoverDepth::from_str(&depth_str);

        // Determine intel category based on depth
        let category = match depth {
            CoverDepth::Shallow => IntelCategory::Resources,
            CoverDepth::Mid => {
                if days % 2 == 0 {
                    IntelCategory::FleetMovements
                } else {
                    IntelCategory::ConstructionQueue
                }
            }
            CoverDepth::Deep => {
                match days % 3 {
                    0 => IntelCategory::ResearchProgress,
                    1 => IntelCategory::FleetComposition,
                    _ => IntelCategory::DefenseLayout,
                }
            }
        };

        let report_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let data = serde_json::json!({
            "target": target,
            "category": category.as_str(),
            "depth": depth.as_str(),
            "days_embedded": days,
        });

        conn.execute(
            "INSERT INTO intel_reports (id, agent_id, category, data_json, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, 1.0, ?5)",
            params![report_id, agent_id, category.as_str(), data.to_string(), now],
        )
        .map_err(|e| {
            ImpForgeError::internal("INTEL_INSERT", format!("Insert failed: {e}"))
        })?;

        // Increment days_embedded
        conn.execute(
            "UPDATE spy_agents SET days_embedded = days_embedded + 1 WHERE id = ?1",
            params![agent_id],
        )
        .map_err(|e| {
            ImpForgeError::internal("SPY_DAY_INC", format!("Day increment failed: {e}"))
        })?;

        Ok(IntelReport {
            id: report_id,
            agent_id: agent_id.to_string(),
            category,
            data,
            confidence: 1.0,
            timestamp: now,
        })
    }

    /// Run detection check. Returns `true` if the agent was detected.
    pub fn espionage_detect_check(&self, agent_id: &str) -> Result<bool, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let (depth_str, days, status_str): (String, u32, String) = conn
            .query_row(
                "SELECT cover_depth, days_embedded, status FROM spy_agents WHERE id = ?1",
                params![agent_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| {
                ImpForgeError::validation("AGENT_NOT_FOUND", format!("Agent not found: {e}"))
            })?;

        if AgentStatus::from_str(&status_str) != AgentStatus::Active {
            return Ok(false); // Already inactive, no detection needed
        }

        let depth = CoverDepth::from_str(&depth_str);
        let chance = detection_chance(days, depth);

        // Deterministic check based on agent state (no RNG in game logic for reproducibility)
        // Use a hash of agent_id + days as a pseudo-random value
        let hash = simple_hash(agent_id, days);
        let roll = (hash % 10000) as f64 / 10000.0;
        let detected = roll < chance;

        if detected {
            conn.execute(
                "UPDATE spy_agents SET status = 'detected' WHERE id = ?1",
                params![agent_id],
            )
            .map_err(|e| {
                ImpForgeError::internal("SPY_DETECT", format!("Status update failed: {e}"))
            })?;
        }

        Ok(detected)
    }

    /// Find enemy agents in one of your colonies.
    pub fn espionage_counter_intel(
        &self,
        colony_id: &str,
    ) -> Result<Vec<SpyAgent>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, target_colony, cover_depth, days_embedded, status
                 FROM spy_agents WHERE target_colony = ?1 AND status = 'active'",
            )
            .map_err(|e| {
                ImpForgeError::internal("COUNTER_INTEL", format!("Prepare failed: {e}"))
            })?;

        let rows = stmt
            .query_map(params![colony_id], |row| {
                let depth_str: String = row.get(2)?;
                let status_str: String = row.get(4)?;
                Ok(SpyAgent {
                    id: row.get(0)?,
                    target_colony: row.get(1)?,
                    cover_depth: CoverDepth::from_str(&depth_str),
                    days_embedded: row.get(3)?,
                    detection_chance_per_tick: detection_chance(
                        row.get(3)?,
                        CoverDepth::from_str(&depth_str),
                    ),
                    intel_gathered: Vec::new(),
                    status: AgentStatus::from_str(&status_str),
                })
            })
            .map_err(|e| {
                ImpForgeError::internal("COUNTER_INTEL", format!("Query failed: {e}"))
            })?;

        let mut agents = Vec::new();
        for row in rows {
            agents.push(row.map_err(|e| {
                ImpForgeError::internal("COUNTER_INTEL_ROW", format!("Row error: {e}"))
            })?);
        }
        Ok(agents)
    }

    pub fn espionage_extract_agent(&self, agent_id: &str) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let updated = conn
            .execute(
                "UPDATE spy_agents SET status = 'extracted' WHERE id = ?1 AND status = 'active'",
                params![agent_id],
            )
            .map_err(|e| {
                ImpForgeError::internal("SPY_EXTRACT", format!("Update failed: {e}"))
            })?;

        if updated == 0 {
            return Err(ImpForgeError::validation(
                "AGENT_NOT_ACTIVE",
                "Agent is not active or does not exist",
            ));
        }
        Ok(())
    }

    // ───────────────────────────────────────────────────────────────────────
    //  Trade operations
    // ───────────────────────────────────────────────────────────────────────

    pub fn trade_create_order(
        &self,
        order_type: OrderType,
        resource: &str,
        amount: f64,
        price_per_unit: f64,
        colony_id: &str,
    ) -> Result<TradeOrder, ImpForgeError> {
        if amount <= 0.0 {
            return Err(ImpForgeError::validation(
                "INVALID_AMOUNT",
                "Trade amount must be positive",
            ));
        }
        if price_per_unit <= 0.0 {
            return Err(ImpForgeError::validation(
                "INVALID_PRICE",
                "Price per unit must be positive",
            ));
        }

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let created_at = now.to_rfc3339();
        let expires_at = (now + chrono::Duration::hours(24)).to_rfc3339();

        conn.execute(
            "INSERT INTO trade_orders (id, order_type, resource, amount, price_per_unit,
                colony_id, created_at, expires_at, filled, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0.0, 'open')",
            params![
                id,
                order_type.as_str(),
                resource,
                amount,
                price_per_unit,
                colony_id,
                created_at,
                expires_at,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("TRADE_INSERT", format!("Insert failed: {e}"))
        })?;

        Ok(TradeOrder {
            id,
            order_type,
            resource: resource.to_string(),
            amount,
            price_per_unit,
            colony_id: colony_id.to_string(),
            created_at,
            expires_at,
            filled: 0.0,
            status: OrderStatus::Open,
        })
    }

    pub fn trade_cancel_order(&self, order_id: &str) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let updated = conn
            .execute(
                "UPDATE trade_orders SET status = 'cancelled' WHERE id = ?1 AND status IN ('open', 'partially_filled')",
                params![order_id],
            )
            .map_err(|e| {
                ImpForgeError::internal("TRADE_CANCEL", format!("Update failed: {e}"))
            })?;

        if updated == 0 {
            return Err(ImpForgeError::validation(
                "ORDER_NOT_CANCELLABLE",
                "Order not found or already completed/cancelled",
            ));
        }
        Ok(())
    }

    pub fn trade_market_prices(&self) -> Result<Vec<MarketPrices>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let resources = ["metal", "crystal", "deuterium", "dark_matter"];
        let mut prices = Vec::with_capacity(resources.len());

        for resource in &resources {
            // Calculate supply (sum of sell order amounts) and demand (sum of buy order amounts)
            let supply: f64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(amount - filled), 0.0) FROM trade_orders
                     WHERE resource = ?1 AND order_type = 'sell' AND status IN ('open', 'partially_filled')",
                    params![resource],
                    |row| row.get(0),
                )
                .unwrap_or(0.0);

            let demand: f64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(amount - filled), 0.0) FROM trade_orders
                     WHERE resource = ?1 AND order_type = 'buy' AND status IN ('open', 'partially_filled')",
                    params![resource],
                    |row| row.get(0),
                )
                .unwrap_or(0.0);

            let base = base_resource_price(resource);
            let current = dynamic_price(base, demand, supply);

            // 24h volume: total filled in last 24 hours
            let volume: f64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(filled), 0.0) FROM trade_orders
                     WHERE resource = ?1 AND created_at >= datetime('now', '-1 day')",
                    params![resource],
                    |row| row.get(0),
                )
                .unwrap_or(0.0);

            let trend = if demand > supply * 1.1 {
                PriceTrend::Rising
            } else if supply > demand * 1.1 {
                PriceTrend::Falling
            } else {
                PriceTrend::Stable
            };

            prices.push(MarketPrices {
                resource: resource.to_string(),
                current_price: current,
                price_24h_ago: base, // Simplified: use base as historical
                volume_24h: volume,
                trend,
            });
        }

        Ok(prices)
    }

    pub fn trade_fill_order(
        &self,
        order_id: &str,
        fill_amount: f64,
    ) -> Result<(), ImpForgeError> {
        if fill_amount <= 0.0 {
            return Err(ImpForgeError::validation(
                "INVALID_FILL",
                "Fill amount must be positive",
            ));
        }

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let (amount, filled, status_str): (f64, f64, String) = conn
            .query_row(
                "SELECT amount, filled, status FROM trade_orders WHERE id = ?1",
                params![order_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| {
                ImpForgeError::validation("ORDER_NOT_FOUND", format!("Order not found: {e}"))
            })?;

        let status = OrderStatus::from_str(&status_str);
        if status != OrderStatus::Open && status != OrderStatus::PartiallyFilled {
            return Err(ImpForgeError::validation(
                "ORDER_NOT_FILLABLE",
                format!("Order status is {}, cannot fill", status.as_str()),
            ));
        }

        let remaining = amount - filled;
        if fill_amount > remaining {
            return Err(ImpForgeError::validation(
                "OVERFILL",
                format!("Cannot fill {fill_amount}, only {remaining} remaining"),
            ));
        }

        let new_filled = filled + fill_amount;
        let new_status = if (new_filled - amount).abs() < f64::EPSILON {
            OrderStatus::Filled
        } else {
            OrderStatus::PartiallyFilled
        };

        conn.execute(
            "UPDATE trade_orders SET filled = ?1, status = ?2 WHERE id = ?3",
            params![new_filled, new_status.as_str(), order_id],
        )
        .map_err(|e| {
            ImpForgeError::internal("TRADE_FILL", format!("Update failed: {e}"))
        })?;

        Ok(())
    }

    pub fn trade_history(
        &self,
        colony_id: &str,
        limit: u32,
    ) -> Result<Vec<TradeOrder>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("LOCK", format!("Mutex poisoned: {e}"))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, order_type, resource, amount, price_per_unit,
                        colony_id, created_at, expires_at, filled, status
                 FROM trade_orders WHERE colony_id = ?1
                 ORDER BY created_at DESC LIMIT ?2",
            )
            .map_err(|e| {
                ImpForgeError::internal("TRADE_HISTORY", format!("Prepare failed: {e}"))
            })?;

        let rows = stmt
            .query_map(params![colony_id, limit], |row| {
                let ot_str: String = row.get(1)?;
                let st_str: String = row.get(9)?;
                Ok(TradeOrder {
                    id: row.get(0)?,
                    order_type: OrderType::from_str(&ot_str),
                    resource: row.get(2)?,
                    amount: row.get(3)?,
                    price_per_unit: row.get(4)?,
                    colony_id: row.get(5)?,
                    created_at: row.get(6)?,
                    expires_at: row.get(7)?,
                    filled: row.get(8)?,
                    status: OrderStatus::from_str(&st_str),
                })
            })
            .map_err(|e| {
                ImpForgeError::internal("TRADE_HISTORY", format!("Query failed: {e}"))
            })?;

        let mut orders = Vec::new();
        for row in rows {
            orders.push(row.map_err(|e| {
                ImpForgeError::internal("TRADE_HISTORY_ROW", format!("Row error: {e}"))
            })?);
        }
        Ok(orders)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Utilities
// ═══════════════════════════════════════════════════════════════════════════

/// Simple deterministic hash for reproducible detection rolls.
fn simple_hash(agent_id: &str, days: u32) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV offset basis
    for b in agent_id.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3); // FNV prime
    }
    h ^= days as u64;
    h = h.wrapping_mul(0x0100_0000_01b3);
    h
}

// ═══════════════════════════════════════════════════════════════════════════
//  Tauri Commands — 15 total (5 Colony + 5 Espionage + 5 Trade)
// ═══════════════════════════════════════════════════════════════════════════

// ─── Colony Commands (5) ─────────────────────────────────────────────────

#[tauri::command]
pub async fn colony_create(
    name: String,
    coord: String,
    faction: String,
    spec: String,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<Colony, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_diplomacy", "game_diplomacy", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_diplomacy", "game_diplomacy");
    crate::synapse_fabric::synapse_session_push("swarm_diplomacy", "game_diplomacy", "colony_create called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_diplomacy", "info", "swarm_diplomacy active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_diplomacy", "negotiate", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"op": "colony_create"}));
    let specialization = ColonySpec::from_str(&spec);
    engine.colony_create(&name, &coord, &faction, specialization)
}

#[tauri::command]
pub async fn colony_list(
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<Vec<Colony>, ImpForgeError> {
    engine.colony_list()
}

#[tauri::command]
pub async fn colony_transfer(
    from_id: String,
    to_id: String,
    metal: f64,
    crystal: f64,
    deuterium: f64,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<(), ImpForgeError> {
    engine.colony_transfer_resources(&from_id, &to_id, metal, crystal, deuterium)
}

#[tauri::command]
pub async fn colony_set_spec(
    colony_id: String,
    spec: String,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<(), ImpForgeError> {
    let specialization = ColonySpec::from_str(&spec);
    engine.colony_set_specialization(&colony_id, specialization)
}

#[tauri::command]
pub async fn colony_slots(
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<ColonySlots, ImpForgeError> {
    engine.colony_slots_available()
}

// ─── Espionage Commands (5) ─────────────────────────────────────────────

#[tauri::command]
pub async fn espionage_plant(
    target_colony: String,
    cover_depth: String,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<SpyAgent, ImpForgeError> {
    let depth = CoverDepth::from_str(&cover_depth);
    engine.espionage_plant_agent(&target_colony, depth)
}

#[tauri::command]
pub async fn espionage_intel(
    agent_id: String,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<IntelReport, ImpForgeError> {
    engine.espionage_gather_intel(&agent_id)
}

#[tauri::command]
pub async fn espionage_detect(
    agent_id: String,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<bool, ImpForgeError> {
    engine.espionage_detect_check(&agent_id)
}

#[tauri::command]
pub async fn espionage_counter(
    colony_id: String,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<Vec<SpyAgent>, ImpForgeError> {
    engine.espionage_counter_intel(&colony_id)
}

#[tauri::command]
pub async fn espionage_extract(
    agent_id: String,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<(), ImpForgeError> {
    engine.espionage_extract_agent(&agent_id)
}

// ─── Trade Commands (5) ─────────────────────────────────────────────────

#[tauri::command]
pub async fn trade_create_order(
    order_type: String,
    resource: String,
    amount: f64,
    price: f64,
    colony_id: String,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<TradeOrder, ImpForgeError> {
    let ot = OrderType::from_str(&order_type);
    engine.trade_create_order(ot, &resource, amount, price, &colony_id)
}

#[tauri::command]
pub async fn trade_cancel_order(
    order_id: String,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<(), ImpForgeError> {
    engine.trade_cancel_order(&order_id)
}

#[tauri::command]
pub async fn trade_prices(
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<Vec<MarketPrices>, ImpForgeError> {
    engine.trade_market_prices()
}

#[tauri::command]
pub async fn trade_fill(
    order_id: String,
    amount: f64,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<(), ImpForgeError> {
    engine.trade_fill_order(&order_id, amount)
}

#[tauri::command]
pub async fn trade_history(
    colony_id: String,
    limit: Option<u32>,
    engine: tauri::State<'_, SwarmDiplomacyEngine>,
) -> Result<Vec<TradeOrder>, ImpForgeError> {
    engine.trade_history(&colony_id, limit.unwrap_or(50))
}

// ═══════════════════════════════════════════════════════════════════════════
//  Additional Tauri Commands — wiring internal helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Get colony specialization info including all multipliers.
#[tauri::command]
pub async fn diplomacy_colony_spec_info(
    spec: String,
) -> Result<serde_json::Value, ImpForgeError> {
    let cs = ColonySpec::from_str(&spec);
    Ok(serde_json::json!({
        "spec": cs.as_str(),
        "resource_multiplier": cs.resource_multiplier(),
        "military_multiplier": cs.military_multiplier(),
    }))
}

/// Get intel category info including required cover depth.
#[tauri::command]
pub async fn diplomacy_intel_category_info(
    category: String,
) -> Result<serde_json::Value, ImpForgeError> {
    let ic = IntelCategory::from_str(&category);
    let depth = ic.min_depth();
    Ok(serde_json::json!({
        "category": ic.as_str(),
        "min_depth": format!("{:?}", depth),
    }))
}

/// Calculate cipher encryption/decryption at given levels.
#[tauri::command]
pub async fn diplomacy_cipher_calc(
    colony_id: String,
    cipher_level: u32,
    enemy_cipher_level: u32,
) -> Result<serde_json::Value, ImpForgeError> {
    let enc = CipherState::calc_encryption(cipher_level);
    let dec = CipherState::calc_decryption(enemy_cipher_level);
    let state = CipherState {
        colony_id,
        encryption_strength: enc,
        decryption_power: dec,
        cipher_level,
        code_cracked: false,
    };
    Ok(serde_json::json!({
        "encryption_strength": enc,
        "decryption_power": dec,
        "crack_chance": state.crack_chance(),
    }))
}

/// Calculate intel confidence after some time has passed.
#[tauri::command]
pub async fn diplomacy_intel_decay(
    initial_confidence: f64,
    days_elapsed: f64,
) -> Result<serde_json::Value, ImpForgeError> {
    let decayed = intel_confidence_decay(initial_confidence, days_elapsed);
    Ok(serde_json::json!({
        "initial": initial_confidence,
        "days_elapsed": days_elapsed,
        "remaining": decayed,
    }))
}

/// Calculate effective probe count for espionage.
#[tauri::command]
pub async fn diplomacy_effective_probes(
    probes: u32,
    your_level: u32,
    enemy_level: u32,
) -> Result<serde_json::Value, ImpForgeError> {
    let eff = effective_probes(probes, your_level, enemy_level);
    Ok(serde_json::json!({
        "effective_probes": eff,
    }))
}

/// Calculate trade tax for a transaction.
#[tauri::command]
pub async fn diplomacy_trade_tax(
    total_value: f64,
    diplomacy_tech_level: u32,
) -> Result<serde_json::Value, ImpForgeError> {
    let tax = trade_tax(total_value, diplomacy_tech_level);
    Ok(serde_json::json!({
        "total_value": total_value,
        "tax_rate": TRADE_TAX_RATE,
        "tax_amount": tax,
        "net_value": total_value - tax,
    }))
}

// ═══════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;


    fn test_engine() -> SwarmDiplomacyEngine {
        let dir = tempfile::tempdir().expect("tempdir");
        SwarmDiplomacyEngine::new(dir.path()).expect("engine init")
    }

    // ─── Formula tests ──────────────────────────────────────────────────

    #[test]
    fn test_detection_chance_shallow_day0() {
        let chance = detection_chance(0, CoverDepth::Shallow);
        assert!((chance - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_detection_chance_deep_day10() {
        // 0.01 * 10 * (1 + 3) = 0.4
        let chance = detection_chance(10, CoverDepth::Deep);
        assert!((chance - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn test_detection_chance_capped_at_1() {
        let chance = detection_chance(100, CoverDepth::Deep);
        assert!((chance - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_intel_confidence_decay_day0() {
        let c = intel_confidence_decay(1.0, 0.0);
        assert!((c - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_intel_confidence_decay_day7() {
        // e^(-1) ~ 0.3679
        let c = intel_confidence_decay(1.0, 7.0);
        assert!((c - 0.3679).abs() < 0.001);
    }

    #[test]
    fn test_intel_confidence_decay_day14() {
        // e^(-2) ~ 0.1353
        let c = intel_confidence_decay(1.0, 14.0);
        assert!((c - 0.1353).abs() < 0.001);
    }

    #[test]
    fn test_effective_probes_equal_levels() {
        assert_eq!(effective_probes(5, 10, 10), 5);
    }

    #[test]
    fn test_effective_probes_higher_level() {
        // diff = 3, bonus = 9
        assert_eq!(effective_probes(5, 13, 10), 14);
    }

    #[test]
    fn test_effective_probes_lower_level() {
        // diff = -2, bonus = 4
        assert_eq!(effective_probes(5, 8, 10), 9);
    }

    #[test]
    fn test_base_resource_prices() {
        assert!((base_resource_price("metal") - 1.0).abs() < f64::EPSILON);
        assert!((base_resource_price("crystal") - 1.5).abs() < f64::EPSILON);
        assert!((base_resource_price("deuterium") - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dynamic_price_balanced() {
        let p = dynamic_price(1.0, 50.0, 50.0);
        assert!((p - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dynamic_price_high_demand() {
        // demand=80, supply=20 -> factor = (80-20)/(80+20) = 0.6
        let p = dynamic_price(1.0, 80.0, 20.0);
        assert!((p - 1.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dynamic_price_zero_total() {
        let p = dynamic_price(2.0, 0.0, 0.0);
        assert!((p - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_trade_tax_default() {
        let tax = trade_tax(1000.0, 0);
        assert!((tax - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_trade_tax_with_tech() {
        // Level 4: rate = 0.05 - 0.005*4 = 0.03
        let tax = trade_tax(1000.0, 4);
        assert!((tax - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_trade_tax_min_1_percent() {
        // Level 100: rate capped at 1%
        let tax = trade_tax(1000.0, 100);
        assert!((tax - 10.0).abs() < f64::EPSILON);
    }

    // ─── Cipher tests ───────────────────────────────────────────────────

    #[test]
    fn test_cipher_encryption_base() {
        assert!((CipherState::calc_encryption(0) - 12000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cipher_encryption_level3() {
        assert!((CipherState::calc_encryption(3) - 24750.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cipher_decryption_base() {
        assert!((CipherState::calc_decryption(0) - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cipher_decryption_max() {
        assert!((CipherState::calc_decryption(10) - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cipher_crack_chance() {
        let state = CipherState {
            colony_id: "test".to_string(),
            encryption_strength: 12000.0,
            decryption_power: 25.0,
            cipher_level: 0,
            code_cracked: false,
        };
        let chance = state.crack_chance();
        assert!((chance - 25.0 / 12000.0).abs() < f64::EPSILON);
    }

    // ─── Colony Spec tests ──────────────────────────────────────────────

    #[test]
    fn test_colony_spec_roundtrip() {
        for spec in &[
            ColonySpec::Balanced,
            ColonySpec::Mining,
            ColonySpec::Military,
            ColonySpec::Research,
            ColonySpec::Trade,
            ColonySpec::Fortress,
        ] {
            assert_eq!(ColonySpec::from_str(spec.as_str()), *spec);
        }
    }

    #[test]
    fn test_mining_resource_multiplier() {
        assert!((ColonySpec::Mining.resource_multiplier() - 1.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fortress_defense_multiplier() {
        assert!((ColonySpec::Fortress.defense_multiplier() - 1.4).abs() < f64::EPSILON);
    }

    // ─── Engine integration tests ───────────────────────────────────────

    #[test]
    fn test_colony_create_and_list() {
        let engine = test_engine();
        let colony = engine
            .colony_create("Alpha Base", "[1:042:07]", "player", ColonySpec::Mining)
            .expect("create colony");
        assert_eq!(colony.name, "Alpha Base");
        assert!(colony.is_capital); // First colony is capital
        assert_eq!(colony.specialization, ColonySpec::Mining);

        let list = engine.colony_list().expect("list colonies");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, colony.id);
    }

    #[test]
    fn test_colony_slot_limit() {
        let engine = test_engine();
        // Default max_slots = 1, so second colony should fail
        engine
            .colony_create("First", "[1:001:01]", "player", ColonySpec::Balanced)
            .expect("first colony");
        let result = engine.colony_create("Second", "[1:001:02]", "player", ColonySpec::Balanced);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "NO_COLONY_SLOTS");
    }

    #[test]
    fn test_colony_set_specialization() {
        let engine = test_engine();
        let colony = engine
            .colony_create("Test", "[1:001:01]", "player", ColonySpec::Balanced)
            .expect("create");
        engine
            .colony_set_specialization(&colony.id, ColonySpec::Fortress)
            .expect("set spec");

        let list = engine.colony_list().expect("list");
        assert_eq!(list[0].specialization, ColonySpec::Fortress);
    }

    #[test]
    fn test_colony_slots_available() {
        let engine = test_engine();
        let slots = engine.colony_slots_available().expect("slots");
        assert_eq!(slots.max_slots, 1);
        assert_eq!(slots.used_slots, 0);
        assert_eq!(slots.unlock_cost_dm, 500);
    }

    #[test]
    fn test_colony_transfer_nonexistent() {
        let engine = test_engine();
        let result = engine.colony_transfer_resources("fake_from", "fake_to", 100.0, 0.0, 0.0);
        assert!(result.is_err());
    }

    // ─── Espionage integration tests ────────────────────────────────────

    #[test]
    fn test_espionage_plant_and_gather() {
        let engine = test_engine();
        let agent = engine
            .espionage_plant_agent("enemy_colony_1", CoverDepth::Shallow)
            .expect("plant agent");
        assert_eq!(agent.status, AgentStatus::Active);
        assert_eq!(agent.days_embedded, 0);

        let report = engine
            .espionage_gather_intel(&agent.id)
            .expect("gather intel");
        assert_eq!(report.category, IntelCategory::Resources);
        assert!((report.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_espionage_extract() {
        let engine = test_engine();
        let agent = engine
            .espionage_plant_agent("enemy_colony_2", CoverDepth::Mid)
            .expect("plant");
        engine.espionage_extract_agent(&agent.id).expect("extract");

        // Cannot extract again
        let result = engine.espionage_extract_agent(&agent.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_espionage_counter_intel() {
        let engine = test_engine();
        engine
            .espionage_plant_agent("target_colony", CoverDepth::Deep)
            .expect("plant");
        engine
            .espionage_plant_agent("target_colony", CoverDepth::Shallow)
            .expect("plant 2");

        let found = engine
            .espionage_counter_intel("target_colony")
            .expect("counter intel");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_espionage_detect_check() {
        let engine = test_engine();
        let agent = engine
            .espionage_plant_agent("enemy", CoverDepth::Shallow)
            .expect("plant");
        // Day 0 shallow: detection chance = 0, should not be detected
        let detected = engine.espionage_detect_check(&agent.id).expect("check");
        assert!(!detected);
    }

    // ─── Trade integration tests ────────────────────────────────────────

    #[test]
    fn test_trade_create_and_cancel() {
        let engine = test_engine();
        let order = engine
            .trade_create_order(OrderType::Sell, "metal", 1000.0, 1.5, "colony_1")
            .expect("create order");
        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(order.resource, "metal");

        engine.trade_cancel_order(&order.id).expect("cancel");

        // Cannot cancel again
        let result = engine.trade_cancel_order(&order.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_trade_fill_order() {
        let engine = test_engine();
        let order = engine
            .trade_create_order(OrderType::Buy, "crystal", 500.0, 2.0, "colony_1")
            .expect("create");

        engine.trade_fill_order(&order.id, 200.0).expect("partial fill");

        // Fill the rest
        engine.trade_fill_order(&order.id, 300.0).expect("complete fill");

        // Cannot fill more
        let result = engine.trade_fill_order(&order.id, 1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_trade_fill_overfill() {
        let engine = test_engine();
        let order = engine
            .trade_create_order(OrderType::Sell, "deuterium", 100.0, 3.0, "colony_1")
            .expect("create");

        let result = engine.trade_fill_order(&order.id, 150.0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "OVERFILL");
    }

    #[test]
    fn test_trade_market_prices() {
        let engine = test_engine();
        let prices = engine.trade_market_prices().expect("prices");
        assert_eq!(prices.len(), 4);
        assert_eq!(prices[0].resource, "metal");
        assert_eq!(prices[1].resource, "crystal");
    }

    #[test]
    fn test_trade_history() {
        let engine = test_engine();
        engine
            .trade_create_order(OrderType::Buy, "metal", 100.0, 1.0, "colony_1")
            .expect("create 1");
        engine
            .trade_create_order(OrderType::Sell, "crystal", 200.0, 1.5, "colony_1")
            .expect("create 2");

        let history = engine.trade_history("colony_1", 10).expect("history");
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_trade_validation_negative_amount() {
        let engine = test_engine();
        let result = engine.trade_create_order(OrderType::Buy, "metal", -10.0, 1.0, "c1");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "INVALID_AMOUNT");
    }

    #[test]
    fn test_trade_validation_zero_price() {
        let engine = test_engine();
        let result = engine.trade_create_order(OrderType::Sell, "metal", 10.0, 0.0, "c1");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "INVALID_PRICE");
    }

    // ─── Cover depth / intel category tests ─────────────────────────────

    #[test]
    fn test_cover_depth_roundtrip() {
        for d in &[CoverDepth::Shallow, CoverDepth::Mid, CoverDepth::Deep] {
            assert_eq!(CoverDepth::from_str(d.as_str()), *d);
        }
    }

    #[test]
    fn test_intel_category_min_depth() {
        assert_eq!(IntelCategory::Resources.min_depth(), CoverDepth::Shallow);
        assert_eq!(IntelCategory::FleetMovements.min_depth(), CoverDepth::Mid);
        assert_eq!(IntelCategory::DefenseLayout.min_depth(), CoverDepth::Deep);
    }

    #[test]
    fn test_order_type_roundtrip() {
        assert_eq!(OrderType::from_str("buy"), OrderType::Buy);
        assert_eq!(OrderType::from_str("sell"), OrderType::Sell);
        assert_eq!(OrderType::from_str("unknown"), OrderType::Buy);
    }

    #[test]
    fn test_order_status_roundtrip() {
        for s in &[
            OrderStatus::Open,
            OrderStatus::PartiallyFilled,
            OrderStatus::Filled,
            OrderStatus::Cancelled,
            OrderStatus::Expired,
        ] {
            assert_eq!(OrderStatus::from_str(s.as_str()), *s);
        }
    }

    #[test]
    fn test_agent_status_roundtrip() {
        for s in &[
            AgentStatus::Active,
            AgentStatus::Detected,
            AgentStatus::Extracted,
            AgentStatus::DoubleAgent,
        ] {
            assert_eq!(AgentStatus::from_str(s.as_str()), *s);
        }
    }
}
