// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge NPC AI Engine -- Strategy AI with Decision Trees & Difficulty Scaling
//!
//! Provides autonomous NPC factions for the SwarmForge game layer.  Each faction
//! has a personality that governs decision-weight tables and a difficulty level
//! that scales resource income, combat effectiveness, and decision quality.
//!
//! ## Personality-driven decisions
//!
//! | Personality   | Primary Strategy                    |
//! |---------------|-------------------------------------|
//! | Aggressive    | Attack when military > 1.5x enemy   |
//! | Defensive     | Turtle until 2x enemy, then push    |
//! | Economic      | Max economy, attack only when 3x    |
//! | Balanced      | Context-adaptive weighting           |
//! | Expansionist  | Grab territory early, fortify later  |
//! | Turtle        | All-in defense, never attack first   |
//! | Rush          | Cheap units fast, attack before T30  |
//! | Random        | Roll dice on every decision          |
//!
//! ## Difficulty scaling
//!
//! | Level     | Income | Combat | Decision Quality |
//! |-----------|--------|--------|------------------|
//! | Easy      | 0.6x   | 0.7x   | 50% optimal      |
//! | Normal    | 1.0x   | 1.0x   | 75% optimal      |
//! | Hard      | 1.3x   | 1.2x   | 90% optimal      |
//! | Nightmare | 1.6x   | 1.4x   | 100% optimal     |
//!
//! Pre-registers 4 NPC factions on construction.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{AppResult, ImpForgeError};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarmforge_npc", "Game");

// ---------------------------------------------------------------------------
//  Core enums
// ---------------------------------------------------------------------------

/// NPC personality archetype -- governs decision-weight tables.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NpcPersonality {
    Aggressive,
    Defensive,
    Economic,
    Balanced,
    Expansionist,
    Turtle,
    Rush,
    Random,
}

/// Difficulty level -- scales income, combat, and decision quality.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DifficultyLevel {
    Easy,
    Normal,
    Hard,
    Nightmare,
}

impl DifficultyLevel {
    /// Resource income multiplier.
    pub(crate) fn income_mult(&self) -> f64 {
        match self {
            Self::Easy => 0.6,
            Self::Normal => 1.0,
            Self::Hard => 1.3,
            Self::Nightmare => 1.6,
        }
    }

    /// Combat effectiveness multiplier.
    pub(crate) fn combat_mult(&self) -> f64 {
        match self {
            Self::Easy => 0.7,
            Self::Normal => 1.0,
            Self::Hard => 1.2,
            Self::Nightmare => 1.4,
        }
    }

    /// Probability (0..1) that the NPC picks the optimal decision.
    pub(crate) fn decision_quality(&self) -> f64 {
        match self {
            Self::Easy => 0.50,
            Self::Normal => 0.75,
            Self::Hard => 0.90,
            Self::Nightmare => 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
//  Data structs
// ---------------------------------------------------------------------------

/// Resources available to an NPC faction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NpcResources {
    pub minerals: f64,
    pub gas: f64,
    pub energy: f64,
    pub dark_matter: f64,
    pub population: u32,
}
impl NpcResources {
    fn starter() -> Self {
        Self {
            minerals: 500.0,
            gas: 200.0,
            energy: 100.0,
            dark_matter: 0.0,
            population: 10,
        }
    }

    /// Scalar economic strength (weighted sum).
    fn economic_score(&self) -> f64 {
        self.minerals + self.gas * 1.5 + self.energy * 2.0 + self.dark_matter * 5.0
    }
}

/// A type of military unit and how many the faction owns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NpcUnit {
    pub unit_type: String,
    pub count: u32,
    pub power: f64,
}

/// A building owned by the faction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NpcBuilding {
    pub building_type: String,
    pub level: u32,
    pub producing: Option<String>,
}

/// A single decision the NPC AI engine can emit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NpcDecision {
    BuildUnit(String),
    BuildBuilding(String),
    Research(String),
    Attack(String),
    Defend,
    Expand,
    GatherResources,
    Trade,
    FormAlliance(String),
    DeclareWar(String),
    Retreat,
    Wait,
}

/// Situational awareness passed into `decide()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DecisionContext {
    pub own_faction_id: String,
    pub enemy_faction_ids: Vec<String>,
    pub tick_count: u64,
    pub threat_level: f64,
    pub resource_ratio: f64,
    pub military_strength: f64,
}

/// Full faction state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NpcFaction {
    pub id: String,
    pub name: String,
    pub personality: NpcPersonality,
    pub difficulty: DifficultyLevel,
    pub resources: NpcResources,
    pub units: Vec<NpcUnit>,
    pub buildings: Vec<NpcBuilding>,
    pub tech_level: u32,
    pub aggression: f64,
    pub expansion_drive: f64,
}

/// Compact stats summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NpcStats {
    pub faction_count: usize,
    pub total_units: u32,
    pub total_buildings: usize,
    pub avg_tech_level: f64,
    pub strongest_faction: Option<String>,
}

// ---------------------------------------------------------------------------
//  NpcAiEngine
// ---------------------------------------------------------------------------

/// The main NPC AI engine -- holds all factions and drives their behaviour.
pub(crate) struct NpcAiEngine {
    factions: Mutex<HashMap<String, NpcFaction>>,
}

impl NpcAiEngine {
    /// Create a new engine with 4 pre-registered NPC factions.
    pub(crate) fn new() -> Self {
        let mut map = HashMap::new();
        for faction in Self::default_factions() {
            map.insert(faction.id.clone(), faction);
        }
        Self {
            factions: Mutex::new(map),
        }
    }

    /// The 4 pre-registered factions.
    fn default_factions() -> Vec<NpcFaction> {
        vec![
            NpcFaction {
                id: "iron_legion".into(),
                name: "Iron Legion".into(),
                personality: NpcPersonality::Aggressive,
                difficulty: DifficultyLevel::Normal,
                resources: NpcResources::starter(),
                units: vec![NpcUnit { unit_type: "infantry".into(), count: 20, power: 1.0 }],
                buildings: vec![NpcBuilding { building_type: "barracks".into(), level: 1, producing: None }],
                tech_level: 1,
                aggression: 0.8,
                expansion_drive: 0.4,
            },
            NpcFaction {
                id: "crystal_hive".into(),
                name: "Crystal Hive".into(),
                personality: NpcPersonality::Defensive,
                difficulty: DifficultyLevel::Normal,
                resources: NpcResources::starter(),
                units: vec![NpcUnit { unit_type: "guardian".into(), count: 15, power: 1.2 }],
                buildings: vec![NpcBuilding { building_type: "shield_gen".into(), level: 1, producing: None }],
                tech_level: 1,
                aggression: 0.2,
                expansion_drive: 0.3,
            },
            NpcFaction {
                id: "void_traders".into(),
                name: "Void Traders".into(),
                personality: NpcPersonality::Economic,
                difficulty: DifficultyLevel::Normal,
                resources: NpcResources {
                    minerals: 800.0,
                    gas: 400.0,
                    energy: 200.0,
                    dark_matter: 10.0,
                    population: 12,
                },
                units: vec![NpcUnit { unit_type: "freighter".into(), count: 5, power: 0.3 }],
                buildings: vec![NpcBuilding { building_type: "trade_hub".into(), level: 2, producing: None }],
                tech_level: 2,
                aggression: 0.1,
                expansion_drive: 0.5,
            },
            NpcFaction {
                id: "swarm_mind".into(),
                name: "Swarm Mind".into(),
                personality: NpcPersonality::Balanced,
                difficulty: DifficultyLevel::Normal,
                resources: NpcResources::starter(),
                units: vec![
                    NpcUnit { unit_type: "drone".into(), count: 30, power: 0.5 },
                    NpcUnit { unit_type: "overlord".into(), count: 2, power: 3.0 },
                ],
                buildings: vec![NpcBuilding { building_type: "hatchery".into(), level: 1, producing: None }],
                tech_level: 1,
                aggression: 0.5,
                expansion_drive: 0.5,
            },
        ]
    }

    // -- Evaluation helpers ------------------------------------------------

    /// Evaluate threat from a list of enemy faction ids (0..1).
    pub(crate) fn evaluate_threat(&self, faction: &NpcFaction, enemy_ids: &[String]) -> f64 {
        let guard = match self.factions.lock() {
            Ok(g) => g,
            Err(_) => return 0.5,
        };
        let own_mil = Self::military_power(faction);
        let mut max_enemy = 0.0_f64;
        for eid in enemy_ids {
            if let Some(ef) = guard.get(eid) {
                let ep = Self::military_power(ef) * ef.difficulty.combat_mult();
                max_enemy = max_enemy.max(ep);
            }
        }
        if own_mil + max_enemy < f64::EPSILON {
            return 0.0;
        }
        (max_enemy / (own_mil + max_enemy)).clamp(0.0, 1.0)
    }

    /// Total military power of a faction.
    fn military_power(faction: &NpcFaction) -> f64 {
        faction
            .units
            .iter()
            .map(|u| u.count as f64 * u.power)
            .sum::<f64>()
            * faction.difficulty.combat_mult()
    }

    /// Evaluate economy on a 0..1 scale (capped at 1.0 = "very strong").
    pub(crate) fn evaluate_economy(faction: &NpcFaction) -> f64 {
        let score = faction.resources.economic_score() * faction.difficulty.income_mult();
        (score / 5000.0).clamp(0.0, 1.0)
    }

    /// Evaluate military on a 0..1 scale.
    pub(crate) fn evaluate_military(faction: &NpcFaction) -> f64 {
        let mp = Self::military_power(faction);
        (mp / 100.0).clamp(0.0, 1.0)
    }

    /// Choose the best build target given personality.
    pub(crate) fn choose_build_target(
        faction: &NpcFaction,
        personality: NpcPersonality,
    ) -> String {
        match personality {
            NpcPersonality::Aggressive | NpcPersonality::Rush => "barracks".into(),
            NpcPersonality::Defensive | NpcPersonality::Turtle => "shield_gen".into(),
            NpcPersonality::Economic => "trade_hub".into(),
            NpcPersonality::Expansionist => "outpost".into(),
            NpcPersonality::Balanced => {
                if faction.resources.minerals > 600.0 {
                    "barracks".into()
                } else {
                    "refinery".into()
                }
            }
            NpcPersonality::Random => "barracks".into(),
        }
    }

    /// Choose the weakest enemy faction to attack (or None).
    pub(crate) fn choose_attack_target(
        &self,
        _faction: &NpcFaction,
        enemy_ids: &[String],
    ) -> Option<String> {
        let guard = match self.factions.lock() {
            Ok(g) => g,
            Err(_) => return None,
        };
        let mut weakest: Option<(String, f64)> = None;
        for eid in enemy_ids {
            if let Some(ef) = guard.get(eid) {
                let power = Self::military_power(ef);
                match &weakest {
                    None => weakest = Some((eid.clone(), power)),
                    Some((_, wp)) if power < *wp => weakest = Some((eid.clone(), power)),
                    _ => {}
                }
            }
        }
        weakest.map(|(id, _)| id)
    }

    // -- Core decision logic -----------------------------------------------

    /// Make a single decision for a faction given context.
    pub(crate) fn decide(
        &self,
        faction: &NpcFaction,
        ctx: &DecisionContext,
    ) -> NpcDecision {
        let mut rng = rand::thread_rng();

        // If difficulty says we should sub-optimally decide, sometimes just wait
        if rng.gen::<f64>() > faction.difficulty.decision_quality() {
            return NpcDecision::Wait;
        }

        match faction.personality {
            NpcPersonality::Aggressive => self.decide_aggressive(faction, ctx),
            NpcPersonality::Defensive => self.decide_defensive(faction, ctx),
            NpcPersonality::Economic => self.decide_economic(faction, ctx),
            NpcPersonality::Balanced => self.decide_balanced(faction, ctx),
            NpcPersonality::Expansionist => self.decide_expansionist(faction, ctx),
            NpcPersonality::Turtle => self.decide_turtle(faction, ctx),
            NpcPersonality::Rush => self.decide_rush(faction, ctx),
            NpcPersonality::Random => self.decide_random(),
        }
    }

    fn decide_aggressive(&self, faction: &NpcFaction, ctx: &DecisionContext) -> NpcDecision {
        let own_mil = Self::evaluate_military(faction);
        if own_mil > 0.6 && ctx.threat_level < 0.5 {
            if let Some(target) = self.choose_attack_target(faction, &ctx.enemy_faction_ids) {
                return NpcDecision::Attack(target);
            }
        }
        if own_mil < 0.3 {
            return NpcDecision::BuildUnit("infantry".into());
        }
        NpcDecision::BuildUnit(Self::choose_build_target(faction, faction.personality))
    }

    fn decide_defensive(&self, faction: &NpcFaction, ctx: &DecisionContext) -> NpcDecision {
        if ctx.threat_level > 0.6 {
            return NpcDecision::Defend;
        }
        let own_mil = Self::evaluate_military(faction);
        if own_mil > 0.8 && ctx.threat_level < 0.3 {
            if let Some(target) = self.choose_attack_target(faction, &ctx.enemy_faction_ids) {
                return NpcDecision::Attack(target);
            }
        }
        NpcDecision::BuildBuilding("shield_gen".into())
    }

    fn decide_economic(&self, faction: &NpcFaction, ctx: &DecisionContext) -> NpcDecision {
        if ctx.threat_level > 0.7 {
            return NpcDecision::Defend;
        }
        let econ = Self::evaluate_economy(faction);
        if econ < 0.6 {
            return NpcDecision::GatherResources;
        }
        let own_mil = Self::evaluate_military(faction);
        if own_mil > 0.9 {
            if let Some(target) = self.choose_attack_target(faction, &ctx.enemy_faction_ids) {
                return NpcDecision::Attack(target);
            }
        }
        NpcDecision::BuildBuilding("trade_hub".into())
    }

    fn decide_balanced(&self, faction: &NpcFaction, ctx: &DecisionContext) -> NpcDecision {
        if ctx.threat_level > 0.7 {
            return NpcDecision::Defend;
        }
        let econ = Self::evaluate_economy(faction);
        let mil = Self::evaluate_military(faction);
        if econ < 0.4 {
            return NpcDecision::GatherResources;
        }
        if mil < 0.4 {
            return NpcDecision::BuildUnit("drone".into());
        }
        if mil > 0.6 && ctx.threat_level < 0.4 {
            if let Some(target) = self.choose_attack_target(faction, &ctx.enemy_faction_ids) {
                return NpcDecision::Attack(target);
            }
        }
        NpcDecision::BuildBuilding(Self::choose_build_target(faction, faction.personality))
    }

    fn decide_expansionist(&self, faction: &NpcFaction, ctx: &DecisionContext) -> NpcDecision {
        if ctx.threat_level > 0.7 {
            return NpcDecision::Defend;
        }
        if faction.expansion_drive > 0.4 && faction.resources.minerals > 300.0 {
            return NpcDecision::Expand;
        }
        NpcDecision::BuildBuilding("outpost".into())
    }

    fn decide_turtle(&self, _faction: &NpcFaction, ctx: &DecisionContext) -> NpcDecision {
        if ctx.threat_level > 0.3 {
            return NpcDecision::Defend;
        }
        NpcDecision::BuildBuilding("shield_gen".into())
    }

    fn decide_rush(&self, faction: &NpcFaction, ctx: &DecisionContext) -> NpcDecision {
        if ctx.tick_count < 30 {
            return NpcDecision::BuildUnit("zergling".into());
        }
        if let Some(target) = self.choose_attack_target(faction, &ctx.enemy_faction_ids) {
            return NpcDecision::Attack(target);
        }
        NpcDecision::BuildUnit("zergling".into())
    }

    fn decide_random(&self) -> NpcDecision {
        let mut rng = rand::thread_rng();
        let roll: u32 = rng.gen_range(0..8);
        match roll {
            0 => NpcDecision::BuildUnit("random_unit".into()),
            1 => NpcDecision::BuildBuilding("random_building".into()),
            2 => NpcDecision::Research("random_tech".into()),
            3 => NpcDecision::Defend,
            4 => NpcDecision::Expand,
            5 => NpcDecision::GatherResources,
            6 => NpcDecision::Trade,
            _ => NpcDecision::Wait,
        }
    }

    // -- Mutation -----------------------------------------------------------

    /// Apply a decision's side effects to a faction.
    pub(crate) fn update_faction(
        &self,
        faction_id: &str,
        decision: &NpcDecision,
    ) -> AppResult<()> {
        let mut guard = self.factions.lock().map_err(|_| {
            ImpForgeError::internal("NPC_LOCK_POISON", "NPC engine lock poisoned")
        })?;
        let faction = guard.get_mut(faction_id).ok_or_else(|| {
            ImpForgeError::validation("NPC_FACTION_NOT_FOUND", format!("No faction: {faction_id}"))
        })?;

        match decision {
            NpcDecision::BuildUnit(utype) => {
                if faction.resources.minerals < 50.0 {
                    return Ok(()); // not enough resources, skip
                }
                faction.resources.minerals -= 50.0;
                if let Some(u) = faction.units.iter_mut().find(|u| u.unit_type == *utype) {
                    u.count += 1;
                } else {
                    faction.units.push(NpcUnit {
                        unit_type: utype.clone(),
                        count: 1,
                        power: 1.0,
                    });
                }
            }
            NpcDecision::BuildBuilding(btype) => {
                if faction.resources.minerals < 100.0 {
                    return Ok(());
                }
                faction.resources.minerals -= 100.0;
                if let Some(b) = faction.buildings.iter_mut().find(|b| b.building_type == *btype) {
                    b.level += 1;
                } else {
                    faction.buildings.push(NpcBuilding {
                        building_type: btype.clone(),
                        level: 1,
                        producing: None,
                    });
                }
            }
            NpcDecision::Research(_) => {
                if faction.resources.gas >= 100.0 {
                    faction.resources.gas -= 100.0;
                    faction.tech_level += 1;
                }
            }
            NpcDecision::GatherResources => {
                let mult = faction.difficulty.income_mult();
                faction.resources.minerals += 50.0 * mult;
                faction.resources.gas += 20.0 * mult;
                faction.resources.energy += 10.0 * mult;
            }
            NpcDecision::Expand => {
                if faction.resources.minerals >= 200.0 {
                    faction.resources.minerals -= 200.0;
                    faction.expansion_drive += 0.1;
                    faction.resources.population += 5;
                }
            }
            NpcDecision::Defend => {
                faction.aggression = (faction.aggression - 0.05).max(0.0);
            }
            NpcDecision::Attack(_) => {
                faction.aggression = (faction.aggression + 0.05).min(1.0);
            }
            NpcDecision::Retreat => {
                faction.aggression = (faction.aggression - 0.1).max(0.0);
            }
            NpcDecision::Trade => {
                faction.resources.minerals += 30.0;
                faction.resources.gas += 15.0;
            }
            NpcDecision::FormAlliance(_) | NpcDecision::DeclareWar(_) | NpcDecision::Wait => {}
        }
        Ok(())
    }

    /// Run one game tick for a faction -- returns all decisions made.
    pub(crate) fn tick(&self, faction_id: &str) -> AppResult<Vec<NpcDecision>> {
        // Snapshot the faction for read-only evaluation
        let faction = {
            let guard = self.factions.lock().map_err(|_| {
                ImpForgeError::internal("NPC_LOCK_POISON", "NPC engine lock poisoned")
            })?;
            guard.get(faction_id).cloned().ok_or_else(|| {
                ImpForgeError::validation(
                    "NPC_FACTION_NOT_FOUND",
                    format!("No faction: {faction_id}"),
                )
            })?
        };

        // Build context from current state
        let enemy_ids: Vec<String> = {
            let guard = self.factions.lock().map_err(|_| {
                ImpForgeError::internal("NPC_LOCK_POISON", "NPC engine lock poisoned")
            })?;
            guard.keys().filter(|k| *k != faction_id).cloned().collect()
        };

        let threat = self.evaluate_threat(&faction, &enemy_ids);
        let ctx = DecisionContext {
            own_faction_id: faction_id.to_string(),
            enemy_faction_ids: enemy_ids,
            tick_count: 0,
            threat_level: threat,
            resource_ratio: Self::evaluate_economy(&faction),
            military_strength: Self::evaluate_military(&faction),
        };

        // NPC makes 1-3 decisions per tick depending on difficulty
        let decision_count = match faction.difficulty {
            DifficultyLevel::Easy => 1,
            DifficultyLevel::Normal => 2,
            DifficultyLevel::Hard => 2,
            DifficultyLevel::Nightmare => 3,
        };

        let mut decisions = Vec::with_capacity(decision_count);
        for _ in 0..decision_count {
            let d = self.decide(&faction, &ctx);
            self.update_faction(faction_id, &d)?;
            decisions.push(d);
        }

        // Passive income every tick
        {
            let mut guard = self.factions.lock().map_err(|_| {
                ImpForgeError::internal("NPC_LOCK_POISON", "NPC engine lock poisoned")
            })?;
            if let Some(f) = guard.get_mut(faction_id) {
                let mult = f.difficulty.income_mult();
                f.resources.minerals += 10.0 * mult;
                f.resources.gas += 5.0 * mult;
                f.resources.energy += 3.0 * mult;
            }
        }

        Ok(decisions)
    }

    /// Compute aggregate stats across all factions.
    pub(crate) fn stats(&self) -> AppResult<NpcStats> {
        let guard = self.factions.lock().map_err(|_| {
            ImpForgeError::internal("NPC_LOCK_POISON", "NPC engine lock poisoned")
        })?;

        let faction_count = guard.len();
        let total_units: u32 = guard.values().flat_map(|f| &f.units).map(|u| u.count).sum();
        let total_buildings: usize = guard.values().map(|f| f.buildings.len()).sum();
        let avg_tech: f64 = if faction_count > 0 {
            guard.values().map(|f| f.tech_level as f64).sum::<f64>() / faction_count as f64
        } else {
            0.0
        };

        let strongest = guard
            .values()
            .max_by(|a, b| {
                Self::military_power(a)
                    .partial_cmp(&Self::military_power(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|f| f.name.clone());

        Ok(NpcStats {
            faction_count,
            total_units,
            total_buildings,
            avg_tech_level: avg_tech,
            strongest_faction: strongest,
        })
    }

    /// List all factions (snapshot).
    pub(crate) fn list_factions(&self) -> AppResult<Vec<NpcFaction>> {
        let guard = self.factions.lock().map_err(|_| {
            ImpForgeError::internal("NPC_LOCK_POISON", "NPC engine lock poisoned")
        })?;
        Ok(guard.values().cloned().collect())
    }

    /// Get a single faction's decision for external context.
    pub(crate) fn decide_for(
        &self,
        faction_id: &str,
        ctx: &DecisionContext,
    ) -> AppResult<NpcDecision> {
        let guard = self.factions.lock().map_err(|_| {
            ImpForgeError::internal("NPC_LOCK_POISON", "NPC engine lock poisoned")
        })?;
        let faction = guard.get(faction_id).ok_or_else(|| {
            ImpForgeError::validation(
                "NPC_FACTION_NOT_FOUND",
                format!("No faction: {faction_id}"),
            )
        })?;
        Ok(self.decide(faction, ctx))
    }
}

// ---------------------------------------------------------------------------
//  Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn npc_tick(
    faction_id: String,
    engine: tauri::State<'_, NpcAiEngine>,
) -> Result<Vec<NpcDecision>, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarmforge_npc", "swarmnpc", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarmforge_npc", "swarmnpc");
    crate::synapse_fabric::synapse_session_push("swarmforge_npc", "swarmnpc", "npc_tick called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarmforge_npc", "info", "swarmforge_npc active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarmforge_npc", "spawn", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"faction": faction_id}));
    engine.tick(&faction_id)
}

#[tauri::command]
pub async fn npc_decide(
    faction_id: String,
    tick_count: u64,
    engine: tauri::State<'_, NpcAiEngine>,
) -> Result<NpcDecision, ImpForgeError> {
    let ctx = DecisionContext {
        own_faction_id: faction_id.clone(),
        enemy_faction_ids: Vec::new(),
        tick_count,
        threat_level: 0.5,
        resource_ratio: 0.5,
        military_strength: 0.5,
    };
    engine.decide_for(&faction_id, &ctx)
}

#[tauri::command]
pub async fn npc_factions(
    engine: tauri::State<'_, NpcAiEngine>,
) -> Result<Vec<NpcFaction>, ImpForgeError> {
    engine.list_factions()
}

#[tauri::command]
pub async fn npc_stats(
    engine: tauri::State<'_, NpcAiEngine>,
) -> Result<NpcStats, ImpForgeError> {
    engine.stats()
}

// ---------------------------------------------------------------------------
//  Additional Tauri Commands — wiring internal helpers
// ---------------------------------------------------------------------------

/// Get starter resource amounts for NPC factions.
#[tauri::command]
pub async fn npc_starter_resources() -> Result<NpcResources, ImpForgeError> {
    Ok(NpcResources::starter())
}

/// Get default faction list (4 pre-registered NPC factions).
#[tauri::command]
pub async fn npc_default_factions() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    let engine = NpcAiEngine::new();
    let factions = engine.list_factions()?;
    Ok(factions
        .iter()
        .map(|f| serde_json::json!({
            "id": f.id,
            "name": f.name,
            "personality": format!("{:?}", f.personality),
        }))
        .collect())
}

// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;


    fn make_engine() -> NpcAiEngine {
        NpcAiEngine::new()
    }

    #[test]
    fn test_engine_has_four_factions() {
        let engine = make_engine();
        let factions = engine.list_factions().unwrap_or_default();
        assert_eq!(factions.len(), 4);
    }

    #[test]
    fn test_faction_ids() {
        let engine = make_engine();
        let factions = engine.list_factions().unwrap_or_default();
        let ids: Vec<String> = factions.iter().map(|f| f.id.clone()).collect();
        assert!(ids.contains(&"iron_legion".to_string()));
        assert!(ids.contains(&"crystal_hive".to_string()));
        assert!(ids.contains(&"void_traders".to_string()));
        assert!(ids.contains(&"swarm_mind".to_string()));
    }

    #[test]
    fn test_personalities_assigned() {
        let engine = make_engine();
        let guard = engine.factions.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(guard["iron_legion"].personality, NpcPersonality::Aggressive);
        assert_eq!(guard["crystal_hive"].personality, NpcPersonality::Defensive);
        assert_eq!(guard["void_traders"].personality, NpcPersonality::Economic);
        assert_eq!(guard["swarm_mind"].personality, NpcPersonality::Balanced);
    }

    #[test]
    fn test_difficulty_multipliers() {
        assert!((DifficultyLevel::Easy.income_mult() - 0.6).abs() < f64::EPSILON);
        assert!((DifficultyLevel::Normal.combat_mult() - 1.0).abs() < f64::EPSILON);
        assert!((DifficultyLevel::Hard.income_mult() - 1.3).abs() < f64::EPSILON);
        assert!((DifficultyLevel::Nightmare.decision_quality() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_evaluate_economy() {
        let faction = NpcFaction {
            id: "test".into(),
            name: "Test".into(),
            personality: NpcPersonality::Balanced,
            difficulty: DifficultyLevel::Normal,
            resources: NpcResources::starter(),
            units: vec![],
            buildings: vec![],
            tech_level: 1,
            aggression: 0.5,
            expansion_drive: 0.5,
        };
        let econ = NpcAiEngine::evaluate_economy(&faction);
        assert!(econ > 0.0);
        assert!(econ <= 1.0);
    }

    #[test]
    fn test_evaluate_military() {
        let faction = NpcFaction {
            id: "test".into(),
            name: "Test".into(),
            personality: NpcPersonality::Aggressive,
            difficulty: DifficultyLevel::Normal,
            resources: NpcResources::starter(),
            units: vec![NpcUnit { unit_type: "inf".into(), count: 10, power: 2.0 }],
            buildings: vec![],
            tech_level: 1,
            aggression: 0.8,
            expansion_drive: 0.3,
        };
        let mil = NpcAiEngine::evaluate_military(&faction);
        assert!(mil > 0.0);
        assert!(mil <= 1.0);
    }

    #[test]
    fn test_tick_returns_decisions() {
        let engine = make_engine();
        let decisions = engine.tick("iron_legion");
        assert!(decisions.is_ok());
        let ds = decisions.unwrap_or_default();
        assert!(!ds.is_empty());
    }

    #[test]
    fn test_tick_unknown_faction() {
        let engine = make_engine();
        let result = engine.tick("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_decide_aggressive_attacks_when_strong() {
        let engine = make_engine();
        let faction = NpcFaction {
            id: "attacker".into(),
            name: "Attacker".into(),
            personality: NpcPersonality::Aggressive,
            difficulty: DifficultyLevel::Nightmare,
            resources: NpcResources::starter(),
            units: vec![NpcUnit { unit_type: "tank".into(), count: 100, power: 5.0 }],
            buildings: vec![],
            tech_level: 5,
            aggression: 0.9,
            expansion_drive: 0.2,
        };
        let ctx = DecisionContext {
            own_faction_id: "attacker".into(),
            enemy_faction_ids: vec!["iron_legion".into()],
            tick_count: 50,
            threat_level: 0.1,
            resource_ratio: 0.8,
            military_strength: 0.95,
        };
        let d = engine.decide(&faction, &ctx);
        // With Nightmare difficulty (100% optimal) and high military, should attack
        assert!(
            matches!(d, NpcDecision::Attack(_)),
            "Expected Attack, got {d:?}"
        );
    }

    #[test]
    fn test_decide_defensive_defends_under_threat() {
        let engine = make_engine();
        let faction = NpcFaction {
            id: "def".into(),
            name: "Defender".into(),
            personality: NpcPersonality::Defensive,
            difficulty: DifficultyLevel::Nightmare,
            resources: NpcResources::starter(),
            units: vec![NpcUnit { unit_type: "guard".into(), count: 5, power: 1.0 }],
            buildings: vec![],
            tech_level: 1,
            aggression: 0.1,
            expansion_drive: 0.1,
        };
        let ctx = DecisionContext {
            own_faction_id: "def".into(),
            enemy_faction_ids: vec![],
            tick_count: 10,
            threat_level: 0.8,
            resource_ratio: 0.5,
            military_strength: 0.2,
        };
        let d = engine.decide(&faction, &ctx);
        assert_eq!(d, NpcDecision::Defend);
    }

    #[test]
    fn test_decide_economic_gathers_when_poor() {
        let engine = make_engine();
        let faction = NpcFaction {
            id: "eco".into(),
            name: "Eco".into(),
            personality: NpcPersonality::Economic,
            difficulty: DifficultyLevel::Nightmare,
            resources: NpcResources {
                minerals: 50.0,
                gas: 10.0,
                energy: 5.0,
                dark_matter: 0.0,
                population: 5,
            },
            units: vec![],
            buildings: vec![],
            tech_level: 1,
            aggression: 0.0,
            expansion_drive: 0.3,
        };
        let ctx = DecisionContext {
            own_faction_id: "eco".into(),
            enemy_faction_ids: vec![],
            tick_count: 20,
            threat_level: 0.1,
            resource_ratio: 0.1,
            military_strength: 0.0,
        };
        let d = engine.decide(&faction, &ctx);
        assert_eq!(d, NpcDecision::GatherResources);
    }

    #[test]
    fn test_update_faction_build_unit() {
        let engine = make_engine();
        engine.update_faction("iron_legion", &NpcDecision::BuildUnit("infantry".into()))
            .unwrap_or(());
        let guard = engine.factions.lock().unwrap_or_else(|e| e.into_inner());
        let inf = guard["iron_legion"]
            .units
            .iter()
            .find(|u| u.unit_type == "infantry");
        assert!(inf.is_some());
        assert!(inf.map(|u| u.count).unwrap_or(0) >= 21);
    }

    #[test]
    fn test_update_faction_build_building() {
        let engine = make_engine();
        engine.update_faction("crystal_hive", &NpcDecision::BuildBuilding("lab".into()))
            .unwrap_or(());
        let guard = engine.factions.lock().unwrap_or_else(|e| e.into_inner());
        let lab = guard["crystal_hive"]
            .buildings
            .iter()
            .find(|b| b.building_type == "lab");
        assert!(lab.is_some());
    }

    #[test]
    fn test_update_faction_gather_resources() {
        let engine = make_engine();
        let before = {
            let g = engine.factions.lock().unwrap_or_else(|e| e.into_inner());
            g["void_traders"].resources.minerals
        };
        engine.update_faction("void_traders", &NpcDecision::GatherResources)
            .unwrap_or(());
        let after = {
            let g = engine.factions.lock().unwrap_or_else(|e| e.into_inner());
            g["void_traders"].resources.minerals
        };
        assert!(after > before);
    }

    #[test]
    fn test_stats() {
        let engine = make_engine();
        let stats = engine.stats();
        assert!(stats.is_ok());
        let s = stats.unwrap_or_else(|_| NpcStats {
            faction_count: 0,
            total_units: 0,
            total_buildings: 0,
            avg_tech_level: 0.0,
            strongest_faction: None,
        });
        assert_eq!(s.faction_count, 4);
        assert!(s.total_units > 0);
        assert!(s.strongest_faction.is_some());
    }

    #[test]
    fn test_choose_build_target_personalities() {
        let f = NpcFaction {
            id: "x".into(), name: "X".into(),
            personality: NpcPersonality::Aggressive,
            difficulty: DifficultyLevel::Normal,
            resources: NpcResources::starter(),
            units: vec![], buildings: vec![],
            tech_level: 1, aggression: 0.5, expansion_drive: 0.5,
        };
        assert_eq!(NpcAiEngine::choose_build_target(&f, NpcPersonality::Aggressive), "barracks");
        assert_eq!(NpcAiEngine::choose_build_target(&f, NpcPersonality::Defensive), "shield_gen");
        assert_eq!(NpcAiEngine::choose_build_target(&f, NpcPersonality::Economic), "trade_hub");
        assert_eq!(NpcAiEngine::choose_build_target(&f, NpcPersonality::Expansionist), "outpost");
    }

    #[test]
    fn test_evaluate_threat_no_enemies() {
        let engine = make_engine();
        let guard = engine.factions.lock().unwrap_or_else(|e| e.into_inner());
        let faction = guard.get("iron_legion").cloned();
        drop(guard);
        if let Some(f) = faction {
            let threat = engine.evaluate_threat(&f, &[]);
            assert!((threat - 0.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_npc_resources_economic_score() {
        let r = NpcResources {
            minerals: 100.0,
            gas: 100.0,
            energy: 100.0,
            dark_matter: 10.0,
            population: 20,
        };
        let score = r.economic_score();
        // 100 + 150 + 200 + 50 = 500
        assert!((score - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_decide_rush_builds_early() {
        let engine = make_engine();
        let faction = NpcFaction {
            id: "rush".into(),
            name: "Rush".into(),
            personality: NpcPersonality::Rush,
            difficulty: DifficultyLevel::Nightmare,
            resources: NpcResources::starter(),
            units: vec![],
            buildings: vec![],
            tech_level: 1,
            aggression: 0.7,
            expansion_drive: 0.2,
        };
        let ctx = DecisionContext {
            own_faction_id: "rush".into(),
            enemy_faction_ids: vec![],
            tick_count: 5,
            threat_level: 0.1,
            resource_ratio: 0.5,
            military_strength: 0.1,
        };
        let d = engine.decide(&faction, &ctx);
        assert_eq!(d, NpcDecision::BuildUnit("zergling".into()));
    }

    #[test]
    fn test_decide_turtle_defends() {
        let engine = make_engine();
        let faction = NpcFaction {
            id: "t".into(),
            name: "Turtle".into(),
            personality: NpcPersonality::Turtle,
            difficulty: DifficultyLevel::Nightmare,
            resources: NpcResources::starter(),
            units: vec![],
            buildings: vec![],
            tech_level: 1,
            aggression: 0.0,
            expansion_drive: 0.0,
        };
        let ctx = DecisionContext {
            own_faction_id: "t".into(),
            enemy_faction_ids: vec![],
            tick_count: 100,
            threat_level: 0.5,
            resource_ratio: 0.5,
            military_strength: 0.5,
        };
        let d = engine.decide(&faction, &ctx);
        assert_eq!(d, NpcDecision::Defend);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let decision = NpcDecision::Attack("enemy_1".into());
        let json = serde_json::to_string(&decision).unwrap_or_default();
        assert!(!json.is_empty());
        let back: NpcDecision = serde_json::from_str(&json).unwrap_or(NpcDecision::Wait);
        assert_eq!(back, decision);
    }

    #[test]
    fn test_faction_serialization() {
        let factions = NpcAiEngine::default_factions();
        for f in &factions {
            let json = serde_json::to_string(f).unwrap_or_default();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn test_update_unknown_faction_errors() {
        let engine = make_engine();
        let result = engine.update_faction("nobody", &NpcDecision::Wait);
        assert!(result.is_err());
    }
}
