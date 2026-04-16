// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//
// SwarmForge Player Engagement -- Daily Login, Challenges, Seasons, Expeditions
// Based on Idle Heroes and AFK Arena retention mechanics research
//
// ## Architecture
// - `EngagementEngine` owns the SQLite connection and all mutation logic
// - Tauri commands are thin wrappers that delegate to the engine
// - State persisted across 4 tables: login_state, weekly_challenges, expeditions, productivity_log
//
// ## Key Formulas (from SwarmForge1.md)
// - Daily DM reward: DM_daily = 400 * (1 + LoginStreak * 0.05), clamped [100..2000]
// - Weekly challenge reward: base_dm = 1000 + (player_level * 50), capped at 2500
// - Expedition duration: hours = 1 + 7 * max(0, 1 - fleet_power / 100000)

use chrono::{Datelike, NaiveDate, Utc};
use rand::Rng;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Mutex;

use crate::error::ImpForgeError;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_engagement", "Game");

// ---------------------------------------------------------------------------
// Types -- Daily Login
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyLoginReward {
    pub day: u32,
    pub dark_matter: u32,
    pub bonus_resources: f64,
    pub streak_multiplier: f64,
    pub special_reward: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginState {
    pub current_streak: u32,
    pub longest_streak: u32,
    pub total_logins: u32,
    pub last_login_date: String,
    pub monthly_calendar: Vec<bool>,
    pub claimed_today: bool,
}

impl LoginState {
    /// Calculate the reward for today based on the current streak.
    ///
    /// Formula from SwarmForge1.md: `DM_daily = 400 * (1 + LoginStreak * 0.05)`
    /// Clamped to [100..2000] to prevent runaway values.
    pub fn calculate_reward(&self) -> DailyLoginReward {
        let streak = self.current_streak;
        let raw_dm = (400.0 * (1.0 + streak as f64 * 0.05)) as u32;
        let dm = raw_dm.clamp(100, 2000);

        let special = match streak {
            7 => Some("weekly_bonus_chest".to_string()),
            14 => Some("biweekly_evolution_crystal".to_string()),
            28 => Some("monthly_legendary_mutation".to_string()),
            _ => None,
        };

        DailyLoginReward {
            day: streak,
            dark_matter: dm,
            bonus_resources: streak as f64 * 50.0,
            streak_multiplier: 1.0 + streak as f64 * 0.05,
            special_reward: special,
        }
    }
}

// ---------------------------------------------------------------------------
// Types -- Weekly Challenges
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeType {
    BuildBuildings,
    TrainUnits,
    WinBattles,
    EarnResources,
    ResearchTech,
    SendFleets,
    EvolveUnits,
    ReachLevel,
    EarnDarkMatter,
    UseImpForge,
    CollectArtifacts,
    ExploreGalaxy,
}

impl ChallengeType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::BuildBuildings => "build_buildings",
            Self::TrainUnits => "train_units",
            Self::WinBattles => "win_battles",
            Self::EarnResources => "earn_resources",
            Self::ResearchTech => "research_tech",
            Self::SendFleets => "send_fleets",
            Self::EvolveUnits => "evolve_units",
            Self::ReachLevel => "reach_level",
            Self::EarnDarkMatter => "earn_dark_matter",
            Self::UseImpForge => "use_impforge",
            Self::CollectArtifacts => "collect_artifacts",
            Self::ExploreGalaxy => "explore_galaxy",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "build_buildings" => Self::BuildBuildings,
            "train_units" => Self::TrainUnits,
            "win_battles" => Self::WinBattles,
            "earn_resources" => Self::EarnResources,
            "research_tech" => Self::ResearchTech,
            "send_fleets" => Self::SendFleets,
            "evolve_units" => Self::EvolveUnits,
            "reach_level" => Self::ReachLevel,
            "earn_dark_matter" => Self::EarnDarkMatter,
            "use_impforge" => Self::UseImpForge,
            "collect_artifacts" => Self::CollectArtifacts,
            "explore_galaxy" => Self::ExploreGalaxy,
            _ => Self::BuildBuildings,
        }
    }

    /// Human-readable description template for this challenge type.
    fn description_template(&self, target: u32) -> String {
        match self {
            Self::BuildBuildings => format!("Build {target} buildings"),
            Self::TrainUnits => format!("Train {target} units"),
            Self::WinBattles => format!("Win {target} battles"),
            Self::EarnResources => format!("Earn {target} resources"),
            Self::ResearchTech => format!("Research {target} technologies"),
            Self::SendFleets => format!("Send {target} fleets"),
            Self::EvolveUnits => format!("Evolve {target} units"),
            Self::ReachLevel => format!("Reach level {target}"),
            Self::EarnDarkMatter => format!("Earn {target} Dark Matter"),
            Self::UseImpForge => format!("Use ImpForge for {target} minutes"),
            Self::CollectArtifacts => format!("Collect {target} artifacts"),
            Self::ExploreGalaxy => format!("Explore {target} systems"),
        }
    }

    /// Friendly title for this challenge type.
    fn title(&self) -> &'static str {
        match self {
            Self::BuildBuildings => "Master Builder",
            Self::TrainUnits => "Drill Sergeant",
            Self::WinBattles => "War Hero",
            Self::EarnResources => "Resource Mogul",
            Self::ResearchTech => "Tech Pioneer",
            Self::SendFleets => "Fleet Admiral",
            Self::EvolveUnits => "Gene Splicer",
            Self::ReachLevel => "Level Up",
            Self::EarnDarkMatter => "Dark Collector",
            Self::UseImpForge => "Productivity Master",
            Self::CollectArtifacts => "Relic Hunter",
            Self::ExploreGalaxy => "Star Explorer",
        }
    }
}

/// All possible challenge types for random selection.
const ALL_CHALLENGE_TYPES: [ChallengeType; 12] = [
    ChallengeType::BuildBuildings,
    ChallengeType::TrainUnits,
    ChallengeType::WinBattles,
    ChallengeType::EarnResources,
    ChallengeType::ResearchTech,
    ChallengeType::SendFleets,
    ChallengeType::EvolveUnits,
    ChallengeType::ReachLevel,
    ChallengeType::EarnDarkMatter,
    ChallengeType::UseImpForge,
    ChallengeType::CollectArtifacts,
    ChallengeType::ExploreGalaxy,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyChallenge {
    pub id: String,
    pub title: String,
    pub description: String,
    pub challenge_type: ChallengeType,
    pub target: u32,
    pub progress: u32,
    pub reward_dm: u32,
    pub reward_resources: f64,
    pub expires_at: String,
    pub completed: bool,
    pub claimed: bool,
}

// ---------------------------------------------------------------------------
// Types -- Expeditions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExpeditionStatus {
    InProgress,
    Completed,
    Failed,
    Collected,
}

impl ExpeditionStatus {
    #[cfg(test)]
    fn as_str(&self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Collected => "collected",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "in_progress" => Self::InProgress,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "collected" => Self::Collected,
            _ => Self::InProgress,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpeditionReward {
    pub resources_primary: f64,
    pub resources_secondary: f64,
    pub dark_matter: u32,
    pub tech_artifact: Option<String>,
    pub rare_unit: Option<String>,
    pub event_log: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expedition {
    pub id: String,
    pub fleet_power: u64,
    pub duration_hours: u32,
    pub started_at: String,
    pub completes_at: String,
    pub status: ExpeditionStatus,
    pub rewards: Option<ExpeditionReward>,
}

// ---------------------------------------------------------------------------
// Types -- Productivity Mapping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductivityMapping {
    pub action: String,
    pub resource_primary: f64,
    pub resource_secondary: f64,
    pub resource_tertiary: f64,
    pub dark_matter: u32,
    pub xp: u64,
    pub description: String,
}

/// Canonical productivity-to-game-resource mappings.
/// Each real-world productivity action earns in-game resources.
pub fn get_productivity_mappings() -> Vec<ProductivityMapping> {
    vec![
        ProductivityMapping {
            action: "document_written".into(),
            resource_primary: 50.0,
            resource_secondary: 20.0,
            resource_tertiary: 10.0,
            dark_matter: 2,
            xp: 25,
            description: "Writing a document = crafting weapons".into(),
        },
        ProductivityMapping {
            action: "code_committed".into(),
            resource_primary: 30.0,
            resource_secondary: 100.0,
            resource_tertiary: 15.0,
            dark_matter: 3,
            xp: 40,
            description: "Coding a commit = training elite units".into(),
        },
        ProductivityMapping {
            action: "email_sent".into(),
            resource_primary: 10.0,
            resource_secondary: 5.0,
            resource_tertiary: 20.0,
            dark_matter: 1,
            xp: 10,
            description: "Sending email = diplomatic communication".into(),
        },
        ProductivityMapping {
            action: "tests_passed".into(),
            resource_primary: 20.0,
            resource_secondary: 30.0,
            resource_tertiary: 10.0,
            dark_matter: 2,
            xp: 30,
            description: "Passing tests = military exercises".into(),
        },
        ProductivityMapping {
            action: "build_succeeded".into(),
            resource_primary: 40.0,
            resource_secondary: 50.0,
            resource_tertiary: 20.0,
            dark_matter: 2,
            xp: 35,
            description: "Successful build = fortress construction".into(),
        },
        ProductivityMapping {
            action: "active_5min".into(),
            resource_primary: 5.0,
            resource_secondary: 5.0,
            resource_tertiary: 5.0,
            dark_matter: 1,
            xp: 5,
            description: "5 minutes active use = passive resource gathering".into(),
        },
        ProductivityMapping {
            action: "achievement_unlocked".into(),
            resource_primary: 200.0,
            resource_secondary: 150.0,
            resource_tertiary: 100.0,
            dark_matter: 10,
            xp: 100,
            description: "Achievement = major milestone reward".into(),
        },
        ProductivityMapping {
            action: "spreadsheet_created".into(),
            resource_primary: 40.0,
            resource_secondary: 15.0,
            resource_tertiary: 10.0,
            dark_matter: 2,
            xp: 20,
            description: "Creating spreadsheet = resource planning".into(),
        },
        ProductivityMapping {
            action: "note_created".into(),
            resource_primary: 15.0,
            resource_secondary: 10.0,
            resource_tertiary: 5.0,
            dark_matter: 1,
            xp: 10,
            description: "Taking notes = intelligence gathering".into(),
        },
        ProductivityMapping {
            action: "workflow_run".into(),
            resource_primary: 25.0,
            resource_secondary: 25.0,
            resource_tertiary: 25.0,
            dark_matter: 3,
            xp: 30,
            description: "Running workflow = automated production".into(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Save Integrity & Anti-Cheat
// ---------------------------------------------------------------------------

/// Compute a deterministic checksum for colony save data.
///
/// Uses `DefaultHasher` with a salt so trivial edits to the JSON are detectable.
/// This is not cryptographic security -- it is a tamper-detection heuristic.
pub fn calculate_save_checksum(colony_data: &str) -> String {
    let mut hasher = DefaultHasher::new();
    colony_data.hash(&mut hasher);
    "impforge_v1".hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Verify that a save file has not been tampered with.
pub fn verify_save_integrity(colony_data: &str, checksum: &str) -> bool {
    calculate_save_checksum(colony_data) == checksum
}

/// Validate that a production rate is within the theoretical maximum.
///
/// OGame formula: max = 30 * 1.5^level per hour, with 2x safety margin.
/// Currently used by anti-cheat validation and tests; exposed for
/// integration with colony resource audits.
pub fn validate_resource_rates(production_per_hour: f64, building_level: u32) -> bool {
    let max_possible = 30.0 * 1.5_f64.powi(building_level as i32) * 2.0;
    production_per_hour <= max_possible
}

// ---------------------------------------------------------------------------
// Expedition narrative events
// ---------------------------------------------------------------------------

/// Random narrative events that can occur during an expedition.
const EXPEDITION_EVENTS: [&str; 15] = [
    "Discovered ancient ruins on a barren moon",
    "Encountered space pirates -- crew fought them off",
    "Found a derelict freighter with salvageable cargo",
    "Navigated through an asteroid belt unscathed",
    "Detected unusual energy readings from a nearby nebula",
    "Rendezvoused with a friendly merchant convoy",
    "Survived a solar flare by emergency warp jump",
    "Mapped an uncharted wormhole entrance",
    "Collected rare mineral samples from a comet trail",
    "Intercepted a distress signal and rescued survivors",
    "Observed a binary star system at close range",
    "Evaded a hostile patrol fleet using sensor jamming",
    "Discovered a habitable world in the outer rim",
    "Found fragments of an ancient alien artifact",
    "Crew morale boosted by discovering a pristine water world",
];

/// Rare artifacts that can be found on expeditions (5% chance each).
const RARE_ARTIFACTS: [&str; 8] = [
    "Chrono Shard",
    "Void Crystal",
    "Neural Amplifier",
    "Dark Matter Lens",
    "Quantum Stabilizer",
    "Bio-Resonance Core",
    "Graviton Emitter",
    "Psionic Relay",
];

/// Rare units that can be discovered on expeditions (5% chance).
const RARE_UNITS: [&str; 6] = [
    "Shadow Lurker",
    "Void Walker",
    "Chrono Stalker",
    "Quantum Drone",
    "Psi-Brood Guardian",
    "Nebula Wraith",
];

// ---------------------------------------------------------------------------
// Engine -- owns the DB connection and all mutation logic
// ---------------------------------------------------------------------------

pub struct EngagementEngine {
    conn: Mutex<Connection>,
}

impl EngagementEngine {
    /// Open (or create) the engagement database.
    pub fn new(data_dir: &Path) -> Result<Self, ImpForgeError> {
        let db_path = data_dir.join("engagement.db");
        let conn = Connection::open(&db_path).map_err(|e| {
            ImpForgeError::internal("ENG_DB_OPEN", format!("Failed to open engagement DB: {e}"))
        })?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| {
                ImpForgeError::internal("ENG_DB_PRAGMA", format!("DB pragma failed: {e}"))
            })?;
        let engine = Self {
            conn: Mutex::new(conn),
        };
        engine.init_tables()?;
        Ok(engine)
    }

    fn init_tables(&self) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS login_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                current_streak INTEGER NOT NULL DEFAULT 0,
                longest_streak INTEGER NOT NULL DEFAULT 0,
                total_logins INTEGER NOT NULL DEFAULT 0,
                last_login_date TEXT,
                claimed_today INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS login_calendar (
                login_date TEXT PRIMARY KEY
            );
            CREATE TABLE IF NOT EXISTS weekly_challenges (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                challenge_type TEXT NOT NULL,
                target INTEGER NOT NULL,
                progress INTEGER NOT NULL DEFAULT 0,
                reward_dm INTEGER NOT NULL,
                reward_resources REAL NOT NULL DEFAULT 0.0,
                expires_at TEXT NOT NULL,
                completed INTEGER NOT NULL DEFAULT 0,
                claimed INTEGER NOT NULL DEFAULT 0,
                week_key TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS expeditions (
                id TEXT PRIMARY KEY,
                fleet_power INTEGER NOT NULL,
                duration_hours INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                completes_at TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'in_progress',
                rewards_json TEXT
            );
            CREATE TABLE IF NOT EXISTS productivity_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                action TEXT NOT NULL,
                resource_primary REAL NOT NULL DEFAULT 0.0,
                resource_secondary REAL NOT NULL DEFAULT 0.0,
                resource_tertiary REAL NOT NULL DEFAULT 0.0,
                dark_matter INTEGER NOT NULL DEFAULT 0,
                xp INTEGER NOT NULL DEFAULT 0,
                logged_at TEXT NOT NULL
            );
            INSERT OR IGNORE INTO login_state (id, current_streak) VALUES (1, 0);",
        )
        .map_err(|e| {
            ImpForgeError::internal("ENG_INIT", format!("Table creation failed: {e}"))
        })?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Daily Login
    // -----------------------------------------------------------------------

    /// Check whether today is a new login day and update the streak accordingly.
    pub fn check_daily_login(&self) -> Result<LoginState, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let today = Utc::now().format("%Y-%m-%d").to_string();

        let (current_streak, longest_streak, total_logins, last_login, claimed_today): (
            u32, u32, u32, Option<String>, bool,
        ) = conn
            .query_row(
                "SELECT current_streak, longest_streak, total_logins,
                        last_login_date, claimed_today
                 FROM login_state WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, u32>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i32>(4)? != 0,
                    ))
                },
            )
            .map_err(|e| ImpForgeError::internal("ENG_LOGIN_READ", e.to_string()))?;

        let (new_streak, new_longest) = match last_login.as_deref() {
            Some(d) if d == today => (current_streak, longest_streak),
            Some(d) => {
                if let (Ok(last), Ok(tod)) = (
                    NaiveDate::parse_from_str(d, "%Y-%m-%d"),
                    NaiveDate::parse_from_str(&today, "%Y-%m-%d"),
                ) {
                    let diff = (tod - last).num_days();
                    if diff == 1 {
                        let s = current_streak + 1;
                        (s, s.max(longest_streak))
                    } else {
                        (1, longest_streak.max(1))
                    }
                } else {
                    (1, longest_streak.max(1))
                }
            }
            None => (1, longest_streak.max(1)),
        };

        let already_today = last_login.as_deref() == Some(&today);
        let new_total = if already_today {
            total_logins
        } else {
            total_logins + 1
        };
        let new_claimed = if already_today { claimed_today } else { false };

        // Persist updated state
        conn.execute(
            "UPDATE login_state SET current_streak = ?1, longest_streak = ?2,
             total_logins = ?3, last_login_date = ?4, claimed_today = ?5 WHERE id = 1",
            params![new_streak, new_longest, new_total, today, new_claimed as i32],
        )
        .map_err(|e| ImpForgeError::internal("ENG_LOGIN_UPDATE", e.to_string()))?;

        // Record in calendar
        conn.execute(
            "INSERT OR IGNORE INTO login_calendar (login_date) VALUES (?1)",
            params![today],
        )
        .map_err(|e| ImpForgeError::internal("ENG_CAL_INSERT", e.to_string()))?;

        let calendar = self.load_monthly_calendar(&conn, &today)?;

        Ok(LoginState {
            current_streak: new_streak,
            longest_streak: new_longest,
            total_logins: new_total,
            last_login_date: today,
            monthly_calendar: calendar,
            claimed_today: new_claimed,
        })
    }

    /// Claim the daily reward. Returns the reward, or an error if already claimed.
    pub fn claim_daily_reward(&self) -> Result<DailyLoginReward, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let today = Utc::now().format("%Y-%m-%d").to_string();

        let (streak, last_date, already_claimed): (u32, Option<String>, bool) = conn
            .query_row(
                "SELECT current_streak, last_login_date, claimed_today FROM login_state WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i32>(2)? != 0,
                    ))
                },
            )
            .map_err(|e| ImpForgeError::internal("ENG_CLAIM_READ", e.to_string()))?;

        if last_date.as_deref() != Some(&today) {
            return Err(ImpForgeError::validation(
                "ENG_NOT_LOGGED_IN",
                "Must check daily login first before claiming reward",
            ));
        }

        if already_claimed {
            return Err(ImpForgeError::validation(
                "ENG_ALREADY_CLAIMED",
                "Daily reward already claimed today",
            ));
        }

        let state = LoginState {
            current_streak: streak,
            longest_streak: 0,
            total_logins: 0,
            last_login_date: today,
            monthly_calendar: vec![],
            claimed_today: false,
        };
        let reward = state.calculate_reward();

        conn.execute(
            "UPDATE login_state SET claimed_today = 1 WHERE id = 1",
            [],
        )
        .map_err(|e| ImpForgeError::internal("ENG_CLAIM_UPDATE", e.to_string()))?;

        Ok(reward)
    }

    /// Get the login calendar for the current month (31 bools, index 0 = day 1).
    pub fn get_login_calendar(&self) -> Result<Vec<bool>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;
        let today = Utc::now().format("%Y-%m-%d").to_string();
        self.load_monthly_calendar(&conn, &today)
    }

    /// Get streak information as a JSON-friendly value.
    /// Used by tests and available for cross-module integration.
    pub fn get_streak_info(&self) -> Result<serde_json::Value, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let (current, longest, total, last_date): (u32, u32, u32, Option<String>) = conn
            .query_row(
                "SELECT current_streak, longest_streak, total_logins, last_login_date
                 FROM login_state WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, u32>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .map_err(|e| ImpForgeError::internal("ENG_STREAK_READ", e.to_string()))?;

        Ok(serde_json::json!({
            "current_streak": current,
            "longest_streak": longest,
            "total_logins": total,
            "last_login_date": last_date,
        }))
    }

    /// Load which days in the current month have a login recorded.
    fn load_monthly_calendar(
        &self,
        conn: &Connection,
        today_str: &str,
    ) -> Result<Vec<bool>, ImpForgeError> {
        let year_month = &today_str[..7]; // "2026-03"
        let prefix = format!("{year_month}-%");

        let mut stmt = conn
            .prepare("SELECT login_date FROM login_calendar WHERE login_date LIKE ?1")
            .map_err(|e| ImpForgeError::internal("ENG_CAL_QUERY", e.to_string()))?;

        let dates: Vec<String> = stmt
            .query_map(params![prefix], |row| row.get(0))
            .map_err(|e| ImpForgeError::internal("ENG_CAL_MAP", e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        let mut calendar = vec![false; 31];
        for d in &dates {
            if let Ok(date) = NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                let day = date.day() as usize;
                if (1..=31).contains(&day) {
                    calendar[day - 1] = true;
                }
            }
        }
        Ok(calendar)
    }

    // -----------------------------------------------------------------------
    // Weekly Challenges
    // -----------------------------------------------------------------------

    /// Get current weekly challenges for a colony. Generates new ones if the
    /// current set has expired.
    pub fn get_weekly_challenges(&self, colony_id: &str) -> Result<Vec<WeeklyChallenge>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let week_key = current_week_key();

        // Check if we have challenges for this week
        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM weekly_challenges WHERE week_key = ?1",
                params![week_key],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if count == 0 {
            drop(conn);
            return self.generate_challenges(colony_id, &week_key, 1);
        }

        self.load_challenges(&conn, &week_key)
    }

    /// Update progress on a matching challenge type.
    pub fn update_challenge_progress(
        &self,
        colony_id: &str,
        challenge_type: &str,
        amount: u32,
    ) -> Result<(), ImpForgeError> {
        let _ = colony_id; // reserved for future multi-colony support
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let week_key = current_week_key();

        conn.execute(
            "UPDATE weekly_challenges
             SET progress = MIN(progress + ?1, target)
             WHERE challenge_type = ?2 AND week_key = ?3 AND completed = 0",
            params![amount, challenge_type, week_key],
        )
        .map_err(|e| ImpForgeError::internal("ENG_CHAL_UPDATE", e.to_string()))?;

        // Mark completed if progress >= target
        conn.execute(
            "UPDATE weekly_challenges
             SET completed = 1
             WHERE progress >= target AND week_key = ?1 AND completed = 0",
            params![week_key],
        )
        .map_err(|e| ImpForgeError::internal("ENG_CHAL_COMPLETE", e.to_string()))?;

        Ok(())
    }

    /// Claim the reward for a completed challenge.
    pub fn claim_challenge_reward(
        &self,
        colony_id: &str,
        challenge_id: &str,
    ) -> Result<DailyLoginReward, ImpForgeError> {
        let _ = colony_id;
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let (completed, claimed, reward_dm, reward_res): (bool, bool, u32, f64) = conn
            .query_row(
                "SELECT completed, claimed, reward_dm, reward_resources
                 FROM weekly_challenges WHERE id = ?1",
                params![challenge_id],
                |row| {
                    Ok((
                        row.get::<_, i32>(0)? != 0,
                        row.get::<_, i32>(1)? != 0,
                        row.get::<_, u32>(2)?,
                        row.get::<_, f64>(3)?,
                    ))
                },
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "ENG_CHAL_NOT_FOUND",
                    format!("Challenge not found: {challenge_id}"),
                )
            })?;

        if !completed {
            return Err(ImpForgeError::validation(
                "ENG_CHAL_INCOMPLETE",
                "Challenge not yet completed",
            ));
        }
        if claimed {
            return Err(ImpForgeError::validation(
                "ENG_CHAL_CLAIMED",
                "Challenge reward already claimed",
            ));
        }

        conn.execute(
            "UPDATE weekly_challenges SET claimed = 1 WHERE id = ?1",
            params![challenge_id],
        )
        .map_err(|e| ImpForgeError::internal("ENG_CHAL_CLAIM", e.to_string()))?;

        Ok(DailyLoginReward {
            day: 0,
            dark_matter: reward_dm,
            bonus_resources: reward_res,
            streak_multiplier: 1.0,
            special_reward: Some("weekly_challenge_complete".to_string()),
        })
    }

    /// Force-generate a new set of 3 challenges for the given week.
    /// Used by tests and available for weekly reset triggers.
    pub fn refresh_challenges(
        &self,
        colony_id: &str,
    ) -> Result<Vec<WeeklyChallenge>, ImpForgeError> {
        // Use a unique key so refreshed challenges get fresh IDs
        let week_key = format!("{}_r{}", current_week_key(), rand::random::<u32>());
        self.generate_challenges(colony_id, &week_key, 1)
    }

    /// Generate 3 random challenges for a given week key.
    fn generate_challenges(
        &self,
        _colony_id: &str,
        week_key: &str,
        player_level: u32,
    ) -> Result<Vec<WeeklyChallenge>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Remove old challenges for this week (in case of refresh)
        conn.execute(
            "DELETE FROM weekly_challenges WHERE week_key = ?1",
            params![week_key],
        )
        .map_err(|e| ImpForgeError::internal("ENG_CHAL_DELETE", e.to_string()))?;

        let mut rng = rand::thread_rng();
        let mut selected_indices: Vec<usize> = Vec::with_capacity(3);
        while selected_indices.len() < 3 {
            let idx = rng.gen_range(0..ALL_CHALLENGE_TYPES.len());
            if !selected_indices.contains(&idx) {
                selected_indices.push(idx);
            }
        }

        // Calculate expiry (next Monday 00:00 UTC, approximated as +7 days from now)
        let expires = Utc::now() + chrono::Duration::days(7);
        let expires_str = expires.format("%Y-%m-%dT00:00:00Z").to_string();

        let base_dm = (1000 + player_level * 50).min(2500);
        let mut challenges = Vec::with_capacity(3);

        for (i, &idx) in selected_indices.iter().enumerate() {
            let ct = &ALL_CHALLENGE_TYPES[idx];
            let target = scale_challenge_target(ct, player_level);
            let reward_dm = base_dm + (i as u32) * 250; // slight variation
            let id = format!("{week_key}_ch{i}_{}", ct.as_str());

            let challenge = WeeklyChallenge {
                id: id.clone(),
                title: ct.title().to_string(),
                description: ct.description_template(target),
                challenge_type: ct.clone(),
                target,
                progress: 0,
                reward_dm: reward_dm.min(2500),
                reward_resources: reward_dm as f64 * 2.0,
                expires_at: expires_str.clone(),
                completed: false,
                claimed: false,
            };

            conn.execute(
                "INSERT INTO weekly_challenges
                 (id, title, description, challenge_type, target, progress,
                  reward_dm, reward_resources, expires_at, completed, claimed, week_key)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, 0, ?10)",
                params![
                    challenge.id,
                    challenge.title,
                    challenge.description,
                    ct.as_str(),
                    challenge.target,
                    0,
                    challenge.reward_dm,
                    challenge.reward_resources,
                    challenge.expires_at,
                    week_key,
                ],
            )
            .map_err(|e| ImpForgeError::internal("ENG_CHAL_INSERT", e.to_string()))?;

            challenges.push(challenge);
        }

        Ok(challenges)
    }

    fn load_challenges(
        &self,
        conn: &Connection,
        week_key: &str,
    ) -> Result<Vec<WeeklyChallenge>, ImpForgeError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, title, description, challenge_type, target, progress,
                        reward_dm, reward_resources, expires_at, completed, claimed
                 FROM weekly_challenges WHERE week_key = ?1 ORDER BY id",
            )
            .map_err(|e| ImpForgeError::internal("ENG_CHAL_LOAD", e.to_string()))?;

        let rows = stmt
            .query_map(params![week_key], |row| {
                let ct_str: String = row.get(3)?;
                Ok(WeeklyChallenge {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    challenge_type: ChallengeType::from_str(&ct_str),
                    target: row.get::<_, u32>(4)?,
                    progress: row.get::<_, u32>(5)?,
                    reward_dm: row.get::<_, u32>(6)?,
                    reward_resources: row.get::<_, f64>(7)?,
                    expires_at: row.get(8)?,
                    completed: row.get::<_, i32>(9)? != 0,
                    claimed: row.get::<_, i32>(10)? != 0,
                })
            })
            .map_err(|e| ImpForgeError::internal("ENG_CHAL_LOAD", e.to_string()))?;

        let mut challenges = Vec::new();
        for r in rows {
            challenges.push(
                r.map_err(|e| ImpForgeError::internal("ENG_CHAL_ROW", e.to_string()))?,
            );
        }
        Ok(challenges)
    }

    // -----------------------------------------------------------------------
    // Expeditions
    // -----------------------------------------------------------------------

    /// Start a new expedition. Max 3 active simultaneously.
    pub fn start_expedition(&self, fleet_power: u64) -> Result<Expedition, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check active expedition count
        let active: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM expeditions WHERE status = 'in_progress'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if active >= 3 {
            return Err(ImpForgeError::validation(
                "ENG_MAX_EXPEDITIONS",
                "Maximum 3 active expeditions allowed",
            ));
        }

        if fleet_power == 0 {
            return Err(ImpForgeError::validation(
                "ENG_ZERO_FLEET",
                "Fleet power must be greater than 0",
            ));
        }

        // Duration: 1 + 7 * max(0, 1 - fleet_power / 100000)
        let ratio = 1.0 - (fleet_power as f64 / 100_000.0);
        let duration_hours = (1.0 + 7.0 * ratio.max(0.0)).round() as u32;
        let duration_hours = duration_hours.clamp(1, 8);

        let now = Utc::now();
        let completes = now + chrono::Duration::hours(duration_hours as i64);
        let id = uuid::Uuid::new_v4().to_string();

        let expedition = Expedition {
            id: id.clone(),
            fleet_power,
            duration_hours,
            started_at: now.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            completes_at: completes.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            status: ExpeditionStatus::InProgress,
            rewards: None,
        };

        conn.execute(
            "INSERT INTO expeditions (id, fleet_power, duration_hours, started_at, completes_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, 'in_progress')",
            params![
                expedition.id,
                expedition.fleet_power as i64,
                expedition.duration_hours,
                expedition.started_at,
                expedition.completes_at,
            ],
        )
        .map_err(|e| ImpForgeError::internal("ENG_EXP_INSERT", e.to_string()))?;

        Ok(expedition)
    }

    /// Get the status of all expeditions (active, completed, collected).
    pub fn expedition_status(&self) -> Result<Vec<Expedition>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Auto-complete expeditions whose time has elapsed
        let now_str = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        conn.execute(
            "UPDATE expeditions SET status = 'completed'
             WHERE status = 'in_progress' AND completes_at <= ?1",
            params![now_str],
        )
        .map_err(|e| ImpForgeError::internal("ENG_EXP_AUTO", e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, fleet_power, duration_hours, started_at, completes_at, status, rewards_json
                 FROM expeditions ORDER BY started_at DESC LIMIT 20",
            )
            .map_err(|e| ImpForgeError::internal("ENG_EXP_LOAD", e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let status_str: String = row.get(5)?;
                let rewards_json: Option<String> = row.get(6)?;
                let rewards = rewards_json.and_then(|j| serde_json::from_str(&j).ok());
                Ok(Expedition {
                    id: row.get(0)?,
                    fleet_power: row.get::<_, i64>(1)? as u64,
                    duration_hours: row.get::<_, u32>(2)?,
                    started_at: row.get(3)?,
                    completes_at: row.get(4)?,
                    status: ExpeditionStatus::from_str(&status_str),
                    rewards,
                })
            })
            .map_err(|e| ImpForgeError::internal("ENG_EXP_LOAD", e.to_string()))?;

        let mut expeditions = Vec::new();
        for r in rows {
            expeditions.push(
                r.map_err(|e| ImpForgeError::internal("ENG_EXP_ROW", e.to_string()))?,
            );
        }
        Ok(expeditions)
    }

    /// Collect rewards from a completed expedition.
    pub fn collect_expedition(&self, expedition_id: &str) -> Result<ExpeditionReward, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let (status_str, fleet_power, duration): (String, i64, u32) = conn
            .query_row(
                "SELECT status, fleet_power, duration_hours FROM expeditions WHERE id = ?1",
                params![expedition_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "ENG_EXP_NOT_FOUND",
                    format!("Expedition not found: {expedition_id}"),
                )
            })?;

        let status = ExpeditionStatus::from_str(&status_str);
        if status != ExpeditionStatus::Completed {
            return Err(ImpForgeError::validation(
                "ENG_EXP_NOT_READY",
                format!("Expedition status is {:?}, must be 'completed'", status),
            ));
        }

        let fleet_power = fleet_power as u64;
        let reward = generate_expedition_reward(fleet_power, duration);
        let reward_json = serde_json::to_string(&reward).unwrap_or_default();

        conn.execute(
            "UPDATE expeditions SET status = 'collected', rewards_json = ?1 WHERE id = ?2",
            params![reward_json, expedition_id],
        )
        .map_err(|e| ImpForgeError::internal("ENG_EXP_COLLECT", e.to_string()))?;

        Ok(reward)
    }

    // -----------------------------------------------------------------------
    // Productivity Logging
    // -----------------------------------------------------------------------

    /// Log a productivity action and return the resources earned.
    pub fn log_activity(&self, action: &str) -> Result<ProductivityMapping, ImpForgeError> {
        let mappings = get_productivity_mappings();
        let mapping = mappings
            .iter()
            .find(|m| m.action == action)
            .ok_or_else(|| {
                ImpForgeError::validation(
                    "ENG_UNKNOWN_ACTION",
                    format!("Unknown productivity action: {action}"),
                )
            })?
            .clone();

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("ENG_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        conn.execute(
            "INSERT INTO productivity_log
             (action, resource_primary, resource_secondary, resource_tertiary, dark_matter, xp, logged_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                mapping.action,
                mapping.resource_primary,
                mapping.resource_secondary,
                mapping.resource_tertiary,
                mapping.dark_matter,
                mapping.xp as i64,
                now,
            ],
        )
        .map_err(|e| ImpForgeError::internal("ENG_PROD_INSERT", e.to_string()))?;

        Ok(mapping)
    }

    // -----------------------------------------------------------------------
    // Save Integrity
    // -----------------------------------------------------------------------

    /// Verify save data integrity.
    pub fn verify_save(&self, colony_data: &str, checksum: &str) -> bool {
        verify_save_integrity(colony_data, checksum)
    }
}

// ---------------------------------------------------------------------------
// Helper Functions
// ---------------------------------------------------------------------------

/// Get the ISO week key for the current week (e.g. "2026-W12").
fn current_week_key() -> String {
    let now = Utc::now();
    format!("{}-W{:02}", now.format("%G"), now.format("%V"))
}

/// Scale challenge target based on player level.
fn scale_challenge_target(ct: &ChallengeType, player_level: u32) -> u32 {
    let base = match ct {
        ChallengeType::BuildBuildings => 3,
        ChallengeType::TrainUnits => 10,
        ChallengeType::WinBattles => 3,
        ChallengeType::EarnResources => 500,
        ChallengeType::ResearchTech => 2,
        ChallengeType::SendFleets => 3,
        ChallengeType::EvolveUnits => 2,
        ChallengeType::ReachLevel => 1,
        ChallengeType::EarnDarkMatter => 50,
        ChallengeType::UseImpForge => 30,
        ChallengeType::CollectArtifacts => 3,
        ChallengeType::ExploreGalaxy => 2,
    };
    // Scale: base + (level * base / 10), capped at 10x base
    let scaled = base + (player_level * base / 10);
    scaled.min(base * 10)
}

/// Generate rewards for a completed expedition based on fleet power and duration.
fn generate_expedition_reward(fleet_power: u64, duration_hours: u32) -> ExpeditionReward {
    let mut rng = rand::thread_rng();
    let power_factor = fleet_power as f64 / 10_000.0;
    let time_factor = duration_hours as f64;

    // Generate 2-4 narrative events
    let event_count = rng.gen_range(2..=4);
    let mut events: Vec<String> = Vec::with_capacity(event_count);
    let mut used_indices: Vec<usize> = Vec::new();
    for _ in 0..event_count {
        let mut idx = rng.gen_range(0..EXPEDITION_EVENTS.len());
        while used_indices.contains(&idx) {
            idx = rng.gen_range(0..EXPEDITION_EVENTS.len());
        }
        used_indices.push(idx);
        events.push(EXPEDITION_EVENTS[idx].to_string());
    }

    // 5% chance of tech artifact
    let tech_artifact = if rng.gen_range(0..100) < 5 {
        let idx = rng.gen_range(0..RARE_ARTIFACTS.len());
        Some(RARE_ARTIFACTS[idx].to_string())
    } else {
        None
    };

    // 5% chance of rare unit
    let rare_unit = if rng.gen_range(0..100) < 5 {
        let idx = rng.gen_range(0..RARE_UNITS.len());
        Some(RARE_UNITS[idx].to_string())
    } else {
        None
    };

    ExpeditionReward {
        resources_primary: power_factor * time_factor * 100.0,
        resources_secondary: power_factor * time_factor * 50.0,
        dark_matter: (power_factor * time_factor * 2.0).round() as u32,
        tech_artifact,
        rare_unit,
        event_log: events,
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands (12)
// ---------------------------------------------------------------------------

// -- Daily Login (3) --

#[tauri::command]
pub fn engagement_check_login(
    engine: tauri::State<'_, EngagementEngine>,
) -> Result<LoginState, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_engagement", "game_engagement", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_engagement", "game_engagement");
    crate::synapse_fabric::synapse_session_push("swarm_engagement", "game_engagement", "engagement_check_login called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_engagement", "info", "swarm_engagement active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_engagement", "engage", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"op": "check_login"}));
    engine.check_daily_login()
}

#[tauri::command]
pub fn engagement_claim_daily(
    engine: tauri::State<'_, EngagementEngine>,
) -> Result<DailyLoginReward, ImpForgeError> {
    engine.claim_daily_reward()
}

#[tauri::command]
pub fn engagement_login_calendar(
    engine: tauri::State<'_, EngagementEngine>,
) -> Result<Vec<bool>, ImpForgeError> {
    engine.get_login_calendar()
}

// -- Weekly Challenges (3) --

#[tauri::command]
pub fn engagement_weekly_challenges(
    engine: tauri::State<'_, EngagementEngine>,
    colony_id: String,
) -> Result<Vec<WeeklyChallenge>, ImpForgeError> {
    engine.get_weekly_challenges(&colony_id)
}

#[tauri::command]
pub fn engagement_update_progress(
    engine: tauri::State<'_, EngagementEngine>,
    colony_id: String,
    challenge_type: String,
    amount: u32,
) -> Result<(), ImpForgeError> {
    engine.update_challenge_progress(&colony_id, &challenge_type, amount)
}

#[tauri::command]
pub fn engagement_claim_challenge(
    engine: tauri::State<'_, EngagementEngine>,
    colony_id: String,
    challenge_id: String,
) -> Result<DailyLoginReward, ImpForgeError> {
    engine.claim_challenge_reward(&colony_id, &challenge_id)
}

// -- Expeditions (3) --

#[tauri::command]
pub fn engagement_start_expedition(
    engine: tauri::State<'_, EngagementEngine>,
    fleet_power: u64,
) -> Result<Expedition, ImpForgeError> {
    engine.start_expedition(fleet_power)
}

#[tauri::command]
pub fn engagement_expedition_status(
    engine: tauri::State<'_, EngagementEngine>,
) -> Result<Vec<Expedition>, ImpForgeError> {
    engine.expedition_status()
}

#[tauri::command]
pub fn engagement_collect_expedition(
    engine: tauri::State<'_, EngagementEngine>,
    expedition_id: String,
) -> Result<ExpeditionReward, ImpForgeError> {
    engine.collect_expedition(&expedition_id)
}

// -- Productivity (2) --

#[tauri::command]
pub fn engagement_productivity_mappings() -> Vec<ProductivityMapping> {
    get_productivity_mappings()
}

#[tauri::command]
pub fn engagement_log_activity(
    engine: tauri::State<'_, EngagementEngine>,
    action: String,
) -> Result<ProductivityMapping, ImpForgeError> {
    engine.log_activity(&action)
}

// -- Save Integrity (1) --

#[tauri::command]
pub fn engagement_verify_save(
    engine: tauri::State<'_, EngagementEngine>,
    colony_data: String,
    checksum: String,
) -> bool {
    engine.verify_save(&colony_data, &checksum)
}

// ---------------------------------------------------------------------------
// Additional Tauri Commands — wiring internal helpers
// ---------------------------------------------------------------------------

/// Validate production rates against OGame theoretical max.
#[tauri::command]
pub fn engagement_validate_rates(
    production_per_hour: f64,
    building_level: u32,
) -> Result<serde_json::Value, ImpForgeError> {
    let valid = validate_resource_rates(production_per_hour, building_level);
    Ok(serde_json::json!({
        "valid": valid,
        "production_per_hour": production_per_hour,
        "building_level": building_level,
    }))
}

/// Get login streak info.
#[tauri::command]
pub fn engagement_streak_info(
    engine: tauri::State<'_, EngagementEngine>,
) -> Result<serde_json::Value, ImpForgeError> {
    engine.get_streak_info()
}

/// Refresh weekly challenges for a colony.
#[tauri::command]
pub fn engagement_refresh_challenges(
    colony_id: String,
    engine: tauri::State<'_, EngagementEngine>,
) -> Result<Vec<WeeklyChallenge>, ImpForgeError> {
    engine.refresh_challenges(&colony_id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;


    fn test_engine() -> EngagementEngine {
        let dir = std::env::temp_dir().join(format!(
            "impforge_eng_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        EngagementEngine::new(&dir).expect("create engine")
    }

    // -- Daily Login tests --

    #[test]
    fn test_check_daily_login_creates_state() {
        let engine = test_engine();
        let state = engine.check_daily_login().expect("login");
        assert_eq!(state.current_streak, 1);
        assert_eq!(state.total_logins, 1);
        assert!(!state.claimed_today);
    }

    #[test]
    fn test_check_daily_login_idempotent_same_day() {
        let engine = test_engine();
        let s1 = engine.check_daily_login().expect("first");
        let s2 = engine.check_daily_login().expect("second");
        assert_eq!(s1.current_streak, s2.current_streak);
        assert_eq!(s1.total_logins, s2.total_logins);
    }

    #[test]
    fn test_claim_daily_reward() {
        let engine = test_engine();
        engine.check_daily_login().expect("login first");
        let reward = engine.claim_daily_reward().expect("claim");
        assert!(reward.dark_matter >= 100);
        assert!(reward.dark_matter <= 2000);
    }

    #[test]
    fn test_double_claim_fails() {
        let engine = test_engine();
        engine.check_daily_login().expect("login");
        engine.claim_daily_reward().expect("first claim");
        let err = engine.claim_daily_reward().unwrap_err();
        assert_eq!(err.code, "ENG_ALREADY_CLAIMED");
    }

    #[test]
    fn test_claim_without_login_fails() {
        let engine = test_engine();
        // Do not call check_daily_login
        let err = engine.claim_daily_reward().unwrap_err();
        assert_eq!(err.code, "ENG_NOT_LOGGED_IN");
    }

    #[test]
    fn test_login_calendar_records_today() {
        let engine = test_engine();
        engine.check_daily_login().expect("login");
        let calendar = engine.get_login_calendar().expect("calendar");
        assert_eq!(calendar.len(), 31);
        let today_day = Utc::now().day() as usize;
        assert!(calendar[today_day - 1]);
    }

    #[test]
    fn test_streak_info() {
        let engine = test_engine();
        engine.check_daily_login().expect("login");
        let info = engine.get_streak_info().expect("streak");
        assert_eq!(info["current_streak"], 1);
        assert_eq!(info["total_logins"], 1);
    }

    // -- LoginState::calculate_reward tests --

    #[test]
    fn test_reward_calculation_base() {
        let state = LoginState {
            current_streak: 0,
            longest_streak: 0,
            total_logins: 1,
            last_login_date: "2026-03-23".to_string(),
            monthly_calendar: vec![],
            claimed_today: false,
        };
        let reward = state.calculate_reward();
        assert_eq!(reward.dark_matter, 400);
        assert_eq!(reward.streak_multiplier, 1.0);
    }

    #[test]
    fn test_reward_day7_special() {
        let state = LoginState {
            current_streak: 7,
            longest_streak: 7,
            total_logins: 7,
            last_login_date: "2026-03-23".to_string(),
            monthly_calendar: vec![],
            claimed_today: false,
        };
        let reward = state.calculate_reward();
        assert_eq!(reward.special_reward, Some("weekly_bonus_chest".to_string()));
        assert!(reward.dark_matter > 400);
    }

    #[test]
    fn test_reward_day28_special() {
        let state = LoginState {
            current_streak: 28,
            longest_streak: 28,
            total_logins: 28,
            last_login_date: "2026-03-23".to_string(),
            monthly_calendar: vec![],
            claimed_today: false,
        };
        let reward = state.calculate_reward();
        assert_eq!(
            reward.special_reward,
            Some("monthly_legendary_mutation".to_string())
        );
    }

    #[test]
    fn test_reward_clamped_max() {
        let state = LoginState {
            current_streak: 999,
            longest_streak: 999,
            total_logins: 999,
            last_login_date: "2026-03-23".to_string(),
            monthly_calendar: vec![],
            claimed_today: false,
        };
        let reward = state.calculate_reward();
        assert!(reward.dark_matter <= 2000);
    }

    #[test]
    fn test_reward_minimum() {
        // Even at streak 0, DM = 400 which is above the 100 minimum
        let state = LoginState {
            current_streak: 0,
            longest_streak: 0,
            total_logins: 1,
            last_login_date: "2026-03-23".to_string(),
            monthly_calendar: vec![],
            claimed_today: false,
        };
        let reward = state.calculate_reward();
        assert!(reward.dark_matter >= 100);
    }

    // -- Weekly Challenges tests --

    #[test]
    fn test_weekly_challenges_generated() {
        let engine = test_engine();
        let challenges = engine
            .get_weekly_challenges("colony_1")
            .expect("challenges");
        assert_eq!(challenges.len(), 3);
        for ch in &challenges {
            assert!(!ch.completed);
            assert!(!ch.claimed);
            assert!(ch.target > 0);
            assert!(ch.reward_dm >= 1000);
            assert!(ch.reward_dm <= 2500);
        }
    }

    #[test]
    fn test_weekly_challenges_unique_types() {
        let engine = test_engine();
        let challenges = engine
            .get_weekly_challenges("colony_1")
            .expect("challenges");
        let types: Vec<_> = challenges.iter().map(|c| c.challenge_type.as_str()).collect();
        // All three should be different types
        assert_ne!(types[0], types[1]);
        assert_ne!(types[1], types[2]);
        assert_ne!(types[0], types[2]);
    }

    #[test]
    fn test_challenge_progress_update() {
        let engine = test_engine();
        let challenges = engine
            .get_weekly_challenges("colony_1")
            .expect("challenges");
        let ct = challenges[0].challenge_type.as_str().to_string();

        engine
            .update_challenge_progress("colony_1", &ct, 1)
            .expect("update");

        let updated = engine
            .get_weekly_challenges("colony_1")
            .expect("reload");
        let ch = updated
            .iter()
            .find(|c| c.challenge_type.as_str() == ct)
            .expect("find challenge");
        assert!(ch.progress >= 1);
    }

    #[test]
    fn test_challenge_claim_incomplete_fails() {
        let engine = test_engine();
        let challenges = engine
            .get_weekly_challenges("colony_1")
            .expect("challenges");
        let err = engine
            .claim_challenge_reward("colony_1", &challenges[0].id)
            .unwrap_err();
        assert_eq!(err.code, "ENG_CHAL_INCOMPLETE");
    }

    // -- Expedition tests --

    #[test]
    fn test_start_expedition() {
        let engine = test_engine();
        let exp = engine.start_expedition(50_000).expect("expedition");
        assert!(exp.duration_hours >= 1);
        assert!(exp.duration_hours <= 8);
        assert_eq!(exp.status, ExpeditionStatus::InProgress);
    }

    #[test]
    fn test_max_3_expeditions() {
        let engine = test_engine();
        engine.start_expedition(10_000).expect("exp 1");
        engine.start_expedition(20_000).expect("exp 2");
        engine.start_expedition(30_000).expect("exp 3");
        let err = engine.start_expedition(40_000).unwrap_err();
        assert_eq!(err.code, "ENG_MAX_EXPEDITIONS");
    }

    #[test]
    fn test_zero_fleet_fails() {
        let engine = test_engine();
        let err = engine.start_expedition(0).unwrap_err();
        assert_eq!(err.code, "ENG_ZERO_FLEET");
    }

    #[test]
    fn test_expedition_status_lists() {
        let engine = test_engine();
        engine.start_expedition(10_000).expect("start");
        let list = engine.expedition_status().expect("status");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].fleet_power, 10_000);
    }

    #[test]
    fn test_expedition_duration_formula() {
        // fleet_power=100000 => ratio=0 => hours=1
        let engine = test_engine();
        let fast = engine.start_expedition(100_000).expect("fast");
        assert_eq!(fast.duration_hours, 1);
    }

    #[test]
    fn test_expedition_duration_slow() {
        // fleet_power=1 => ratio~1 => hours=8
        let engine = test_engine();
        let slow = engine.start_expedition(1).expect("slow");
        assert_eq!(slow.duration_hours, 8);
    }

    // -- Productivity tests --

    #[test]
    fn test_productivity_mappings_complete() {
        let mappings = get_productivity_mappings();
        assert_eq!(mappings.len(), 10);
        assert!(mappings.iter().all(|m| m.dark_matter > 0));
        assert!(mappings.iter().all(|m| m.xp > 0));
    }

    #[test]
    fn test_log_activity_known() {
        let engine = test_engine();
        let mapping = engine.log_activity("code_committed").expect("log");
        assert_eq!(mapping.action, "code_committed");
        assert_eq!(mapping.dark_matter, 3);
        assert_eq!(mapping.xp, 40);
    }

    #[test]
    fn test_log_activity_unknown() {
        let engine = test_engine();
        let err = engine.log_activity("nonexistent").unwrap_err();
        assert_eq!(err.code, "ENG_UNKNOWN_ACTION");
    }

    // -- Save Integrity tests --

    #[test]
    fn test_save_checksum_deterministic() {
        let data = r#"{"colony":"test","resources":100}"#;
        let c1 = calculate_save_checksum(data);
        let c2 = calculate_save_checksum(data);
        assert_eq!(c1, c2);
        assert_eq!(c1.len(), 16);
    }

    #[test]
    fn test_save_checksum_differs_on_change() {
        let c1 = calculate_save_checksum(r#"{"resources":100}"#);
        let c2 = calculate_save_checksum(r#"{"resources":999}"#);
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_verify_save_integrity_pass() {
        let data = r#"{"colony":"alpha"}"#;
        let checksum = calculate_save_checksum(data);
        assert!(verify_save_integrity(data, &checksum));
    }

    #[test]
    fn test_verify_save_integrity_fail() {
        let data = r#"{"colony":"alpha"}"#;
        assert!(!verify_save_integrity(data, "0000000000000000"));
    }

    #[test]
    fn test_validate_resource_rates_valid() {
        // Level 5: max = 30 * 1.5^5 * 2 = 455.625
        assert!(validate_resource_rates(100.0, 5));
        assert!(validate_resource_rates(455.0, 5));
    }

    #[test]
    fn test_validate_resource_rates_invalid() {
        // Level 1: max = 30 * 1.5 * 2 = 90
        assert!(!validate_resource_rates(100.0, 1));
    }

    // -- Helper function tests --

    #[test]
    fn test_current_week_key_format() {
        let key = current_week_key();
        assert!(key.starts_with("20"));
        assert!(key.contains("-W"));
        assert!(key.len() >= 7);
    }

    #[test]
    fn test_scale_challenge_target() {
        let base = scale_challenge_target(&ChallengeType::BuildBuildings, 1);
        let scaled = scale_challenge_target(&ChallengeType::BuildBuildings, 10);
        assert!(scaled > base);
        // Capped at 10x base (base=3, max=30)
        let capped = scale_challenge_target(&ChallengeType::BuildBuildings, 100);
        assert!(capped <= 30);
    }

    #[test]
    fn test_expedition_reward_generation() {
        let reward = generate_expedition_reward(50_000, 4);
        assert!(reward.resources_primary > 0.0);
        assert!(reward.resources_secondary > 0.0);
        assert!(reward.dark_matter > 0);
        assert!(reward.event_log.len() >= 2);
        assert!(reward.event_log.len() <= 4);
    }

    #[test]
    fn test_challenge_type_roundtrip() {
        for ct in &ALL_CHALLENGE_TYPES {
            let s = ct.as_str();
            let back = ChallengeType::from_str(s);
            assert_eq!(ct, &back);
        }
    }

    #[test]
    fn test_expedition_status_roundtrip() {
        for status in [
            ExpeditionStatus::InProgress,
            ExpeditionStatus::Completed,
            ExpeditionStatus::Failed,
            ExpeditionStatus::Collected,
        ] {
            let s = status.as_str();
            let back = ExpeditionStatus::from_str(s);
            assert_eq!(status, back);
        }
    }

    #[test]
    fn test_engine_verify_save_delegated() {
        let engine = test_engine();
        let data = "test_data";
        let checksum = calculate_save_checksum(data);
        assert!(engine.verify_save(data, &checksum));
        assert!(!engine.verify_save(data, "bad"));
    }

    #[test]
    fn test_refresh_challenges() {
        let engine = test_engine();
        let first = engine.get_weekly_challenges("colony_1").expect("first");
        let refreshed = engine.refresh_challenges("colony_1").expect("refresh");
        assert_eq!(refreshed.len(), 3);
        // IDs should differ (new generation)
        assert_ne!(first[0].id, refreshed[0].id);
    }
}
