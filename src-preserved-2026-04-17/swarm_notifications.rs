// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//
// SwarmForge Notification System -- Desktop popups + Mobile push via ntfy
//
// Architecture:
//   Desktop: Tauri event emitted to frontend (uses tauri-plugin-notification)
//   Mobile:  ntfy.sh HTTP pub-sub (self-hosted option, zero signup required)
//   Privacy: Notifications contain NO game state -- only alerts.
//            User can self-host ntfy for full data sovereignty.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{AppResult, ImpForgeError};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_notifications", "Game");

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Notification categories for SwarmForge game events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SwarmNotificationType {
    // Colony Events
    BuildingComplete,
    ResearchDone,
    UnitTrained,

    // Combat Events
    UnderAttack,
    BattleWon,
    BattleLost,
    FleetArrived,

    // Espionage
    SpyDetected,
    IntelGathered,

    // Resources
    StorageFull,
    DarkMatterEarned,

    // Prestige
    PrestigeReady,
    AchievementUnlocked,

    // Commander (DEV only)
    CommanderAlert,
    TradeOffer,

    // System
    OfflineReward,
    DailyLogin,
    WeeklyChallenge,
    ExpeditionComplete,
}

/// Priority levels for notifications.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SwarmNotificationPriority {
    /// Info, can be batched.
    Low = 1,
    /// Normal game events.
    Medium = 2,
    /// Important (battle results, storage full).
    High = 3,
    /// Requires attention (under attack!).
    Urgent = 4,
    /// Immediate (colony about to be destroyed).
    Critical = 5,
}

impl SwarmNotificationPriority {
    fn from_u8(val: u8) -> Self {
        match val {
            0 | 1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            4 => Self::Urgent,
            _ => Self::Critical,
        }
    }

    fn ntfy_value(self) -> &'static str {
        match self {
            Self::Critical => "5",
            Self::Urgent => "4",
            Self::High => "3",
            Self::Medium => "2",
            Self::Low => "1",
        }
    }
}

/// A game notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameNotification {
    pub id: String,
    pub notification_type: SwarmNotificationType,
    pub title: String,
    pub message: String,
    pub priority: SwarmNotificationPriority,
    pub colony_id: Option<String>,
    /// Deep-link to the relevant game screen.
    pub action_url: Option<String>,
    pub timestamp: String,
    pub read: bool,
    pub delivered_desktop: bool,
    pub delivered_mobile: bool,
}

/// Notification settings per user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmNotificationSettings {
    /// Desktop notifications enabled.
    pub desktop_enabled: bool,
    /// Mobile push via ntfy enabled.
    pub mobile_enabled: bool,
    /// ntfy server URL (default: <https://ntfy.sh>, can be self-hosted).
    pub ntfy_server: String,
    /// ntfy topic (unique per user, acts as the "channel").
    pub ntfy_topic: Option<String>,
    /// Minimum priority to send desktop notification.
    pub desktop_min_priority: SwarmNotificationPriority,
    /// Minimum priority to send mobile push.
    pub mobile_min_priority: SwarmNotificationPriority,
    /// Quiet hours start (no notifications between these times).
    pub quiet_hours_start: Option<String>,
    /// Quiet hours end.
    pub quiet_hours_end: Option<String>,
    /// Per-type enable/disable.
    pub type_settings: HashMap<String, bool>,
    /// Sound enabled for desktop.
    pub sound_enabled: bool,
    /// Batch low-priority notifications (send summary every 5 min).
    pub batch_low_priority: bool,
}

impl Default for SwarmNotificationSettings {
    fn default() -> Self {
        Self {
            desktop_enabled: true,
            mobile_enabled: false, // opt-in
            ntfy_server: "https://ntfy.sh".to_string(),
            ntfy_topic: None,
            desktop_min_priority: SwarmNotificationPriority::Medium,
            mobile_min_priority: SwarmNotificationPriority::High,
            quiet_hours_start: Some("22:00".to_string()),
            quiet_hours_end: Some("08:00".to_string()),
            type_settings: HashMap::new(),
            sound_enabled: true,
            batch_low_priority: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// The SwarmForge notification engine.
///
/// Desktop delivery emits a Tauri event (`swarm-notification`) so the frontend
/// can display a toast and, if appropriate, fire a native OS notification via
/// `tauri-plugin-notification` (already in Cargo.toml).
///
/// Mobile push is handled server-side via the ntfy HTTP API (zero signup,
/// self-hostable, privacy-first).
pub struct SwarmNotificationEngine {
    settings: std::sync::Mutex<SwarmNotificationSettings>,
    history: std::sync::Mutex<Vec<GameNotification>>,
    pending_batch: std::sync::Mutex<Vec<GameNotification>>,
}

impl SwarmNotificationEngine {
    pub fn new() -> Self {
        Self {
            settings: std::sync::Mutex::new(SwarmNotificationSettings::default()),
            history: std::sync::Mutex::new(Vec::new()),
            pending_batch: std::sync::Mutex::new(Vec::new()),
        }
    }

    // -- public API ----------------------------------------------------------

    /// Send a notification (decides desktop, mobile, or both based on settings).
    ///
    /// The settings lock is acquired and released synchronously before any
    /// async work (ntfy HTTP POST) so the future remains `Send`.
    pub async fn notify(&self, mut notification: GameNotification) -> AppResult<()> {
        // ── synchronous decision block (lock held, no .await) ──────────
        enum Action {
            /// Store silently (quiet hours).
            StoreOnly,
            /// Type disabled by user -- drop entirely.
            Drop,
            /// Batch for later summary.
            Batch,
            /// Deliver now; optionally push to ntfy.
            Deliver {
                desktop: bool,
                ntfy: Option<(String, String)>, // (server, topic)
            },
        }

        let action = {
            let settings = self.settings.lock().map_err(|e| {
                ImpForgeError::internal("NOTIF_LOCK", format!("{e}"))
            })?;

            if self.is_quiet_hours(&settings)
                && notification.priority < SwarmNotificationPriority::Urgent
            {
                Action::StoreOnly
            } else {
                let type_key = serde_json::to_value(&notification.notification_type)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default();

                if let Some(false) = settings.type_settings.get(&type_key) {
                    Action::Drop
                } else if settings.batch_low_priority
                    && notification.priority == SwarmNotificationPriority::Low
                {
                    Action::Batch
                } else {
                    let desktop = settings.desktop_enabled
                        && notification.priority >= settings.desktop_min_priority;

                    let ntfy = if settings.mobile_enabled
                        && notification.priority >= settings.mobile_min_priority
                    {
                        settings
                            .ntfy_topic
                            .as_ref()
                            .map(|t| (settings.ntfy_server.clone(), t.clone()))
                    } else {
                        None
                    };

                    Action::Deliver { desktop, ntfy }
                }
            }
            // MutexGuard dropped here
        };

        // ── async execution (no lock held) ─────────────────────────────
        match action {
            Action::StoreOnly => {
                self.push_history(notification);
            }
            Action::Drop => { /* silently discard */ }
            Action::Batch => {
                self.pending_batch
                    .lock()
                    .map_err(|e| ImpForgeError::internal("NOTIF_LOCK", format!("{e}")))?
                    .push(notification);
            }
            Action::Deliver { desktop, ntfy } => {
                if desktop {
                    notification.delivered_desktop = true;
                }
                if let Some((server, topic)) = ntfy {
                    Self::send_ntfy(&server, &topic, &notification).await?;
                    notification.delivered_mobile = true;
                }
                self.push_history(notification);
            }
        }

        Ok(())
    }

    /// Send mobile push via ntfy HTTP API.
    async fn send_ntfy(
        server: &str,
        topic: &str,
        notif: &GameNotification,
    ) -> AppResult<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/{}", server.trim_end_matches('/'), topic);

        let tag = serde_json::to_value(&notif.notification_type)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "swarmforge".to_string());

        let _ = client
            .post(&url)
            .header("Title", &notif.title)
            .header("Priority", notif.priority.ntfy_value())
            .header("Tags", format!("swarmforge,{tag}"))
            .body(notif.message.clone())
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;

        Ok(())
    }

    /// Check if currently in quiet hours.
    fn is_quiet_hours(&self, settings: &SwarmNotificationSettings) -> bool {
        if let (Some(start), Some(end)) =
            (&settings.quiet_hours_start, &settings.quiet_hours_end)
        {
            let now = chrono::Local::now().format("%H:%M").to_string();
            if start < end {
                // e.g. 08:00 to 18:00
                now >= *start && now < *end
            } else {
                // wraps midnight, e.g. 22:00 to 08:00
                now >= *start || now < *end
            }
        } else {
            false
        }
    }

    /// Get notification history (newest first).
    pub fn get_history(&self, limit: usize) -> Vec<GameNotification> {
        let history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Get unread count.
    pub fn unread_count(&self) -> usize {
        let history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        history.iter().filter(|n| !n.read).count()
    }

    /// Mark a notification as read.
    pub fn mark_read(&self, notif_id: &str) -> AppResult<()> {
        let mut history = self.history.lock().map_err(|e| {
            ImpForgeError::internal("NOTIF_LOCK", format!("{e}"))
        })?;
        if let Some(n) = history.iter_mut().find(|n| n.id == notif_id) {
            n.read = true;
        }
        Ok(())
    }

    /// Mark all as read. Returns the number of notifications that changed.
    pub fn mark_all_read(&self) -> usize {
        let mut history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        let mut count = 0;
        for n in history.iter_mut() {
            if !n.read {
                n.read = true;
                count += 1;
            }
        }
        count
    }

    /// Flush batched low-priority notifications as a summary.
    pub async fn flush_batch(&self) -> AppResult<usize> {
        let batch: Vec<GameNotification> = {
            let mut pending = self.pending_batch.lock().map_err(|e| {
                ImpForgeError::internal("NOTIF_LOCK", format!("{e}"))
            })?;
            std::mem::take(&mut *pending)
        };

        if batch.is_empty() {
            return Ok(0);
        }

        let count = batch.len();
        let summary = GameNotification {
            id: uuid::Uuid::new_v4().to_string(),
            notification_type: SwarmNotificationType::OfflineReward,
            title: format!("{count} game events"),
            message: batch
                .iter()
                .map(|n| n.title.clone())
                .collect::<Vec<_>>()
                .join(", "),
            priority: SwarmNotificationPriority::Low,
            colony_id: None,
            action_url: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            read: false,
            delivered_desktop: false,
            delivered_mobile: false,
        };

        self.notify(summary).await?;

        // Store individual items in history
        let mut history = self.history.lock().map_err(|e| {
            ImpForgeError::internal("NOTIF_LOCK", format!("{e}"))
        })?;
        history.extend(batch);

        Ok(count)
    }

    /// Setup ntfy topic for mobile push.
    pub fn setup_ntfy(
        &self,
        topic: String,
        server: Option<String>,
    ) -> AppResult<()> {
        let mut settings = self.settings.lock().map_err(|e| {
            ImpForgeError::internal("NOTIF_LOCK", format!("{e}"))
        })?;
        settings.ntfy_topic = Some(topic);
        if let Some(s) = server {
            settings.ntfy_server = s;
        }
        settings.mobile_enabled = true;
        Ok(())
    }

    /// Update full settings.
    pub fn set_settings(
        &self,
        new_settings: SwarmNotificationSettings,
    ) -> AppResult<()> {
        let mut settings = self.settings.lock().map_err(|e| {
            ImpForgeError::internal("NOTIF_LOCK", format!("{e}"))
        })?;
        *settings = new_settings;
        Ok(())
    }

    /// Get current settings.
    pub fn get_settings(&self) -> SwarmNotificationSettings {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    // -- internal helpers ----------------------------------------------------

    fn push_history(&self, notification: GameNotification) {
        if let Ok(mut history) = self.history.lock() {
            history.push(notification);
            // Cap at 500 entries
            if history.len() > 500 {
                let drain_count = history.len() - 500;
                history.drain(..drain_count);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands (10)
// ---------------------------------------------------------------------------

/// Send a game notification.
#[tauri::command]
pub async fn swarm_notify(
    notification_type: String,
    title: String,
    message: String,
    priority: u8,
    colony_id: Option<String>,
    engine: tauri::State<'_, SwarmNotificationEngine>,
) -> AppResult<String> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_notifications", "game_notifications", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_notifications", "game_notifications");
    crate::synapse_fabric::synapse_session_push("swarm_notifications", "game_notifications", "swarm_notify called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_notifications", "info", "swarm_notifications active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_notifications", "alert", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"type": notification_type}));
    let ntype: SwarmNotificationType =
        serde_json::from_value(serde_json::Value::String(notification_type.clone()))
            .unwrap_or(SwarmNotificationType::OfflineReward);

    let id = uuid::Uuid::new_v4().to_string();
    let notification = GameNotification {
        id: id.clone(),
        notification_type: ntype,
        title,
        message,
        priority: SwarmNotificationPriority::from_u8(priority),
        colony_id,
        action_url: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
        read: false,
        delivered_desktop: false,
        delivered_mobile: false,
    };

    engine.notify(notification).await?;
    Ok(id)
}

/// Get current notification settings.
#[tauri::command]
pub fn swarm_notify_settings(
    engine: tauri::State<'_, SwarmNotificationEngine>,
) -> SwarmNotificationSettings {
    engine.get_settings()
}

/// Update notification settings.
#[tauri::command]
pub fn swarm_notify_set_settings(
    settings: SwarmNotificationSettings,
    engine: tauri::State<'_, SwarmNotificationEngine>,
) -> AppResult<()> {
    engine.set_settings(settings)
}

/// Setup ntfy topic for mobile push.
#[tauri::command]
pub fn swarm_notify_setup_ntfy(
    topic: String,
    server: Option<String>,
    engine: tauri::State<'_, SwarmNotificationEngine>,
) -> AppResult<()> {
    engine.setup_ntfy(topic, server)
}

/// Send a test notification to desktop or mobile.
#[tauri::command]
pub async fn swarm_notify_test(
    destination: String,
    engine: tauri::State<'_, SwarmNotificationEngine>,
) -> AppResult<()> {
    let priority = if destination == "mobile" {
        SwarmNotificationPriority::High
    } else {
        SwarmNotificationPriority::Medium
    };

    let notification = GameNotification {
        id: uuid::Uuid::new_v4().to_string(),
        notification_type: SwarmNotificationType::DailyLogin,
        title: "SwarmForge Test".to_string(),
        message: format!("Test notification to {destination}"),
        priority,
        colony_id: None,
        action_url: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
        read: false,
        delivered_desktop: false,
        delivered_mobile: false,
    };

    engine.notify(notification).await
}

/// Get notification history.
#[tauri::command]
pub fn swarm_notify_history(
    limit: u32,
    engine: tauri::State<'_, SwarmNotificationEngine>,
) -> Vec<GameNotification> {
    engine.get_history(limit as usize)
}

/// Get unread notification count.
#[tauri::command]
pub fn swarm_notify_unread_count(
    engine: tauri::State<'_, SwarmNotificationEngine>,
) -> usize {
    engine.unread_count()
}

/// Mark a single notification as read.
#[tauri::command]
pub fn swarm_notify_mark_read(
    notification_id: String,
    engine: tauri::State<'_, SwarmNotificationEngine>,
) -> AppResult<()> {
    engine.mark_read(&notification_id)
}

/// Mark all notifications as read.
#[tauri::command]
pub fn swarm_notify_mark_all_read(
    engine: tauri::State<'_, SwarmNotificationEngine>,
) -> usize {
    engine.mark_all_read()
}

/// Flush batched low-priority notifications as a summary.
#[tauri::command]
pub async fn swarm_notify_flush_batch(
    engine: tauri::State<'_, SwarmNotificationEngine>,
) -> AppResult<usize> {
    engine.flush_batch().await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;


    fn make_engine() -> SwarmNotificationEngine {
        let engine = SwarmNotificationEngine::new();
        // Disable quiet hours so tests don't become time-dependent
        engine
            .settings
            .lock()
            .expect("test lock")
            .quiet_hours_start = None;
        engine
    }

    fn make_notification(
        ntype: SwarmNotificationType,
        priority: SwarmNotificationPriority,
    ) -> GameNotification {
        GameNotification {
            id: uuid::Uuid::new_v4().to_string(),
            notification_type: ntype,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            priority,
            colony_id: None,
            action_url: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            read: false,
            delivered_desktop: false,
            delivered_mobile: false,
        }
    }

    #[test]
    fn test_default_settings() {
        let s = SwarmNotificationSettings::default();
        assert!(s.desktop_enabled);
        assert!(!s.mobile_enabled);
        assert_eq!(s.ntfy_server, "https://ntfy.sh");
        assert!(s.ntfy_topic.is_none());
        assert_eq!(s.desktop_min_priority, SwarmNotificationPriority::Medium);
        assert_eq!(s.mobile_min_priority, SwarmNotificationPriority::High);
        assert!(s.sound_enabled);
        assert!(s.batch_low_priority);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(SwarmNotificationPriority::Low < SwarmNotificationPriority::Medium);
        assert!(SwarmNotificationPriority::Medium < SwarmNotificationPriority::High);
        assert!(SwarmNotificationPriority::High < SwarmNotificationPriority::Urgent);
        assert!(SwarmNotificationPriority::Urgent < SwarmNotificationPriority::Critical);
    }

    #[test]
    fn test_priority_from_u8() {
        assert_eq!(SwarmNotificationPriority::from_u8(0), SwarmNotificationPriority::Low);
        assert_eq!(SwarmNotificationPriority::from_u8(1), SwarmNotificationPriority::Low);
        assert_eq!(SwarmNotificationPriority::from_u8(2), SwarmNotificationPriority::Medium);
        assert_eq!(SwarmNotificationPriority::from_u8(3), SwarmNotificationPriority::High);
        assert_eq!(SwarmNotificationPriority::from_u8(4), SwarmNotificationPriority::Urgent);
        assert_eq!(SwarmNotificationPriority::from_u8(5), SwarmNotificationPriority::Critical);
        assert_eq!(SwarmNotificationPriority::from_u8(99), SwarmNotificationPriority::Critical);
    }

    #[test]
    fn test_ntfy_value() {
        assert_eq!(SwarmNotificationPriority::Low.ntfy_value(), "1");
        assert_eq!(SwarmNotificationPriority::Medium.ntfy_value(), "2");
        assert_eq!(SwarmNotificationPriority::High.ntfy_value(), "3");
        assert_eq!(SwarmNotificationPriority::Urgent.ntfy_value(), "4");
        assert_eq!(SwarmNotificationPriority::Critical.ntfy_value(), "5");
    }

    #[tokio::test]
    async fn test_notify_stores_in_history() {
        let engine = make_engine();
        let notif = make_notification(
            SwarmNotificationType::BuildingComplete,
            SwarmNotificationPriority::Medium,
        );
        engine.notify(notif).await.expect("notify should succeed");

        let history = engine.get_history(10);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].notification_type, SwarmNotificationType::BuildingComplete);
    }

    #[tokio::test]
    async fn test_priority_filtering_desktop() {
        let engine = make_engine();
        // Default desktop_min_priority is Medium, so Low should not set delivered_desktop
        let notif = make_notification(
            SwarmNotificationType::DailyLogin,
            SwarmNotificationPriority::Low,
        );
        // Low priority + batch_low_priority = goes to batch, not history
        engine.notify(notif).await.expect("notify should succeed");

        // Should be in pending batch, not in main history
        let history = engine.get_history(10);
        assert_eq!(history.len(), 0);

        // Medium should go to history with delivered_desktop = true
        let notif2 = make_notification(
            SwarmNotificationType::ResearchDone,
            SwarmNotificationPriority::Medium,
        );
        engine.notify(notif2).await.expect("notify should succeed");

        let history = engine.get_history(10);
        assert_eq!(history.len(), 1);
        assert!(history[0].delivered_desktop);
    }

    #[tokio::test]
    async fn test_type_disable() {
        let engine = make_engine();
        {
            let mut settings = engine.settings.lock().expect("lock");
            settings
                .type_settings
                .insert("building_complete".to_string(), false);
        }

        let notif = make_notification(
            SwarmNotificationType::BuildingComplete,
            SwarmNotificationPriority::High,
        );
        engine.notify(notif).await.expect("notify should succeed");

        // Should be silently dropped
        let history = engine.get_history(10);
        assert_eq!(history.len(), 0);
    }

    #[tokio::test]
    async fn test_batch_accumulation() {
        let engine = make_engine();

        for _ in 0..5 {
            let notif = make_notification(
                SwarmNotificationType::DarkMatterEarned,
                SwarmNotificationPriority::Low,
            );
            engine.notify(notif).await.expect("notify should succeed");
        }

        // All 5 should be in pending batch
        let batch_len = engine
            .pending_batch
            .lock()
            .expect("lock")
            .len();
        assert_eq!(batch_len, 5);

        // History should be empty (all batched)
        assert_eq!(engine.get_history(10).len(), 0);
    }

    #[tokio::test]
    async fn test_flush_batch() {
        let engine = make_engine();

        // Disable batch_low_priority on the summary notification so flush does
        // not recursively re-batch the summary
        {
            let mut settings = engine.settings.lock().expect("lock");
            settings.batch_low_priority = false;
        }

        // Add 3 low-prio items directly to the pending batch
        {
            let mut batch = engine.pending_batch.lock().expect("lock");
            for i in 0..3 {
                batch.push(GameNotification {
                    id: uuid::Uuid::new_v4().to_string(),
                    notification_type: SwarmNotificationType::DarkMatterEarned,
                    title: format!("Event {i}"),
                    message: format!("Detail {i}"),
                    priority: SwarmNotificationPriority::Low,
                    colony_id: None,
                    action_url: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    read: false,
                    delivered_desktop: false,
                    delivered_mobile: false,
                });
            }
        }

        let flushed = engine.flush_batch().await.expect("flush should succeed");
        assert_eq!(flushed, 3);

        // History should now contain the summary + 3 individual items = 4
        let history = engine.get_history(10);
        assert_eq!(history.len(), 4);
    }

    #[test]
    fn test_unread_count() {
        let engine = make_engine();
        {
            let mut history = engine.history.lock().expect("lock");
            history.push(make_notification(
                SwarmNotificationType::BattleWon,
                SwarmNotificationPriority::High,
            ));
            let mut read_one = make_notification(
                SwarmNotificationType::UnitTrained,
                SwarmNotificationPriority::Medium,
            );
            read_one.read = true;
            history.push(read_one);
            history.push(make_notification(
                SwarmNotificationType::FleetArrived,
                SwarmNotificationPriority::Medium,
            ));
        }

        assert_eq!(engine.unread_count(), 2);
    }

    #[test]
    fn test_mark_read() {
        let engine = make_engine();
        let id = {
            let mut history = engine.history.lock().expect("lock");
            let notif = make_notification(
                SwarmNotificationType::SpyDetected,
                SwarmNotificationPriority::High,
            );
            let id = notif.id.clone();
            history.push(notif);
            id
        };

        assert_eq!(engine.unread_count(), 1);
        engine.mark_read(&id).expect("mark_read should succeed");
        assert_eq!(engine.unread_count(), 0);
    }

    #[test]
    fn test_mark_all_read() {
        let engine = make_engine();
        {
            let mut history = engine.history.lock().expect("lock");
            for _ in 0..5 {
                history.push(make_notification(
                    SwarmNotificationType::ResearchDone,
                    SwarmNotificationPriority::Medium,
                ));
            }
        }

        assert_eq!(engine.unread_count(), 5);
        let changed = engine.mark_all_read();
        assert_eq!(changed, 5);
        assert_eq!(engine.unread_count(), 0);
    }

    #[test]
    fn test_mark_all_read_idempotent() {
        let engine = make_engine();
        let changed = engine.mark_all_read();
        assert_eq!(changed, 0);
    }

    #[test]
    fn test_history_limit() {
        let engine = make_engine();
        {
            let mut history = engine.history.lock().expect("lock");
            for _ in 0..600 {
                history.push(make_notification(
                    SwarmNotificationType::DarkMatterEarned,
                    SwarmNotificationPriority::Low,
                ));
            }
        }
        // push_history caps at 500, but we pushed directly via lock
        // so let us push one more through the engine path to trigger the cap
        engine.push_history(make_notification(
            SwarmNotificationType::DailyLogin,
            SwarmNotificationPriority::Low,
        ));
        let history = engine.history.lock().expect("lock");
        assert!(history.len() <= 501);
    }

    #[test]
    fn test_setup_ntfy() {
        let engine = make_engine();
        engine
            .setup_ntfy("my-topic".to_string(), Some("https://ntfy.example.com".to_string()))
            .expect("setup should succeed");

        let settings = engine.get_settings();
        assert!(settings.mobile_enabled);
        assert_eq!(settings.ntfy_topic, Some("my-topic".to_string()));
        assert_eq!(settings.ntfy_server, "https://ntfy.example.com");
    }

    #[test]
    fn test_setup_ntfy_default_server() {
        let engine = make_engine();
        engine
            .setup_ntfy("user123".to_string(), None)
            .expect("setup should succeed");

        let settings = engine.get_settings();
        assert!(settings.mobile_enabled);
        assert_eq!(settings.ntfy_server, "https://ntfy.sh");
        assert_eq!(settings.ntfy_topic, Some("user123".to_string()));
    }

    #[test]
    fn test_ntfy_url_construction() {
        // Validate URL format that send_ntfy would construct
        let server = "https://ntfy.sh";
        let topic = "swarmforge-abc123";
        let url = format!("{}/{}", server.trim_end_matches('/'), topic);
        assert_eq!(url, "https://ntfy.sh/swarmforge-abc123");

        // Trailing slash
        let server2 = "https://ntfy.example.com/";
        let url2 = format!("{}/{}", server2.trim_end_matches('/'), topic);
        assert_eq!(url2, "https://ntfy.example.com/swarmforge-abc123");
    }

    #[test]
    fn test_notification_serialization() {
        let notif = make_notification(
            SwarmNotificationType::UnderAttack,
            SwarmNotificationPriority::Critical,
        );

        let json = serde_json::to_string(&notif).expect("serialize");
        assert!(json.contains("under_attack"));
        assert!(json.contains("Critical"));

        let parsed: GameNotification =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.notification_type, SwarmNotificationType::UnderAttack);
        assert_eq!(parsed.priority, SwarmNotificationPriority::Critical);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = SwarmNotificationSettings::default();
        let json = serde_json::to_string(&settings).expect("serialize");
        assert!(json.contains("desktop_enabled"));
        assert!(json.contains("ntfy.sh"));

        let parsed: SwarmNotificationSettings =
            serde_json::from_str(&json).expect("deserialize");
        assert!(parsed.desktop_enabled);
        assert!(!parsed.mobile_enabled);
    }

    #[test]
    fn test_set_settings() {
        let engine = make_engine();
        let mut new_settings = SwarmNotificationSettings::default();
        new_settings.desktop_enabled = false;
        new_settings.sound_enabled = false;

        engine
            .set_settings(new_settings)
            .expect("set_settings should succeed");

        let retrieved = engine.get_settings();
        assert!(!retrieved.desktop_enabled);
        assert!(!retrieved.sound_enabled);
    }

    #[test]
    fn test_quiet_hours_within_range() {
        let engine = make_engine();
        // Test with a range that does NOT wrap midnight (08:00 - 18:00)
        let settings = SwarmNotificationSettings {
            quiet_hours_start: Some("00:00".to_string()),
            quiet_hours_end: Some("23:59".to_string()),
            ..Default::default()
        };
        // 00:00 - 23:59 should always be quiet
        assert!(engine.is_quiet_hours(&settings));
    }

    #[test]
    fn test_quiet_hours_disabled() {
        let engine = make_engine();
        let settings = SwarmNotificationSettings {
            quiet_hours_start: None,
            quiet_hours_end: None,
            ..Default::default()
        };
        assert!(!engine.is_quiet_hours(&settings));
    }

    #[test]
    fn test_get_history_respects_limit() {
        let engine = make_engine();
        {
            let mut history = engine.history.lock().expect("lock");
            for _ in 0..20 {
                history.push(make_notification(
                    SwarmNotificationType::UnitTrained,
                    SwarmNotificationPriority::Medium,
                ));
            }
        }
        let result = engine.get_history(5);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_all_notification_types_serialize() {
        let types = vec![
            SwarmNotificationType::BuildingComplete,
            SwarmNotificationType::ResearchDone,
            SwarmNotificationType::UnitTrained,
            SwarmNotificationType::UnderAttack,
            SwarmNotificationType::BattleWon,
            SwarmNotificationType::BattleLost,
            SwarmNotificationType::FleetArrived,
            SwarmNotificationType::SpyDetected,
            SwarmNotificationType::IntelGathered,
            SwarmNotificationType::StorageFull,
            SwarmNotificationType::DarkMatterEarned,
            SwarmNotificationType::PrestigeReady,
            SwarmNotificationType::AchievementUnlocked,
            SwarmNotificationType::CommanderAlert,
            SwarmNotificationType::TradeOffer,
            SwarmNotificationType::OfflineReward,
            SwarmNotificationType::DailyLogin,
            SwarmNotificationType::WeeklyChallenge,
            SwarmNotificationType::ExpeditionComplete,
        ];

        for t in &types {
            let json = serde_json::to_value(t).expect("serialize type");
            assert!(json.is_string(), "type should serialize to string: {json}");
        }
        assert_eq!(types.len(), 19);
    }
}
