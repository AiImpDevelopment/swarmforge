// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Presentation Layer — Sound, Tutorial & Isometric Config
//!
//! Three subsystems that provide DATA and CONFIGURATION to the Svelte/PixiJS
//! frontend.  The actual rendering, audio playback, and tutorial UI happen
//! entirely in the browser — this module delivers the structured blueprints.
//!
//! ## Sound & Music System
//!
//! ~80 sound asset descriptors covering ambient, combat, UI, music, voice,
//! and environment categories.  An adaptive music state machine drives
//! crossfade transitions between Calm, Combat, Victory, Epic, etc.
//!
//! ## Tutorial & Onboarding
//!
//! A 20-step guided sequence that teaches the core gameplay loop: faction
//! choice, resource buildings, research, shipyard, galaxy exploration,
//! mutation, Dark Matter, alliances, and offline progression.  Steps
//! carry optional rewards (DM, XP, resources) and track completion in
//! `TutorialProgress`.
//!
//! ## Isometric Rendering Configuration
//!
//! Tile-based diamond isometric constants (64x32 tiles), sprite sheet
//! metadata (8 directions x 8 actions x 8 frames), camera zoom bounds,
//! biome-to-texture mappings for all 14 biome types, and coordinate
//! conversion helpers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{AppResult, ImpForgeError};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_presentation", "Game");

// ===========================================================================
// PART 1: Sound & Music System
// ===========================================================================

/// Sound effect categories.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SoundCategory {
    /// Background loops (wind, cave drips, hive hum).
    Ambient,
    /// Battle sounds (explosions, clashes, death cries).
    Combat,
    /// Interface feedback (click, build complete, error).
    Ui,
    /// Background music tracks.
    Music,
    /// Unit voice responses ("ready", "attacking", "retreating").
    Voice,
    /// Weather & terrain (rain, lava bubbles, thunder).
    Environment,
}

/// A sound asset reference.  The frontend loads the actual file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundAsset {
    pub id: String,
    pub name: String,
    pub category: SoundCategory,
    /// Relative path under `assets/sounds/`.
    pub file_path: String,
    pub duration_secs: f64,
    pub loop_enabled: bool,
    /// Default volume 0.0..=1.0.
    pub volume: f64,
    /// Faction-specific sounds (None = universal).
    pub faction: Option<String>,
}

/// Adaptive music state machine.
///
/// The frontend monitors game events and transitions between states.
/// Each state maps to a pool of tracks filtered by faction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MusicState {
    /// Peaceful, base building.
    Calm,
    /// Active construction / research.
    Building,
    /// Enemy detected nearby.
    Tense,
    /// Active battle.
    Combat,
    /// Battle won.
    Victory,
    /// Battle lost.
    Defeat,
    /// Boss fight / major event.
    Epic,
}
impl MusicState {
    pub(crate) fn all() -> &'static [MusicState] {
        &[
            Self::Calm,
            Self::Building,
            Self::Tense,
            Self::Combat,
            Self::Victory,
            Self::Defeat,
            Self::Epic,
        ]
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Calm => "calm",
            Self::Building => "building",
            Self::Tense => "tense",
            Self::Combat => "combat",
            Self::Victory => "victory",
            Self::Defeat => "defeat",
            Self::Epic => "epic",
        }
    }

    pub(crate) fn from_str_name(s: &str) -> Option<Self> {
        match s {
            "calm" => Some(Self::Calm),
            "building" => Some(Self::Building),
            "tense" => Some(Self::Tense),
            "combat" => Some(Self::Combat),
            "victory" => Some(Self::Victory),
            "defeat" => Some(Self::Defeat),
            "epic" => Some(Self::Epic),
            _ => None,
        }
    }
}

/// Player-facing audio settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    pub master_volume: f64,
    pub music_volume: f64,
    pub sfx_volume: f64,
    pub ambient_volume: f64,
    pub voice_volume: f64,
    pub muted: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master_volume: 0.8,
            music_volume: 0.6,
            sfx_volume: 0.8,
            ambient_volume: 0.5,
            voice_volume: 0.7,
            muted: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Sound helpers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn sa(
    id: &str,
    name: &str,
    cat: SoundCategory,
    path: &str,
    dur: f64,
    looped: bool,
    vol: f64,
    faction: Option<&str>,
) -> SoundAsset {
    SoundAsset {
        id: id.into(),
        name: name.into(),
        category: cat,
        file_path: path.into(),
        duration_secs: dur,
        loop_enabled: looped,
        volume: vol,
        faction: faction.map(Into::into),
    }
}

/// Complete sound registry (~80 entries across all categories).
pub(crate) fn get_sound_registry() -> Vec<SoundAsset> {
    let mut sounds = Vec::with_capacity(80);

    // ── Ambient (4 faction tracks + 4 environment loops = 8) ──────────
    sounds.push(sa("amb_insect_hive", "Insect Hive Buzz", SoundCategory::Ambient,
        "ambient/insect_hive_buzz.ogg", 60.0, true, 0.35, Some("insect")));
    sounds.push(sa("amb_demon_fire", "Hellfire Crackle", SoundCategory::Ambient,
        "ambient/demon_hellfire_crackle.ogg", 60.0, true, 0.35, Some("demon")));
    sounds.push(sa("amb_undead_ghostly", "Ghostly Whispers", SoundCategory::Ambient,
        "ambient/undead_ghostly_whispers.ogg", 60.0, true, 0.30, Some("undead")));
    sounds.push(sa("amb_human_medieval", "Medieval Town", SoundCategory::Ambient,
        "ambient/human_medieval_town.ogg", 60.0, true, 0.35, Some("human")));
    sounds.push(sa("amb_wind", "Wind Loop", SoundCategory::Ambient,
        "ambient/wind_loop.ogg", 30.0, true, 0.25, None));
    sounds.push(sa("amb_rain", "Rain Loop", SoundCategory::Ambient,
        "ambient/rain_loop.ogg", 30.0, true, 0.30, None));
    sounds.push(sa("amb_cave_drip", "Cave Drip", SoundCategory::Ambient,
        "ambient/cave_drip.ogg", 20.0, true, 0.20, None));
    sounds.push(sa("amb_lava_bubble", "Lava Bubbling", SoundCategory::Ambient,
        "ambient/lava_bubble.ogg", 25.0, true, 0.30, None));

    // ── Combat — damage type hits (7) ─────────────────────────────────
    sounds.push(sa("dmg_fire_hit", "Fire Hit", SoundCategory::Combat,
        "combat/fire_hit.ogg", 0.8, false, 0.7, None));
    sounds.push(sa("dmg_plasma_hit", "Plasma Hit", SoundCategory::Combat,
        "combat/plasma_hit.ogg", 0.6, false, 0.7, None));
    sounds.push(sa("dmg_electricity_hit", "Electricity Hit", SoundCategory::Combat,
        "combat/electricity_hit.ogg", 0.5, false, 0.7, None));
    sounds.push(sa("dmg_corrosion_hit", "Corrosion Hit", SoundCategory::Combat,
        "combat/corrosion_hit.ogg", 0.9, false, 0.65, None));
    sounds.push(sa("dmg_slash_hit", "Slash Hit", SoundCategory::Combat,
        "combat/slash_hit.ogg", 0.4, false, 0.7, None));
    sounds.push(sa("dmg_stab_hit", "Stab Hit", SoundCategory::Combat,
        "combat/stab_hit.ogg", 0.35, false, 0.7, None));
    sounds.push(sa("dmg_blunt_hit", "Blunt Hit", SoundCategory::Combat,
        "combat/blunt_hit.ogg", 0.5, false, 0.75, None));

    // ── Combat — battle sounds (8) ────────────────────────────────────
    sounds.push(sa("cbt_sword_clash", "Sword Clash", SoundCategory::Combat,
        "combat/sword_clash.ogg", 0.6, false, 0.75, None));
    sounds.push(sa("cbt_spell_cast", "Spell Cast", SoundCategory::Combat,
        "combat/spell_cast.ogg", 1.2, false, 0.65, None));
    sounds.push(sa("cbt_explosion", "Explosion", SoundCategory::Combat,
        "combat/explosion.ogg", 1.5, false, 0.80, None));
    sounds.push(sa("cbt_death_cry", "Death Cry", SoundCategory::Combat,
        "combat/death_cry.ogg", 1.0, false, 0.60, None));
    sounds.push(sa("cbt_charge", "Charge", SoundCategory::Combat,
        "combat/charge.ogg", 2.0, false, 0.70, None));
    sounds.push(sa("cbt_retreat", "Retreat Horn", SoundCategory::Combat,
        "combat/retreat_horn.ogg", 1.8, false, 0.65, None));
    sounds.push(sa("cbt_shield_block", "Shield Block", SoundCategory::Combat,
        "combat/shield_block.ogg", 0.4, false, 0.70, None));
    sounds.push(sa("cbt_crit_hit", "Critical Hit", SoundCategory::Combat,
        "combat/crit_hit.ogg", 0.5, false, 0.85, None));

    // ── UI sounds (10) ────────────────────────────────────────────────
    sounds.push(sa("ui_click", "Click", SoundCategory::Ui,
        "ui/click.ogg", 0.1, false, 0.50, None));
    sounds.push(sa("ui_hover", "Hover", SoundCategory::Ui,
        "ui/hover.ogg", 0.08, false, 0.30, None));
    sounds.push(sa("ui_build_complete", "Build Complete", SoundCategory::Ui,
        "ui/build_complete.ogg", 1.5, false, 0.70, None));
    sounds.push(sa("ui_research_done", "Research Done", SoundCategory::Ui,
        "ui/research_done.ogg", 2.0, false, 0.70, None));
    sounds.push(sa("ui_level_up", "Level Up", SoundCategory::Ui,
        "ui/level_up.ogg", 2.5, false, 0.80, None));
    sounds.push(sa("ui_achievement", "Achievement Unlocked", SoundCategory::Ui,
        "ui/achievement.ogg", 3.0, false, 0.85, None));
    sounds.push(sa("ui_error", "Error", SoundCategory::Ui,
        "ui/error.ogg", 0.5, false, 0.60, None));
    sounds.push(sa("ui_notification", "Notification", SoundCategory::Ui,
        "ui/notification.ogg", 0.8, false, 0.55, None));
    sounds.push(sa("ui_tab_switch", "Tab Switch", SoundCategory::Ui,
        "ui/tab_switch.ogg", 0.15, false, 0.35, None));
    sounds.push(sa("ui_dark_matter", "Dark Matter Earned", SoundCategory::Ui,
        "ui/dark_matter.ogg", 2.0, false, 0.75, None));

    // ── Music state tracks (4 per state x 7 states = 28) ──────────────
    // Each state has a universal track plus 3 faction-flavored variants.
    for (state_id, state_name, dur) in &[
        ("calm",    "Calm",    180.0),
        ("building","Building",150.0),
        ("tense",   "Tense",   120.0),
        ("combat",  "Combat",  150.0),
        ("victory", "Victory",  90.0),
        ("defeat",  "Defeat",   90.0),
        ("epic",    "Epic",    180.0),
    ] {
        sounds.push(sa(
            &format!("mus_{state_id}_universal"),
            &format!("{state_name} Theme"),
            SoundCategory::Music,
            &format!("music/{state_id}_universal.ogg"),
            *dur, true, 0.50, None,
        ));
        for (fac_id, fac_name) in &[("insect","Insect"), ("demon","Demon"), ("undead","Undead")] {
            sounds.push(sa(
                &format!("mus_{state_id}_{fac_id}"),
                &format!("{state_name} ({fac_name})"),
                SoundCategory::Music,
                &format!("music/{state_id}_{fac_id}.ogg"),
                *dur, true, 0.50, Some(fac_id),
            ));
        }
    }

    // ── Voice lines — 4 factions x 3 responses = 12 ──────────────────
    for (fac_id, fac_name) in &[("insect","Insect"), ("demon","Demon"), ("undead","Undead"), ("human","Human")] {
        for (resp_id, resp_name, dur) in &[
            ("ready",     "Ready",     1.2),
            ("attacking",  "Attacking", 1.0),
            ("retreating", "Retreating",1.5),
        ] {
            sounds.push(sa(
                &format!("vox_{fac_id}_{resp_id}"),
                &format!("{fac_name} {resp_name}"),
                SoundCategory::Voice,
                &format!("voice/{fac_id}_{resp_id}.ogg"),
                *dur, false, 0.70, Some(fac_id),
            ));
        }
    }

    // ── Environment (4) ───────────────────────────────────────────────
    sounds.push(sa("env_thunder", "Thunder", SoundCategory::Environment,
        "environment/thunder.ogg", 3.0, false, 0.65, None));
    sounds.push(sa("env_earthquake", "Earthquake Rumble", SoundCategory::Environment,
        "environment/earthquake_rumble.ogg", 4.0, false, 0.70, None));
    sounds.push(sa("env_warp_jump", "Warp Jump", SoundCategory::Environment,
        "environment/warp_jump.ogg", 2.5, false, 0.75, None));
    sounds.push(sa("env_corruption_spread", "Corruption Spread", SoundCategory::Environment,
        "environment/corruption_spread.ogg", 5.0, false, 0.45, None));

    sounds
}

/// Return music tracks matching a given state and faction.
///
/// Falls back to universal tracks when no faction-specific track exists.
pub(crate) fn get_music_for_state(state: &MusicState, faction: &str) -> Vec<SoundAsset> {
    let state_str = state.as_str();
    get_sound_registry()
        .into_iter()
        .filter(|s| {
            s.category == SoundCategory::Music
                && s.file_path.contains(state_str)
                && (s.faction.is_none()
                    || s.faction.as_deref() == Some(faction))
        })
        .collect()
}
/// Return crossfade metadata for a music transition.
fn transition_music(from: &MusicState, to: &MusicState) -> serde_json::Value {
    let (duration_ms, curve) = match (from, to) {
        (MusicState::Calm, MusicState::Tense)
        | (MusicState::Building, MusicState::Tense) => (2000, "ease_in"),
        (MusicState::Tense, MusicState::Combat) => (1000, "linear"),
        (MusicState::Combat, MusicState::Victory) => (3000, "ease_out"),
        (MusicState::Combat, MusicState::Defeat) => (3000, "ease_out"),
        (MusicState::Victory, MusicState::Calm)
        | (MusicState::Defeat, MusicState::Calm) => (4000, "ease_in_out"),
        (_, MusicState::Epic) => (500, "linear"),
        (MusicState::Epic, _) => (2000, "ease_out"),
        _ => (2000, "ease_in_out"),
    };

    serde_json::json!({
        "from": from.as_str(),
        "to": to.as_str(),
        "crossfade_ms": duration_ms,
        "curve": curve,
    })
}

// ===========================================================================
// PART 2: Tutorial & Onboarding System
// ===========================================================================

/// An action the player must take to complete a tutorial step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum TutorialAction {
    ClickButton { button_id: String },
    BuildStructure { building_type: String },
    TrainUnit { unit_type: String },
    StartResearch { tech: String },
    SendFleet,
    OpenTab { tab_name: String },
    CollectResources,
    EvolveUnit,
    ViewGalaxy,
    /// Just click "continue" — informational slide.
    AnyAction,
}

/// Reward granted on step completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialReward {
    pub dark_matter: u32,
    pub resources: HashMap<String, f64>,
    pub xp: u64,
    pub achievement_id: Option<String>,
}

impl TutorialReward {
    fn dm(amount: u32) -> Self {
        Self {
            dark_matter: amount,
            resources: HashMap::new(),
            xp: 0,
            achievement_id: None,
        }
    }

    fn xp(amount: u64) -> Self {
        Self {
            dark_matter: 0,
            resources: HashMap::new(),
            xp: amount,
            achievement_id: None,
        }
    }

    fn resources(pairs: &[(&str, f64)]) -> Self {
        Self {
            dark_matter: 0,
            resources: pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            xp: 0,
            achievement_id: None,
        }
    }

    fn full(dm: u32, xp: u64, ach: Option<&str>) -> Self {
        Self {
            dark_matter: dm,
            resources: HashMap::new(),
            xp,
            achievement_id: ach.map(Into::into),
        }
    }
}

/// A single step in the tutorial sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialStep {
    pub id: String,
    pub title: String,
    pub description: String,
    /// CSS selector the frontend should highlight (spotlight effect).
    pub highlight_element: Option<String>,
    pub required_action: TutorialAction,
    pub reward: Option<TutorialReward>,
    /// ID of the next step, or `None` if this is the final step.
    pub next_step: Option<String>,
    /// If true, the step text/highlight adapts to the chosen faction.
    pub faction_specific: bool,
}

/// Tracks a player's progress through the tutorial.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialProgress {
    pub current_step: String,
    pub completed_steps: Vec<String>,
    pub skipped: bool,
    pub started_at: String,
    pub faction_chosen: Option<String>,
}

impl Default for TutorialProgress {
    fn default() -> Self {
        Self {
            current_step: "tut_01_welcome".into(),
            completed_steps: Vec::new(),
            skipped: false,
            started_at: String::new(),
            faction_chosen: None,
        }
    }
}

/// Build the full 20-step tutorial sequence.
pub(crate) fn get_tutorial_steps() -> Vec<TutorialStep> {
    vec![
        // 1. Welcome & faction choice
        TutorialStep {
            id: "tut_01_welcome".into(),
            title: "Welcome to SwarmForge!".into(),
            description: "Your journey begins here. Choose a faction to lead: \
                Insects (swarm tactics), Demons (raw power), Undead (attrition), \
                or Humans (balanced). Your faction determines units, buildings, \
                tech trees, and lore.".into(),
            highlight_element: Some("#faction-select-panel".into()),
            required_action: TutorialAction::ClickButton { button_id: "btn-choose-faction".into() },
            reward: Some(TutorialReward::dm(10)),
            next_step: Some("tut_02_overview".into()),
            faction_specific: false,
        },
        // 2. Overview tab
        TutorialStep {
            id: "tut_02_overview".into(),
            title: "Your Colony at a Glance".into(),
            description: "The Overview tab shows your resource production rates, \
                active builds, fleet status, and incoming attacks. Keep an eye \
                on it — information is power.".into(),
            highlight_element: Some("#tab-overview".into()),
            required_action: TutorialAction::OpenTab { tab_name: "overview".into() },
            reward: Some(TutorialReward::xp(50)),
            next_step: Some("tut_03_first_building".into()),
            faction_specific: false,
        },
        // 3. Build first resource building
        TutorialStep {
            id: "tut_03_first_building".into(),
            title: "Build a Resource Extractor".into(),
            description: "Resources power everything. Build your first extractor \
                to start harvesting biomass. Click the Build tab and select \
                your faction's primary resource building.".into(),
            highlight_element: Some("#tab-buildings".into()),
            required_action: TutorialAction::BuildStructure { building_type: "resource_extractor".into() },
            reward: Some(TutorialReward::resources(&[("biomass", 200.0), ("minerals", 100.0)])),
            next_step: Some("tut_04_idle_mechanic".into()),
            faction_specific: true,
        },
        // 4. Wait for resources (idle mechanic)
        TutorialStep {
            id: "tut_04_idle_mechanic".into(),
            title: "Resources Grow While You Work".into(),
            description: "SwarmForge is idle-powered: resources accumulate even \
                when you are working on real projects in ImpForge. Use your AI \
                workstation and your colony thrives. Come back later for a \
                pleasant surprise!".into(),
            highlight_element: Some("#resource-bar".into()),
            required_action: TutorialAction::AnyAction,
            reward: Some(TutorialReward::xp(25)),
            next_step: Some("tut_05_second_building".into()),
            faction_specific: false,
        },
        // 5. Build second building
        TutorialStep {
            id: "tut_05_second_building".into(),
            title: "Expand Your Base".into(),
            description: "One extractor is not enough! Build a second structure — \
                perhaps a storage silo to increase your resource cap, or a second \
                extractor to double production.".into(),
            highlight_element: Some("#tab-buildings".into()),
            required_action: TutorialAction::BuildStructure { building_type: "any".into() },
            reward: Some(TutorialReward::resources(&[("biomass", 100.0)])),
            next_step: Some("tut_06_research_tab".into()),
            faction_specific: false,
        },
        // 6. Open research tab
        TutorialStep {
            id: "tut_06_research_tab".into(),
            title: "Unlock New Technologies".into(),
            description: "The Research tab contains your tech tree. Each tier \
                unlocks stronger units, buildings, and abilities. Start researching \
                to stay ahead of your rivals.".into(),
            highlight_element: Some("#tab-research".into()),
            required_action: TutorialAction::OpenTab { tab_name: "research".into() },
            reward: Some(TutorialReward::xp(50)),
            next_step: Some("tut_07_first_research".into()),
            faction_specific: false,
        },
        // 7. Start first research
        TutorialStep {
            id: "tut_07_first_research".into(),
            title: "Begin Your First Research".into(),
            description: "Select a Tier 1 technology and begin researching. \
                Research takes time but happens in the background while you \
                continue using ImpForge.".into(),
            highlight_element: Some("#research-tree".into()),
            required_action: TutorialAction::StartResearch { tech: "tier_1_any".into() },
            reward: Some(TutorialReward::dm(5)),
            next_step: Some("tut_08_shipyard".into()),
            faction_specific: true,
        },
        // 8. Open shipyard
        TutorialStep {
            id: "tut_08_shipyard".into(),
            title: "The Shipyard Awaits".into(),
            description: "Ships are your military and logistic backbone. Open \
                the Shipyard tab to see your faction's available vessels.".into(),
            highlight_element: Some("#tab-shipyard".into()),
            required_action: TutorialAction::OpenTab { tab_name: "shipyard".into() },
            reward: Some(TutorialReward::xp(50)),
            next_step: Some("tut_09_first_ship".into()),
            faction_specific: false,
        },
        // 9. Build first ship
        TutorialStep {
            id: "tut_09_first_ship".into(),
            title: "Commission Your First Ship".into(),
            description: "Build a scout or light fighter. Every fleet starts \
                small — even a single ship can explore nearby systems and \
                gather intelligence.".into(),
            highlight_element: Some("#ship-build-list".into()),
            required_action: TutorialAction::TrainUnit { unit_type: "scout".into() },
            reward: Some(TutorialReward::resources(&[("minerals", 200.0)])),
            next_step: Some("tut_10_galaxy".into()),
            faction_specific: true,
        },
        // 10. View galaxy browser
        TutorialStep {
            id: "tut_10_galaxy".into(),
            title: "Explore the Galaxy".into(),
            description: "Open the Galaxy Browser to see the star map. Your \
                colony is at a specific coordinate [G:SSS:PP]. Nearby systems \
                may contain abandoned colonies, asteroid fields, or enemies.".into(),
            highlight_element: Some("#tab-galaxy".into()),
            required_action: TutorialAction::ViewGalaxy,
            reward: Some(TutorialReward::xp(75)),
            next_step: Some("tut_11_send_fleet".into()),
            faction_specific: false,
        },
        // 11. Send first fleet
        TutorialStep {
            id: "tut_11_send_fleet".into(),
            title: "Launch Your First Fleet".into(),
            description: "Select your ship and send it on an Expedition mission \
                to a nearby system. Expeditions can discover resources, \
                technology fragments, or neutral monsters.".into(),
            highlight_element: Some("#fleet-dispatch".into()),
            required_action: TutorialAction::SendFleet,
            reward: Some(TutorialReward::dm(10)),
            next_step: Some("tut_12_evolve".into()),
            faction_specific: false,
        },
        // 12. Evolve first unit
        TutorialStep {
            id: "tut_12_evolve".into(),
            title: "Evolve a Unit".into(),
            description: "Units gain mutations through combat and research. \
                Open the Mutation Lab and apply an evolution to one of your \
                units to make it stronger.".into(),
            highlight_element: Some("#tab-mutations".into()),
            required_action: TutorialAction::EvolveUnit,
            reward: Some(TutorialReward::xp(100)),
            next_step: Some("tut_13_mutations".into()),
            faction_specific: true,
        },
        // 13. Check mutations
        TutorialStep {
            id: "tut_13_mutations".into(),
            title: "The Genome Browser".into(),
            description: "Each unit carries a 64-gene genome. Mutations change \
                individual genes, affecting stats, abilities, and even appearance. \
                Check the Genome tab to see how your unit's DNA looks.".into(),
            highlight_element: Some("#genome-viewer".into()),
            required_action: TutorialAction::AnyAction,
            reward: Some(TutorialReward::xp(50)),
            next_step: Some("tut_14_dark_matter".into()),
            faction_specific: false,
        },
        // 14. Open Dark Matter shop
        TutorialStep {
            id: "tut_14_dark_matter".into(),
            title: "Dark Matter — Premium Currency".into(),
            description: "Dark Matter (DM) lets you speed up builds, buy cosmetics, \
                and unlock premium evolutions. You earn DM by using ImpForge \
                productively — coding, writing, designing — not by spending money.".into(),
            highlight_element: Some("#tab-dark-matter".into()),
            required_action: TutorialAction::OpenTab { tab_name: "dark_matter".into() },
            reward: Some(TutorialReward::dm(15)),
            next_step: Some("tut_15_earn_dm".into()),
            faction_specific: false,
        },
        // 15. Earn DM through ImpForge usage
        TutorialStep {
            id: "tut_15_earn_dm".into(),
            title: "Productivity Powers Your Colony".into(),
            description: "Every commit, document edit, AI chat, and completed \
                task in ImpForge earns Dark Matter. The more you build in \
                the real world, the stronger your colony becomes.".into(),
            highlight_element: Some("#dm-earnings-panel".into()),
            required_action: TutorialAction::AnyAction,
            reward: Some(TutorialReward::xp(50)),
            next_step: Some("tut_16_defense".into()),
            faction_specific: false,
        },
        // 16. Build defense structure
        TutorialStep {
            id: "tut_16_defense".into(),
            title: "Fortify Your Colony".into(),
            description: "Other players will scout and attack you. Build a \
                defense turret to protect your resources. Defenders always \
                have the terrain advantage.".into(),
            highlight_element: Some("#defense-build".into()),
            required_action: TutorialAction::BuildStructure { building_type: "defense_turret".into() },
            reward: Some(TutorialReward::resources(&[("minerals", 300.0), ("biomass", 150.0)])),
            next_step: Some("tut_17_scout_enemy".into()),
            faction_specific: true,
        },
        // 17. Scout an enemy
        TutorialStep {
            id: "tut_17_scout_enemy".into(),
            title: "Know Your Enemy".into(),
            description: "Send an Espionage probe to a nearby occupied planet. \
                Intel reports reveal enemy fleet strength, resource levels, \
                and defensive capabilities.".into(),
            highlight_element: Some("#espionage-panel".into()),
            required_action: TutorialAction::SendFleet,
            reward: Some(TutorialReward::dm(5)),
            next_step: Some("tut_18_alliance".into()),
            faction_specific: false,
        },
        // 18. Join or create alliance
        TutorialStep {
            id: "tut_18_alliance".into(),
            title: "Strength in Numbers".into(),
            description: "Alliances let you coordinate attacks, share \
                intelligence, and trade resources with allies. Join an \
                existing alliance or create your own.".into(),
            highlight_element: Some("#tab-alliance".into()),
            required_action: TutorialAction::OpenTab { tab_name: "alliance".into() },
            reward: Some(TutorialReward::xp(100)),
            next_step: Some("tut_19_offline".into()),
            faction_specific: false,
        },
        // 19. Set up offline progression
        TutorialStep {
            id: "tut_19_offline".into(),
            title: "Offline Progression".into(),
            description: "When you close ImpForge, your colony keeps producing. \
                Offline earnings are calculated on next login based on elapsed \
                time. Set your build queue and research queue before logging \
                off for maximum efficiency.".into(),
            highlight_element: Some("#offline-settings".into()),
            required_action: TutorialAction::AnyAction,
            reward: Some(TutorialReward::dm(10)),
            next_step: Some("tut_20_complete".into()),
            faction_specific: false,
        },
        // 20. Tutorial complete!
        TutorialStep {
            id: "tut_20_complete".into(),
            title: "Tutorial Complete!".into(),
            description: "Congratulations, Commander! You have learned the \
                basics of SwarmForge. Your colony is ready to grow. Build, \
                research, fight, evolve — and most importantly, keep using \
                ImpForge to power your empire!".into(),
            highlight_element: None,
            required_action: TutorialAction::AnyAction,
            reward: Some(TutorialReward::full(100, 500, Some("ach_tutorial_complete"))),
            next_step: None,
            faction_specific: false,
        },
    ]
}

// ===========================================================================
// PART 3: Isometric Rendering Configuration
// ===========================================================================

/// Core isometric rendering constants consumed by the PixiJS frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoConfig {
    /// Tile width in pixels (diamond width).
    pub tile_width: u32,
    /// Tile height in pixels (half tile_width for standard iso).
    pub tile_height: u32,
    /// Sprite sheet columns (one per direction).
    pub sprite_sheet_cols: u32,
    /// Sprite sheet rows (one per action).
    pub sprite_sheet_rows: u32,
    /// Animation frames per action.
    pub frames_per_action: u32,
    /// Playback frames per second.
    pub animation_fps: u32,
    /// Minimum camera zoom level.
    pub camera_zoom_min: f64,
    /// Maximum camera zoom level.
    pub camera_zoom_max: f64,
    /// Default camera zoom on load.
    pub camera_zoom_default: f64,
    /// Minimap widget size in pixels.
    pub minimap_size: u32,
}

impl Default for IsoConfig {
    fn default() -> Self {
        Self {
            tile_width: 64,
            tile_height: 32,
            sprite_sheet_cols: 8,
            sprite_sheet_rows: 8,
            frames_per_action: 8,
            animation_fps: 12,
            camera_zoom_min: 0.5,
            camera_zoom_max: 3.0,
            camera_zoom_default: 1.0,
            minimap_size: 200,
        }
    }
}

/// Rendering metadata for a single map tile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileRenderInfo {
    pub biome: String,
    /// Texture atlas ID.
    pub texture_id: String,
    /// 8-bit bitmask for terrain-edge transitions (N/NE/E/SE/S/SW/W/NW).
    pub neighbor_mask: u8,
    /// Vertical pixel offset (elevation).
    pub elevation_offset: f64,
    /// Whether this tile is animated (lava, corruption, water).
    pub animated: bool,
    /// Optional particle effect name (fire_embers, spore_cloud, etc.).
    pub particle_effect: Option<String>,
}

/// Sprite sheet metadata for a unit type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitSpriteInfo {
    pub unit_type: String,
    pub faction: String,
    /// Relative path to the sprite sheet image.
    pub sprite_sheet: String,
    pub frame_count: u32,
    /// Number of facing directions.
    pub directions: u32,
    /// Visual scale multiplier (1.0 normal, 2.0 for large units).
    pub scale: f64,
    /// Whether to render a drop shadow beneath the unit.
    pub shadow: bool,
    /// Hero units emit a coloured glow (CSS colour string).
    pub glow_color: Option<String>,
}

// ---------------------------------------------------------------------------
// Biome definitions (14 types)
// ---------------------------------------------------------------------------

/// All 14 biome types in SwarmForge.
const BIOME_TYPES: &[(&str, &str, bool, Option<&str>)] = &[
    // (id, texture_id, animated, particle_effect)
    ("grassland",           "tiles/grassland_64.png",           false, None),
    ("desert",              "tiles/desert_64.png",              false, None),
    ("tundra",              "tiles/tundra_64.png",              false, None),
    ("swamp",               "tiles/swamp_64.png",               false, None),
    ("volcanic",            "tiles/volcanic_64.png",            true,  Some("fire_embers")),
    ("crystalline",         "tiles/crystalline_64.png",         true,  Some("crystal_shimmer")),
    ("chitinous_resin",     "tiles/chitinous_resin_64.png",     true,  Some("spore_cloud")),
    ("hellfire_corruption", "tiles/hellfire_corruption_64.png", true,  Some("fire_embers")),
    ("necrosis",            "tiles/necrosis_64.png",            true,  Some("death_mist")),
    ("human_settlement",    "tiles/human_settlement_64.png",    false, None),
    ("ocean",               "tiles/ocean_64.png",               true,  None),
    ("asteroid_field",      "tiles/asteroid_field_64.png",      false, None),
    ("void",                "tiles/void_64.png",                true,  Some("void_tendrils")),
    ("nexus",               "tiles/nexus_64.png",               true,  Some("nexus_pulse")),
];

/// Build render info for a tile at the given axial coordinates.
fn build_tile_render_info(q: i32, r: i32) -> TileRenderInfo {
    // Deterministic biome selection from coords (seeded hash).
    let hash = ((q.wrapping_mul(73856093)) ^ (r.wrapping_mul(19349663))).unsigned_abs() as usize;
    let idx = hash % BIOME_TYPES.len();
    let (biome, texture_id, animated, particle) = BIOME_TYPES[idx];

    // Elevation derived from coordinate hash — range -8.0..+8.0
    let elev = ((hash % 17) as f64 - 8.0).clamp(-8.0, 8.0);

    TileRenderInfo {
        biome: biome.into(),
        texture_id: texture_id.into(),
        neighbor_mask: (hash % 256) as u8,
        elevation_offset: elev,
        animated,
        particle_effect: particle.map(Into::into),
    }
}

// ---------------------------------------------------------------------------
// Coordinate conversion
// ---------------------------------------------------------------------------

/// Convert axial (q,r) isometric coordinates to screen pixel position.
pub(crate) fn iso_to_screen(q: i32, r: i32, config: &IsoConfig) -> (f64, f64) {
    let tw = config.tile_width as f64;
    let th = config.tile_height as f64;
    let x = (q as f64 - r as f64) * tw / 2.0;
    let y = (q as f64 + r as f64) * th / 2.0;
    (x, y)
}

/// Convert screen pixel position back to the nearest axial (q,r) tile.
pub(crate) fn screen_to_iso(x: f64, y: f64, config: &IsoConfig) -> (i32, i32) {
    let tw = config.tile_width as f64;
    let th = config.tile_height as f64;
    let q = ((x / tw) + (y / th)).round() as i32;
    let r = ((y / th) - (x / tw)).round() as i32;
    (q, r)
}

/// Look up sprite info for a unit type + faction.
fn build_unit_sprite_info(unit_type: &str, faction: &str) -> UnitSpriteInfo {
    let is_hero = unit_type.contains("hero")
        || unit_type.contains("champion")
        || unit_type.contains("legend");

    let scale = if unit_type.contains("titan")
        || unit_type.contains("leviathan")
        || unit_type.contains("colossus")
    {
        2.0
    } else if unit_type.contains("lord") || unit_type.contains("queen") {
        1.5
    } else {
        1.0
    };

    let glow = if is_hero {
        Some(match faction {
            "insect" => "#44ff44",
            "demon"  => "#ff4444",
            "undead" => "#8844ff",
            "human"  => "#ffdd44",
            _        => "#ffffff",
        }.into())
    } else {
        None
    };

    UnitSpriteInfo {
        unit_type: unit_type.into(),
        faction: faction.into(),
        sprite_sheet: format!("sprites/{faction}/{unit_type}_64x64.png"),
        frame_count: 64, // 8 directions x 8 frames
        directions: 8,
        scale,
        shadow: true,
        glow_color: glow,
    }
}

// ===========================================================================
// Tauri Commands (12 total)
// ===========================================================================

// ---- Sound (4) ------------------------------------------------------------

/// Get the complete sound asset registry.
#[tauri::command]
pub async fn sound_registry() -> AppResult<Vec<SoundAsset>> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_presentation", "game_presentation", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_presentation", "game_presentation");
    crate::synapse_fabric::synapse_session_push("swarm_presentation", "game_presentation", "sound_registry called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_presentation", "info", "swarm_presentation active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_presentation", "render", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"action": "sound_registry"}));
    Ok(get_sound_registry())
}

/// Get music tracks for a given state and faction.
#[tauri::command]
pub async fn sound_for_state(state: String, faction: String) -> AppResult<Vec<SoundAsset>> {
    let ms = MusicState::from_str_name(&state).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_MUSIC_STATE",
            format!(
                "Unknown music state: '{state}'. Valid: calm, building, tense, \
                 combat, victory, defeat, epic."
            ),
        )
    })?;
    Ok(get_music_for_state(&ms, &faction))
}

/// Get the current audio settings (defaults).
#[tauri::command]
pub async fn sound_settings_get() -> AppResult<AudioSettings> {
    // In a full implementation this would read from SQLite / settings file.
    // For now, return defaults so the frontend has a baseline.
    Ok(AudioSettings::default())
}

/// Persist audio settings.
#[tauri::command]
pub async fn sound_settings_set(settings: AudioSettings) -> AppResult<()> {
    // Validate ranges
    if settings.master_volume < 0.0 || settings.master_volume > 1.0 {
        return Err(ImpForgeError::validation(
            "VOLUME_OUT_OF_RANGE",
            "master_volume must be between 0.0 and 1.0",
        ));
    }
    if settings.music_volume < 0.0 || settings.music_volume > 1.0 {
        return Err(ImpForgeError::validation(
            "VOLUME_OUT_OF_RANGE",
            "music_volume must be between 0.0 and 1.0",
        ));
    }
    if settings.sfx_volume < 0.0 || settings.sfx_volume > 1.0 {
        return Err(ImpForgeError::validation(
            "VOLUME_OUT_OF_RANGE",
            "sfx_volume must be between 0.0 and 1.0",
        ));
    }
    if settings.ambient_volume < 0.0 || settings.ambient_volume > 1.0 {
        return Err(ImpForgeError::validation(
            "VOLUME_OUT_OF_RANGE",
            "ambient_volume must be between 0.0 and 1.0",
        ));
    }
    if settings.voice_volume < 0.0 || settings.voice_volume > 1.0 {
        return Err(ImpForgeError::validation(
            "VOLUME_OUT_OF_RANGE",
            "voice_volume must be between 0.0 and 1.0",
        ));
    }
    // Future: persist to SQLite settings table.
    log::info!("Audio settings updated: master={}", settings.master_volume);
    Ok(())
}

// ---- Tutorial (4) ---------------------------------------------------------

/// Get the full tutorial step sequence.
#[tauri::command]
pub async fn tutorial_steps() -> AppResult<Vec<TutorialStep>> {
    Ok(get_tutorial_steps())
}

/// Get current tutorial progress.
#[tauri::command]
pub async fn tutorial_progress() -> AppResult<TutorialProgress> {
    // Future: load from SQLite.  For now return a fresh start.
    Ok(TutorialProgress::default())
}

/// Mark a tutorial step as completed and return the reward (if any).
#[tauri::command]
pub async fn tutorial_complete_step(step_id: String) -> AppResult<Option<TutorialReward>> {
    let steps = get_tutorial_steps();
    let step = steps.iter().find(|s| s.id == step_id).ok_or_else(|| {
        ImpForgeError::validation(
            "TUTORIAL_STEP_NOT_FOUND",
            format!("Unknown tutorial step: '{step_id}'."),
        )
    })?;
    // Future: persist completion to SQLite, advance current_step.
    log::info!("Tutorial step completed: {step_id}");
    Ok(step.reward.clone())
}

/// Skip the entire tutorial.
#[tauri::command]
pub async fn tutorial_skip() -> AppResult<()> {
    // Future: set skipped=true in SQLite, grant a small consolation reward.
    log::info!("Tutorial skipped by player");
    Ok(())
}

// ---- Isometric Config (4) -------------------------------------------------

/// Get the isometric rendering configuration.
#[tauri::command]
pub async fn iso_config() -> AppResult<IsoConfig> {
    Ok(IsoConfig::default())
}

/// Get tile render info for a specific axial coordinate.
#[tauri::command]
pub async fn iso_tile_render(q: i32, r: i32) -> AppResult<TileRenderInfo> {
    Ok(build_tile_render_info(q, r))
}

/// Get sprite metadata for a unit type + faction.
#[tauri::command]
pub async fn iso_unit_sprite(unit_type: String, faction: String) -> AppResult<UnitSpriteInfo> {
    if faction.is_empty() {
        return Err(ImpForgeError::validation(
            "MISSING_FACTION",
            "faction parameter is required",
        ));
    }
    Ok(build_unit_sprite_info(&unit_type, &faction))
}

/// Convert axial (q,r) to screen coordinates and return both representations.
#[tauri::command]
pub async fn iso_convert_coords(q: i32, r: i32) -> AppResult<serde_json::Value> {
    let config = IsoConfig::default();
    let (sx, sy) = iso_to_screen(q, r, &config);
    let (rq, rr) = screen_to_iso(sx, sy, &config);
    Ok(serde_json::json!({
        "axial": { "q": q, "r": r },
        "screen": { "x": sx, "y": sy },
        "roundtrip": { "q": rq, "r": rr },
    }))
}

// ===========================================================================
// Additional Tauri Commands — wiring internal helpers
// ===========================================================================

/// List all music states.
#[tauri::command]
pub async fn presentation_music_states() -> AppResult<Vec<serde_json::Value>> {
    Ok(MusicState::all()
        .iter()
        .map(|s| serde_json::json!({ "state": s.as_str() }))
        .collect())
}

/// Get crossfade metadata for a music state transition.
#[tauri::command]
pub async fn presentation_music_transition(
    from: String,
    to: String,
) -> AppResult<serde_json::Value> {
    let from_state = MusicState::from_str_name(&from).unwrap_or(MusicState::Calm);
    let to_state = MusicState::from_str_name(&to).unwrap_or(MusicState::Calm);
    Ok(transition_music(&from_state, &to_state))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_sound_registry_count() {
        let sounds = get_sound_registry();
        // 8 ambient + 7 damage + 8 combat + 10 UI + 28 music + 12 voice + 4 env = 77
        assert!(sounds.len() >= 70, "Expected >= 70 sounds, got {}", sounds.len());
    }

    #[test]
    fn test_sound_ids_unique() {
        let sounds = get_sound_registry();
        let mut seen = std::collections::HashSet::new();
        for s in &sounds {
            assert!(seen.insert(&s.id), "Duplicate sound id: {}", s.id);
        }
    }

    #[test]
    fn test_sound_volumes_in_range() {
        for s in get_sound_registry() {
            assert!(
                (0.0..=1.0).contains(&s.volume),
                "Sound {} has out-of-range volume: {}",
                s.id,
                s.volume
            );
        }
    }

    #[test]
    fn test_music_state_roundtrip() {
        for state in MusicState::all() {
            let s = state.as_str();
            let parsed = MusicState::from_str_name(s);
            assert_eq!(parsed.as_ref(), Some(state), "Roundtrip failed for {s}");
        }
    }

    #[test]
    fn test_music_for_state_returns_tracks() {
        let tracks = get_music_for_state(&MusicState::Combat, "demon");
        assert!(
            !tracks.is_empty(),
            "Expected at least one combat track for demon"
        );
        // Should include the universal track plus the demon-specific one.
        assert!(tracks.len() >= 2, "Expected >= 2 tracks, got {}", tracks.len());
    }

    #[test]
    fn test_music_transition_json() {
        let t = transition_music(&MusicState::Calm, &MusicState::Tense);
        assert_eq!(t["crossfade_ms"], 2000);
        assert_eq!(t["curve"], "ease_in");
        assert_eq!(t["from"], "calm");
        assert_eq!(t["to"], "tense");
    }

    #[test]
    fn test_audio_settings_default() {
        let s = AudioSettings::default();
        assert!((s.master_volume - 0.8).abs() < f64::EPSILON);
        assert!(!s.muted);
    }

    #[test]
    fn test_tutorial_steps_count() {
        let steps = get_tutorial_steps();
        assert_eq!(steps.len(), 20, "Expected 20 tutorial steps");
    }

    #[test]
    fn test_tutorial_step_ids_unique() {
        let steps = get_tutorial_steps();
        let mut seen = std::collections::HashSet::new();
        for s in &steps {
            assert!(seen.insert(&s.id), "Duplicate tutorial step id: {}", s.id);
        }
    }

    #[test]
    fn test_tutorial_chain_linked() {
        let steps = get_tutorial_steps();
        // Every step except the last should have a next_step that exists.
        let ids: std::collections::HashSet<&str> =
            steps.iter().map(|s| s.id.as_str()).collect();
        for (i, step) in steps.iter().enumerate() {
            if i < steps.len() - 1 {
                let next = step.next_step.as_deref()
                    .unwrap_or_else(|| panic!("Step {} missing next_step", step.id));
                assert!(
                    ids.contains(next),
                    "Step {} points to non-existent next_step: {}",
                    step.id,
                    next
                );
            } else {
                // Last step should have no next.
                assert!(
                    step.next_step.is_none(),
                    "Last step should have next_step=None"
                );
            }
        }
    }

    #[test]
    fn test_tutorial_final_reward() {
        let steps = get_tutorial_steps();
        let last = steps.last().expect("No tutorial steps");
        let reward = last.reward.as_ref().expect("Last step should have a reward");
        assert_eq!(reward.dark_matter, 100);
        assert_eq!(reward.xp, 500);
        assert_eq!(
            reward.achievement_id.as_deref(),
            Some("ach_tutorial_complete")
        );
    }

    #[test]
    fn test_tutorial_progress_default() {
        let p = TutorialProgress::default();
        assert_eq!(p.current_step, "tut_01_welcome");
        assert!(!p.skipped);
        assert!(p.completed_steps.is_empty());
    }

    #[test]
    fn test_iso_config_defaults() {
        let c = IsoConfig::default();
        assert_eq!(c.tile_width, 64);
        assert_eq!(c.tile_height, 32);
        assert_eq!(c.sprite_sheet_cols, 8);
        assert_eq!(c.animation_fps, 12);
        assert!((c.camera_zoom_default - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_iso_to_screen_origin() {
        let config = IsoConfig::default();
        let (x, y) = iso_to_screen(0, 0, &config);
        assert!((x).abs() < f64::EPSILON);
        assert!((y).abs() < f64::EPSILON);
    }

    #[test]
    fn test_iso_coordinate_roundtrip() {
        let config = IsoConfig::default();
        for q in -5..=5 {
            for r in -5..=5 {
                let (sx, sy) = iso_to_screen(q, r, &config);
                let (rq, rr) = screen_to_iso(sx, sy, &config);
                assert_eq!(
                    (q, r),
                    (rq, rr),
                    "Roundtrip failed for ({q},{r}) -> ({sx},{sy}) -> ({rq},{rr})"
                );
            }
        }
    }

    #[test]
    fn test_tile_render_info_deterministic() {
        let a = build_tile_render_info(3, 7);
        let b = build_tile_render_info(3, 7);
        assert_eq!(a.biome, b.biome);
        assert_eq!(a.texture_id, b.texture_id);
        assert_eq!(a.neighbor_mask, b.neighbor_mask);
    }

    #[test]
    fn test_tile_render_info_biome_valid() {
        let valid: std::collections::HashSet<&str> =
            BIOME_TYPES.iter().map(|(id, _, _, _)| *id).collect();
        for q in -10..=10 {
            for r in -10..=10 {
                let info = build_tile_render_info(q, r);
                assert!(
                    valid.contains(info.biome.as_str()),
                    "Invalid biome '{}' at ({q},{r})",
                    info.biome
                );
            }
        }
    }

    #[test]
    fn test_biome_count() {
        assert_eq!(BIOME_TYPES.len(), 14, "Expected exactly 14 biome types");
    }

    #[test]
    fn test_unit_sprite_hero_glow() {
        let sprite = build_unit_sprite_info("demon_hero", "demon");
        assert_eq!(sprite.glow_color.as_deref(), Some("#ff4444"));
        assert!(sprite.shadow);
    }

    #[test]
    fn test_unit_sprite_normal_no_glow() {
        let sprite = build_unit_sprite_info("swarmling", "insect");
        assert!(sprite.glow_color.is_none());
        assert!((sprite.scale - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_unit_sprite_titan_scale() {
        let sprite = build_unit_sprite_info("void_titan", "undead");
        assert!((sprite.scale - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_unit_sprite_queen_scale() {
        let sprite = build_unit_sprite_info("brood_queen", "insect");
        assert!((sprite.scale - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_faction_ambient_sounds() {
        let sounds = get_sound_registry();
        let factions = ["insect", "demon", "undead", "human"];
        for fac in &factions {
            let ambient: Vec<_> = sounds
                .iter()
                .filter(|s| {
                    s.category == SoundCategory::Ambient
                        && s.faction.as_deref() == Some(fac)
                })
                .collect();
            assert!(
                !ambient.is_empty(),
                "Missing ambient sound for faction: {fac}"
            );
        }
    }

    #[test]
    fn test_voice_lines_per_faction() {
        let sounds = get_sound_registry();
        let factions = ["insect", "demon", "undead", "human"];
        for fac in &factions {
            let voices: Vec<_> = sounds
                .iter()
                .filter(|s| {
                    s.category == SoundCategory::Voice
                        && s.faction.as_deref() == Some(fac)
                })
                .collect();
            assert_eq!(
                voices.len(),
                3,
                "Expected 3 voice lines for {fac}, got {}",
                voices.len()
            );
        }
    }
}
