// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Integration Hub -- Connects ALL 22 game modules into a coherent system
//!
//! This is the "nervous system" of SwarmForge. When something happens in one module,
//! this hub ensures all dependent modules are notified and updated.
//!
//! ## Event Flow Examples
//!
//!   Combat kill -> Undead gain Leichenteile -> can raise more units
//!   Building complete -> Notification sent -> Achievement check -> XP earned -> DM generated
//!   Player writes document in ImpForge -> DM earned -> resources generated -> colony grows
//!   Kultisten sacrificed -> energy drops -> summoning speed changes -> large demon spawned
//!
//! ## Architecture
//!
//! - Central `SwarmEvent` enum covers ALL cross-module game events
//! - `IntegrationHub` processes events and produces cascading side-effects
//! - Event routing table documents which modules listen to which events
//! - Tauri commands expose the hub to the Svelte frontend
//!
//! ## Module Dependency Graph (22 modules)
//!
//! ```text
//! forge_quest (RPG) ----+
//! swarm_combat ----------+---> IntegrationHub ---> swarm_notifications
//! swarm_factions --------+                   |---> swarm_engagement
//! swarm_engagement ------+                   |---> swarm_meta (prestige)
//! swarm_diplomacy -------+                   |---> offline_progression
//! swarm_advanced --------+                   |---> achievements
//! swarm_special ---------+                   |---> swarm_factions (resources)
//! swarm_worldgen --------+                   |---> swarm_combat (terrain)
//! swarm_gameloop --------+                   |---> swarm_ai (blackboard)
//! ```

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_integration", "Game");


// ============================================================================
// SwarmEvent -- All cross-module game events
// ============================================================================

/// All possible game events that flow between modules.
///
/// Each variant carries the minimum data needed for dependent modules to react.
/// Events are processed by `IntegrationHub::process_event`, which returns zero
/// or more cascading events that should also be processed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum SwarmEvent {
    // -- Combat Events --

    /// A unit was killed in combat. Triggers corpse/corruption/faith mechanics.
    UnitKilled {
        killer_faction: String,
        victim_faction: String,
        victim_type: String,
        hex_q: i32,
        hex_r: i32,
    },

    /// A battle was won. Triggers prestige checks and notifications.
    BattleWon {
        winner: String,
        loser: String,
        location: String,
    },

    /// A battle was lost. Triggers defense alerts and morale effects.
    BattleLost {
        loser: String,
        winner: String,
        location: String,
    },

    /// A fleet was destroyed. Creates debris field and potential moon.
    FleetDestroyed {
        fleet_id: String,
        debris_resources: f64,
    },

    // -- Colony Events --

    /// A building finished construction. Triggers unlock checks and notifications.
    BuildingCompleted {
        colony_id: String,
        building_type: String,
        level: u32,
    },

    /// Research finished. Triggers unit/building unlocks and notifications.
    ResearchCompleted {
        colony_id: String,
        tech: String,
        level: u32,
    },

    /// Units finished training. Updates colony military strength.
    UnitTrained {
        colony_id: String,
        unit_type: String,
        count: u32,
    },

    /// A new colony was established. Updates galaxy map and diplomacy.
    ColonyFounded {
        colony_id: String,
        coord: String,
        faction: String,
    },

    // -- Resource Events --

    /// Resources were earned (production tick, loot, trade).
    ResourcesEarned {
        colony_id: String,
        resource: String,
        amount: f64,
    },

    /// Dark Matter earned from any source.
    DarkMatterEarned { amount: u32, source: String },

    /// A storage silo is full -- production will be wasted.
    StorageFull {
        colony_id: String,
        resource: String,
    },

    // -- Faction-Specific Events --

    /// Insect queen injected larvae into a hatchery.
    LarvaeInjected { colony_id: String, count: u32 },

    /// Demon Kultisten were sacrificed to summon a unit.
    KultistenSacrificed {
        colony_id: String,
        count: u32,
        for_unit: String,
    },

    /// Undead harvested Leichenteile from a corpse.
    CorpseHarvested {
        colony_id: String,
        leichenteile: f64,
    },

    /// Undead Adept spread Blight to a hex tile.
    BlightSpread {
        hex_q: i32,
        hex_r: i32,
        adept_id: String,
    },

    /// Human Faith resource changed.
    FaithGained { amount: f64, from_victory: bool },

    /// Demon Corruption decayed (5%/hour without combat).
    CorruptionDecayed {
        colony_id: String,
        amount: f64,
    },

    // -- Terrain Events --

    /// Faction terrain spread to a hex (Creep, Corruption, Blight, Terraform).
    TerrainSpread {
        faction: String,
        hex_q: i32,
        hex_r: i32,
        strength: f64,
    },

    /// Two or more factions contest the same hex tile.
    TerrainContested {
        hex_q: i32,
        hex_r: i32,
        factions: Vec<String>,
    },

    // -- Prestige Events --

    /// Player performed a prestige reset (Molt / Ascension / Lichdom / Transzendenz).
    PrestigePerformed {
        tier: String,
        phoenix_ash_earned: u64,
    },

    /// Achievement unlocked -- grants XP and possibly Dark Matter.
    AchievementUnlocked {
        achievement_id: String,
        xp: u64,
    },

    /// Player leveled up -- grants talent points.
    LevelUp { new_level: u32 },

    // -- Espionage Events --

    /// A spy was planted in an enemy colony.
    SpyPlanted {
        target_colony: String,
        cover_depth: String,
    },

    /// An enemy spy was detected in our colony.
    SpyDetected {
        colony_id: String,
        spy_faction: String,
    },

    /// Intelligence report gathered from espionage.
    IntelGathered { target: String, category: String },

    // -- ImpForge Integration Events --

    /// User performed a productive action in ImpForge (document, workflow, AI query).
    /// This bridges the productivity app to the game layer.
    ProductivityAction {
        action: String,
        dm_earned: u32,
        resources_earned: f64,
    },

    // -- Commander Events --

    /// God-Emperor NPC AI issued a directive.
    CommanderDirective {
        directive_type: String,
        target: String,
    },

    /// A trade offer was received from another player or NPC.
    TradeOfferReceived {
        player_id: String,
        resources: f64,
    },
}

impl std::fmt::Display for SwarmEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnitKilled { killer_faction, victim_type, .. } => {
                write!(f, "UnitKilled({killer_faction} killed {victim_type})")
            }
            Self::BattleWon { winner, loser, .. } => {
                write!(f, "BattleWon({winner} defeated {loser})")
            }
            Self::BattleLost { loser, winner, .. } => {
                write!(f, "BattleLost({loser} lost to {winner})")
            }
            Self::FleetDestroyed { fleet_id, .. } => {
                write!(f, "FleetDestroyed({fleet_id})")
            }
            Self::BuildingCompleted { building_type, level, .. } => {
                write!(f, "BuildingCompleted({building_type} L{level})")
            }
            Self::ResearchCompleted { tech, level, .. } => {
                write!(f, "ResearchCompleted({tech} L{level})")
            }
            Self::UnitTrained { unit_type, count, .. } => {
                write!(f, "UnitTrained({count}x {unit_type})")
            }
            Self::ColonyFounded { colony_id, faction, .. } => {
                write!(f, "ColonyFounded({colony_id} [{faction}])")
            }
            Self::ResourcesEarned { resource, amount, .. } => {
                write!(f, "ResourcesEarned({amount:.1} {resource})")
            }
            Self::DarkMatterEarned { amount, source } => {
                write!(f, "DarkMatterEarned({amount} from {source})")
            }
            Self::StorageFull { resource, .. } => {
                write!(f, "StorageFull({resource})")
            }
            Self::LarvaeInjected { count, .. } => write!(f, "LarvaeInjected({count})"),
            Self::KultistenSacrificed { count, for_unit, .. } => {
                write!(f, "KultistenSacrificed({count} for {for_unit})")
            }
            Self::CorpseHarvested { leichenteile, .. } => {
                write!(f, "CorpseHarvested({leichenteile:.1})")
            }
            Self::BlightSpread { hex_q, hex_r, .. } => {
                write!(f, "BlightSpread({hex_q},{hex_r})")
            }
            Self::FaithGained { amount, .. } => write!(f, "FaithGained({amount:.1})"),
            Self::CorruptionDecayed { amount, .. } => {
                write!(f, "CorruptionDecayed({amount:.2})")
            }
            Self::TerrainSpread { faction, hex_q, hex_r, .. } => {
                write!(f, "TerrainSpread({faction} at {hex_q},{hex_r})")
            }
            Self::TerrainContested { hex_q, hex_r, .. } => {
                write!(f, "TerrainContested({hex_q},{hex_r})")
            }
            Self::PrestigePerformed { tier, .. } => {
                write!(f, "PrestigePerformed({tier})")
            }
            Self::AchievementUnlocked { achievement_id, xp } => {
                write!(f, "AchievementUnlocked({achievement_id} +{xp}xp)")
            }
            Self::LevelUp { new_level } => write!(f, "LevelUp({new_level})"),
            Self::SpyPlanted { target_colony, .. } => {
                write!(f, "SpyPlanted(target={target_colony})")
            }
            Self::SpyDetected { spy_faction, .. } => {
                write!(f, "SpyDetected({spy_faction})")
            }
            Self::IntelGathered { target, category } => {
                write!(f, "IntelGathered({category} on {target})")
            }
            Self::ProductivityAction { action, dm_earned, .. } => {
                write!(f, "ProductivityAction({action} +{dm_earned}DM)")
            }
            Self::CommanderDirective { directive_type, target } => {
                write!(f, "CommanderDirective({directive_type} -> {target})")
            }
            Self::TradeOfferReceived { player_id, resources } => {
                write!(f, "TradeOfferReceived(from {player_id}, {resources:.0} res)")
            }
        }
    }
}

// ============================================================================
// Integration Hub -- Central event processor
// ============================================================================

/// Tracks which modules are wired into the integration hub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleStatus {
    pub name: String,
    pub connected: bool,
    pub event_count: u64,
    pub last_event: Option<String>,
}

/// Central event processor that routes game events between all 22 SwarmForge modules.
///
/// The hub does not own any game state -- it reads events and produces cascading
/// events.  The actual state lives in each module's own engine (SwarmCombatEngine,
/// SwarmFactionEngine, etc.).
pub struct IntegrationHub {
    /// Total events processed since startup.
    total_events: u64,
    /// Per-module event counters and status.
    modules: Vec<ModuleStatus>,
    /// Event history (ring buffer, last 200 events).
    event_log: Vec<EventLogEntry>,
}

/// One entry in the event log ring buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub timestamp_ms: u64,
    pub event: String,
    pub cascaded_count: usize,
}

/// Maximum number of events kept in the ring buffer.
const EVENT_LOG_CAPACITY: usize = 200;

/// All 22 SwarmForge module names for status tracking.
const MODULE_NAMES: &[&str] = &[
    "forge_quest",
    "swarm_combat",
    "swarm_factions",
    "swarm_engagement",
    "swarm_diplomacy",
    "swarm_advanced",
    "swarm_special",
    "swarm_worldgen",
    "swarm_gameloop",
    "swarm_galaxy",
    "swarm_progression",
    "swarm_meta",
    "swarm_genome",
    "swarm_heroes",
    "swarm_ai",
    "swarm_presentation",
    "swarm_multiplayer",
    "swarm_polish",
    "swarm_database",
    "swarm_notifications",
    "offline_progression",
    "achievements",
];

impl IntegrationHub {
    /// Create a new integration hub with all 22 modules registered.
    pub(crate) fn new() -> Self {
        let modules = MODULE_NAMES
            .iter()
            .map(|name| ModuleStatus {
                name: name.to_string(),
                connected: true,
                event_count: 0,
                last_event: None,
            })
            .collect();

        Self {
            total_events: 0,
            modules,
            event_log: Vec::with_capacity(EVENT_LOG_CAPACITY),
        }
    }

    /// Process a game event through ALL relevant subsystems.
    ///
    /// Returns zero or more cascading events that should also be processed.
    /// The caller should recursively feed cascading events back (with a depth
    /// limit to prevent infinite loops).
    pub(crate) fn process_event(&mut self, event: &SwarmEvent) -> Vec<SwarmEvent> {
        self.total_events += 1;
        let mut cascading = Vec::new();

        match event {
            // ── Combat kill -> multiple cascading effects ──
            SwarmEvent::UnitKilled {
                killer_faction,
                victim_faction: _,
                victim_type: _,
                hex_q,
                hex_r,
            } => {
                // 1. Undead gain Leichenteile from kills (0.5 per kill average)
                if killer_faction == "undead" {
                    self.touch_module("swarm_factions", event);
                    cascading.push(SwarmEvent::CorpseHarvested {
                        colony_id: "nearest_undead".into(),
                        leichenteile: 0.5,
                    });
                }

                // 2. Demons gain Corruption on the combat hex
                if killer_faction == "demons" {
                    self.touch_module("swarm_combat", event);
                    cascading.push(SwarmEvent::TerrainSpread {
                        faction: "demons".into(),
                        hex_q: *hex_q,
                        hex_r: *hex_r,
                        strength: 0.1,
                    });
                }

                // 3. Humans gain Faith from victories
                if killer_faction == "humans" {
                    self.touch_module("swarm_factions", event);
                    cascading.push(SwarmEvent::FaithGained {
                        amount: 1.0,
                        from_victory: true,
                    });
                }

                // 4. Insects: killed units drop Biomass on Creep terrain
                if killer_faction == "insects" {
                    self.touch_module("swarm_factions", event);
                    cascading.push(SwarmEvent::ResourcesEarned {
                        colony_id: "nearest_insect".into(),
                        resource: "biomass".into(),
                        amount: 0.3,
                    });
                }

                // 5. Always notify + check engagement challenges
                self.touch_module("swarm_notifications", event);
                self.touch_module("swarm_engagement", event);
            }

            // ── Battle won -> faith, prestige check, notification ──
            SwarmEvent::BattleWon {
                winner,
                loser: _,
                location: _,
            } => {
                self.touch_module("swarm_notifications", event);
                self.touch_module("swarm_engagement", event);
                self.touch_module("swarm_meta", event);

                // Winners gain Faith if human
                if winner == "humans" {
                    cascading.push(SwarmEvent::FaithGained {
                        amount: 5.0,
                        from_victory: true,
                    });
                }
            }

            // ── Battle lost -> alert, morale penalty ──
            SwarmEvent::BattleLost { .. } => {
                self.touch_module("swarm_notifications", event);
                self.touch_module("swarm_engagement", event);
            }

            // ── Building complete -> notification + achievement + XP ──
            SwarmEvent::BuildingCompleted {
                colony_id: _,
                building_type,
                level,
            } => {
                self.touch_module("swarm_notifications", event);
                self.touch_module("forge_quest", event);
                self.touch_module("swarm_engagement", event);

                // Achievement: first building, 10 buildings, etc.
                cascading.push(SwarmEvent::AchievementUnlocked {
                    achievement_id: format!("build_{building_type}"),
                    xp: u64::from(*level) * 10,
                });
            }

            // ── Research complete -> unlock check + notification ──
            SwarmEvent::ResearchCompleted { .. } => {
                self.touch_module("swarm_notifications", event);
                self.touch_module("forge_quest", event);
                self.touch_module("swarm_engagement", event);
            }

            // ── Unit trained -> military strength update ──
            SwarmEvent::UnitTrained { .. } => {
                self.touch_module("swarm_notifications", event);
                self.touch_module("swarm_combat", event);
            }

            // ── Colony founded -> galaxy map + diplomacy ──
            SwarmEvent::ColonyFounded { .. } => {
                self.touch_module("swarm_galaxy", event);
                self.touch_module("swarm_diplomacy", event);
                self.touch_module("swarm_notifications", event);
                self.touch_module("swarm_worldgen", event);
            }

            // ── Resources earned -> storage check ──
            SwarmEvent::ResourcesEarned { .. } => {
                self.touch_module("swarm_factions", event);
                self.touch_module("swarm_database", event);
            }

            // ── Dark Matter earned -> forge quest + engagement ──
            SwarmEvent::DarkMatterEarned { .. } => {
                self.touch_module("forge_quest", event);
                self.touch_module("swarm_engagement", event);
            }

            // ── Storage full -> notification warning ──
            SwarmEvent::StorageFull { .. } => {
                self.touch_module("swarm_notifications", event);
            }

            // ── Larvae injected -> faction resource update ──
            SwarmEvent::LarvaeInjected { .. } => {
                self.touch_module("swarm_factions", event);
            }

            // ── Kultisten sacrificed -> energy drops, demon spawned ──
            SwarmEvent::KultistenSacrificed {
                colony_id: _,
                count,
                for_unit,
            } => {
                self.touch_module("swarm_factions", event);
                self.touch_module("swarm_notifications", event);

                // Large sacrifice triggers a notification warning
                if *count >= 5 {
                    self.touch_module("swarm_combat", event);
                }

                // Dark Matter cost for large summons
                if for_unit.contains("lord") || for_unit.contains("demon") {
                    cascading.push(SwarmEvent::DarkMatterEarned {
                        amount: *count * 10,
                        source: format!("sacrifice_for_{for_unit}"),
                    });
                }
            }

            // ── Corpse harvested -> Undead economy ──
            SwarmEvent::CorpseHarvested { .. } => {
                self.touch_module("swarm_factions", event);
            }

            // ── Blight spread -> terrain + worldgen fog update ──
            SwarmEvent::BlightSpread { hex_q, hex_r, .. } => {
                self.touch_module("swarm_special", event);
                self.touch_module("swarm_worldgen", event);

                cascading.push(SwarmEvent::TerrainSpread {
                    faction: "undead".into(),
                    hex_q: *hex_q,
                    hex_r: *hex_r,
                    strength: 0.8,
                });
            }

            // ── Faith gained -> faction resource ──
            SwarmEvent::FaithGained { .. } => {
                self.touch_module("swarm_factions", event);
            }

            // ── Corruption decayed -> faction resource penalty ──
            SwarmEvent::CorruptionDecayed { .. } => {
                self.touch_module("swarm_factions", event);
            }

            // ── Terrain spread -> combat effects + fog update ──
            SwarmEvent::TerrainSpread { .. } => {
                self.touch_module("swarm_combat", event);
                self.touch_module("swarm_worldgen", event);
                self.touch_module("swarm_special", event);
            }

            // ── Terrain contested -> combat + notification ──
            SwarmEvent::TerrainContested { .. } => {
                self.touch_module("swarm_combat", event);
                self.touch_module("swarm_notifications", event);
            }

            // ── Prestige performed -> multipliers + notification + achievement ──
            SwarmEvent::PrestigePerformed {
                tier,
                phoenix_ash_earned: _,
            } => {
                self.touch_module("offline_progression", event);
                self.touch_module("swarm_notifications", event);
                self.touch_module("swarm_meta", event);

                cascading.push(SwarmEvent::AchievementUnlocked {
                    achievement_id: format!("prestige_{tier}"),
                    xp: 500,
                });
            }

            // ── Achievement unlocked -> quest XP + notification ──
            SwarmEvent::AchievementUnlocked { .. } => {
                self.touch_module("achievements", event);
                self.touch_module("swarm_notifications", event);
                self.touch_module("forge_quest", event);
            }

            // ── Level up -> talent points + notification ──
            SwarmEvent::LevelUp { .. } => {
                self.touch_module("swarm_meta", event);
                self.touch_module("swarm_notifications", event);
                self.touch_module("achievements", event);
            }

            // ── Spy planted -> diplomacy ──
            SwarmEvent::SpyPlanted { .. } => {
                self.touch_module("swarm_diplomacy", event);
            }

            // ── Spy detected -> counter-intel + notification ──
            SwarmEvent::SpyDetected { .. } => {
                self.touch_module("swarm_diplomacy", event);
                self.touch_module("swarm_notifications", event);
                self.touch_module("swarm_advanced", event);
            }

            // ── Intel gathered -> diplomacy ──
            SwarmEvent::IntelGathered { .. } => {
                self.touch_module("swarm_diplomacy", event);
            }

            // ── ImpForge productivity -> game resources ──
            SwarmEvent::ProductivityAction {
                action,
                dm_earned,
                resources_earned: _,
            } => {
                self.touch_module("forge_quest", event);
                self.touch_module("swarm_engagement", event);
                self.touch_module("swarm_special", event);

                // Bridge productivity to Dark Matter
                cascading.push(SwarmEvent::DarkMatterEarned {
                    amount: *dm_earned,
                    source: action.clone(),
                });
            }

            // ── Fleet destroyed -> debris field + moon chance ──
            SwarmEvent::FleetDestroyed { .. } => {
                self.touch_module("swarm_advanced", event);
                self.touch_module("swarm_notifications", event);
            }

            // ── Commander directive -> AI system ──
            SwarmEvent::CommanderDirective { .. } => {
                self.touch_module("swarm_advanced", event);
                self.touch_module("swarm_ai", event);
            }

            // ── Trade offer received -> commander evaluation + notification ──
            SwarmEvent::TradeOfferReceived { .. } => {
                self.touch_module("swarm_advanced", event);
                self.touch_module("swarm_notifications", event);
            }
        }

        // Log this event
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let entry = EventLogEntry {
            timestamp_ms: now,
            event: event.to_string(),
            cascaded_count: cascading.len(),
        };

        if self.event_log.len() >= EVENT_LOG_CAPACITY {
            self.event_log.remove(0);
        }
        self.event_log.push(entry);

        cascading
    }

    /// Process an event and recursively process all cascading events (depth-limited).
    pub(crate) fn process_event_recursive(
        &mut self,
        event: &SwarmEvent,
        max_depth: u32,
    ) -> Vec<SwarmEvent> {
        if max_depth == 0 {
            return Vec::new();
        }

        let cascading = self.process_event(event);
        let mut all_events = cascading.clone();

        for cascaded in &cascading {
            let sub = self.process_event_recursive(cascaded, max_depth - 1);
            all_events.extend(sub);
        }

        all_events
    }

    /// Record that a module was touched by an event.
    fn touch_module(&mut self, module_name: &str, event: &SwarmEvent) {
        if let Some(m) = self.modules.iter_mut().find(|m| m.name == module_name) {
            m.event_count += 1;
            m.last_event = Some(event.to_string());
        }
    }

    /// Get status of all 22 modules.
    pub(crate) fn module_status(&self) -> &[ModuleStatus] {
        &self.modules
    }

    /// Get total events processed.
    pub(crate) fn total_events(&self) -> u64 {
        self.total_events
    }

    /// Get recent event log entries.
    pub(crate) fn recent_events(&self, limit: usize) -> Vec<&EventLogEntry> {
        let start = self.event_log.len().saturating_sub(limit);
        self.event_log[start..].iter().collect()
    }
}

// ============================================================================
// Event Routing Table -- Documents which modules react to which events
// ============================================================================

/// Returns a JSON object documenting which modules listen to each event type.
///
/// This is reference documentation for frontend developers and for the game
/// designer to understand the full event flow.
pub(crate) fn get_event_routing_table() -> serde_json::Value {
    serde_json::json!({
        "UnitKilled": [
            "swarm_factions (corpses/biomass/faith)",
            "swarm_combat (terrain corruption)",
            "swarm_engagement (kill challenges)",
            "swarm_notifications (kill feed)"
        ],
        "BattleWon": [
            "swarm_notifications (victory alert)",
            "swarm_engagement (battle challenges)",
            "swarm_meta (prestige progress)",
            "swarm_factions (faith for humans)"
        ],
        "BattleLost": [
            "swarm_notifications (defeat alert)",
            "swarm_engagement (battle tracking)"
        ],
        "BuildingCompleted": [
            "swarm_notifications (completion alert)",
            "forge_quest (XP earned)",
            "swarm_engagement (build challenges)",
            "achievements (build milestones)"
        ],
        "ResearchCompleted": [
            "swarm_notifications (research done)",
            "forge_quest (unlock units/buildings)",
            "swarm_engagement (research challenges)"
        ],
        "UnitTrained": [
            "swarm_notifications (training done)",
            "swarm_combat (military strength)"
        ],
        "ColonyFounded": [
            "swarm_galaxy (map update)",
            "swarm_diplomacy (neighbor detection)",
            "swarm_notifications (colony alert)",
            "swarm_worldgen (generate terrain)"
        ],
        "ResourcesEarned": [
            "swarm_factions (resource state)",
            "swarm_database (persistence)"
        ],
        "DarkMatterEarned": [
            "forge_quest (DM balance)",
            "swarm_engagement (DM challenges)"
        ],
        "StorageFull": [
            "swarm_notifications (overflow warning)"
        ],
        "LarvaeInjected": [
            "swarm_factions (insect economy)"
        ],
        "KultistenSacrificed": [
            "swarm_factions (energy decrease)",
            "swarm_notifications (sacrifice alert)",
            "swarm_combat (spawn unit if large)"
        ],
        "CorpseHarvested": [
            "swarm_factions (undead economy)"
        ],
        "BlightSpread": [
            "swarm_special (terrain spreader)",
            "swarm_worldgen (fog update)",
            "swarm_combat (terrain effects via TerrainSpread cascade)"
        ],
        "FaithGained": [
            "swarm_factions (human economy)"
        ],
        "CorruptionDecayed": [
            "swarm_factions (demon economy)"
        ],
        "TerrainSpread": [
            "swarm_combat (terrain combat effects)",
            "swarm_worldgen (fog update)",
            "swarm_special (spreader chain rules)"
        ],
        "TerrainContested": [
            "swarm_combat (contested zone penalties)",
            "swarm_notifications (territory alert)"
        ],
        "PrestigePerformed": [
            "offline_progression (multipliers)",
            "swarm_notifications (prestige alert)",
            "swarm_meta (prestige state)",
            "achievements (prestige milestone)"
        ],
        "AchievementUnlocked": [
            "achievements (persist)",
            "swarm_notifications (popup)",
            "forge_quest (XP reward)"
        ],
        "LevelUp": [
            "swarm_meta (talent points)",
            "swarm_notifications (level popup)",
            "achievements (level milestones)"
        ],
        "SpyPlanted": [
            "swarm_diplomacy (espionage state)"
        ],
        "SpyDetected": [
            "swarm_diplomacy (counter-intel)",
            "swarm_notifications (spy alert)",
            "swarm_advanced (alertness level)"
        ],
        "IntelGathered": [
            "swarm_diplomacy (intel report)"
        ],
        "ProductivityAction": [
            "forge_quest (DM + resources)",
            "swarm_engagement (productivity challenges)",
            "swarm_special (visual swarm overlay)",
            "DarkMatterEarned (cascade)"
        ],
        "FleetDestroyed": [
            "swarm_advanced (debris field, moon check)",
            "swarm_notifications (fleet loss alert)"
        ],
        "CommanderDirective": [
            "swarm_advanced (commander AI)",
            "swarm_ai (NPC blackboard)"
        ],
        "TradeOfferReceived": [
            "swarm_advanced (commander evaluate)",
            "swarm_notifications (trade alert)"
        ]
    })
}

// ============================================================================
// Game Summary -- Aggregated state from all modules
// ============================================================================

/// Comprehensive game state summary aggregated from all modules.
///
/// This provides a single JSON snapshot of the entire game for the frontend
/// dashboard.  In a production build each field would query its respective
/// engine via Tauri managed state; here we provide the structural contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSummary {
    pub player: PlayerSummary,
    pub colonies: Vec<ColonySummary>,
    pub fleet: FleetSummary,
    pub prestige: PrestigeSummary,
    pub engagement: EngagementSummary,
    pub commander: CommanderSummary,
    pub notifications: NotificationSummary,
    pub game_loop: GameLoopSummary,
    pub integration: IntegrationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSummary {
    pub level: u32,
    pub xp: u64,
    pub dark_matter: u64,
    pub faction: String,
    pub talent_points_available: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonySummary {
    pub name: String,
    pub coord: String,
    pub ore: f64,
    pub crystal: f64,
    pub essence: f64,
    pub buildings: u32,
    pub units: u32,
    pub defense_rating: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetSummary {
    pub active_fleets: u32,
    pub total_ships: u32,
    pub missions_in_progress: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrestigeSummary {
    pub cycle_count: u32,
    pub phoenix_ash: u64,
    pub current_tier: String,
    pub offline_efficiency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngagementSummary {
    pub login_streak: u32,
    pub challenges_completed: u32,
    pub challenges_active: u32,
    pub expeditions_active: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommanderSummary {
    pub npc_ai_active: bool,
    pub strategy: String,
    pub alertness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSummary {
    pub unread: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLoopSummary {
    pub tick: u64,
    pub paused: bool,
    pub speed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationSummary {
    pub total_events_processed: u64,
    pub modules_connected: u32,
    pub modules_total: u32,
}

impl IntegrationHub {
    /// Build a game summary snapshot.
    ///
    /// Returns a default-populated summary.  In production, each section would
    /// query its respective Tauri-managed engine.  The structure defines the
    /// contract between Rust and the Svelte frontend.
    pub(crate) fn build_game_summary(&self) -> GameSummary {
        let connected = self.modules.iter().filter(|m| m.connected).count() as u32;

        GameSummary {
            player: PlayerSummary {
                level: 1,
                xp: 0,
                dark_matter: 0,
                faction: "insects".into(),
                talent_points_available: 0,
            },
            colonies: vec![ColonySummary {
                name: "Homeworld".into(),
                coord: "[1:001:01]".into(),
                ore: 500.0,
                crystal: 300.0,
                essence: 100.0,
                buildings: 3,
                units: 10,
                defense_rating: 50.0,
            }],
            fleet: FleetSummary {
                active_fleets: 0,
                total_ships: 0,
                missions_in_progress: 0,
            },
            prestige: PrestigeSummary {
                cycle_count: 0,
                phoenix_ash: 0,
                current_tier: "none".into(),
                offline_efficiency: 0.80,
            },
            engagement: EngagementSummary {
                login_streak: 0,
                challenges_completed: 0,
                challenges_active: 0,
                expeditions_active: 0,
            },
            commander: CommanderSummary {
                npc_ai_active: false,
                strategy: "balanced".into(),
                alertness: 0.0,
            },
            notifications: NotificationSummary {
                unread: 0,
                total: 0,
            },
            game_loop: GameLoopSummary {
                tick: 0,
                paused: true,
                speed: 1.0,
            },
            integration: IntegrationSummary {
                total_events_processed: self.total_events,
                modules_connected: connected,
                modules_total: MODULE_NAMES.len() as u32,
            },
        }
    }
}

// ============================================================================
// Productivity Bridge -- ImpForge actions -> SwarmForge resources
// ============================================================================

/// Productivity action categories and their Dark Matter / resource rewards.
///
/// These values define how real user actions in ImpForge translate to
/// in-game resources.  The formula is intentionally generous for "deep work"
/// actions (documents, code) and modest for passive actions (browsing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductivityReward {
    pub action: String,
    pub dark_matter: u32,
    pub ore: f64,
    pub crystal: f64,
    pub essence: f64,
    pub xp: u64,
    pub description: String,
}

/// Calculate productivity rewards for a given action.
///
/// Rewards are tuned so that an active ImpForge user earns meaningful
/// game progress without needing to explicitly play the idle game.
pub(crate) fn calculate_productivity_reward(action: &str) -> ProductivityReward {
    match action {
        "document_saved" => ProductivityReward {
            action: action.into(),
            dark_matter: 5,
            ore: 10.0,
            crystal: 5.0,
            essence: 2.0,
            xp: 25,
            description: "Saved a document in ForgeWriter".into(),
        },
        "workflow_completed" => ProductivityReward {
            action: action.into(),
            dark_matter: 10,
            ore: 20.0,
            crystal: 10.0,
            essence: 5.0,
            xp: 50,
            description: "Completed a ForgeFlow workflow".into(),
        },
        "ai_query" => ProductivityReward {
            action: action.into(),
            dark_matter: 2,
            ore: 3.0,
            crystal: 2.0,
            essence: 8.0,
            xp: 15,
            description: "Asked the AI assistant a question".into(),
        },
        "code_commit" => ProductivityReward {
            action: action.into(),
            dark_matter: 15,
            ore: 25.0,
            crystal: 15.0,
            essence: 10.0,
            xp: 75,
            description: "Committed code in CodeForge".into(),
        },
        "email_sent" => ProductivityReward {
            action: action.into(),
            dark_matter: 3,
            ore: 5.0,
            crystal: 3.0,
            essence: 1.0,
            xp: 10,
            description: "Sent an email from ForgeMail".into(),
        },
        "presentation_created" => ProductivityReward {
            action: action.into(),
            dark_matter: 8,
            ore: 15.0,
            crystal: 8.0,
            essence: 4.0,
            xp: 40,
            description: "Created a presentation in ForgeSlides".into(),
        },
        "spreadsheet_saved" => ProductivityReward {
            action: action.into(),
            dark_matter: 4,
            ore: 8.0,
            crystal: 4.0,
            essence: 2.0,
            xp: 20,
            description: "Saved a spreadsheet in ForgeSheets".into(),
        },
        "note_created" => ProductivityReward {
            action: action.into(),
            dark_matter: 3,
            ore: 5.0,
            crystal: 3.0,
            essence: 2.0,
            xp: 15,
            description: "Created a note in ForgeNotes".into(),
        },
        "browser_research" => ProductivityReward {
            action: action.into(),
            dark_matter: 1,
            ore: 2.0,
            crystal: 1.0,
            essence: 3.0,
            xp: 8,
            description: "Browsed research material in ForgeBrowser".into(),
        },
        "calendar_event" => ProductivityReward {
            action: action.into(),
            dark_matter: 2,
            ore: 3.0,
            crystal: 2.0,
            essence: 1.0,
            xp: 10,
            description: "Scheduled an event in ForgeCalendar".into(),
        },
        _ => ProductivityReward {
            action: action.into(),
            dark_matter: 1,
            ore: 1.0,
            crystal: 1.0,
            essence: 1.0,
            xp: 5,
            description: format!("Performed action: {action}"),
        },
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Process a game event through the integration hub and return all cascading events.
///
/// Events are recursively processed with a depth limit of 5 to prevent infinite
/// loops.  The returned vector contains ALL cascading events that were generated.
#[tauri::command]
pub fn swarm_process_event(
    event: SwarmEvent,
    hub: tauri::State<'_, Mutex<IntegrationHub>>,
) -> Result<Vec<SwarmEvent>, String> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_integration", "game_integration", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_integration", "game_integration");
    crate::synapse_fabric::synapse_session_push("swarm_integration", "game_integration", "swarm_process_event called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_integration", "info", "swarm_integration active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_integration", "process", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"event": "process"}));
    let mut hub = hub.lock().map_err(|e| format!("Hub lock failed: {e}"))?;
    let cascading = hub.process_event_recursive(&event, 5);
    Ok(cascading)
}

/// Get the event routing table showing which modules react to which events.
#[tauri::command]
pub fn swarm_event_routing() -> serde_json::Value {
    get_event_routing_table()
}

/// Get integration status for all 22 connected modules.
#[tauri::command]
pub fn swarm_integration_status(
    hub: tauri::State<'_, Mutex<IntegrationHub>>,
) -> Result<serde_json::Value, String> {
    let hub = hub.lock().map_err(|e| format!("Hub lock failed: {e}"))?;

    let modules: Vec<serde_json::Value> = hub
        .module_status()
        .iter()
        .map(|m| {
            serde_json::json!({
                "name": m.name,
                "connected": m.connected,
                "event_count": m.event_count,
                "last_event": m.last_event,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "total_events": hub.total_events(),
        "modules": modules,
        "recent_events": hub.recent_events(20),
    }))
}

/// Bridge an ImpForge productivity action into SwarmForge game resources.
///
/// Call this from ANY ImpForge module when a trackable user action occurs.
/// Returns the reward breakdown and any cascading game events.
#[tauri::command]
pub fn swarm_trigger_productivity(
    action: String,
    hub: tauri::State<'_, Mutex<IntegrationHub>>,
) -> Result<serde_json::Value, String> {
    let reward = calculate_productivity_reward(&action);

    let event = SwarmEvent::ProductivityAction {
        action: action.clone(),
        dm_earned: reward.dark_matter,
        resources_earned: reward.ore + reward.crystal + reward.essence,
    };

    let mut hub = hub.lock().map_err(|e| format!("Hub lock failed: {e}"))?;
    let cascading = hub.process_event_recursive(&event, 5);

    Ok(serde_json::json!({
        "reward": reward,
        "cascading_events": cascading.len(),
        "events": cascading,
    }))
}

/// Get a comprehensive game state summary from all modules.
///
/// Returns a single JSON object with player, colonies, fleet, prestige,
/// engagement, commander, notifications, game loop, and integration status.
#[tauri::command]
pub fn swarm_game_summary(
    hub: tauri::State<'_, Mutex<IntegrationHub>>,
) -> Result<GameSummary, String> {
    let hub = hub.lock().map_err(|e| format!("Hub lock failed: {e}"))?;
    Ok(hub.build_game_summary())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_hub_creation() {
        let hub = IntegrationHub::new();
        assert_eq!(hub.total_events(), 0);
        assert_eq!(hub.module_status().len(), MODULE_NAMES.len());
        assert_eq!(hub.module_status().len(), 22);
    }

    #[test]
    fn test_all_modules_connected_on_init() {
        let hub = IntegrationHub::new();
        for m in hub.module_status() {
            assert!(m.connected, "Module {} should be connected", m.name);
            assert_eq!(m.event_count, 0);
            assert!(m.last_event.is_none());
        }
    }

    #[test]
    fn test_unit_killed_undead_generates_corpse_harvest() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::UnitKilled {
            killer_faction: "undead".into(),
            victim_faction: "humans".into(),
            victim_type: "soldier".into(),
            hex_q: 3,
            hex_r: 5,
        };

        let cascading = hub.process_event(&event);
        assert!(!cascading.is_empty(), "Undead kill should generate cascading events");

        let has_corpse = cascading.iter().any(|e| {
            matches!(e, SwarmEvent::CorpseHarvested { leichenteile, .. } if *leichenteile == 0.5)
        });
        assert!(has_corpse, "Should contain CorpseHarvested with 0.5 leichenteile");
    }

    #[test]
    fn test_unit_killed_demons_generates_terrain_spread() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::UnitKilled {
            killer_faction: "demons".into(),
            victim_faction: "insects".into(),
            victim_type: "drone".into(),
            hex_q: 1,
            hex_r: 2,
        };

        let cascading = hub.process_event(&event);
        let has_terrain = cascading.iter().any(|e| {
            matches!(e, SwarmEvent::TerrainSpread { faction, hex_q, hex_r, .. }
                if faction == "demons" && *hex_q == 1 && *hex_r == 2)
        });
        assert!(has_terrain, "Demon kill should spread corruption");
    }

    #[test]
    fn test_unit_killed_humans_generates_faith() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::UnitKilled {
            killer_faction: "humans".into(),
            victim_faction: "demons".into(),
            victim_type: "imp".into(),
            hex_q: 0,
            hex_r: 0,
        };

        let cascading = hub.process_event(&event);
        let has_faith = cascading.iter().any(|e| {
            matches!(e, SwarmEvent::FaithGained { amount, from_victory }
                if *amount == 1.0 && *from_victory)
        });
        assert!(has_faith, "Human kill should grant Faith");
    }

    #[test]
    fn test_unit_killed_insects_generates_biomass() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::UnitKilled {
            killer_faction: "insects".into(),
            victim_faction: "undead".into(),
            victim_type: "skeleton".into(),
            hex_q: 5,
            hex_r: 5,
        };

        let cascading = hub.process_event(&event);
        let has_biomass = cascading.iter().any(|e| {
            matches!(e, SwarmEvent::ResourcesEarned { resource, .. } if resource == "biomass")
        });
        assert!(has_biomass, "Insect kill should grant biomass");
    }

    #[test]
    fn test_building_completed_generates_achievement() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::BuildingCompleted {
            colony_id: "c1".into(),
            building_type: "mine".into(),
            level: 3,
        };

        let cascading = hub.process_event(&event);
        let has_achievement = cascading.iter().any(|e| {
            matches!(e, SwarmEvent::AchievementUnlocked { achievement_id, xp }
                if achievement_id == "build_mine" && *xp == 30)
        });
        assert!(has_achievement, "Building should unlock achievement with level*10 XP");
    }

    #[test]
    fn test_productivity_action_generates_dark_matter() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::ProductivityAction {
            action: "document_saved".into(),
            dm_earned: 5,
            resources_earned: 17.0,
        };

        let cascading = hub.process_event(&event);
        let has_dm = cascading.iter().any(|e| {
            matches!(e, SwarmEvent::DarkMatterEarned { amount, source }
                if *amount == 5 && source == "document_saved")
        });
        assert!(has_dm, "Productivity should cascade to DarkMatterEarned");
    }

    #[test]
    fn test_prestige_generates_achievement() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::PrestigePerformed {
            tier: "molt".into(),
            phoenix_ash_earned: 100,
        };

        let cascading = hub.process_event(&event);
        let has_ach = cascading.iter().any(|e| {
            matches!(e, SwarmEvent::AchievementUnlocked { achievement_id, .. }
                if achievement_id == "prestige_molt")
        });
        assert!(has_ach, "Prestige should generate achievement");
    }

    #[test]
    fn test_recursive_processing_depth_limit() {
        let mut hub = IntegrationHub::new();
        // BlightSpread cascades to TerrainSpread, which touches more modules
        let event = SwarmEvent::BlightSpread {
            hex_q: 4,
            hex_r: 7,
            adept_id: "adept_1".into(),
        };

        let all = hub.process_event_recursive(&event, 5);
        // Should have at least the TerrainSpread cascade from BlightSpread
        assert!(!all.is_empty(), "Recursive processing should produce events");
        // Depth limit prevents infinite loops
        assert!(all.len() < 100, "Depth limit should prevent runaway cascades");
    }

    #[test]
    fn test_event_log_ring_buffer() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::FaithGained {
            amount: 1.0,
            from_victory: false,
        };

        // Process more events than the ring buffer capacity
        for _ in 0..EVENT_LOG_CAPACITY + 50 {
            hub.process_event(&event);
        }

        assert_eq!(
            hub.event_log.len(),
            EVENT_LOG_CAPACITY,
            "Ring buffer should cap at {EVENT_LOG_CAPACITY}"
        );
        assert_eq!(hub.total_events(), (EVENT_LOG_CAPACITY + 50) as u64);
    }

    #[test]
    fn test_module_touch_tracking() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::SpyDetected {
            colony_id: "c1".into(),
            spy_faction: "demons".into(),
        };

        hub.process_event(&event);

        let diplomacy = hub
            .module_status()
            .iter()
            .find(|m| m.name == "swarm_diplomacy")
            .expect("swarm_diplomacy should exist");
        assert_eq!(diplomacy.event_count, 1);
        assert!(diplomacy.last_event.is_some());

        let notifications = hub
            .module_status()
            .iter()
            .find(|m| m.name == "swarm_notifications")
            .expect("swarm_notifications should exist");
        assert_eq!(notifications.event_count, 1);
    }

    #[test]
    fn test_event_routing_table_completeness() {
        let table = get_event_routing_table();
        let obj = table.as_object().expect("Should be a JSON object");

        // Every event variant should have a routing entry
        let expected_events = [
            "UnitKilled",
            "BattleWon",
            "BattleLost",
            "BuildingCompleted",
            "ResearchCompleted",
            "UnitTrained",
            "ColonyFounded",
            "ResourcesEarned",
            "DarkMatterEarned",
            "StorageFull",
            "LarvaeInjected",
            "KultistenSacrificed",
            "CorpseHarvested",
            "BlightSpread",
            "FaithGained",
            "CorruptionDecayed",
            "TerrainSpread",
            "TerrainContested",
            "PrestigePerformed",
            "AchievementUnlocked",
            "LevelUp",
            "SpyPlanted",
            "SpyDetected",
            "IntelGathered",
            "ProductivityAction",
            "FleetDestroyed",
            "CommanderDirective",
            "TradeOfferReceived",
        ];

        for event_name in &expected_events {
            assert!(
                obj.contains_key(*event_name),
                "Routing table missing entry for {event_name}"
            );
        }
    }

    #[test]
    fn test_productivity_reward_calculation() {
        let reward = calculate_productivity_reward("code_commit");
        assert_eq!(reward.dark_matter, 15);
        assert_eq!(reward.xp, 75);
        assert!(reward.ore > 0.0);
        assert!(!reward.description.is_empty());
    }

    #[test]
    fn test_productivity_reward_unknown_action() {
        let reward = calculate_productivity_reward("unknown_thing");
        assert_eq!(reward.dark_matter, 1);
        assert_eq!(reward.xp, 5);
        assert!(reward.description.contains("unknown_thing"));
    }

    #[test]
    fn test_game_summary_structure() {
        let hub = IntegrationHub::new();
        let summary = hub.build_game_summary();

        assert_eq!(summary.player.level, 1);
        assert_eq!(summary.player.faction, "insects");
        assert!(!summary.colonies.is_empty());
        assert_eq!(summary.integration.modules_total, 22);
        assert_eq!(summary.integration.modules_connected, 22);
        assert_eq!(summary.prestige.offline_efficiency, 0.80);
        assert!(summary.game_loop.paused);
    }

    #[test]
    fn test_swarm_event_display() {
        let event = SwarmEvent::UnitKilled {
            killer_faction: "insects".into(),
            victim_faction: "humans".into(),
            victim_type: "marine".into(),
            hex_q: 1,
            hex_r: 2,
        };
        let display = format!("{event}");
        assert!(display.contains("insects"));
        assert!(display.contains("marine"));
    }

    #[test]
    fn test_battle_won_humans_faith_cascade() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::BattleWon {
            winner: "humans".into(),
            loser: "undead".into(),
            location: "field_1".into(),
        };

        let cascading = hub.process_event(&event);
        let has_faith = cascading.iter().any(|e| {
            matches!(e, SwarmEvent::FaithGained { amount, from_victory }
                if *amount == 5.0 && *from_victory)
        });
        assert!(has_faith, "Human battle victory should grant 5.0 Faith");
    }

    #[test]
    fn test_kultisten_large_sacrifice_dark_matter() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::KultistenSacrificed {
            colony_id: "c1".into(),
            count: 10,
            for_unit: "pit_lord".into(),
        };

        let cascading = hub.process_event(&event);
        let has_dm = cascading.iter().any(|e| {
            matches!(e, SwarmEvent::DarkMatterEarned { amount, .. } if *amount == 100)
        });
        assert!(has_dm, "Large Kultisten sacrifice should yield DM (count * 10)");
    }

    #[test]
    fn test_event_serialization_roundtrip() {
        let event = SwarmEvent::BuildingCompleted {
            colony_id: "test_colony".into(),
            building_type: "barracks".into(),
            level: 5,
        };

        let json = serde_json::to_string(&event).expect("Should serialize");
        assert!(json.contains("BuildingCompleted"));
        assert!(json.contains("barracks"));

        let deserialized: SwarmEvent =
            serde_json::from_str(&json).expect("Should deserialize");
        match deserialized {
            SwarmEvent::BuildingCompleted {
                colony_id,
                building_type,
                level,
            } => {
                assert_eq!(colony_id, "test_colony");
                assert_eq!(building_type, "barracks");
                assert_eq!(level, 5);
            }
            _ => panic!("Wrong event variant after roundtrip"),
        }
    }

    #[test]
    fn test_recent_events_limit() {
        let mut hub = IntegrationHub::new();
        let event = SwarmEvent::FaithGained {
            amount: 1.0,
            from_victory: false,
        };

        for _ in 0..50 {
            hub.process_event(&event);
        }

        let recent_5 = hub.recent_events(5);
        assert_eq!(recent_5.len(), 5);

        let recent_100 = hub.recent_events(100);
        assert_eq!(recent_100.len(), 50);
    }
}
