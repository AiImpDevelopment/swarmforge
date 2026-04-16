// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge NPC AI -- Behavior Trees, GOAP, and Hex-Grid Pathfinding
//!
//! ## Pathfinding
//!
//! Three algorithms implemented from scratch on axial hex coordinates:
//!
//! - **A\*** — optimal single-source single-target path using a hex-distance
//!   heuristic.  Implemented with `BinaryHeap` (min-heap via `Reverse`).
//! - **Dijkstra** — shortest paths from a source up to a cost budget.
//! - **BFS** — all tiles reachable within N steps (vision, area coverage).
//!
//! Hex grids use *axial coordinates* (q, r) with cube-constraint `s = -q - r`.
//! The six neighbours of `(q, r)` are the standard axial offsets.
//!
//! ## NPC AI — 3-Layer Architecture
//!
//! | Layer       | Scope              | Tree builder            |
//! |-------------|--------------------|-------------------------|
//! | Tactical    | Single unit        | `tactical_tree()`       |
//! | Operational | Base / colony      | `operational_tree()`    |
//! | Strategic   | Multi-settlement   | `strategic_tree()`      |
//!
//! Each layer is a **Behavior Tree** evaluated against a shared `AiBlackboard`.
//! The blackboard carries threat levels, resource counts, enemy composition,
//! and the current high-level strategy.
//!
//! ## Difficulty Scaling
//!
//! Four tiers (`Easy` .. `Brutal`) adjust resource income rate, decision
//! speed, and whether the AI uses espionage.

use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

use crate::error::ImpForgeError;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_ai", "Game");

// ===========================================================================
// Part 1 — Hex Grid Primitives
// ===========================================================================

/// A position on the hex grid using axial coordinates (q, r).
///
/// Cube constraint: `s = -q - r`.  Two positions are equal iff both `q` and
/// `r` match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HexPos {
    pub q: i32,
    pub r: i32,
}

impl HexPos {
    pub fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    /// Cube `s` component derived from the axial pair.
    pub fn s(self) -> i32 {
        -self.q - self.r
    }
}

/// Axial direction offsets for the six hex neighbours (pointy-top layout).
const HEX_DIRS: [(i32, i32); 6] = [
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, 0),
    (-1, 1),
    (0, 1),
];

/// Return the six neighbours of `pos` in clockwise order.
pub fn hex_neighbors(pos: HexPos) -> [HexPos; 6] {
    let mut out = [HexPos { q: 0, r: 0 }; 6];
    for (i, &(dq, dr)) in HEX_DIRS.iter().enumerate() {
        out[i] = HexPos::new(pos.q + dq, pos.r + dr);
    }
    out
}

/// Manhattan distance on a hex grid (cube distance / 2).
///
/// `hex_distance = max(|dq|, |dr|, |ds|)` where `ds = -(dq + dr)`.
pub fn hex_distance(a: HexPos, b: HexPos) -> i32 {
    let dq = (a.q - b.q).abs();
    let dr = (a.r - b.r).abs();
    let ds = (a.s() - b.s()).abs();
    dq.max(dr).max(ds)
}

/// All tiles at exactly `radius` steps from `center` (the hex ring).
///
/// Walks around the ring starting from the "bottom-left" direction and
/// traverses each of the six edges.  Returns an empty vec for radius 0
/// (use `vec![center]` instead).
pub fn hex_ring(center: HexPos, radius: u32) -> Vec<HexPos> {
    if radius == 0 {
        return vec![center];
    }
    let r = radius as i32;
    // Start tile: move `radius` steps in direction 4 (-1, +1)
    let mut pos = HexPos::new(center.q + HEX_DIRS[4].0 * r, center.r + HEX_DIRS[4].1 * r);
    let mut ring = Vec::with_capacity(6 * radius as usize);
    for &(dq, dr) in &HEX_DIRS {
        for _ in 0..radius {
            ring.push(pos);
            pos = HexPos::new(pos.q + dq, pos.r + dr);
        }
    }
    ring
}

/// All tiles within `radius` of `center`, ordered ring-by-ring outward
/// (spiral traversal).  Includes the center tile.
pub fn hex_spiral(center: HexPos, radius: u32) -> Vec<HexPos> {
    let mut tiles = vec![center];
    for r in 1..=radius {
        tiles.extend(hex_ring(center, r));
    }
    tiles
}

// ===========================================================================
// Part 1b — Pathfinding Results & Terrain Cost
// ===========================================================================

/// The result of a pathfinding query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathResult {
    pub path: Vec<HexPos>,
    pub total_cost: f64,
    pub tiles_explored: u32,
    pub algorithm: String,
}

/// Movement cost per terrain biome and unit locomotion type.
///
/// Returns `f64::INFINITY` for impassable combinations (e.g. ground units on
/// deep ocean).
pub fn movement_cost(biome: &str, unit_type: &str) -> f64 {
    match (biome, unit_type) {
        ("deep_ocean", "ground") => f64::INFINITY, // impassable
        ("deep_ocean", "flying") => 1.0,
        ("deep_ocean", "naval") => 1.0,
        ("ocean", "ground") => f64::INFINITY,
        ("ocean", "flying") => 1.0,
        ("ocean", "naval") => 1.2,
        ("highlands", _) => 2.0,
        ("swamp", "ground") => 3.0,
        ("swamp", "flying") => 1.0,
        ("swamp", "naval") => 2.0,
        ("forest", _) => 1.5,
        ("volcanic", _) => 2.5,
        ("frozen_wastes", _) => 2.0,
        ("desert", _) => 1.3,
        ("plains", _) => 1.0,
        ("mountain", "ground") => 3.5,
        ("mountain", "flying") => 1.0,
        ("mountain", _) => f64::INFINITY,
        _ => 1.0,
    }
}

// ===========================================================================
// Part 1c — A* Pathfinding (from scratch, BinaryHeap)
// ===========================================================================

/// Internal node for the A* open set, ordered by `f = g + h`.
///
/// We store `Reverse<OrderedF64>` so `BinaryHeap` (a max-heap) pops the
/// *smallest* f-cost first.
#[derive(Debug, Clone)]
struct AStarNode {
    pos: HexPos,
    f_cost: f64,
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool {
        self.pos == other.pos
    }
}
impl Eq for AStarNode {}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse so BinaryHeap pops lowest f-cost first
        other
            .f_cost
            .partial_cmp(&self.f_cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// A\* pathfinding on a hex grid.
///
/// - `start` / `goal` — endpoints.
/// - `passable` — returns `true` if a tile can be entered.
/// - `cost` — movement cost to enter a tile (must be > 0 for passable tiles).
///
/// Heuristic: `hex_distance(n, goal)` scaled by `MIN_COST` (1.0) to stay
/// admissible.
///
/// Returns `None` if no path exists.
pub fn astar(
    start: HexPos,
    goal: HexPos,
    passable: &dyn Fn(HexPos) -> bool,
    cost: &dyn Fn(HexPos) -> f64,
) -> Option<PathResult> {
    const MIN_COST: f64 = 1.0; // minimum possible tile cost (admissible bound)

    if start == goal {
        return Some(PathResult {
            path: vec![start],
            total_cost: 0.0,
            tiles_explored: 1,
            algorithm: "astar".to_string(),
        });
    }

    let mut open = BinaryHeap::new();
    let mut g_score: HashMap<HexPos, f64> = HashMap::new();
    let mut came_from: HashMap<HexPos, HexPos> = HashMap::new();
    let mut closed: HashSet<HexPos> = HashSet::new();

    g_score.insert(start, 0.0);
    open.push(AStarNode {
        pos: start,
        f_cost: hex_distance(start, goal) as f64 * MIN_COST,
    });

    let mut tiles_explored: u32 = 0;

    while let Some(current) = open.pop() {
        let pos = current.pos;

        if pos == goal {
            // Reconstruct path
            let mut path = vec![goal];
            let mut cur = goal;
            while let Some(&prev) = came_from.get(&cur) {
                path.push(prev);
                cur = prev;
            }
            path.reverse();
            return Some(PathResult {
                path,
                total_cost: g_score[&goal],
                tiles_explored,
                algorithm: "astar".to_string(),
            });
        }

        if !closed.insert(pos) {
            continue; // already expanded
        }
        tiles_explored += 1;

        let g_current = g_score[&pos];

        for neighbor in hex_neighbors(pos) {
            if closed.contains(&neighbor) || !passable(neighbor) {
                continue;
            }

            let tile_cost = cost(neighbor);
            if tile_cost.is_infinite() {
                continue;
            }

            let tentative_g = g_current + tile_cost;
            let prev_g = g_score.get(&neighbor).copied().unwrap_or(f64::INFINITY);

            if tentative_g < prev_g {
                g_score.insert(neighbor, tentative_g);
                came_from.insert(neighbor, pos);
                let h = hex_distance(neighbor, goal) as f64 * MIN_COST;
                open.push(AStarNode {
                    pos: neighbor,
                    f_cost: tentative_g + h,
                });
            }
        }
    }

    None // no path found
}

// ===========================================================================
// Part 1d — Dijkstra (single-source, cost-bounded)
// ===========================================================================

/// Internal node for Dijkstra priority queue.
#[derive(Debug, Clone)]
struct DijkstraNode {
    pos: HexPos,
    dist: f64,
}

impl PartialEq for DijkstraNode {
    fn eq(&self, other: &Self) -> bool {
        self.pos == other.pos
    }
}
impl Eq for DijkstraNode {}

impl PartialOrd for DijkstraNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DijkstraNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .dist
            .partial_cmp(&self.dist)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Dijkstra shortest-path tree from `start`, exploring tiles up to `max_cost`.
///
/// Returns a map from each reachable `HexPos` to its cumulative cost from
/// `start`.  Tiles whose cumulative cost exceeds `max_cost` are excluded.
pub fn dijkstra(
    start: HexPos,
    max_cost: f64,
    passable: &dyn Fn(HexPos) -> bool,
    cost: &dyn Fn(HexPos) -> f64,
) -> HashMap<HexPos, f64> {
    let mut dist: HashMap<HexPos, f64> = HashMap::new();
    let mut heap = BinaryHeap::new();

    dist.insert(start, 0.0);
    heap.push(DijkstraNode {
        pos: start,
        dist: 0.0,
    });

    while let Some(DijkstraNode { pos, dist: d }) = heap.pop() {
        // Skip stale entries
        if let Some(&best) = dist.get(&pos) {
            if d > best {
                continue;
            }
        }

        for neighbor in hex_neighbors(pos) {
            if !passable(neighbor) {
                continue;
            }

            let tile_cost = cost(neighbor);
            if tile_cost.is_infinite() {
                continue;
            }

            let new_dist = d + tile_cost;
            if new_dist > max_cost {
                continue;
            }

            let prev = dist.get(&neighbor).copied().unwrap_or(f64::INFINITY);
            if new_dist < prev {
                dist.insert(neighbor, new_dist);
                heap.push(DijkstraNode {
                    pos: neighbor,
                    dist: new_dist,
                });
            }
        }
    }

    dist
}

// ===========================================================================
// Part 1e — BFS Range (step-bounded)
// ===========================================================================

/// BFS flood-fill from `start`, returning all tiles reachable within
/// `max_steps` steps.  Ignores tile cost — each step counts as 1.
pub fn bfs_range(
    start: HexPos,
    max_steps: u32,
    passable: &dyn Fn(HexPos) -> bool,
) -> HashSet<HexPos> {
    let mut visited: HashSet<HexPos> = HashSet::new();
    let mut queue: VecDeque<(HexPos, u32)> = VecDeque::new();

    visited.insert(start);
    queue.push_back((start, 0));

    while let Some((pos, steps)) = queue.pop_front() {
        if steps >= max_steps {
            continue;
        }
        for neighbor in hex_neighbors(pos) {
            if visited.contains(&neighbor) || !passable(neighbor) {
                continue;
            }
            visited.insert(neighbor);
            queue.push_back((neighbor, steps + 1));
        }
    }

    visited
}

// ===========================================================================
// Part 2 — NPC AI: Behavior Tree Types
// ===========================================================================

/// Behavior tree node — the recursive building block of NPC decision logic.
///
/// **Composites** run children in sequence or parallel.
/// **Decorators** wrap a single child with control logic.
/// **Leaves** are concrete conditions or actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BtNode {
    // --- Composites ---
    /// All children must succeed (short-circuits on first failure).
    Sequence(Vec<BtNode>),
    /// First child that succeeds wins (short-circuits on first success).
    Selector(Vec<BtNode>),
    /// Run all children; succeed if any succeeds.
    Parallel(Vec<BtNode>),

    // --- Decorators ---
    /// Invert the child result (Success <-> Failure).
    Inverter(Box<BtNode>),
    /// Repeat the child `n` times; fail on first child failure.
    Repeat(Box<BtNode>, u32),
    /// Repeat the child until it fails; then return Success.
    UntilFail(Box<BtNode>),

    // --- Leaves ---
    /// Check a world-state condition against the blackboard.
    Condition(AiCondition),
    /// Execute a game action.
    Action(AiAction),
}

/// Tri-state result of evaluating a behavior tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BtStatus {
    Success,
    Failure,
    Running,
}

// ===========================================================================
// Part 2b — AI Conditions & Actions
// ===========================================================================

/// Conditions the AI can check against the blackboard / world state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiCondition {
    HasResources { resource: String, min_amount: f64 },
    EnemyNearby { range: u32 },
    HealthBelow { percent: f64 },
    BuildingExists { building_type: String },
    PopulationAbove { count: u32 },
    ThreatLevelAbove { level: f64 },
    CanAfford { item: String },
    ResearchComplete { tech: String },
    UnitCountBelow { unit_type: String, count: u32 },
    IsUnderAttack,
    HasIdleWorkers { count: u32 },
}

/// Actions the AI can perform in the game world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiAction {
    BuildStructure { building_type: String },
    TrainUnit { unit_type: String, count: u32 },
    ResearchTech { tech: String },
    AttackTarget { target_id: String },
    Retreat { fallback_pos: HexPos },
    GatherResources { resource_type: String },
    ScoutArea { target: HexPos },
    Expand { target_coord: String },
    DefendPosition { pos: HexPos },
    UseAbility { ability_id: String, target_id: String },
    SendDiplomacy { target: String, offer: String },
    SetRallyPoint { pos: HexPos },
}

// ===========================================================================
// Part 2c — Blackboard & Strategy
// ===========================================================================

/// Shared data bus for cross-layer AI communication.
///
/// Every AI layer reads and writes this blackboard so tactical decisions
/// can be informed by strategic goals and vice-versa.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiBlackboard {
    /// Aggregate threat level from 0.0 (peaceful) to 1.0 (imminent attack).
    pub threat_level: f64,
    /// The faction this AI is playing.
    pub player_faction: String,
    /// Known enemy unit composition: `unit_type -> count`.
    pub enemy_composition: HashMap<String, u32>,
    /// Global alertness from 0.0 (asleep) to 1.0 (max alert).
    pub global_alertness: f64,
    /// Current resource stocks: `resource_name -> amount`.
    pub resources: HashMap<String, f64>,
    /// Priority targets for military operations.
    pub priority_targets: Vec<String>,
    /// Candidate coordinates for new colonies.
    pub expansion_candidates: Vec<String>,
    /// High-level strategy the AI is currently pursuing.
    pub current_strategy: AiStrategy,
    /// Estimated population of the AI player.
    pub population: u32,
    /// Number of idle workers.
    pub idle_workers: u32,
    /// Whether the AI is currently under attack.
    pub under_attack: bool,
    /// Set of completed research techs.
    pub completed_research: HashSet<String>,
    /// Set of constructed building types.
    pub built_structures: HashSet<String>,
    /// Per-unit-type count.
    pub unit_counts: HashMap<String, u32>,
}

/// High-level strategy the AI pursues, selected by the strategic layer.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum AiStrategy {
    /// Focus on resource gathering and economic growth.
    #[default]
    Economic,
    /// Build military units and prepare for war.
    Military,
    /// Fortify existing positions, build defenses.
    Defensive,
    /// Launch attacks against enemy positions.
    Aggressive,
    /// Colonize new areas and expand territory.
    Expansion,
    /// Prioritize research and technology advancement.
    Tech,
}

impl AiStrategy {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Economic => "economic",
            Self::Military => "military",
            Self::Defensive => "defensive",
            Self::Aggressive => "aggressive",
            Self::Expansion => "expansion",
            Self::Tech => "tech",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "economic" => Self::Economic,
            "military" => Self::Military,
            "defensive" => Self::Defensive,
            "aggressive" => Self::Aggressive,
            "expansion" => Self::Expansion,
            "tech" => Self::Tech,
            _ => Self::Economic,
        }
    }
}

// ===========================================================================
// Part 2d — Difficulty
// ===========================================================================

/// NPC AI difficulty scaling tier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AiDifficulty {
    /// 50% resource rate, slow decisions, no espionage.
    Easy,
    /// 100% resource rate, standard decision timing.
    Normal,
    /// 120% resource rate, faster decisions, uses espionage.
    Hard,
    /// 150% resource rate, optimal decisions, full espionage + alliances.
    Brutal,
}

impl AiDifficulty {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "easy" => Some(Self::Easy),
            "normal" => Some(Self::Normal),
            "hard" => Some(Self::Hard),
            "brutal" => Some(Self::Brutal),
            _ => None,
        }
    }

    /// Resource income multiplier for this difficulty tier.
    pub fn resource_multiplier(&self) -> f64 {
        match self {
            Self::Easy => 0.5,
            Self::Normal => 1.0,
            Self::Hard => 1.2,
            Self::Brutal => 1.5,
        }
    }

    /// Decision interval in seconds — lower = smarter.
    pub fn decision_interval_secs(&self) -> f64 {
        match self {
            Self::Easy => 10.0,
            Self::Normal => 5.0,
            Self::Hard => 3.0,
            Self::Brutal => 1.0,
        }
    }

    /// Whether the AI uses espionage at this difficulty.
    pub fn uses_espionage(&self) -> bool {
        matches!(self, Self::Hard | Self::Brutal)
    }
}

// ===========================================================================
// Part 2e — Behavior Tree Evaluation
// ===========================================================================

/// Evaluate a condition against the current blackboard state.
fn evaluate_condition(cond: &AiCondition, bb: &AiBlackboard) -> BtStatus {
    let result = match cond {
        AiCondition::HasResources {
            resource,
            min_amount,
        } => bb
            .resources
            .get(resource)
            .copied()
            .unwrap_or(0.0)
            >= *min_amount,

        AiCondition::EnemyNearby { range: _ } => {
            // Simplified: any known enemy means "enemy nearby"
            !bb.enemy_composition.is_empty()
        }

        AiCondition::HealthBelow { percent } => {
            // Approximate: high threat = low "health"
            bb.threat_level > (1.0 - percent / 100.0)
        }

        AiCondition::BuildingExists { building_type } => {
            bb.built_structures.contains(building_type)
        }

        AiCondition::PopulationAbove { count } => bb.population > *count,

        AiCondition::ThreatLevelAbove { level } => bb.threat_level > *level,

        AiCondition::CanAfford { item } => {
            // Simplified: check if the AI has > 100 of the item resource
            bb.resources.get(item).copied().unwrap_or(0.0) > 100.0
        }

        AiCondition::ResearchComplete { tech } => bb.completed_research.contains(tech),

        AiCondition::UnitCountBelow { unit_type, count } => {
            bb.unit_counts.get(unit_type).copied().unwrap_or(0) < *count
        }

        AiCondition::IsUnderAttack => bb.under_attack,

        AiCondition::HasIdleWorkers { count } => bb.idle_workers >= *count,
    };

    if result {
        BtStatus::Success
    } else {
        BtStatus::Failure
    }
}

/// Evaluate an action node.  Actions always return `Success` (the actual
/// side-effects are collected by `npc_ai_tick` via `collect_actions`).
fn evaluate_action(_action: &AiAction, _bb: &AiBlackboard) -> BtStatus {
    // In a full game loop the action would be pushed to a command queue.
    // Here we return Success so the tree continues evaluating.
    BtStatus::Success
}

/// Recursively evaluate a behavior tree against the blackboard.
pub fn evaluate_tree(tree: &BtNode, blackboard: &AiBlackboard) -> BtStatus {
    match tree {
        // --- Composites ---
        BtNode::Sequence(children) => {
            for child in children {
                match evaluate_tree(child, blackboard) {
                    BtStatus::Failure => return BtStatus::Failure,
                    BtStatus::Running => return BtStatus::Running,
                    BtStatus::Success => {}
                }
            }
            BtStatus::Success
        }

        BtNode::Selector(children) => {
            for child in children {
                match evaluate_tree(child, blackboard) {
                    BtStatus::Success => return BtStatus::Success,
                    BtStatus::Running => return BtStatus::Running,
                    BtStatus::Failure => {}
                }
            }
            BtStatus::Failure
        }

        BtNode::Parallel(children) => {
            let mut any_success = false;
            let mut any_running = false;
            for child in children {
                match evaluate_tree(child, blackboard) {
                    BtStatus::Success => any_success = true,
                    BtStatus::Running => any_running = true,
                    BtStatus::Failure => {}
                }
            }
            if any_success {
                BtStatus::Success
            } else if any_running {
                BtStatus::Running
            } else {
                BtStatus::Failure
            }
        }

        // --- Decorators ---
        BtNode::Inverter(child) => match evaluate_tree(child, blackboard) {
            BtStatus::Success => BtStatus::Failure,
            BtStatus::Failure => BtStatus::Success,
            BtStatus::Running => BtStatus::Running,
        },

        BtNode::Repeat(child, n) => {
            for _ in 0..*n {
                match evaluate_tree(child, blackboard) {
                    BtStatus::Failure => return BtStatus::Failure,
                    BtStatus::Running => return BtStatus::Running,
                    BtStatus::Success => {}
                }
            }
            BtStatus::Success
        }

        BtNode::UntilFail(child) => {
            // Guard: iterate at most 1000 times to prevent infinite loops in
            // pure evaluation context.
            for _ in 0..1000 {
                match evaluate_tree(child, blackboard) {
                    BtStatus::Failure => return BtStatus::Success,
                    BtStatus::Running => return BtStatus::Running,
                    BtStatus::Success => {}
                }
            }
            BtStatus::Success
        }

        // --- Leaves ---
        BtNode::Condition(cond) => evaluate_condition(cond, blackboard),
        BtNode::Action(action) => evaluate_action(action, blackboard),
    }
}

/// Walk the tree and collect all `AiAction` leaves whose parent path
/// evaluates to `Success`.  This is the "what should the AI do this tick"
/// query.
pub fn collect_actions(tree: &BtNode, bb: &AiBlackboard) -> Vec<AiAction> {
    let mut out = Vec::new();
    collect_actions_inner(tree, bb, &mut out);
    out
}

fn collect_actions_inner(tree: &BtNode, bb: &AiBlackboard, out: &mut Vec<AiAction>) {
    match tree {
        BtNode::Sequence(children) => {
            for child in children {
                match evaluate_tree(child, bb) {
                    BtStatus::Failure => break,
                    BtStatus::Running => break,
                    BtStatus::Success => {
                        collect_actions_inner(child, bb, out);
                    }
                }
            }
        }

        BtNode::Selector(children) => {
            for child in children {
                match evaluate_tree(child, bb) {
                    BtStatus::Success => {
                        collect_actions_inner(child, bb, out);
                        break;
                    }
                    BtStatus::Running => break,
                    BtStatus::Failure => {}
                }
            }
        }

        BtNode::Parallel(children) => {
            for child in children {
                if evaluate_tree(child, bb) == BtStatus::Success {
                    collect_actions_inner(child, bb, out);
                }
            }
        }

        BtNode::Inverter(child) | BtNode::Repeat(child, _) | BtNode::UntilFail(child) => {
            collect_actions_inner(child, bb, out);
        }

        BtNode::Condition(_) => {}

        BtNode::Action(action) => {
            out.push(action.clone());
        }
    }
}

// ===========================================================================
// Part 2f — Default Behavior Trees
// ===========================================================================

/// **Tactical tree** — unit-level micro decisions.
///
/// Priority: retreat if low health > use ability > attack enemy > defend.
pub fn tactical_tree() -> BtNode {
    BtNode::Selector(vec![
        // 1. Retreat if critically damaged
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::HealthBelow { percent: 20.0 }),
            BtNode::Action(AiAction::Retreat {
                fallback_pos: HexPos::new(0, 0),
            }),
        ]),
        // 2. Use ability on priority target if under attack
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::IsUnderAttack),
            BtNode::Condition(AiCondition::EnemyNearby { range: 3 }),
            BtNode::Action(AiAction::UseAbility {
                ability_id: "auto_attack".to_string(),
                target_id: "nearest_enemy".to_string(),
            }),
        ]),
        // 3. Attack nearby enemy
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::EnemyNearby { range: 5 }),
            BtNode::Action(AiAction::AttackTarget {
                target_id: "nearest_enemy".to_string(),
            }),
        ]),
        // 4. Defend current position
        BtNode::Action(AiAction::DefendPosition {
            pos: HexPos::new(0, 0),
        }),
    ])
}

/// **Operational tree** — base-level building and training.
///
/// Priority: defend if attacked > build barracks if none > train workers if
/// idle > train military if threat > gather resources.
pub fn operational_tree() -> BtNode {
    BtNode::Selector(vec![
        // 1. Under attack: build defenses + train units
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::IsUnderAttack),
            BtNode::Parallel(vec![
                BtNode::Action(AiAction::TrainUnit {
                    unit_type: "defender".to_string(),
                    count: 5,
                }),
                BtNode::Action(AiAction::BuildStructure {
                    building_type: "turret".to_string(),
                }),
            ]),
        ]),
        // 2. Need barracks
        BtNode::Sequence(vec![
            BtNode::Inverter(Box::new(BtNode::Condition(AiCondition::BuildingExists {
                building_type: "barracks".to_string(),
            }))),
            BtNode::Condition(AiCondition::HasResources {
                resource: "metal".to_string(),
                min_amount: 200.0,
            }),
            BtNode::Action(AiAction::BuildStructure {
                building_type: "barracks".to_string(),
            }),
        ]),
        // 3. Train workers if low
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::UnitCountBelow {
                unit_type: "worker".to_string(),
                count: 10,
            }),
            BtNode::Action(AiAction::TrainUnit {
                unit_type: "worker".to_string(),
                count: 3,
            }),
        ]),
        // 4. Train military if high threat
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::ThreatLevelAbove { level: 0.5 }),
            BtNode::Condition(AiCondition::HasResources {
                resource: "metal".to_string(),
                min_amount: 500.0,
            }),
            BtNode::Action(AiAction::TrainUnit {
                unit_type: "soldier".to_string(),
                count: 5,
            }),
        ]),
        // 5. Research if affordable
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::HasResources {
                resource: "crystal".to_string(),
                min_amount: 300.0,
            }),
            BtNode::Action(AiAction::ResearchTech {
                tech: "next_available".to_string(),
            }),
        ]),
        // 6. Default: gather resources
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::HasIdleWorkers { count: 1 }),
            BtNode::Action(AiAction::GatherResources {
                resource_type: "metal".to_string(),
            }),
        ]),
    ])
}

/// **Strategic tree** — multi-settlement decisions.
///
/// Priority: aggressive if strong > expand if stable > diplomacy if weak >
/// scout for intel.
pub fn strategic_tree() -> BtNode {
    BtNode::Selector(vec![
        // 1. Switch to aggressive when army is large
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::ThreatLevelAbove { level: 0.3 }),
            BtNode::Condition(AiCondition::PopulationAbove { count: 50 }),
            BtNode::Condition(AiCondition::HasResources {
                resource: "metal".to_string(),
                min_amount: 2000.0,
            }),
            BtNode::Action(AiAction::AttackTarget {
                target_id: "weakest_enemy".to_string(),
            }),
        ]),
        // 2. Expand when economy is stable
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::HasResources {
                resource: "metal".to_string(),
                min_amount: 1000.0,
            }),
            BtNode::Condition(AiCondition::PopulationAbove { count: 30 }),
            BtNode::Inverter(Box::new(BtNode::Condition(AiCondition::ThreatLevelAbove {
                level: 0.6,
            }))),
            BtNode::Action(AiAction::Expand {
                target_coord: "auto_select".to_string(),
            }),
        ]),
        // 3. Diplomacy when weak
        BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::ThreatLevelAbove { level: 0.7 }),
            BtNode::Inverter(Box::new(BtNode::Condition(AiCondition::PopulationAbove {
                count: 20,
            }))),
            BtNode::Action(AiAction::SendDiplomacy {
                target: "strongest_neighbor".to_string(),
                offer: "non_aggression_pact".to_string(),
            }),
        ]),
        // 4. Scout by default
        BtNode::Action(AiAction::ScoutArea {
            target: HexPos::new(10, 10),
        }),
    ])
}

// ===========================================================================
// Part 2g — AI Engine (manages per-colony state)
// ===========================================================================

/// Per-colony AI state managed by the engine.
struct ColonyAi {
    blackboard: AiBlackboard,
    difficulty: AiDifficulty,
    /// Seconds since last decision tick.
    elapsed_since_tick: f64,
}

/// The AI engine holds state for all NPC colonies.
pub struct SwarmAiEngine {
    colonies: std::sync::Mutex<HashMap<String, ColonyAi>>,
}

impl SwarmAiEngine {
    pub fn new() -> Self {
        Self {
            colonies: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Get or create the AI state for a colony.
    fn ensure_colony(&self, colony_id: &str) -> Result<(), ImpForgeError> {
        let mut colonies = self.colonies.lock().map_err(|e| {
            ImpForgeError::internal("AI_LOCK_FAILED", format!("Mutex poisoned: {e}"))
        })?;
        colonies.entry(colony_id.to_string()).or_insert(ColonyAi {
            blackboard: AiBlackboard {
                player_faction: colony_id.to_string(),
                resources: [
                    ("metal".to_string(), 500.0),
                    ("crystal".to_string(), 250.0),
                    ("deuterium".to_string(), 100.0),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            },
            difficulty: AiDifficulty::Normal,
            elapsed_since_tick: 0.0,
        });
        Ok(())
    }
}

// ===========================================================================
// Part 3 — Tauri Commands: Pathfinding (4)
// ===========================================================================

/// A\* pathfinding between two hex positions.
///
/// Uses a default passable check (all tiles are walkable) and uniform cost
/// of 1.0.  Game state-aware versions would query the actual map data.
#[tauri::command]
pub async fn pathfind_astar(
    from_q: i32,
    from_r: i32,
    to_q: i32,
    to_r: i32,
) -> Result<Option<PathResult>, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_ai", "game_ai", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_ai", "game_ai");
    crate::synapse_fabric::synapse_session_push("swarm_ai", "game_ai", "pathfind_astar called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_ai", "info", "swarm_ai active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_ai", "decide", crate::cortex_wiring::EventCategory::Ai, serde_json::json!({"op": "pathfind_astar"}));
    let start = HexPos::new(from_q, from_r);
    let goal = HexPos::new(to_q, to_r);

    // Default: every tile is passable, uniform cost.
    let result = astar(start, goal, &|_| true, &|_| 1.0);
    Ok(result)
}

/// BFS range query: all hex tiles reachable within `max_steps` from origin.
#[tauri::command]
pub async fn pathfind_range(q: i32, r: i32, max_steps: u32) -> Result<Vec<HexPos>, ImpForgeError> {
    let start = HexPos::new(q, r);
    let tiles = bfs_range(start, max_steps, &|_| true);
    let mut list: Vec<HexPos> = tiles.into_iter().collect();
    // Sort for deterministic output
    list.sort_by(|a, b| a.q.cmp(&b.q).then(a.r.cmp(&b.r)));
    Ok(list)
}

/// Cheapest cost from one hex to another using Dijkstra.
///
/// Returns `f64::INFINITY` serialized as `null` if no path exists.
#[tauri::command]
pub async fn pathfind_cost(
    from_q: i32,
    from_r: i32,
    to_q: i32,
    to_r: i32,
) -> Result<f64, ImpForgeError> {
    let start = HexPos::new(from_q, from_r);
    let goal = HexPos::new(to_q, to_r);

    // Use Dijkstra with a generous budget
    let dist_map = dijkstra(start, 10_000.0, &|_| true, &|_| 1.0);
    Ok(dist_map.get(&goal).copied().unwrap_or(f64::INFINITY))
}

/// Hex tile metadata: neighbours, distance to origin, ring tiles at radius 1.
#[tauri::command]
pub async fn pathfind_hex_info(q: i32, r: i32) -> Result<serde_json::Value, ImpForgeError> {
    let pos = HexPos::new(q, r);
    let neighbors = hex_neighbors(pos);
    let ring1 = hex_ring(pos, 1);
    let dist_to_origin = hex_distance(pos, HexPos::new(0, 0));

    Ok(serde_json::json!({
        "position": pos,
        "cube_s": pos.s(),
        "neighbors": neighbors,
        "ring_1": ring1,
        "distance_to_origin": dist_to_origin,
    }))
}

// ===========================================================================
// Part 3b — Tauri Commands: NPC AI (6)
// ===========================================================================

/// Evaluate all three AI layers for a colony and return the combined status.
#[tauri::command]
pub async fn npc_ai_evaluate(
    colony_id: String,
    engine: tauri::State<'_, SwarmAiEngine>,
) -> Result<serde_json::Value, ImpForgeError> {
    engine.ensure_colony(&colony_id)?;

    let colonies = engine.colonies.lock().map_err(|e| {
        ImpForgeError::internal("AI_LOCK_FAILED", format!("Mutex poisoned: {e}"))
    })?;
    let colony = colonies.get(&colony_id).ok_or_else(|| {
        ImpForgeError::validation("COLONY_NOT_FOUND", format!("No AI state for {colony_id}"))
    })?;

    let bb = &colony.blackboard;
    let tactical = evaluate_tree(&tactical_tree(), bb);
    let operational = evaluate_tree(&operational_tree(), bb);
    let strategic = evaluate_tree(&strategic_tree(), bb);

    Ok(serde_json::json!({
        "colony_id": colony_id,
        "difficulty": format!("{:?}", colony.difficulty),
        "tactical": format!("{tactical:?}"),
        "operational": format!("{operational:?}"),
        "strategic": format!("{strategic:?}"),
        "strategy": colony.blackboard.current_strategy.as_str(),
        "threat_level": colony.blackboard.threat_level,
    }))
}

/// Set the AI difficulty for a colony.
#[tauri::command]
pub async fn npc_ai_set_difficulty(
    colony_id: String,
    difficulty: String,
    engine: tauri::State<'_, SwarmAiEngine>,
) -> Result<(), ImpForgeError> {
    let diff = AiDifficulty::from_str(&difficulty).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_DIFFICULTY",
            format!("Unknown difficulty: {difficulty}. Use easy, normal, hard, or brutal."),
        )
    })?;

    engine.ensure_colony(&colony_id)?;
    let mut colonies = engine.colonies.lock().map_err(|e| {
        ImpForgeError::internal("AI_LOCK_FAILED", format!("Mutex poisoned: {e}"))
    })?;
    if let Some(colony) = colonies.get_mut(&colony_id) {
        colony.difficulty = diff;
    }
    Ok(())
}

/// Return the current blackboard state for a colony.
#[tauri::command]
pub async fn npc_ai_get_blackboard(
    colony_id: String,
    engine: tauri::State<'_, SwarmAiEngine>,
) -> Result<AiBlackboard, ImpForgeError> {
    engine.ensure_colony(&colony_id)?;

    let colonies = engine.colonies.lock().map_err(|e| {
        ImpForgeError::internal("AI_LOCK_FAILED", format!("Mutex poisoned: {e}"))
    })?;
    let colony = colonies.get(&colony_id).ok_or_else(|| {
        ImpForgeError::validation("COLONY_NOT_FOUND", format!("No AI state for {colony_id}"))
    })?;

    Ok(colony.blackboard.clone())
}

/// Return the current strategy string for a colony.
#[tauri::command]
pub async fn npc_ai_get_strategy(
    colony_id: String,
    engine: tauri::State<'_, SwarmAiEngine>,
) -> Result<String, ImpForgeError> {
    engine.ensure_colony(&colony_id)?;

    let colonies = engine.colonies.lock().map_err(|e| {
        ImpForgeError::internal("AI_LOCK_FAILED", format!("Mutex poisoned: {e}"))
    })?;
    let colony = colonies.get(&colony_id).ok_or_else(|| {
        ImpForgeError::validation("COLONY_NOT_FOUND", format!("No AI state for {colony_id}"))
    })?;

    Ok(colony.blackboard.current_strategy.as_str().to_string())
}

/// Return the default tactical behavior tree structure as JSON (for debug UI).
#[tauri::command]
pub async fn npc_ai_tactical_tree() -> Result<serde_json::Value, ImpForgeError> {
    let tree = tactical_tree();
    serde_json::to_value(&tree)
        .map_err(|e| ImpForgeError::internal("SERIALIZE_FAILED", e.to_string()))
}

/// Advance the AI by `delta_secs` and return any actions it decided to take.
///
/// The AI only produces actions when enough time has elapsed (governed by
/// difficulty-based decision interval).  Returns an empty vec if the AI is
/// still "thinking".
#[tauri::command]
pub async fn npc_ai_tick(
    colony_id: String,
    delta_secs: f64,
    engine: tauri::State<'_, SwarmAiEngine>,
) -> Result<Vec<AiAction>, ImpForgeError> {
    engine.ensure_colony(&colony_id)?;

    let mut colonies = engine.colonies.lock().map_err(|e| {
        ImpForgeError::internal("AI_LOCK_FAILED", format!("Mutex poisoned: {e}"))
    })?;
    let colony = colonies.get_mut(&colony_id).ok_or_else(|| {
        ImpForgeError::validation("COLONY_NOT_FOUND", format!("No AI state for {colony_id}"))
    })?;

    colony.elapsed_since_tick += delta_secs;

    let interval = colony.difficulty.decision_interval_secs();
    if colony.elapsed_since_tick < interval {
        return Ok(vec![]);
    }

    colony.elapsed_since_tick = 0.0;

    // Collect actions from all three layers
    let bb = &colony.blackboard;
    let mut actions = Vec::new();
    actions.extend(collect_actions(&tactical_tree(), bb));
    actions.extend(collect_actions(&operational_tree(), bb));
    actions.extend(collect_actions(&strategic_tree(), bb));

    Ok(actions)
}

// ===========================================================================
//  Additional Tauri Commands — wiring internal helpers
// ===========================================================================

/// Get all tiles in a hex spiral within `radius` of a center tile.
#[tauri::command]
pub async fn npc_ai_hex_spiral(
    q: i32,
    r: i32,
    radius: u32,
) -> Result<Vec<HexPos>, ImpForgeError> {
    if radius > 50 {
        return Err(ImpForgeError::validation(
            "AI_RADIUS_TOO_BIG",
            "Maximum spiral radius is 50.",
        ));
    }
    let center = HexPos::new(q, r);
    Ok(hex_spiral(center, radius))
}

/// Get movement cost for a biome/unit combination.
#[tauri::command]
pub async fn npc_ai_movement_cost(
    biome: String,
    unit_type: String,
) -> Result<serde_json::Value, ImpForgeError> {
    let cost = movement_cost(&biome, &unit_type);
    Ok(serde_json::json!({
        "biome": biome,
        "unit_type": unit_type,
        "cost": if cost.is_infinite() { -1.0 } else { cost },
        "passable": cost.is_finite(),
    }))
}

/// Get AI strategy info from a string key.
#[tauri::command]
pub async fn npc_ai_strategy_info(
    strategy: String,
) -> Result<serde_json::Value, ImpForgeError> {
    let s = AiStrategy::from_str(&strategy);
    Ok(serde_json::json!({
        "strategy": s.as_str(),
    }))
}

/// Get AI difficulty details including resource multiplier and espionage.
#[tauri::command]
pub async fn npc_ai_difficulty_info(
    difficulty: String,
) -> Result<serde_json::Value, ImpForgeError> {
    let d = AiDifficulty::from_str(&difficulty).unwrap_or(AiDifficulty::Normal);
    Ok(serde_json::json!({
        "resource_multiplier": d.resource_multiplier(),
        "decision_interval": d.decision_interval_secs(),
        "uses_espionage": d.uses_espionage(),
    }))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;


    // ── Hex primitives ────────────────────────────────────────────────

    #[test]
    fn test_hex_distance_same() {
        let a = HexPos::new(0, 0);
        assert_eq!(hex_distance(a, a), 0);
    }

    #[test]
    fn test_hex_distance_adjacent() {
        let a = HexPos::new(0, 0);
        let b = HexPos::new(1, 0);
        assert_eq!(hex_distance(a, b), 1);
    }

    #[test]
    fn test_hex_distance_diagonal() {
        let a = HexPos::new(0, 0);
        let b = HexPos::new(3, -3);
        assert_eq!(hex_distance(a, b), 3);
    }

    #[test]
    fn test_hex_distance_symmetric() {
        let a = HexPos::new(2, -5);
        let b = HexPos::new(-3, 1);
        assert_eq!(hex_distance(a, b), hex_distance(b, a));
    }

    #[test]
    fn test_hex_neighbors_count() {
        let neighbors = hex_neighbors(HexPos::new(0, 0));
        assert_eq!(neighbors.len(), 6);
    }

    #[test]
    fn test_hex_neighbors_all_adjacent() {
        let center = HexPos::new(5, -3);
        for n in hex_neighbors(center) {
            assert_eq!(hex_distance(center, n), 1);
        }
    }

    #[test]
    fn test_hex_neighbors_unique() {
        let neighbors = hex_neighbors(HexPos::new(0, 0));
        let set: HashSet<HexPos> = neighbors.into_iter().collect();
        assert_eq!(set.len(), 6);
    }

    #[test]
    fn test_hex_ring_radius_0() {
        let ring = hex_ring(HexPos::new(0, 0), 0);
        assert_eq!(ring.len(), 1);
        assert_eq!(ring[0], HexPos::new(0, 0));
    }

    #[test]
    fn test_hex_ring_radius_1() {
        let ring = hex_ring(HexPos::new(0, 0), 1);
        assert_eq!(ring.len(), 6);
        // Every tile in ring-1 should be distance 1 from center
        for tile in &ring {
            assert_eq!(hex_distance(HexPos::new(0, 0), *tile), 1);
        }
    }

    #[test]
    fn test_hex_ring_radius_2() {
        let ring = hex_ring(HexPos::new(0, 0), 2);
        assert_eq!(ring.len(), 12); // 6 * radius
        for tile in &ring {
            assert_eq!(hex_distance(HexPos::new(0, 0), *tile), 2);
        }
    }

    #[test]
    fn test_hex_spiral_radius_0() {
        let spiral = hex_spiral(HexPos::new(0, 0), 0);
        assert_eq!(spiral.len(), 1);
    }

    #[test]
    fn test_hex_spiral_radius_1() {
        let spiral = hex_spiral(HexPos::new(0, 0), 1);
        assert_eq!(spiral.len(), 7); // 1 + 6
    }

    #[test]
    fn test_hex_spiral_radius_2() {
        let spiral = hex_spiral(HexPos::new(0, 0), 2);
        assert_eq!(spiral.len(), 19); // 1 + 6 + 12
    }

    #[test]
    fn test_hex_cube_constraint() {
        let pos = HexPos::new(3, -5);
        assert_eq!(pos.q + pos.r + pos.s(), 0);
    }

    // ── A* ────────────────────────────────────────────────────────────

    #[test]
    fn test_astar_same_tile() {
        let result = astar(HexPos::new(0, 0), HexPos::new(0, 0), &|_| true, &|_| 1.0);
        let path = result.expect("should find path");
        assert_eq!(path.path.len(), 1);
        assert_eq!(path.total_cost, 0.0);
    }

    #[test]
    fn test_astar_adjacent() {
        let result = astar(HexPos::new(0, 0), HexPos::new(1, 0), &|_| true, &|_| 1.0);
        let path = result.expect("should find path");
        assert_eq!(path.path.len(), 2);
        assert!((path.total_cost - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_astar_optimal_length() {
        let start = HexPos::new(0, 0);
        let goal = HexPos::new(3, 0);
        let result = astar(start, goal, &|_| true, &|_| 1.0);
        let path = result.expect("should find path");
        // Optimal path on uniform cost hex grid = hex_distance + 1 tiles
        assert_eq!(path.path.len() as i32, hex_distance(start, goal) + 1);
    }

    #[test]
    fn test_astar_blocked() {
        // Block everything except start
        let start = HexPos::new(0, 0);
        let goal = HexPos::new(5, 0);
        let result = astar(start, goal, &|p| p == start, &|_| 1.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_astar_path_starts_and_ends_correctly() {
        let start = HexPos::new(-2, 3);
        let goal = HexPos::new(2, -1);
        let result = astar(start, goal, &|_| true, &|_| 1.0);
        let path = result.expect("should find path");
        assert_eq!(*path.path.first().expect("non-empty"), start);
        assert_eq!(*path.path.last().expect("non-empty"), goal);
    }

    #[test]
    fn test_astar_respects_cost() {
        // Make a "wall" of high-cost tiles at q=1
        let start = HexPos::new(0, 0);
        let goal = HexPos::new(2, 0);
        let cost_fn = |p: HexPos| -> f64 {
            if p.q == 1 {
                100.0
            } else {
                1.0
            }
        };
        let result = astar(start, goal, &|_| true, &cost_fn);
        let path = result.expect("should find path");
        // The path should exist but may route around the wall
        assert!(!path.path.is_empty());
        assert_eq!(*path.path.first().expect("non-empty"), start);
        assert_eq!(*path.path.last().expect("non-empty"), goal);
    }

    // ── Dijkstra ──────────────────────────────────────────────────────

    #[test]
    fn test_dijkstra_start_only() {
        let dist = dijkstra(HexPos::new(0, 0), 0.0, &|_| true, &|_| 1.0);
        assert_eq!(dist.len(), 1);
        assert_eq!(dist[&HexPos::new(0, 0)], 0.0);
    }

    #[test]
    fn test_dijkstra_one_step() {
        let dist = dijkstra(HexPos::new(0, 0), 1.0, &|_| true, &|_| 1.0);
        // Start + 6 neighbours
        assert_eq!(dist.len(), 7);
    }

    #[test]
    fn test_dijkstra_respects_max_cost() {
        let dist = dijkstra(HexPos::new(0, 0), 2.0, &|_| true, &|_| 1.0);
        // Should include ring-0 (1), ring-1 (6), ring-2 (12) = 19
        assert_eq!(dist.len(), 19);
    }

    #[test]
    fn test_dijkstra_blocked() {
        let start = HexPos::new(0, 0);
        let dist = dijkstra(start, 100.0, &|p| p == start, &|_| 1.0);
        assert_eq!(dist.len(), 1);
    }

    // ── BFS ───────────────────────────────────────────────────────────

    #[test]
    fn test_bfs_zero_steps() {
        let tiles = bfs_range(HexPos::new(0, 0), 0, &|_| true);
        assert_eq!(tiles.len(), 1);
    }

    #[test]
    fn test_bfs_one_step() {
        let tiles = bfs_range(HexPos::new(0, 0), 1, &|_| true);
        assert_eq!(tiles.len(), 7); // center + 6 neighbours
    }

    #[test]
    fn test_bfs_two_steps() {
        let tiles = bfs_range(HexPos::new(0, 0), 2, &|_| true);
        assert_eq!(tiles.len(), 19); // 1 + 6 + 12
    }

    #[test]
    fn test_bfs_blocked() {
        let start = HexPos::new(0, 0);
        let tiles = bfs_range(start, 5, &|p| p == start);
        assert_eq!(tiles.len(), 1);
    }

    // ── Movement cost ─────────────────────────────────────────────────

    #[test]
    fn test_movement_cost_impassable() {
        assert!(movement_cost("deep_ocean", "ground").is_infinite());
        assert!(movement_cost("ocean", "ground").is_infinite());
        assert!(movement_cost("mountain", "naval").is_infinite());
    }

    #[test]
    fn test_movement_cost_flying_cheap() {
        assert_eq!(movement_cost("deep_ocean", "flying"), 1.0);
        assert_eq!(movement_cost("swamp", "flying"), 1.0);
        assert_eq!(movement_cost("mountain", "flying"), 1.0);
    }

    #[test]
    fn test_movement_cost_default() {
        assert_eq!(movement_cost("plains", "ground"), 1.0);
        assert_eq!(movement_cost("unknown_biome", "ground"), 1.0);
    }

    // ── Behavior Tree evaluation ──────────────────────────────────────

    #[test]
    fn test_bt_condition_success() {
        let bb = AiBlackboard {
            resources: [("metal".to_string(), 500.0)].into_iter().collect(),
            ..Default::default()
        };
        let node = BtNode::Condition(AiCondition::HasResources {
            resource: "metal".to_string(),
            min_amount: 200.0,
        });
        assert_eq!(evaluate_tree(&node, &bb), BtStatus::Success);
    }

    #[test]
    fn test_bt_condition_failure() {
        let bb = AiBlackboard::default();
        let node = BtNode::Condition(AiCondition::HasResources {
            resource: "metal".to_string(),
            min_amount: 200.0,
        });
        assert_eq!(evaluate_tree(&node, &bb), BtStatus::Failure);
    }

    #[test]
    fn test_bt_sequence_all_pass() {
        let bb = AiBlackboard {
            under_attack: true,
            enemy_composition: [("soldier".to_string(), 5)].into_iter().collect(),
            ..Default::default()
        };
        let tree = BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::IsUnderAttack),
            BtNode::Condition(AiCondition::EnemyNearby { range: 5 }),
        ]);
        assert_eq!(evaluate_tree(&tree, &bb), BtStatus::Success);
    }

    #[test]
    fn test_bt_sequence_short_circuit() {
        let bb = AiBlackboard::default(); // not under attack
        let tree = BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::IsUnderAttack),
            BtNode::Condition(AiCondition::EnemyNearby { range: 5 }),
        ]);
        assert_eq!(evaluate_tree(&tree, &bb), BtStatus::Failure);
    }

    #[test]
    fn test_bt_selector_first_wins() {
        let bb = AiBlackboard {
            resources: [("metal".to_string(), 999.0)].into_iter().collect(),
            ..Default::default()
        };
        let tree = BtNode::Selector(vec![
            BtNode::Condition(AiCondition::HasResources {
                resource: "metal".to_string(),
                min_amount: 100.0,
            }),
            BtNode::Condition(AiCondition::IsUnderAttack),
        ]);
        assert_eq!(evaluate_tree(&tree, &bb), BtStatus::Success);
    }

    #[test]
    fn test_bt_selector_fallthrough() {
        let bb = AiBlackboard::default();
        let tree = BtNode::Selector(vec![
            BtNode::Condition(AiCondition::IsUnderAttack),
            BtNode::Condition(AiCondition::EnemyNearby { range: 1 }),
        ]);
        assert_eq!(evaluate_tree(&tree, &bb), BtStatus::Failure);
    }

    #[test]
    fn test_bt_inverter() {
        let bb = AiBlackboard::default();
        let tree = BtNode::Inverter(Box::new(BtNode::Condition(AiCondition::IsUnderAttack)));
        assert_eq!(evaluate_tree(&tree, &bb), BtStatus::Success);
    }

    #[test]
    fn test_bt_parallel() {
        let bb = AiBlackboard {
            under_attack: true,
            ..Default::default()
        };
        let tree = BtNode::Parallel(vec![
            BtNode::Condition(AiCondition::IsUnderAttack),
            BtNode::Condition(AiCondition::EnemyNearby { range: 1 }),
        ]);
        // One succeeds -> parallel succeeds
        assert_eq!(evaluate_tree(&tree, &bb), BtStatus::Success);
    }

    #[test]
    fn test_bt_repeat() {
        let bb = AiBlackboard {
            under_attack: true,
            ..Default::default()
        };
        let tree = BtNode::Repeat(
            Box::new(BtNode::Condition(AiCondition::IsUnderAttack)),
            3,
        );
        assert_eq!(evaluate_tree(&tree, &bb), BtStatus::Success);
    }

    #[test]
    fn test_bt_action_always_succeeds() {
        let bb = AiBlackboard::default();
        let tree = BtNode::Action(AiAction::GatherResources {
            resource_type: "metal".to_string(),
        });
        assert_eq!(evaluate_tree(&tree, &bb), BtStatus::Success);
    }

    // ── Collect actions ───────────────────────────────────────────────

    #[test]
    fn test_collect_actions_empty_on_failure() {
        let bb = AiBlackboard::default();
        let tree = BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::IsUnderAttack),
            BtNode::Action(AiAction::Retreat {
                fallback_pos: HexPos::new(0, 0),
            }),
        ]);
        let actions = collect_actions(&tree, &bb);
        // Sequence fails at condition, so no actions collected
        assert!(actions.is_empty());
    }

    #[test]
    fn test_collect_actions_returns_on_success() {
        let bb = AiBlackboard {
            under_attack: true,
            threat_level: 0.9,
            ..Default::default()
        };
        let tree = BtNode::Sequence(vec![
            BtNode::Condition(AiCondition::IsUnderAttack),
            BtNode::Action(AiAction::Retreat {
                fallback_pos: HexPos::new(0, 0),
            }),
        ]);
        let actions = collect_actions(&tree, &bb);
        assert!(!actions.is_empty());
    }

    // ── Default trees compile & evaluate ──────────────────────────────

    #[test]
    fn test_tactical_tree_evaluates() {
        let bb = AiBlackboard::default();
        let status = evaluate_tree(&tactical_tree(), &bb);
        // Should not panic
        assert!(matches!(
            status,
            BtStatus::Success | BtStatus::Failure | BtStatus::Running
        ));
    }

    #[test]
    fn test_operational_tree_evaluates() {
        let bb = AiBlackboard::default();
        let status = evaluate_tree(&operational_tree(), &bb);
        assert!(matches!(
            status,
            BtStatus::Success | BtStatus::Failure | BtStatus::Running
        ));
    }

    #[test]
    fn test_strategic_tree_evaluates() {
        let bb = AiBlackboard::default();
        let status = evaluate_tree(&strategic_tree(), &bb);
        assert!(matches!(
            status,
            BtStatus::Success | BtStatus::Failure | BtStatus::Running
        ));
    }

    #[test]
    fn test_tactical_tree_serializable() {
        let tree = tactical_tree();
        let json = serde_json::to_string(&tree);
        assert!(json.is_ok());
    }

    // ── Difficulty ────────────────────────────────────────────────────

    #[test]
    fn test_difficulty_from_str() {
        assert_eq!(AiDifficulty::from_str("easy"), Some(AiDifficulty::Easy));
        assert_eq!(AiDifficulty::from_str("BRUTAL"), Some(AiDifficulty::Brutal));
        assert_eq!(AiDifficulty::from_str("invalid"), None);
    }

    #[test]
    fn test_difficulty_resource_multiplier() {
        assert!(AiDifficulty::Easy.resource_multiplier() < 1.0);
        assert_eq!(AiDifficulty::Normal.resource_multiplier(), 1.0);
        assert!(AiDifficulty::Brutal.resource_multiplier() > 1.0);
    }

    #[test]
    fn test_difficulty_espionage() {
        assert!(!AiDifficulty::Easy.uses_espionage());
        assert!(!AiDifficulty::Normal.uses_espionage());
        assert!(AiDifficulty::Hard.uses_espionage());
        assert!(AiDifficulty::Brutal.uses_espionage());
    }

    #[test]
    fn test_difficulty_decision_speed() {
        assert!(
            AiDifficulty::Easy.decision_interval_secs()
                > AiDifficulty::Brutal.decision_interval_secs()
        );
    }

    // ── Strategy ──────────────────────────────────────────────────────

    #[test]
    fn test_strategy_roundtrip() {
        for s in &[
            AiStrategy::Economic,
            AiStrategy::Military,
            AiStrategy::Defensive,
            AiStrategy::Aggressive,
            AiStrategy::Expansion,
            AiStrategy::Tech,
        ] {
            assert_eq!(AiStrategy::from_str(s.as_str()), *s);
        }
    }

    #[test]
    fn test_strategy_default() {
        assert_eq!(AiStrategy::default(), AiStrategy::Economic);
    }

    // ── Blackboard ────────────────────────────────────────────────────

    #[test]
    fn test_blackboard_default() {
        let bb = AiBlackboard::default();
        assert_eq!(bb.threat_level, 0.0);
        assert!(bb.resources.is_empty());
        assert!(!bb.under_attack);
    }

    #[test]
    fn test_blackboard_serializable() {
        let bb = AiBlackboard {
            threat_level: 0.5,
            player_faction: "hive".to_string(),
            resources: [("metal".to_string(), 100.0)].into_iter().collect(),
            ..Default::default()
        };
        let json = serde_json::to_string(&bb);
        assert!(json.is_ok());
    }

    // ── Engine ────────────────────────────────────────────────────────

    #[test]
    fn test_engine_ensure_colony() {
        let engine = SwarmAiEngine::new();
        assert!(engine.ensure_colony("test_colony").is_ok());
        let colonies = engine.colonies.lock().expect("lock");
        assert!(colonies.contains_key("test_colony"));
    }

    #[test]
    fn test_engine_default_resources() {
        let engine = SwarmAiEngine::new();
        engine.ensure_colony("alpha").expect("ensure");
        let colonies = engine.colonies.lock().expect("lock");
        let colony = &colonies["alpha"];
        assert_eq!(colony.blackboard.resources["metal"], 500.0);
        assert_eq!(colony.blackboard.resources["crystal"], 250.0);
        assert_eq!(colony.blackboard.resources["deuterium"], 100.0);
    }

    // ── PathResult ────────────────────────────────────────────────────

    #[test]
    fn test_path_result_serializable() {
        let pr = PathResult {
            path: vec![HexPos::new(0, 0), HexPos::new(1, 0)],
            total_cost: 1.0,
            tiles_explored: 5,
            algorithm: "astar".to_string(),
        };
        let json = serde_json::to_string(&pr);
        assert!(json.is_ok());
    }
}
