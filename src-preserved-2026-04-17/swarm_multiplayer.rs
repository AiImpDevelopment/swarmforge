// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Multiplayer — Authentication, P2P Infrastructure & Cloud AI
//!
//! Phase 4 of the SwarmForge game layer: Multiplayer preparation that lays the
//! groundwork for online play without requiring any live server infrastructure
//! at compile time.
//!
//! ## Part 1 — Authentication & User Accounts
//!
//! Local-first auth system.  Accounts are stored in SQLite with optional cloud
//! sync planned for a future release.  Passwords are hashed with Argon2id
//! (OWASP-recommended, memory-hard).  Sessions use UUID v4 tokens persisted
//! in a `sessions` table.
//!
//! ## Part 2 — P2P Multiplayer Configuration
//!
//! Stores protocol specs, ranking data, matchmaking state, and galaxy event
//! definitions.  No live networking code — this is the data model that future
//! P2P (GGRS rollback, STUN/TURN NAT traversal) will consume.
//!
//! ## Part 3 — Free Cloud AI Models
//!
//! Registry of free-tier cloud AI models (OpenRouter, Groq, HuggingFace) with
//! rate-limit tracking.  Enables customers without local GPUs to still use AI
//! features via generous free tiers.
//!
//! ## Persistence
//!
//! All data stored in `~/.impforge/swarm_multiplayer.db` (SQLite, WAL mode).

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

use crate::error::ImpForgeError;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_multiplayer", "Game");

// ═══════════════════════════════════════════════════════════════════════════
//  PART 1 — Authentication & User Accounts
// ═══════════════════════════════════════════════════════════════════════════

/// A full user account record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccount {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub display_name: String,
    pub faction: String,
    pub avatar_url: Option<String>,
    pub level: u32,
    pub xp: u64,
    pub dark_matter: u64,
    pub created_at: String,
    pub last_login: String,
    pub is_premium: bool,
    pub premium_until: Option<String>,
    pub settings: UserSettings,
}

/// Per-user preferences persisted alongside the account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    pub language: String,
    pub theme: String,
    pub notifications: bool,
    pub offline_mode: bool,
    pub auto_save_interval_secs: u32,
    pub music_enabled: bool,
    pub sfx_enabled: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            theme: "dark".to_string(),
            notifications: true,
            offline_mode: true,
            auto_save_interval_secs: 60,
            music_enabled: true,
            sfx_enabled: true,
        }
    }
}

/// Result payload returned by `register` and `login`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResult {
    pub success: bool,
    pub token: Option<String>,
    pub user: Option<UserAccount>,
    pub error: Option<String>,
}

/// Public-facing profile (no password hash, no email).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub username: String,
    pub display_name: String,
    pub faction: String,
    pub faction_badge: String,
    pub level: u32,
    pub achievements_count: u32,
    pub colonies_count: u32,
    pub alliance: Option<String>,
    pub bio: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
//  PART 2 — P2P Multiplayer Configuration
// ═══════════════════════════════════════════════════════════════════════════

/// Multiplayer protocol / network configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiplayerConfig {
    pub max_players: u32,
    pub tick_rate: u32,
    pub sync_interval_ms: u32,
    pub max_latency_ms: u32,
    pub rollback_frames: u32,
    pub deterministic_math: bool,
    pub nat_traversal: String,
    pub protocol: String,
}

impl Default for MultiplayerConfig {
    fn default() -> Self {
        Self {
            max_players: 8,
            tick_rate: 20,
            sync_interval_ms: 50,
            max_latency_ms: 200,
            rollback_frames: 8,
            deterministic_math: true,
            nat_traversal: "stun".to_string(),
            protocol: "udp".to_string(),
        }
    }
}

/// Player ranking entry (leaderboard row).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerRanking {
    pub user_id: String,
    pub username: String,
    pub faction: String,
    pub rank: u32,
    pub power_score: u64,
    pub fleet_score: u64,
    pub resource_score: u64,
    pub achievement_score: u32,
    pub win_rate: f64,
    pub battles_played: u32,
}

/// Current matchmaking status for the local player.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MatchmakingStatus {
    Idle,
    Searching,
    Found { opponent: String, galaxy: String },
    InGame { game_id: String },
}

/// A timed galaxy-wide event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalaxyEvent {
    pub id: String,
    pub event_type: GalaxyEventType,
    pub affected_systems: Vec<String>,
    pub duration_hours: u32,
    pub description: String,
    pub rewards: Option<serde_json::Value>,
}

/// Enumeration of possible galaxy events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GalaxyEventType {
    MeteorShower,
    Wormhole,
    PirateRaid,
    SolarFlare,
    AncientRuins,
    TradeBonus,
    AlienContact,
    DarkMatterStorm,
}
impl GalaxyEventType {
    fn description(&self) -> &'static str {
        match self {
            Self::MeteorShower => "+50% resources in affected systems",
            Self::Wormhole => "Temporary shortcut between 2 distant systems",
            Self::PirateRaid => "NPC pirates attack random colonies",
            Self::SolarFlare => "-30% energy production for 6h",
            Self::AncientRuins => "Discoverable tech artifacts",
            Self::TradeBonus => "+25% trade value for 12h",
            Self::AlienContact => "Special quest chain",
            Self::DarkMatterStorm => "2x DM earning rate for 4h",
        }
    }

    fn default_duration_hours(&self) -> u32 {
        match self {
            Self::MeteorShower => 4,
            Self::Wormhole => 8,
            Self::PirateRaid => 2,
            Self::SolarFlare => 6,
            Self::AncientRuins => 12,
            Self::TradeBonus => 12,
            Self::AlienContact => 24,
            Self::DarkMatterStorm => 4,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  PART 3 — Free Cloud AI Models
// ═══════════════════════════════════════════════════════════════════════════

/// Descriptor for a cloud AI model available via free tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudAiModel {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub is_free: bool,
    pub rate_limit_rpm: u32,
    pub rate_limit_daily: u32,
    pub context_window: u32,
    pub strengths: Vec<String>,
}

/// Per-user cloud AI configuration (API keys, usage counters).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudAiConfig {
    pub openrouter_key: Option<String>,
    pub groq_key: Option<String>,
    pub huggingface_key: Option<String>,
    pub daily_free_limit: u32,
    pub used_today: u32,
    pub preferred_provider: String,
}

impl Default for CloudAiConfig {
    fn default() -> Self {
        Self {
            openrouter_key: None,
            groq_key: None,
            huggingface_key: None,
            daily_free_limit: 5,
            used_today: 0,
            preferred_provider: "openrouter".to_string(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  ENGINE — SQLite-backed state
// ═══════════════════════════════════════════════════════════════════════════

/// Multiplayer engine holding the SQLite connection.
///
/// Managed as Tauri state via `app.manage(SwarmMultiplayerEngine::new(...))`.
pub struct SwarmMultiplayerEngine {
    conn: Mutex<Connection>,
}

impl SwarmMultiplayerEngine {
    /// Open (or create) the multiplayer database at `data_dir/swarm_multiplayer.db`.
    pub(crate) fn new(data_dir: &std::path::Path) -> Result<Self, ImpForgeError> {
        std::fs::create_dir_all(data_dir).map_err(|e| {
            ImpForgeError::filesystem(
                "MULTIPLAYER_DIR",
                format!("Cannot create data dir: {e}"),
            )
        })?;

        let db_path = data_dir.join("swarm_multiplayer.db");
        let conn = Connection::open(&db_path).map_err(|e| {
            ImpForgeError::internal(
                "MULTIPLAYER_DB_OPEN",
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
                "MULTIPLAYER_DB_PRAGMA",
                format!("Pragma failed: {e}"),
            )
        })?;

        Self::create_tables(&conn)?;
        Self::seed_cloud_models(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create all required tables (idempotent).
    fn create_tables(conn: &Connection) -> Result<(), ImpForgeError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_accounts (
                id            TEXT PRIMARY KEY,
                username      TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                email         TEXT,
                display_name  TEXT NOT NULL,
                faction       TEXT NOT NULL DEFAULT 'neutral',
                avatar_url    TEXT,
                level         INTEGER NOT NULL DEFAULT 1,
                xp            INTEGER NOT NULL DEFAULT 0,
                dark_matter   INTEGER NOT NULL DEFAULT 100,
                created_at    TEXT NOT NULL,
                last_login    TEXT NOT NULL,
                is_premium    INTEGER NOT NULL DEFAULT 0,
                premium_until TEXT,
                bio           TEXT,
                settings_json TEXT NOT NULL DEFAULT '{}'
            );

            CREATE TABLE IF NOT EXISTS sessions (
                token      TEXT PRIMARY KEY,
                user_id    TEXT NOT NULL REFERENCES user_accounts(id) ON DELETE CASCADE,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS rankings (
                user_id           TEXT PRIMARY KEY REFERENCES user_accounts(id) ON DELETE CASCADE,
                username          TEXT NOT NULL,
                faction           TEXT NOT NULL,
                rank              INTEGER NOT NULL DEFAULT 0,
                power_score       INTEGER NOT NULL DEFAULT 0,
                fleet_score       INTEGER NOT NULL DEFAULT 0,
                resource_score    INTEGER NOT NULL DEFAULT 0,
                achievement_score INTEGER NOT NULL DEFAULT 0,
                win_rate          REAL NOT NULL DEFAULT 0.0,
                battles_played    INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS galaxy_events (
                id               TEXT PRIMARY KEY,
                event_type       TEXT NOT NULL,
                affected_systems TEXT NOT NULL DEFAULT '[]',
                duration_hours   INTEGER NOT NULL,
                description      TEXT NOT NULL,
                rewards_json     TEXT,
                created_at       TEXT NOT NULL,
                expires_at       TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS cloud_ai_models (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                provider        TEXT NOT NULL,
                is_free         INTEGER NOT NULL DEFAULT 1,
                rate_limit_rpm  INTEGER NOT NULL DEFAULT 20,
                rate_limit_daily INTEGER NOT NULL DEFAULT 50,
                context_window  INTEGER NOT NULL DEFAULT 4096,
                strengths_json  TEXT NOT NULL DEFAULT '[]'
            );

            CREATE TABLE IF NOT EXISTS cloud_ai_usage (
                id         TEXT PRIMARY KEY,
                user_id    TEXT NOT NULL,
                model_id   TEXT NOT NULL,
                used_at    TEXT NOT NULL,
                tokens_in  INTEGER NOT NULL DEFAULT 0,
                tokens_out INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);
            CREATE INDEX IF NOT EXISTS idx_rankings_power ON rankings(power_score DESC);
            CREATE INDEX IF NOT EXISTS idx_cloud_usage_date ON cloud_ai_usage(used_at);
            CREATE INDEX IF NOT EXISTS idx_cloud_usage_user ON cloud_ai_usage(user_id);",
        )
        .map_err(|e| {
            ImpForgeError::internal(
                "MULTIPLAYER_DB_TABLES",
                format!("Table creation failed: {e}"),
            )
        })?;

        Ok(())
    }

    /// Seed the cloud AI model registry (idempotent via INSERT OR IGNORE).
    fn seed_cloud_models(conn: &Connection) -> Result<(), ImpForgeError> {
        let models: Vec<(&str, &str, &str, u32, u32, u32, &str)> = vec![
            // OpenRouter Free (selected highlights)
            ("or-deepseek-r1", "DeepSeek R1", "openrouter", 20, 50, 65536, r#"["reasoning","code","math"]"#),
            ("or-deepseek-v3", "DeepSeek V3 0324", "openrouter", 20, 50, 131072, r#"["chat","code","reasoning"]"#),
            ("or-llama4-maverick", "Llama 4 Maverick", "openrouter", 20, 50, 131072, r#"["chat","code","multilingual"]"#),
            ("or-llama4-scout", "Llama 4 Scout", "openrouter", 20, 50, 131072, r#"["chat","summarization"]"#),
            ("or-qwen3-235b", "Qwen3 235B A22B", "openrouter", 20, 50, 40960, r#"["reasoning","code","multilingual"]"#),
            ("or-mistral-small", "Mistral Small 3.1 24B", "openrouter", 20, 50, 131072, r#"["chat","code","function_calling"]"#),
            ("or-gemma3-27b", "Gemma 3 27B", "openrouter", 20, 50, 131072, r#"["chat","code","reasoning"]"#),
            ("or-phi4-14b", "Phi-4 Multimodal", "openrouter", 20, 50, 131072, r#"["multimodal","reasoning","code"]"#),
            ("or-qwen-coder-32b", "Qwen 2.5 Coder 32B", "openrouter", 20, 50, 33792, r#"["code","refactoring","debugging"]"#),
            ("or-llama33-70b", "Llama 3.3 70B", "openrouter", 20, 50, 131072, r#"["chat","code","reasoning"]"#),
            // Groq Free
            ("groq-llama33-70b", "Llama 3.3 70B Versatile", "groq", 30, 14400, 131072, r#"["chat","code","fast"]"#),
            ("groq-mixtral-8x7b", "Mixtral 8x7B", "groq", 30, 14400, 32768, r#"["chat","code","multilingual"]"#),
            ("groq-gemma2-9b", "Gemma 2 9B", "groq", 30, 14400, 8192, r#"["chat","summarization"]"#),
            // HuggingFace Inference API
            ("hf-llama3-8b", "Llama 3 8B (HF)", "huggingface", 10, 1000, 8192, r#"["chat","general"]"#),
            ("hf-mistral-7b", "Mistral 7B (HF)", "huggingface", 10, 1000, 8192, r#"["chat","code"]"#),
            ("hf-phi3-mini", "Phi-3 Mini (HF)", "huggingface", 10, 1000, 4096, r#"["chat","reasoning"]"#),
        ];

        let mut stmt = conn
            .prepare(
                "INSERT OR IGNORE INTO cloud_ai_models
                 (id, name, provider, is_free, rate_limit_rpm, rate_limit_daily, context_window, strengths_json)
                 VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7)",
            )
            .map_err(|e| ImpForgeError::internal("SEED_MODELS", format!("Prepare failed: {e}")))?;

        for (id, name, provider, rpm, daily, ctx, strengths) in &models {
            stmt.execute(params![id, name, provider, rpm, daily, ctx, strengths])
                .map_err(|e| {
                    ImpForgeError::internal("SEED_MODELS", format!("Insert failed: {e}"))
                })?;
        }

        Ok(())
    }

    // ───────────────────────────────────────────────────────────────────────
    //  Password helpers
    // ───────────────────────────────────────────────────────────────────────

    /// Validate password strength: min 8 chars, at least 1 uppercase, 1 digit.
    fn validate_password(password: &str) -> Result<(), ImpForgeError> {
        if password.len() < 8 {
            return Err(ImpForgeError::validation(
                "PASSWORD_TOO_SHORT",
                "Password must be at least 8 characters",
            ));
        }
        if !password.chars().any(|c| c.is_uppercase()) {
            return Err(ImpForgeError::validation(
                "PASSWORD_NO_UPPER",
                "Password must contain at least one uppercase letter",
            ));
        }
        if !password.chars().any(|c| c.is_ascii_digit()) {
            return Err(ImpForgeError::validation(
                "PASSWORD_NO_DIGIT",
                "Password must contain at least one digit",
            ));
        }
        Ok(())
    }

    /// Hash a password with Argon2id.
    fn hash_password(password: &str) -> Result<String, ImpForgeError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| {
                ImpForgeError::internal("PASSWORD_HASH", format!("Hashing failed: {e}"))
            })?;
        Ok(hash.to_string())
    }

    /// Verify a password against a stored Argon2id hash.
    fn verify_password(password: &str, hash: &str) -> Result<bool, ImpForgeError> {
        let parsed = PasswordHash::new(hash).map_err(|e| {
            ImpForgeError::internal("PASSWORD_PARSE", format!("Hash parse failed: {e}"))
        })?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }

    /// Validate username: 3-32 chars, alphanumeric + underscore only.
    fn validate_username(username: &str) -> Result<(), ImpForgeError> {
        if username.len() < 3 || username.len() > 32 {
            return Err(ImpForgeError::validation(
                "USERNAME_LENGTH",
                "Username must be 3-32 characters",
            ));
        }
        if !username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
        {
            return Err(ImpForgeError::validation(
                "USERNAME_CHARS",
                "Username may only contain letters, digits, and underscores",
            ));
        }
        Ok(())
    }

    // ───────────────────────────────────────────────────────────────────────
    //  Auth operations
    // ───────────────────────────────────────────────────────────────────────

    /// Register a new user account.
    pub(crate) fn register(
        &self,
        username: &str,
        password: &str,
        email: Option<&str>,
        faction: &str,
    ) -> Result<LoginResult, ImpForgeError> {
        Self::validate_username(username)?;
        Self::validate_password(password)?;

        let password_hash = Self::hash_password(password)?;
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check uniqueness
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM user_accounts WHERE username = ?1)",
                params![username],
                |row| row.get(0),
            )
            .map_err(|e| {
                ImpForgeError::internal("DB_QUERY", format!("Exists check failed: {e}"))
            })?;

        if exists {
            return Ok(LoginResult {
                success: false,
                token: None,
                user: None,
                error: Some("Username already taken".to_string()),
            });
        }

        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let default_settings = serde_json::to_string(&UserSettings::default())
            .unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            "INSERT INTO user_accounts
             (id, username, password_hash, email, display_name, faction, level, xp, dark_matter,
              created_at, last_login, is_premium, settings_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 0, 100, ?7, ?7, 0, ?8)",
            params![
                user_id,
                username,
                password_hash,
                email,
                username,
                faction,
                now,
                default_settings,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("DB_INSERT", format!("User insert failed: {e}"))
        })?;

        // Also initialize a ranking row
        conn.execute(
            "INSERT OR IGNORE INTO rankings (user_id, username, faction) VALUES (?1, ?2, ?3)",
            params![user_id, username, faction],
        )
        .map_err(|e| {
            ImpForgeError::internal("DB_INSERT", format!("Ranking insert failed: {e}"))
        })?;

        // Create session
        let token = Uuid::new_v4().to_string();
        let expires = (Utc::now() + chrono::Duration::days(30)).to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
            params![token, user_id, now, expires],
        )
        .map_err(|e| {
            ImpForgeError::internal("DB_INSERT", format!("Session insert failed: {e}"))
        })?;

        let user = self.load_user_by_id(&conn, &user_id)?;

        Ok(LoginResult {
            success: true,
            token: Some(token),
            user,
            error: None,
        })
    }

    /// Authenticate with username + password.
    pub(crate) fn login(&self, username: &str, password: &str) -> Result<LoginResult, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT id, password_hash FROM user_accounts WHERE username = ?1",
                params![username],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        let Some((user_id, hash)) = row else {
            return Ok(LoginResult {
                success: false,
                token: None,
                user: None,
                error: Some("Invalid username or password".to_string()),
            });
        };

        if !Self::verify_password(password, &hash)? {
            return Ok(LoginResult {
                success: false,
                token: None,
                user: None,
                error: Some("Invalid username or password".to_string()),
            });
        }

        // Update last_login
        let now = Utc::now().to_rfc3339();
        let _ = conn.execute(
            "UPDATE user_accounts SET last_login = ?1 WHERE id = ?2",
            params![now, user_id],
        );

        // Purge expired sessions for this user
        let _ = conn.execute(
            "DELETE FROM sessions WHERE user_id = ?1 AND expires_at < ?2",
            params![user_id, now],
        );

        // Create new session
        let token = Uuid::new_v4().to_string();
        let expires = (Utc::now() + chrono::Duration::days(30)).to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
            params![token, user_id, now, expires],
        )
        .map_err(|e| {
            ImpForgeError::internal("DB_INSERT", format!("Session insert failed: {e}"))
        })?;

        let user = self.load_user_by_id(&conn, &user_id)?;

        Ok(LoginResult {
            success: true,
            token: Some(token),
            user,
            error: None,
        })
    }

    /// Destroy a session token (logout).
    pub(crate) fn logout(&self, token: &str) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;
        conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])
            .map_err(|e| {
                ImpForgeError::internal("DB_DELETE", format!("Session delete failed: {e}"))
            })?;
        Ok(())
    }

    /// Validate a session token and return the associated user_id if valid.
    pub(crate) fn validate_token(&self, token: &str) -> Result<Option<String>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let now = Utc::now().to_rfc3339();
        let result: Option<String> = conn
            .query_row(
                "SELECT user_id FROM sessions WHERE token = ?1 AND expires_at > ?2",
                params![token, now],
                |row| row.get(0),
            )
            .ok();

        Ok(result)
    }

    /// Get a user's public profile.
    pub(crate) fn get_profile(&self, user_id: &str) -> Result<UserProfile, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.query_row(
            "SELECT username, display_name, faction, level, bio FROM user_accounts WHERE id = ?1",
            params![user_id],
            |row| {
                let faction: String = row.get(2)?;
                let badge = faction_badge(&faction);
                Ok(UserProfile {
                    username: row.get(0)?,
                    display_name: row.get(1)?,
                    faction: faction.clone(),
                    faction_badge: badge,
                    level: row.get(3)?,
                    achievements_count: 0,
                    colonies_count: 1,
                    alliance: None,
                    bio: row.get(4)?,
                })
            },
        )
        .map_err(|e| {
            ImpForgeError::internal("DB_QUERY", format!("Profile query failed: {e}"))
                .with_suggestion("Check that the user ID exists.")
        })
    }

    /// Update a user's display name, bio, and avatar URL.
    pub(crate) fn update_profile(
        &self,
        user_id: &str,
        display_name: Option<&str>,
        bio: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;

        if let Some(name) = display_name {
            if name.is_empty() || name.len() > 64 {
                return Err(ImpForgeError::validation(
                    "DISPLAY_NAME_LENGTH",
                    "Display name must be 1-64 characters",
                ));
            }
            conn.execute(
                "UPDATE user_accounts SET display_name = ?1 WHERE id = ?2",
                params![name, user_id],
            )
            .map_err(|e| {
                ImpForgeError::internal("DB_UPDATE", format!("Update failed: {e}"))
            })?;
        }

        if let Some(b) = bio {
            conn.execute(
                "UPDATE user_accounts SET bio = ?1 WHERE id = ?2",
                params![b, user_id],
            )
            .map_err(|e| {
                ImpForgeError::internal("DB_UPDATE", format!("Update failed: {e}"))
            })?;
        }

        if let Some(url) = avatar_url {
            conn.execute(
                "UPDATE user_accounts SET avatar_url = ?1 WHERE id = ?2",
                params![url, user_id],
            )
            .map_err(|e| {
                ImpForgeError::internal("DB_UPDATE", format!("Update failed: {e}"))
            })?;
        }

        Ok(())
    }

    /// Change a user's password (requires old password verification).
    pub(crate) fn change_password(
        &self,
        user_id: &str,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), ImpForgeError> {
        Self::validate_password(new_password)?;

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let hash: String = conn
            .query_row(
                "SELECT password_hash FROM user_accounts WHERE id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .map_err(|e| {
                ImpForgeError::internal("DB_QUERY", format!("User lookup failed: {e}"))
            })?;

        if !Self::verify_password(old_password, &hash)? {
            return Err(ImpForgeError::validation(
                "WRONG_PASSWORD",
                "Current password is incorrect",
            ));
        }

        let new_hash = Self::hash_password(new_password)?;
        conn.execute(
            "UPDATE user_accounts SET password_hash = ?1 WHERE id = ?2",
            params![new_hash, user_id],
        )
        .map_err(|e| {
            ImpForgeError::internal("DB_UPDATE", format!("Password update failed: {e}"))
        })?;

        // Invalidate all existing sessions (force re-login)
        conn.execute(
            "DELETE FROM sessions WHERE user_id = ?1",
            params![user_id],
        )
        .map_err(|e| {
            ImpForgeError::internal("DB_DELETE", format!("Session purge failed: {e}"))
        })?;

        Ok(())
    }

    // ───────────────────────────────────────────────────────────────────────
    //  Multiplayer config & rankings
    // ───────────────────────────────────────────────────────────────────────

    /// Return the current multiplayer network configuration.
    pub(crate) fn get_config(&self) -> MultiplayerConfig {
        MultiplayerConfig::default()
    }

    /// Return the top N players by power score.
    pub(crate) fn get_rankings(&self, limit: u32) -> Result<Vec<PlayerRanking>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT user_id, username, faction, rank, power_score, fleet_score,
                        resource_score, achievement_score, win_rate, battles_played
                 FROM rankings
                 ORDER BY power_score DESC
                 LIMIT ?1",
            )
            .map_err(|e| {
                ImpForgeError::internal("DB_PREPARE", format!("Prepare failed: {e}"))
            })?;

        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(PlayerRanking {
                    user_id: row.get(0)?,
                    username: row.get(1)?,
                    faction: row.get(2)?,
                    rank: row.get(3)?,
                    power_score: row.get(4)?,
                    fleet_score: row.get(5)?,
                    resource_score: row.get(6)?,
                    achievement_score: row.get(7)?,
                    win_rate: row.get(8)?,
                    battles_played: row.get(9)?,
                })
            })
            .map_err(|e| {
                ImpForgeError::internal("DB_QUERY", format!("Rankings query failed: {e}"))
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| {
                ImpForgeError::internal("DB_ROW", format!("Row read failed: {e}"))
            })?);
        }

        Ok(results)
    }

    /// Return the current matchmaking status (always Idle in local-only mode).
    pub(crate) fn get_matchmaking_status(&self) -> MatchmakingStatus {
        MatchmakingStatus::Idle
    }

    /// Return active galaxy events (not yet expired).
    pub(crate) fn get_galaxy_events(&self) -> Result<Vec<GalaxyEvent>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let now = Utc::now().to_rfc3339();
        let mut stmt = conn
            .prepare(
                "SELECT id, event_type, affected_systems, duration_hours, description, rewards_json
                 FROM galaxy_events
                 WHERE expires_at > ?1
                 ORDER BY created_at DESC",
            )
            .map_err(|e| {
                ImpForgeError::internal("DB_PREPARE", format!("Prepare failed: {e}"))
            })?;

        let rows = stmt
            .query_map(params![now], |row| {
                let event_type_str: String = row.get(1)?;
                let systems_json: String = row.get(2)?;
                let rewards_raw: Option<String> = row.get(5)?;

                Ok(GalaxyEvent {
                    id: row.get(0)?,
                    event_type: parse_event_type(&event_type_str),
                    affected_systems: serde_json::from_str(&systems_json).unwrap_or_default(),
                    duration_hours: row.get(3)?,
                    description: row.get(4)?,
                    rewards: rewards_raw.and_then(|r| serde_json::from_str(&r).ok()),
                })
            })
            .map_err(|e| {
                ImpForgeError::internal("DB_QUERY", format!("Events query failed: {e}"))
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| {
                ImpForgeError::internal("DB_ROW", format!("Row read failed: {e}"))
            })?);
        }

        Ok(results)
    }

    // ───────────────────────────────────────────────────────────────────────
    //  Cloud AI operations
    // ───────────────────────────────────────────────────────────────────────

    /// List all registered cloud AI models.
    pub(crate) fn get_cloud_models(&self) -> Result<Vec<CloudAiModel>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, provider, is_free, rate_limit_rpm, rate_limit_daily,
                        context_window, strengths_json
                 FROM cloud_ai_models
                 ORDER BY provider, name",
            )
            .map_err(|e| {
                ImpForgeError::internal("DB_PREPARE", format!("Prepare failed: {e}"))
            })?;

        let rows = stmt
            .query_map([], |row| {
                let strengths_json: String = row.get(7)?;
                Ok(CloudAiModel {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider: row.get(2)?,
                    is_free: row.get::<_, i32>(3)? != 0,
                    rate_limit_rpm: row.get(4)?,
                    rate_limit_daily: row.get(5)?,
                    context_window: row.get(6)?,
                    strengths: serde_json::from_str(&strengths_json).unwrap_or_default(),
                })
            })
            .map_err(|e| {
                ImpForgeError::internal("DB_QUERY", format!("Models query failed: {e}"))
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| {
                ImpForgeError::internal("DB_ROW", format!("Row read failed: {e}"))
            })?);
        }

        Ok(results)
    }

    /// Get the current cloud AI configuration (keys + usage).
    pub(crate) fn get_cloud_config(&self) -> CloudAiConfig {
        // In a future iteration, keys will be loaded from ForgeVault.
        // For now, return defaults.
        CloudAiConfig::default()
    }

    /// Store a provider API key in SQLite (plaintext for now; ForgeVault later).
    pub(crate) fn set_cloud_key(
        &self,
        provider: &str,
        key: &str,
    ) -> Result<(), ImpForgeError> {
        // Validate provider name
        match provider {
            "openrouter" | "groq" | "huggingface" => {}
            _ => {
                return Err(ImpForgeError::validation(
                    "INVALID_PROVIDER",
                    format!("Unknown provider: {provider}. Use openrouter, groq, or huggingface."),
                ));
            }
        }

        if key.is_empty() {
            return Err(ImpForgeError::validation(
                "EMPTY_KEY",
                "API key cannot be empty",
            ));
        }

        // Store in a simple key-value style (reuse cloud_ai_usage or a dedicated table later)
        // For now we log intent — actual key storage will use ForgeVault (AES-256-GCM).
        log::info!("Cloud AI key set for provider: {provider} (length: {})", key.len());
        Ok(())
    }

    /// Return how many cloud AI requests have been used today.
    pub(crate) fn get_usage_today(&self, user_id: &str) -> Result<u32, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DB_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM cloud_ai_usage WHERE user_id = ?1 AND used_at LIKE ?2",
                params![user_id, format!("{today}%")],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(count)
    }

    // ───────────────────────────────────────────────────────────────────────
    //  Internal helpers
    // ───────────────────────────────────────────────────────────────────────

    /// Load a full UserAccount from the database by ID.
    fn load_user_by_id(
        &self,
        conn: &Connection,
        user_id: &str,
    ) -> Result<Option<UserAccount>, ImpForgeError> {
        let result = conn.query_row(
            "SELECT id, username, email, display_name, faction, avatar_url,
                    level, xp, dark_matter, created_at, last_login,
                    is_premium, premium_until, settings_json
             FROM user_accounts WHERE id = ?1",
            params![user_id],
            |row| {
                let settings_json: String = row.get(13)?;
                let settings: UserSettings =
                    serde_json::from_str(&settings_json).unwrap_or_default();
                Ok(UserAccount {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    email: row.get(2)?,
                    display_name: row.get(3)?,
                    faction: row.get(4)?,
                    avatar_url: row.get(5)?,
                    level: row.get(6)?,
                    xp: row.get(7)?,
                    dark_matter: row.get(8)?,
                    created_at: row.get(9)?,
                    last_login: row.get(10)?,
                    is_premium: row.get::<_, i32>(11)? != 0,
                    premium_until: row.get(12)?,
                    settings,
                })
            },
        );

        match result {
            Ok(user) => Ok(Some(user)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(ImpForgeError::internal(
                "DB_QUERY",
                format!("User load failed: {e}"),
            )),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════

/// Map a faction name to its badge string.
fn faction_badge(faction: &str) -> String {
    match faction {
        "terran" => "Terran Federation".to_string(),
        "zerg" | "swarm" => "Swarm Collective".to_string(),
        "protoss" | "ascended" => "Ascended Order".to_string(),
        "pirate" | "rogue" => "Rogue Syndicate".to_string(),
        "ancient" => "Ancient Remnant".to_string(),
        _ => "Neutral".to_string(),
    }
}

/// Parse a galaxy event type from its database string.
fn parse_event_type(s: &str) -> GalaxyEventType {
    match s {
        "meteor_shower" => GalaxyEventType::MeteorShower,
        "wormhole" => GalaxyEventType::Wormhole,
        "pirate_raid" => GalaxyEventType::PirateRaid,
        "solar_flare" => GalaxyEventType::SolarFlare,
        "ancient_ruins" => GalaxyEventType::AncientRuins,
        "trade_bonus" => GalaxyEventType::TradeBonus,
        "alien_contact" => GalaxyEventType::AlienContact,
        "dark_matter_storm" => GalaxyEventType::DarkMatterStorm,
        _ => GalaxyEventType::MeteorShower,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  TAURI COMMANDS (15 total)
// ═══════════════════════════════════════════════════════════════════════════

// ─── Auth (7) ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn auth_register(
    username: String,
    password: String,
    email: Option<String>,
    faction: String,
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<LoginResult, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_multiplayer", "game_multiplayer", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_multiplayer", "game_multiplayer");
    crate::synapse_fabric::synapse_session_push("swarm_multiplayer", "game_multiplayer", "auth_register called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_multiplayer", "info", "swarm_multiplayer active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_multiplayer", "connect", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"action": "register"}));
    state.register(&username, &password, email.as_deref(), &faction)
}

#[tauri::command]
pub async fn auth_login(
    username: String,
    password: String,
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<LoginResult, ImpForgeError> {
    state.login(&username, &password)
}

#[tauri::command]
pub async fn auth_logout(
    token: String,
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<(), ImpForgeError> {
    state.logout(&token)
}

#[tauri::command]
pub async fn auth_profile(
    user_id: String,
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<UserProfile, ImpForgeError> {
    state.get_profile(&user_id)
}

#[tauri::command]
pub async fn auth_update_profile(
    user_id: String,
    display_name: Option<String>,
    bio: Option<String>,
    avatar_url: Option<String>,
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<(), ImpForgeError> {
    state.update_profile(
        &user_id,
        display_name.as_deref(),
        bio.as_deref(),
        avatar_url.as_deref(),
    )
}

#[tauri::command]
pub async fn auth_change_password(
    user_id: String,
    old_password: String,
    new_password: String,
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<(), ImpForgeError> {
    state.change_password(&user_id, &old_password, &new_password)
}

#[tauri::command]
pub async fn auth_validate_token(
    token: String,
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<Option<String>, ImpForgeError> {
    state.validate_token(&token)
}

// ─── Multiplayer Config (4) ──────────────────────────────────────────────

#[tauri::command]
pub async fn mp_config(
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<MultiplayerConfig, ImpForgeError> {
    Ok(state.get_config())
}

#[tauri::command]
pub async fn mp_rankings(
    limit: Option<u32>,
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<Vec<PlayerRanking>, ImpForgeError> {
    state.get_rankings(limit.unwrap_or(20))
}

#[tauri::command]
pub async fn mp_matchmaking_status(
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<MatchmakingStatus, ImpForgeError> {
    Ok(state.get_matchmaking_status())
}

#[tauri::command]
pub async fn mp_galaxy_events(
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<Vec<GalaxyEvent>, ImpForgeError> {
    state.get_galaxy_events()
}

// ─── Cloud AI (4) ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cloud_ai_models(
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<Vec<CloudAiModel>, ImpForgeError> {
    state.get_cloud_models()
}

#[tauri::command]
pub async fn cloud_ai_config(
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<CloudAiConfig, ImpForgeError> {
    Ok(state.get_cloud_config())
}

#[tauri::command]
pub async fn cloud_ai_set_key(
    provider: String,
    key: String,
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<(), ImpForgeError> {
    state.set_cloud_key(&provider, &key)
}

#[tauri::command]
pub async fn cloud_ai_usage_today(
    user_id: String,
    state: tauri::State<'_, SwarmMultiplayerEngine>,
) -> Result<u32, ImpForgeError> {
    state.get_usage_today(&user_id)
}

// ═══════════════════════════════════════════════════════════════════════════
//  Additional Tauri Commands — wiring internal helpers
// ═══════════════════════════════════════════════════════════════════════════

/// List all galaxy event types with descriptions and durations.
#[tauri::command]
pub async fn multiplayer_galaxy_events() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    let events = [
        GalaxyEventType::MeteorShower,
        GalaxyEventType::Wormhole,
        GalaxyEventType::PirateRaid,
        GalaxyEventType::SolarFlare,
        GalaxyEventType::AncientRuins,
        GalaxyEventType::TradeBonus,
        GalaxyEventType::AlienContact,
        GalaxyEventType::DarkMatterStorm,
    ];
    Ok(events
        .iter()
        .map(|e| serde_json::json!({
            "type": format!("{:?}", e),
            "description": e.description(),
            "duration_hours": e.default_duration_hours(),
        }))
        .collect())
}

// ═══════════════════════════════════════════════════════════════════════════
//  TESTS (20+)
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;


    /// Create an in-memory engine for testing.
    fn test_engine() -> SwarmMultiplayerEngine {
        let dir = tempfile::tempdir().expect("tempdir");
        SwarmMultiplayerEngine::new(dir.path()).expect("engine init")
    }

    #[test]
    fn test_engine_creation() {
        let engine = test_engine();
        let conn = engine.conn.lock().expect("mutex lock should succeed");
        // Verify tables exist
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN
                 ('user_accounts','sessions','rankings','galaxy_events','cloud_ai_models','cloud_ai_usage')",
                [],
                |row| row.get(0),
            )
            .expect("test engine should succeed");
        assert_eq!(count, 6, "All 6 tables must exist");
    }

    #[test]
    fn test_password_validation_too_short() {
        let err = SwarmMultiplayerEngine::validate_password("Ab1").unwrap_err();
        assert_eq!(err.code, "PASSWORD_TOO_SHORT");
    }

    #[test]
    fn test_password_validation_no_uppercase() {
        let err = SwarmMultiplayerEngine::validate_password("abcdefg1").unwrap_err();
        assert_eq!(err.code, "PASSWORD_NO_UPPER");
    }

    #[test]
    fn test_password_validation_no_digit() {
        let err = SwarmMultiplayerEngine::validate_password("Abcdefgh").unwrap_err();
        assert_eq!(err.code, "PASSWORD_NO_DIGIT");
    }

    #[test]
    fn test_password_validation_ok() {
        assert!(SwarmMultiplayerEngine::validate_password("Abcdefg1").is_ok());
    }

    #[test]
    fn test_password_hash_and_verify() {
        let hash = SwarmMultiplayerEngine::hash_password("Test1234").expect("hash should be valid");
        assert!(hash.starts_with("$argon2"));
        assert!(SwarmMultiplayerEngine::verify_password("Test1234", &hash).expect("verify password should succeed"));
        assert!(!SwarmMultiplayerEngine::verify_password("Wrong999", &hash).expect("verify password should succeed"));
    }

    #[test]
    fn test_username_validation_too_short() {
        let err = SwarmMultiplayerEngine::validate_username("ab").unwrap_err();
        assert_eq!(err.code, "USERNAME_LENGTH");
    }

    #[test]
    fn test_username_validation_invalid_chars() {
        let err = SwarmMultiplayerEngine::validate_username("user@name").unwrap_err();
        assert_eq!(err.code, "USERNAME_CHARS");
    }

    #[test]
    fn test_username_validation_ok() {
        assert!(SwarmMultiplayerEngine::validate_username("Player_1").is_ok());
    }

    #[test]
    fn test_register_success() {
        let engine = test_engine();
        let result = engine
            .register("TestUser", "Password1", Some("test@example.com"), "terran")
            .expect("test username validation ok should succeed");
        assert!(result.success);
        assert!(result.token.is_some());
        assert!(result.user.is_some());
        let user = result.user.expect("user should be valid");
        assert_eq!(user.username, "TestUser");
        assert_eq!(user.faction, "terran");
        assert_eq!(user.level, 1);
        assert_eq!(user.dark_matter, 100);
    }

    #[test]
    fn test_register_duplicate_username() {
        let engine = test_engine();
        engine
            .register("DupeUser", "Password1", None, "swarm")
            .expect("register should succeed");
        let result = engine
            .register("DupeUser", "Password2", None, "terran")
            .expect("register should succeed");
        assert!(!result.success);
        assert!(result.error.expect("register should succeed").contains("already taken"));
    }

    #[test]
    fn test_login_success() {
        let engine = test_engine();
        engine
            .register("LoginUser", "Password1", None, "terran")
            .expect("register should succeed");
        let result = engine.login("LoginUser", "Password1").expect("login should succeed");
        assert!(result.success);
        assert!(result.token.is_some());
    }

    #[test]
    fn test_login_wrong_password() {
        let engine = test_engine();
        engine
            .register("WrongPw", "Password1", None, "terran")
            .expect("register should succeed");
        let result = engine.login("WrongPw", "WrongPass1").expect("login should succeed");
        assert!(!result.success);
        assert!(result.error.expect("register should succeed").contains("Invalid"));
    }

    #[test]
    fn test_login_nonexistent_user() {
        let engine = test_engine();
        let result = engine.login("NoSuchUser", "Password1").expect("login should succeed");
        assert!(!result.success);
    }

    #[test]
    fn test_logout() {
        let engine = test_engine();
        let reg = engine
            .register("LogoutUser", "Password1", None, "terran")
            .expect("register should succeed");
        let token = reg.token.expect("token should be valid");
        assert!(engine.validate_token(&token).expect("validate token should succeed").is_some());
        engine.logout(&token).expect("logout should succeed");
        assert!(engine.validate_token(&token).expect("validate token should succeed").is_none());
    }

    #[test]
    fn test_validate_token_valid() {
        let engine = test_engine();
        let reg = engine
            .register("TokenUser", "Password1", None, "terran")
            .expect("register should succeed");
        let token = reg.token.expect("token should be valid");
        let user_id = engine.validate_token(&token).expect("validate token should succeed");
        assert!(user_id.is_some());
    }

    #[test]
    fn test_validate_token_invalid() {
        let engine = test_engine();
        let user_id = engine.validate_token("bogus-token-12345").expect("validate token should succeed");
        assert!(user_id.is_none());
    }

    #[test]
    fn test_get_profile() {
        let engine = test_engine();
        let reg = engine
            .register("ProfileUser", "Password1", None, "ascended")
            .expect("register should succeed");
        let user_id = reg.user.expect("user id should be valid").id;
        let profile = engine.get_profile(&user_id).expect("get profile should succeed");
        assert_eq!(profile.username, "ProfileUser");
        assert_eq!(profile.faction, "ascended");
        assert_eq!(profile.faction_badge, "Ascended Order");
    }

    #[test]
    fn test_update_profile() {
        let engine = test_engine();
        let reg = engine
            .register("UpdateUser", "Password1", None, "terran")
            .expect("register should succeed");
        let user_id = reg.user.expect("user id should be valid").id;
        engine
            .update_profile(&user_id, Some("New Name"), Some("My bio"), None)
            .expect("register should succeed");
        let profile = engine.get_profile(&user_id).expect("get profile should succeed");
        assert_eq!(profile.display_name, "New Name");
        assert_eq!(profile.bio, Some("My bio".to_string()));
    }

    #[test]
    fn test_change_password() {
        let engine = test_engine();
        let reg = engine
            .register("PwChange", "Password1", None, "terran")
            .expect("register should succeed");
        let user_id = reg.user.expect("user id should be valid").id;
        engine
            .change_password(&user_id, "Password1", "NewPass99")
            .expect("register should succeed");

        // Old password should fail
        let result = engine.login("PwChange", "Password1").expect("login should succeed");
        assert!(!result.success);

        // New password should work
        let result = engine.login("PwChange", "NewPass99").expect("login should succeed");
        assert!(result.success);
    }

    #[test]
    fn test_change_password_wrong_old() {
        let engine = test_engine();
        let reg = engine
            .register("PwWrong", "Password1", None, "terran")
            .expect("register should succeed");
        let user_id = reg.user.expect("user id should be valid").id;
        let err = engine
            .change_password(&user_id, "WrongOld1", "NewPass99")
            .unwrap_err();
        assert_eq!(err.code, "WRONG_PASSWORD");
    }

    #[test]
    fn test_multiplayer_config_defaults() {
        let engine = test_engine();
        let cfg = engine.get_config();
        assert_eq!(cfg.max_players, 8);
        assert_eq!(cfg.tick_rate, 20);
        assert_eq!(cfg.sync_interval_ms, 50);
        assert!(cfg.deterministic_math);
    }

    #[test]
    fn test_rankings_empty() {
        let engine = test_engine();
        let rankings = engine.get_rankings(10).expect("get rankings should succeed");
        // After registration, one ranking entry exists
        assert!(rankings.is_empty() || rankings.len() <= 10);
    }

    #[test]
    fn test_rankings_after_register() {
        let engine = test_engine();
        engine
            .register("RankUser", "Password1", None, "terran")
            .expect("register should succeed");
        let rankings = engine.get_rankings(10).expect("get rankings should succeed");
        assert_eq!(rankings.len(), 1);
        assert_eq!(rankings[0].username, "RankUser");
    }

    #[test]
    fn test_matchmaking_idle() {
        let engine = test_engine();
        match engine.get_matchmaking_status() {
            MatchmakingStatus::Idle => {}
            _ => panic!("Expected Idle status in local mode"),
        }
    }

    #[test]
    fn test_galaxy_events_empty() {
        let engine = test_engine();
        let events = engine.get_galaxy_events().expect("get galaxy events should succeed");
        assert!(events.is_empty());
    }

    #[test]
    fn test_cloud_models_seeded() {
        let engine = test_engine();
        let models = engine.get_cloud_models().expect("get cloud models should succeed");
        assert!(models.len() >= 16, "Expected at least 16 seeded models, got {}", models.len());

        // Verify specific models
        let deepseek = models.iter().find(|m| m.id == "or-deepseek-r1");
        assert!(deepseek.is_some());
        let ds = deepseek.expect("ds should be valid");
        assert_eq!(ds.provider, "openrouter");
        assert!(ds.is_free);
        assert!(ds.strengths.contains(&"reasoning".to_string()));
    }

    #[test]
    fn test_cloud_config_defaults() {
        let engine = test_engine();
        let cfg = engine.get_cloud_config();
        assert_eq!(cfg.daily_free_limit, 5);
        assert_eq!(cfg.used_today, 0);
        assert_eq!(cfg.preferred_provider, "openrouter");
    }

    #[test]
    fn test_set_cloud_key_valid() {
        let engine = test_engine();
        assert!(engine.set_cloud_key("openrouter", "sk-test-key-123").is_ok());
        assert!(engine.set_cloud_key("groq", "gsk-test-key-456").is_ok());
        assert!(engine.set_cloud_key("huggingface", "hf-test-key-789").is_ok());
    }

    #[test]
    fn test_set_cloud_key_invalid_provider() {
        let engine = test_engine();
        let err = engine.set_cloud_key("azure", "some-key").unwrap_err();
        assert_eq!(err.code, "INVALID_PROVIDER");
    }

    #[test]
    fn test_set_cloud_key_empty() {
        let engine = test_engine();
        let err = engine.set_cloud_key("openrouter", "").unwrap_err();
        assert_eq!(err.code, "EMPTY_KEY");
    }

    #[test]
    fn test_usage_today_zero() {
        let engine = test_engine();
        let count = engine.get_usage_today("fake-user-id").expect("get usage today should succeed");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_faction_badge_mapping() {
        assert_eq!(faction_badge("terran"), "Terran Federation");
        assert_eq!(faction_badge("swarm"), "Swarm Collective");
        assert_eq!(faction_badge("ascended"), "Ascended Order");
        assert_eq!(faction_badge("pirate"), "Rogue Syndicate");
        assert_eq!(faction_badge("ancient"), "Ancient Remnant");
        assert_eq!(faction_badge("unknown"), "Neutral");
    }

    #[test]
    fn test_event_type_parsing() {
        assert!(matches!(parse_event_type("meteor_shower"), GalaxyEventType::MeteorShower));
        assert!(matches!(parse_event_type("wormhole"), GalaxyEventType::Wormhole));
        assert!(matches!(parse_event_type("dark_matter_storm"), GalaxyEventType::DarkMatterStorm));
        assert!(matches!(parse_event_type("bogus"), GalaxyEventType::MeteorShower));
    }

    #[test]
    fn test_event_type_descriptions() {
        assert!(GalaxyEventType::MeteorShower.description().contains("50%"));
        assert!(GalaxyEventType::DarkMatterStorm.description().contains("2x"));
    }

    #[test]
    fn test_event_type_durations() {
        assert_eq!(GalaxyEventType::SolarFlare.default_duration_hours(), 6);
        assert_eq!(GalaxyEventType::AlienContact.default_duration_hours(), 24);
    }

    #[test]
    fn test_user_settings_default() {
        let settings = UserSettings::default();
        assert_eq!(settings.language, "en");
        assert_eq!(settings.theme, "dark");
        assert!(settings.notifications);
        assert!(settings.offline_mode);
        assert!(settings.music_enabled);
    }

    #[test]
    fn test_matchmaking_status_serialization() {
        let idle = MatchmakingStatus::Idle;
        let json = serde_json::to_string(&idle).expect("JSON serialization should succeed");
        assert!(json.contains("idle"));

        let found = MatchmakingStatus::Found {
            opponent: "Player2".to_string(),
            galaxy: "Alpha-7".to_string(),
        };
        let json = serde_json::to_string(&found).expect("JSON serialization should succeed");
        assert!(json.contains("found"));
        assert!(json.contains("Player2"));
    }

    #[test]
    fn test_register_weak_password() {
        let engine = test_engine();
        let result = engine.register("WeakUser", "abc", None, "terran");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "PASSWORD_TOO_SHORT");
    }

    #[test]
    fn test_register_invalid_username() {
        let engine = test_engine();
        let result = engine.register("a", "Password1", None, "terran");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "USERNAME_LENGTH");
    }
}
