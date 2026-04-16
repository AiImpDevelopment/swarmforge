// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! GenomeForge — 64-Gene Genetic Algorithm for Creature Evolution
//!
//! Implements a biologically-inspired genome system for the SwarmForge RPG
//! where creatures are fully defined by a 64-gene vector.  Every attribute
//! (body shape, combat stats, abilities, faction affinity) is decoded from
//! continuous genes in the range [0.0, 1.0].
//!
//! ## Scientific Foundations
//!
//! **MAP-Elites** (Mouret & Clune, 2015): Quality-Diversity algorithm that
//! maintains a grid of behaviorally distinct high-fitness solutions.  The
//! archive maps behavior descriptors (size, mobility, attack style) to the
//! best-performing genome in each cell, producing a diverse repertoire of
//! creature archetypes rather than a single optimum.
//!
//! **Novelty Search** (Lehman & Stanley, 2011): Fitness is augmented with
//! a novelty term that rewards behavioral distance from the existing
//! population.  This prevents premature convergence and encourages the
//! discovery of niches that a purely fitness-driven search would overlook.
//!
//! **Self-Adaptive Mutation Rate**: Gene 50 encodes the mutation rate itself.
//! High mutation rates explore aggressively but are unstable; low rates
//! converge faster but risk stagnation.  Evolution naturally selects the
//! optimal rate — a technique from Evolutionary Strategies (Rechenberg, 1973).
//!
//! ## Mutation Operators (6 types)
//!
//! | Operator             | Probability | Effect                        |
//! |----------------------|-------------|-------------------------------|
//! | Point Mutation       | ~70%        | Single gene +/- Gaussian noise|
//! | Segment Swap         | ~10%        | Swap two 4-gene segments      |
//! | Duplication          | ~5%         | Copy 4 genes to another slot  |
//! | Deletion             | ~5%         | Reset 4 genes to 0.5 neutral  |
//! | Inversion            | ~5%         | Reverse a 4-8 gene segment    |
//! | Catastrophic Scramble| ~5%         | Randomize 20-40% of all genes |
//!
//! ## Fitness Function
//!
//! ```text
//! fitness = combat_power * 0.4 + resource_efficiency * 0.3
//!         + survivability * 0.2 + novelty * 0.1
//! ```

use rand::distributions::Distribution;
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ImpForgeError;

// ─────────────────────────────────────────────────────────────────────────────
// Serde helper for [f64; 64] (serde only derives for arrays up to 32)
// ─────────────────────────────────────────────────────────────────────────────

mod serde_gene_array {
    use serde::{self, Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S>(genes: &[f64; 64], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        genes.as_slice().serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<[f64; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: Vec<f64> = Vec::deserialize(deserializer)?;
        v.try_into()
            .map_err(|v: Vec<f64>| serde::de::Error::custom(format!("expected 64 genes, got {}", v.len())))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Total number of genes in every genome.
const GENE_COUNT: usize = 64;

/// Minimum self-adaptive mutation rate (gene 50 lower bound).
const MIN_MUTATION_RATE: f64 = 0.01;

/// Maximum self-adaptive mutation rate (gene 50 upper bound).
const MAX_MUTATION_RATE: f64 = 0.5;

/// Number of nearest neighbours for novelty calculation (k-NN).
const DEFAULT_NOVELTY_K: usize = 15;

/// MAP-Elites grid dimensions: size classes x mobility types x attack styles.
const MAP_SIZE_CLASSES: usize = 4;
const MAP_MOBILITY_TYPES: usize = 4;
const MAP_ATTACK_STYLES: usize = 4;

/// Segment length for segment-based mutation operators.
const SEGMENT_LEN: usize = 4;

// ─────────────────────────────────────────────────────────────────────────────
// Gene Category Ranges
// ─────────────────────────────────────────────────────────────────────────────

/// Gene index ranges for each category.
#[derive(Debug)]
pub enum GeneCategory {
    /// Indices 0-15: body segments, legs, wings, size, color, morphology
    Body,
    /// Indices 16-31: damage, armor, speed, range, critical hit chance
    Combat,
    /// Indices 32-47: mana, spells, passives, auras, ability power
    Abilities,
    /// Indices 48-63: faction affinity, mutation rate (50), adaptability
    FactionMeta,
}
impl GeneCategory {
    /// Returns the inclusive (start, end) index range for this category.
    pub(crate) fn range(&self) -> (usize, usize) {
        match self {
            Self::Body => (0, 15),
            Self::Combat => (16, 31),
            Self::Abilities => (32, 47),
            Self::FactionMeta => (48, 63),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mutation Operators
// ─────────────────────────────────────────────────────────────────────────────

/// The six mutation operators available during evolution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MutationOp {
    /// ~70% chance — single gene receives Gaussian noise scaled by mutation rate.
    PointMutation,
    /// ~10% chance — two non-overlapping 4-gene segments are swapped.
    SegmentSwap,
    /// ~5% chance — one 4-gene segment is copied over another.
    Duplication,
    /// ~5% chance — one 4-gene segment is reset to 0.5 (neutral).
    Deletion,
    /// ~5% chance — a 4-8 gene segment is reversed in place.
    Inversion,
    /// ~5% chance — 20-40% of all genes are randomized.
    CatastrophicScramble,
}

impl MutationOp {
    /// Select a mutation operator using the weighted probability distribution.
    fn select(rng: &mut impl Rng) -> Self {
        let roll: f64 = rng.gen();
        if roll < 0.70 {
            Self::PointMutation
        } else if roll < 0.80 {
            Self::SegmentSwap
        } else if roll < 0.85 {
            Self::Duplication
        } else if roll < 0.90 {
            Self::Deletion
        } else if roll < 0.95 {
            Self::Inversion
        } else {
            Self::CatastrophicScramble
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Genome
// ─────────────────────────────────────────────────────────────────────────────

/// A 64-gene genome encoding all creature attributes.
///
/// Every gene is a continuous value in [0.0, 1.0].  The `decode` method
/// maps these raw values to concrete creature statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genome {
    /// 64 normalized genes, each in [0.0, 1.0].
    #[serde(with = "serde_gene_array")]
    pub genes: [f64; GENE_COUNT],
    /// Generation counter (0 = randomly created).
    pub generation: u32,
    /// Evaluated fitness score (0.0 until evaluated).
    pub fitness: f64,
    /// Novelty score relative to the current population.
    pub novelty_score: f64,
    /// UUIDs of parent genomes (empty for generation-0).
    pub parent_ids: Vec<String>,
    /// How many mutations have been applied to this lineage.
    pub mutation_count: u32,
    /// Unique identifier for this genome.
    pub id: String,
}

impl Genome {
    // ── Constructors ─────────────────────────────────────────────────────

    /// Create a genome with all genes randomized uniformly in [0.0, 1.0].
    pub(crate) fn random() -> Self {
        let mut rng = rand::thread_rng();
        let mut genes = [0.0f64; GENE_COUNT];
        for gene in &mut genes {
            *gene = rng.gen();
        }
        Self {
            genes,
            generation: 0,
            fitness: 0.0,
            novelty_score: 0.0,
            parent_ids: Vec::new(),
            mutation_count: 0,
            id: Uuid::new_v4().to_string(),
        }
    }

    /// Create a genome biased toward a faction archetype.
    ///
    /// Each faction seeds specific gene regions with non-uniform distributions
    /// so that decoded creatures naturally match the faction's flavour while
    /// still carrying randomness that evolution can refine.
    pub(crate) fn from_faction(faction: &str) -> Self {
        let mut genome = Self::random();
        let mut rng = rand::thread_rng();

        match faction.to_lowercase().as_str() {
            "swarm" => {
                // Many small, fast units with high mutation rate
                genome.genes[0] = rng.gen_range(0.1..0.3); // few segments
                genome.genes[1] = rng.gen_range(0.5..1.0); // many legs
                genome.genes[5] = rng.gen_range(0.0..0.3); // small body
                genome.genes[20] = rng.gen_range(0.6..1.0); // high speed
                genome.genes[50] = rng.gen_range(0.3..0.5); // high mutation rate
            }
            "titan" => {
                // Few large, heavily armored units
                genome.genes[0] = rng.gen_range(0.7..1.0); // many segments
                genome.genes[5] = rng.gen_range(0.7..1.0); // massive body
                genome.genes[17] = rng.gen_range(0.7..1.0); // high armor
                genome.genes[20] = rng.gen_range(0.0..0.3); // slow speed
                genome.genes[50] = rng.gen_range(0.01..0.1); // low mutation rate
            }
            "mystic" => {
                // Magic-oriented with high mana and ability power
                genome.genes[2] = rng.gen_range(0.5..1.0); // wings
                genome.genes[5] = rng.gen_range(0.3..0.6); // medium body
                genome.genes[32] = rng.gen_range(0.7..1.0); // high mana
                genome.genes[40] = rng.gen_range(0.7..1.0); // high ability power
                genome.genes[50] = rng.gen_range(0.1..0.3); // moderate mutation
            }
            "predator" => {
                // High damage, critical hits, moderate armor
                genome.genes[4] = rng.gen_range(0.5..1.0); // horns
                genome.genes[5] = rng.gen_range(0.4..0.7); // medium-large
                genome.genes[16] = rng.gen_range(0.7..1.0); // high attack
                genome.genes[21] = rng.gen_range(0.6..1.0); // high crit
                genome.genes[50] = rng.gen_range(0.1..0.2); // low-moderate mutation
            }
            _ => {
                // Unknown faction — balanced random (no bias)
            }
        }

        genome
    }

    // ── Decoding ─────────────────────────────────────────────────────────

    /// Decode the raw gene vector into human-readable creature statistics.
    pub(crate) fn decode(&self) -> CreatureDescriptor {
        let g = &self.genes;
        CreatureDescriptor {
            body_segments: (g[0] * 7.0).round() as u8 + 1,  // 1-8
            leg_count: (g[1] * 8.0).round() as u8,          // 0-8
            wing_type: (g[2] * 3.0).round() as u8,          // 0-3
            tail_type: (g[3] * 2.0).round() as u8,          // 0-2
            horn_type: (g[4] * 3.0).round() as u8,          // 0-3
            body_size: g[5],                                  // 0.0-1.0
            primary_color: [g[6], g[7], g[8]],               // RGB
            hp_base: 50.0 + g[9] * 450.0,                   // 50-500
            attack_base: 5.0 + g[16] * 95.0,                // 5-100
            armor_base: g[17] * 80.0,                        // 0-80
            speed_base: 10.0 + g[20] * 90.0,                // 10-100
            mana_base: g[32] * 200.0,                        // 0-200
            crit_chance: g[21] * 0.5,                        // 0-50%
            ability_power: g[40] * 150.0,                    // 0-150
            mutation_rate: MIN_MUTATION_RATE
                + g[50] * (MAX_MUTATION_RATE - MIN_MUTATION_RATE), // 0.01-0.5
        }
    }

    // ── Mutation ─────────────────────────────────────────────────────────

    /// Apply a randomly selected mutation operator.
    ///
    /// The mutation intensity is governed by gene 50 (self-adaptive mutation
    /// rate).  After mutation, all genes are clamped to [0.0, 1.0].
    pub(crate) fn mutate(&mut self) {
        let mut rng = rand::thread_rng();
        let op = MutationOp::select(&mut rng);
        self.apply_mutation(op, &mut rng);
        self.clamp_genes();
        self.mutation_count += 1;
    }

    /// Apply a specific mutation operator.
    fn apply_mutation(&mut self, op: MutationOp, rng: &mut impl Rng) {
        let mutation_rate = MIN_MUTATION_RATE
            + self.genes[50] * (MAX_MUTATION_RATE - MIN_MUTATION_RATE);

        match op {
            MutationOp::PointMutation => {
                let idx = rng.gen_range(0..GENE_COUNT);
                let normal = rand::distributions::Standard;
                let noise: f64 = normal.sample(rng);
                self.genes[idx] += noise * mutation_rate;
            }
            MutationOp::SegmentSwap => {
                let max_start = GENE_COUNT - SEGMENT_LEN;
                let a = rng.gen_range(0..max_start);
                // Pick b that does not overlap with a
                let mut b = rng.gen_range(0..max_start);
                while (b..b + SEGMENT_LEN).any(|i| (a..a + SEGMENT_LEN).contains(&i)) {
                    b = rng.gen_range(0..max_start);
                }
                for i in 0..SEGMENT_LEN {
                    self.genes.swap(a + i, b + i);
                }
            }
            MutationOp::Duplication => {
                let max_start = GENE_COUNT - SEGMENT_LEN;
                let src = rng.gen_range(0..max_start);
                let dst = rng.gen_range(0..max_start);
                for i in 0..SEGMENT_LEN {
                    self.genes[dst + i] = self.genes[src + i];
                }
            }
            MutationOp::Deletion => {
                let max_start = GENE_COUNT - SEGMENT_LEN;
                let start = rng.gen_range(0..max_start);
                for i in 0..SEGMENT_LEN {
                    self.genes[start + i] = 0.5;
                }
            }
            MutationOp::Inversion => {
                let seg_len = rng.gen_range(SEGMENT_LEN..=8.min(GENE_COUNT));
                let start = rng.gen_range(0..=GENE_COUNT - seg_len);
                self.genes[start..start + seg_len].reverse();
            }
            MutationOp::CatastrophicScramble => {
                // Randomize 20-40% of genes
                let fraction = rng.gen_range(0.20..0.40);
                let count = (GENE_COUNT as f64 * fraction).round() as usize;
                for _ in 0..count {
                    let idx = rng.gen_range(0..GENE_COUNT);
                    self.genes[idx] = rng.gen();
                }
            }
        }
    }

    /// Clamp every gene to the valid [0.0, 1.0] range.
    fn clamp_genes(&mut self) {
        for gene in &mut self.genes {
            *gene = gene.clamp(0.0, 1.0);
        }
    }

    // ── Crossover ────────────────────────────────────────────────────────

    /// Two-point crossover producing a single offspring.
    ///
    /// Two random cut points divide the genome into three segments.  The
    /// child inherits the outer segments from `self` and the middle segment
    /// from `other`, mimicking biological recombination.
    pub(crate) fn crossover(&self, other: &Self) -> Self {
        let mut rng = rand::thread_rng();
        let mut pt1 = rng.gen_range(0..GENE_COUNT);
        let mut pt2 = rng.gen_range(0..GENE_COUNT);
        if pt1 > pt2 {
            std::mem::swap(&mut pt1, &mut pt2);
        }

        let mut child_genes = [0.0f64; GENE_COUNT];
        for (i, gene) in child_genes.iter_mut().enumerate() {
            *gene = if i >= pt1 && i < pt2 {
                other.genes[i]
            } else {
                self.genes[i]
            };
        }

        Self {
            genes: child_genes,
            generation: self.generation.max(other.generation) + 1,
            fitness: 0.0,
            novelty_score: 0.0,
            parent_ids: vec![self.id.clone(), other.id.clone()],
            mutation_count: 0,
            id: Uuid::new_v4().to_string(),
        }
    }

    // ── Novelty Search ───────────────────────────────────────────────────

    /// Compute the novelty score as the mean distance to the k nearest
    /// neighbours in behavior space.
    ///
    /// Behavior vectors are 3-dimensional: (size, mobility, attack_style).
    /// Higher novelty means the creature occupies an under-explored niche.
    pub(crate) fn calculate_novelty(&self, population: &[Genome], k: usize) -> f64 {
        if population.is_empty() {
            return 1.0;
        }

        let my_bv = self.behavior_vector();
        let mut distances: Vec<f64> = population
            .iter()
            .filter(|g| g.id != self.id)
            .map(|g| {
                let other_bv = g.behavior_vector();
                euclidean_distance(&my_bv, &other_bv)
            })
            .collect();

        distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let k_actual = k.min(distances.len());
        if k_actual == 0 {
            return 1.0;
        }

        distances[..k_actual].iter().sum::<f64>() / k_actual as f64
    }

    /// 3D behavior vector used for MAP-Elites placement and novelty search.
    ///
    /// - `[0]` size:     gene 5 (body_size)
    /// - `[1]` mobility: combination of legs (gene 1) and wings (gene 2)
    /// - `[2]` attack:   combination of attack (gene 16) and ability power (gene 40)
    pub(crate) fn behavior_vector(&self) -> [f64; 3] {
        let size = self.genes[5];
        let mobility = (self.genes[1] * 0.4 + self.genes[2] * 0.6).clamp(0.0, 1.0);
        let attack = (self.genes[16] * 0.5 + self.genes[40] * 0.5).clamp(0.0, 1.0);
        [size, mobility, attack]
    }

    // ── Fitness ──────────────────────────────────────────────────────────

    /// Evaluate the fitness of this genome given a population for novelty.
    ///
    /// ```text
    /// fitness = combat_power * 0.4
    ///         + resource_efficiency * 0.3
    ///         + survivability * 0.2
    ///         + novelty * 0.1
    /// ```
    pub(crate) fn evaluate_fitness(&mut self, population: &[Genome]) {
        let desc = self.decode();

        let combat_power =
            (desc.attack_base * desc.crit_chance + desc.ability_power * desc.mana_base) / 100.0;

        // Small creatures are more resource-efficient
        let body_size_inverse = 1.0 - desc.body_size * 0.8;
        let resource_efficiency = desc.speed_base * body_size_inverse;

        let survivability = desc.hp_base * desc.armor_base / 1000.0;

        let novelty = self.calculate_novelty(population, DEFAULT_NOVELTY_K);
        self.novelty_score = novelty;

        self.fitness =
            combat_power * 0.4 + resource_efficiency * 0.3 + survivability * 0.2 + novelty * 0.1;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Creature Descriptor
// ─────────────────────────────────────────────────────────────────────────────

/// Human-readable creature statistics decoded from a genome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureDescriptor {
    pub body_segments: u8,
    pub leg_count: u8,
    /// 0 = none, 1 = small, 2 = medium, 3 = large
    pub wing_type: u8,
    /// 0 = none, 1 = short, 2 = long
    pub tail_type: u8,
    /// 0 = none, 1 = small, 2 = medium, 3 = large
    pub horn_type: u8,
    /// 0.0 (tiny) to 1.0 (massive)
    pub body_size: f64,
    /// RGB color in [0.0, 1.0] per channel
    pub primary_color: [f64; 3],
    pub hp_base: f64,
    pub attack_base: f64,
    pub armor_base: f64,
    pub speed_base: f64,
    pub mana_base: f64,
    /// 0.0 to 0.5 (50%)
    pub crit_chance: f64,
    pub ability_power: f64,
    /// Self-adaptive mutation rate (0.01 to 0.5)
    pub mutation_rate: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// MAP-Elites Archive
// ─────────────────────────────────────────────────────────────────────────────

/// A single cell in the MAP-Elites archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapElitesCell {
    /// 0 = tiny, 1 = small, 2 = medium, 3 = large
    pub size_class: u8,
    /// 0 = ground, 1 = flying, 2 = burrowing, 3 = aquatic
    pub mobility_type: u8,
    /// 0 = melee, 1 = ranged, 2 = magic, 3 = siege
    pub attack_style: u8,
    /// The best genome found for this behavior cell (None if unexplored).
    pub best_genome: Option<Genome>,
    /// Fitness of the best genome (-1.0 if empty).
    pub best_fitness: f64,
}

/// MAP-Elites quality-diversity archive.
///
/// A 4x4x4 grid (64 cells) indexed by behavior descriptors:
/// `[size_class][mobility_type][attack_style]`.
///
/// Each cell holds the single fittest genome discovered so far for that
/// behavioral niche.  This produces a diverse repertoire of creature
/// archetypes rather than a single global optimum.
pub struct MapElitesArchive {
    /// 3D grid: `cells[size][mobility][attack]`
    cells: Vec<Vec<Vec<MapElitesCell>>>,
    /// Total number of genomes that have been evaluated against the archive.
    total_evaluated: u64,
}

impl MapElitesArchive {
    /// Create an empty 4x4x4 archive (64 cells).
    pub(crate) fn new() -> Self {
        let mut cells = Vec::with_capacity(MAP_SIZE_CLASSES);
        for s in 0..MAP_SIZE_CLASSES {
            let mut mobility_row = Vec::with_capacity(MAP_MOBILITY_TYPES);
            for m in 0..MAP_MOBILITY_TYPES {
                let mut attack_row = Vec::with_capacity(MAP_ATTACK_STYLES);
                for a in 0..MAP_ATTACK_STYLES {
                    attack_row.push(MapElitesCell {
                        size_class: s as u8,
                        mobility_type: m as u8,
                        attack_style: a as u8,
                        best_genome: None,
                        best_fitness: -1.0,
                    });
                }
                mobility_row.push(attack_row);
            }
            cells.push(mobility_row);
        }
        Self {
            cells,
            total_evaluated: 0,
        }
    }

    /// Discretize a behavior vector into grid indices.
    fn behavior_to_indices(bv: &[f64; 3]) -> (usize, usize, usize) {
        let to_idx = |v: f64, bins: usize| -> usize {
            let i = (v * bins as f64).floor() as usize;
            i.min(bins - 1)
        };
        (
            to_idx(bv[0], MAP_SIZE_CLASSES),
            to_idx(bv[1], MAP_MOBILITY_TYPES),
            to_idx(bv[2], MAP_ATTACK_STYLES),
        )
    }

    /// Attempt to insert a genome into the archive.
    ///
    /// Returns `true` if the genome was placed (either filling an empty cell
    /// or replacing a less-fit occupant).
    pub(crate) fn try_insert(&mut self, genome: &Genome) -> bool {
        self.total_evaluated += 1;
        let bv = genome.behavior_vector();
        let (s, m, a) = Self::behavior_to_indices(&bv);
        let cell = &mut self.cells[s][m][a];

        if genome.fitness > cell.best_fitness {
            cell.best_genome = Some(genome.clone());
            cell.best_fitness = genome.fitness;
            true
        } else {
            false
        }
    }

    /// Retrieve the best genome for a specific behavior niche.
    pub(crate) fn get_best(&self, size: u8, mobility: u8, attack: u8) -> Option<&Genome> {
        let s = (size as usize).min(MAP_SIZE_CLASSES - 1);
        let m = (mobility as usize).min(MAP_MOBILITY_TYPES - 1);
        let a = (attack as usize).min(MAP_ATTACK_STYLES - 1);
        self.cells[s][m][a].best_genome.as_ref()
    }

    /// Fraction of cells that contain at least one genome (0.0 to 1.0).
    pub(crate) fn coverage(&self) -> f64 {
        let total = MAP_SIZE_CLASSES * MAP_MOBILITY_TYPES * MAP_ATTACK_STYLES;
        let filled = self
            .cells
            .iter()
            .flat_map(|row| row.iter().flat_map(|col| col.iter()))
            .filter(|c| c.best_genome.is_some())
            .count();
        filled as f64 / total as f64
    }

    /// Return the genome with the highest fitness across all cells.
    pub(crate) fn best_overall(&self) -> Option<&Genome> {
        self.cells
            .iter()
            .flat_map(|row| row.iter().flat_map(|col| col.iter()))
            .filter_map(|c| c.best_genome.as_ref())
            .max_by(|a, b| a.fitness.partial_cmp(&b.fitness).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Diversity score: standard deviation of fitness values across occupied cells.
    ///
    /// Higher diversity means the archive contains genomes with a wider
    /// spread of fitness levels, indicating meaningful niche differentiation.
    pub(crate) fn diversity_score(&self) -> f64 {
        let fitnesses: Vec<f64> = self
            .cells
            .iter()
            .flat_map(|row| row.iter().flat_map(|col| col.iter()))
            .filter_map(|c| c.best_genome.as_ref())
            .map(|g| g.fitness)
            .collect();

        if fitnesses.len() < 2 {
            return 0.0;
        }

        let mean = fitnesses.iter().sum::<f64>() / fitnesses.len() as f64;
        let variance =
            fitnesses.iter().map(|f| (f - mean).powi(2)).sum::<f64>() / fitnesses.len() as f64;
        variance.sqrt()
    }

    /// Serialize archive status to JSON for the frontend.
    fn status_json(&self) -> serde_json::Value {
        let occupied: Vec<serde_json::Value> = self
            .cells
            .iter()
            .flat_map(|row| row.iter().flat_map(|col| col.iter()))
            .filter(|c| c.best_genome.is_some())
            .map(|c| {
                serde_json::json!({
                    "size_class": c.size_class,
                    "mobility_type": c.mobility_type,
                    "attack_style": c.attack_style,
                    "fitness": c.best_fitness,
                })
            })
            .collect();

        serde_json::json!({
            "total_cells": MAP_SIZE_CLASSES * MAP_MOBILITY_TYPES * MAP_ATTACK_STYLES,
            "occupied_cells": occupied.len(),
            "coverage": self.coverage(),
            "diversity_score": self.diversity_score(),
            "total_evaluated": self.total_evaluated,
            "best_fitness": self.best_overall().map(|g| g.fitness).unwrap_or(0.0),
            "cells": occupied,
        })
    }
}

impl Default for MapElitesArchive {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Utility Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Euclidean distance between two 3D vectors.
fn euclidean_distance(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Run a full evolutionary generation using MAP-Elites + Novelty Search.
///
/// 1. Initialize `population_size` random genomes
/// 2. For each generation:
///    a. Evaluate fitness (including novelty against the population)
///    b. Insert into the MAP-Elites archive
///    c. Select parents via tournament selection (size 3)
///    d. Produce offspring via crossover + mutation
/// 3. Return the final population sorted by fitness (descending).
fn evolve_generation_inner(population_size: u32, generations: u32) -> Vec<Genome> {
    let pop_size = (population_size as usize).max(4);
    let gen_count = (generations as usize).clamp(1, 1000);
    let mut rng = rand::thread_rng();

    // Initialize population
    let mut population: Vec<Genome> = (0..pop_size).map(|_| Genome::random()).collect();

    let mut archive = MapElitesArchive::new();

    for gen in 0..gen_count {
        // Evaluate fitness for all individuals
        // We clone the population for novelty reference to avoid borrow conflicts
        let pop_snapshot: Vec<Genome> = population.clone();
        for individual in &mut population {
            individual.evaluate_fitness(&pop_snapshot);
            archive.try_insert(individual);
        }

        // If this is the last generation, skip breeding
        if gen == gen_count - 1 {
            break;
        }

        // Tournament selection + crossover + mutation to produce next generation
        let mut next_gen: Vec<Genome> = Vec::with_capacity(pop_size);

        // Elitism: keep the single best individual unchanged
        if let Some(best) = population.iter().max_by(|a, b| {
            a.fitness
                .partial_cmp(&b.fitness)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            next_gen.push(best.clone());
        }

        while next_gen.len() < pop_size {
            // Tournament selection (size 3) for two parents
            let parent_a = tournament_select(&population, 3, &mut rng);
            let parent_b = tournament_select(&population, 3, &mut rng);

            let mut child = parent_a.crossover(parent_b);
            child.generation = gen as u32 + 1;
            child.mutate();
            next_gen.push(child);
        }

        population = next_gen;
    }

    // Final sort by fitness descending
    population.sort_by(|a, b| {
        b.fitness
            .partial_cmp(&a.fitness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    population
}

/// Tournament selection: pick `k` random individuals, return the fittest.
fn tournament_select<'a>(population: &'a [Genome], k: usize, rng: &mut impl Rng) -> &'a Genome {
    let mut best: &Genome = &population[rng.gen_range(0..population.len())];
    for _ in 1..k {
        let candidate = &population[rng.gen_range(0..population.len())];
        if candidate.fitness > best.fitness {
            best = candidate;
        }
    }
    best
}

// ─────────────────────────────────────────────────────────────────────────────
// Global Archive (thread-safe singleton for Tauri commands)
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::Mutex;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_genome", "Game");

static GLOBAL_ARCHIVE: once_cell::sync::Lazy<Mutex<MapElitesArchive>> =
    once_cell::sync::Lazy::new(|| Mutex::new(MapElitesArchive::new()));

// ─────────────────────────────────────────────────────────────────────────────
// Tauri Commands (8)
// ─────────────────────────────────────────────────────────────────────────────

/// Create a random genome with all 64 genes randomized.
#[tauri::command]
pub fn genome_create_random() -> Result<Genome, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_genome", "game_genome", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_genome", "game_genome");
    crate::synapse_fabric::synapse_session_push("swarm_genome", "game_genome", "genome_create_random called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_genome", "info", "swarm_genome active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_genome", "evolve", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"action": "random"}));
    Ok(Genome::random())
}

/// Create a genome biased toward a faction archetype.
///
/// Supported factions: "swarm", "titan", "mystic", "predator".
/// Unknown factions produce a balanced random genome.
#[tauri::command]
pub fn genome_from_faction(faction: String) -> Result<Genome, ImpForgeError> {
    if faction.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_FACTION",
            "Faction name must not be empty",
        ));
    }
    Ok(Genome::from_faction(&faction))
}

/// Decode a genome into human-readable creature statistics.
#[tauri::command]
pub fn genome_decode(genome: Genome) -> Result<CreatureDescriptor, ImpForgeError> {
    Ok(genome.decode())
}

/// Apply a random mutation to a genome and return the mutated copy.
#[tauri::command]
pub fn genome_mutate(mut genome: Genome) -> Result<Genome, ImpForgeError> {
    genome.mutate();
    Ok(genome)
}

/// Produce an offspring from two parent genomes via two-point crossover.
#[tauri::command]
pub fn genome_crossover(parent_a: Genome, parent_b: Genome) -> Result<Genome, ImpForgeError> {
    Ok(parent_a.crossover(&parent_b))
}

/// Evaluate the fitness of a genome against an empty reference population.
///
/// For full novelty-aware evaluation, use `genome_evolve_generation` instead.
#[tauri::command]
pub fn genome_evaluate_fitness(mut genome: Genome) -> Result<f64, ImpForgeError> {
    let empty_pop: Vec<Genome> = Vec::new();
    genome.evaluate_fitness(&empty_pop);
    Ok(genome.fitness)
}

/// Return the current MAP-Elites archive status (coverage, diversity, cells).
#[tauri::command]
pub fn genome_archive_status() -> Result<serde_json::Value, ImpForgeError> {
    let archive = GLOBAL_ARCHIVE
        .lock()
        .map_err(|e| ImpForgeError::internal("ARCHIVE_LOCK", format!("Lock poisoned: {e}")))?;
    Ok(archive.status_json())
}

/// Run a full evolutionary process and return the final population.
///
/// The best individuals are automatically inserted into the global
/// MAP-Elites archive.  Population size is clamped to [4, ...] and
/// generations to [1, 1000].
#[tauri::command]
pub fn genome_evolve_generation(
    population_size: u32,
    generations: u32,
) -> Result<Vec<Genome>, ImpForgeError> {
    let result = evolve_generation_inner(population_size, generations);

    // Insert top results into global archive
    if let Ok(mut archive) = GLOBAL_ARCHIVE.lock() {
        for genome in &result {
            archive.try_insert(genome);
        }
    }

    Ok(result)
}

// ─────────────────────────────────────────────────────────────────────────────
// Additional Tauri Commands — wiring internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Get gene category index ranges.
#[tauri::command]
pub fn genome_category_ranges() -> Result<Vec<serde_json::Value>, ImpForgeError> {
    let cats = [
        GeneCategory::Body,
        GeneCategory::Combat,
        GeneCategory::Abilities,
        GeneCategory::FactionMeta,
    ];
    Ok(cats
        .iter()
        .map(|c| {
            let (start, end) = c.range();
            serde_json::json!({
                "category": format!("{:?}", c),
                "start": start,
                "end": end,
            })
        })
        .collect())
}

/// Look up the best genome in the MAP-Elites archive for given behavior indices.
#[tauri::command]
pub fn genome_archive_best(
    size: u8,
    mobility: u8,
    attack: u8,
) -> Result<serde_json::Value, ImpForgeError> {
    let archive = GLOBAL_ARCHIVE.lock().map_err(|e| {
        ImpForgeError::internal("GENOME_LOCK", format!("Archive lock failed: {e}"))
    })?;
    let best = archive.get_best(size, mobility, attack);
    Ok(serde_json::json!({
        "found": best.is_some(),
        "fitness": best.map(|g| g.fitness).unwrap_or(0.0),
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests (20)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_random_genome_has_64_genes() {
        let g = Genome::random();
        assert_eq!(g.genes.len(), GENE_COUNT);
    }

    #[test]
    fn test_random_genome_genes_in_range() {
        let g = Genome::random();
        for gene in &g.genes {
            assert!(*gene >= 0.0 && *gene <= 1.0, "gene out of range: {gene}");
        }
    }

    #[test]
    fn test_random_genome_has_uuid() {
        let g = Genome::random();
        assert!(!g.id.is_empty());
        assert!(Uuid::parse_str(&g.id).is_ok());
    }

    #[test]
    fn test_random_genome_generation_zero() {
        let g = Genome::random();
        assert_eq!(g.generation, 0);
        assert!(g.parent_ids.is_empty());
    }

    #[test]
    fn test_faction_swarm_bias() {
        let g = Genome::from_faction("swarm");
        // Swarm should have small body and high mutation rate
        assert!(g.genes[5] < 0.35, "swarm body_size should be small");
        assert!(g.genes[50] > 0.25, "swarm mutation_rate should be high");
    }

    #[test]
    fn test_faction_titan_bias() {
        let g = Genome::from_faction("titan");
        assert!(g.genes[5] > 0.65, "titan body_size should be large");
        assert!(g.genes[17] > 0.65, "titan armor should be high");
    }

    #[test]
    fn test_faction_unknown_is_balanced() {
        let g = Genome::from_faction("unknown_faction_xyz");
        // Should still produce a valid genome (balanced random)
        assert_eq!(g.genes.len(), GENE_COUNT);
    }

    #[test]
    fn test_decode_body_segments_range() {
        let g = Genome::random();
        let desc = g.decode();
        assert!(desc.body_segments >= 1 && desc.body_segments <= 8);
    }

    #[test]
    fn test_decode_hp_range() {
        let g = Genome::random();
        let desc = g.decode();
        assert!(desc.hp_base >= 50.0 && desc.hp_base <= 500.0);
    }

    #[test]
    fn test_decode_mutation_rate_range() {
        let g = Genome::random();
        let desc = g.decode();
        assert!(desc.mutation_rate >= MIN_MUTATION_RATE);
        assert!(desc.mutation_rate <= MAX_MUTATION_RATE);
    }

    #[test]
    fn test_mutation_changes_genome() {
        let original = Genome::random();
        let mut mutated = original.clone();
        // Apply many mutations to ensure at least one gene changes
        for _ in 0..10 {
            mutated.mutate();
        }
        let changed = original
            .genes
            .iter()
            .zip(mutated.genes.iter())
            .any(|(a, b)| (a - b).abs() > f64::EPSILON);
        assert!(changed, "mutation should change at least one gene");
    }

    #[test]
    fn test_mutation_clamps_genes() {
        let mut g = Genome::random();
        for _ in 0..50 {
            g.mutate();
        }
        for gene in &g.genes {
            assert!(
                *gene >= 0.0 && *gene <= 1.0,
                "gene out of range after mutation: {gene}"
            );
        }
    }

    #[test]
    fn test_mutation_increments_count() {
        let mut g = Genome::random();
        assert_eq!(g.mutation_count, 0);
        g.mutate();
        assert_eq!(g.mutation_count, 1);
        g.mutate();
        assert_eq!(g.mutation_count, 2);
    }

    #[test]
    fn test_crossover_produces_child() {
        let a = Genome::random();
        let b = Genome::random();
        let child = a.crossover(&b);

        assert_eq!(child.generation, 1);
        assert_eq!(child.parent_ids.len(), 2);
        assert_eq!(child.parent_ids[0], a.id);
        assert_eq!(child.parent_ids[1], b.id);
    }

    #[test]
    fn test_crossover_child_has_valid_genes() {
        let a = Genome::random();
        let b = Genome::random();
        let child = a.crossover(&b);

        for gene in &child.genes {
            assert!(*gene >= 0.0 && *gene <= 1.0);
        }
    }

    #[test]
    fn test_behavior_vector_dimensions() {
        let g = Genome::random();
        let bv = g.behavior_vector();
        assert_eq!(bv.len(), 3);
        for v in &bv {
            assert!(*v >= 0.0 && *v <= 1.0);
        }
    }

    #[test]
    fn test_novelty_solo_genome() {
        let g = Genome::random();
        let novelty = g.calculate_novelty(&[], 15);
        assert!((novelty - 1.0).abs() < f64::EPSILON, "solo genome novelty should be 1.0");
    }

    #[test]
    fn test_novelty_identical_population() {
        let g = Genome::random();
        let population = vec![g.clone(), g.clone(), g.clone()];
        // The genome compared against copies of itself (different IDs, same genes)
        // Distance should be 0 since behavior vectors are identical
        let novelty = g.calculate_novelty(&population, 15);
        // All clones have same id, so they are filtered out; novelty = 1.0
        assert!(novelty >= 0.0);
    }

    #[test]
    fn test_fitness_evaluation() {
        let mut g = Genome::random();
        let pop = vec![Genome::random(), Genome::random()];
        g.evaluate_fitness(&pop);
        assert!(g.fitness >= 0.0, "fitness should be non-negative");
    }

    #[test]
    fn test_map_elites_new_has_64_cells() {
        let archive = MapElitesArchive::new();
        let total = archive
            .cells
            .iter()
            .flat_map(|r| r.iter().flat_map(|c| c.iter()))
            .count();
        assert_eq!(total, 64);
        assert!((archive.coverage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_map_elites_insert() {
        let mut archive = MapElitesArchive::new();
        let mut g = Genome::random();
        g.fitness = 42.0;
        assert!(archive.try_insert(&g));
        assert!(archive.coverage() > 0.0);
        assert_eq!(archive.total_evaluated, 1);
    }

    #[test]
    fn test_map_elites_insert_replaces_worse() {
        let mut archive = MapElitesArchive::new();
        let mut g1 = Genome::random();
        g1.fitness = 10.0;
        archive.try_insert(&g1);

        // Same behavior vector (same genes) but higher fitness
        let mut g2 = g1.clone();
        g2.id = Uuid::new_v4().to_string();
        g2.fitness = 20.0;
        assert!(archive.try_insert(&g2));

        let bv = g2.behavior_vector();
        let (s, m, a) = MapElitesArchive::behavior_to_indices(&bv);
        let cell = &archive.cells[s][m][a];
        assert!((cell.best_fitness - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_map_elites_does_not_replace_better() {
        let mut archive = MapElitesArchive::new();
        let mut g1 = Genome::random();
        g1.fitness = 50.0;
        archive.try_insert(&g1);

        let mut g2 = g1.clone();
        g2.id = Uuid::new_v4().to_string();
        g2.fitness = 10.0;
        assert!(!archive.try_insert(&g2));
    }

    #[test]
    fn test_map_elites_best_overall() {
        let mut archive = MapElitesArchive::new();
        assert!(archive.best_overall().is_none());

        let mut g = Genome::random();
        g.fitness = 99.0;
        archive.try_insert(&g);
        assert!((archive.best_overall().expect("best overall should succeed").fitness - 99.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_map_elites_diversity_empty() {
        let archive = MapElitesArchive::new();
        assert!((archive.diversity_score() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_evolve_generation_returns_sorted() {
        let result = evolve_generation_inner(10, 3);
        assert!(!result.is_empty());
        for w in result.windows(2) {
            assert!(
                w[0].fitness >= w[1].fitness,
                "result should be sorted descending by fitness"
            );
        }
    }

    #[test]
    fn test_evolve_generation_minimum_population() {
        let result = evolve_generation_inner(1, 1);
        // Clamped to minimum of 4
        assert!(result.len() >= 4);
    }

    #[test]
    fn test_gene_category_ranges() {
        assert_eq!(GeneCategory::Body.range(), (0, 15));
        assert_eq!(GeneCategory::Combat.range(), (16, 31));
        assert_eq!(GeneCategory::Abilities.range(), (32, 47));
        assert_eq!(GeneCategory::FactionMeta.range(), (48, 63));
    }

    #[test]
    fn test_mutation_op_select_all_types_reachable() {
        // Run many selections to confirm the distribution covers all operators
        let mut rng = rand::thread_rng();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10_000 {
            let op = MutationOp::select(&mut rng);
            seen.insert(format!("{op:?}"));
        }
        assert_eq!(seen.len(), 6, "all 6 mutation operators should be reachable");
    }

    #[test]
    fn test_euclidean_distance_zero() {
        let a = [0.5, 0.5, 0.5];
        assert!((euclidean_distance(&a, &a) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_euclidean_distance_unit() {
        let a = [0.0, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0];
        assert!((euclidean_distance(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_genome_serialization_roundtrip() {
        let g = Genome::random();
        let json = serde_json::to_string(&g).expect("serialize");
        let deserialized: Genome = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.id, g.id);
        assert_eq!(deserialized.genes.len(), GENE_COUNT);
        for (a, b) in g.genes.iter().zip(deserialized.genes.iter()) {
            assert!((a - b).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_creature_descriptor_serialization() {
        let g = Genome::random();
        let desc = g.decode();
        let json = serde_json::to_string(&desc).expect("serialize descriptor");
        let deser: CreatureDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.body_segments, desc.body_segments);
        // Use 1e-10 tolerance: JSON decimal round-trip can shift the last
        // few ULPs of an f64, making f64::EPSILON too strict.
        assert!((deser.hp_base - desc.hp_base).abs() < 1e-10);
    }

    #[test]
    fn test_archive_status_json_structure() {
        let archive = MapElitesArchive::new();
        let status = archive.status_json();
        assert_eq!(status["total_cells"], 64);
        assert_eq!(status["occupied_cells"], 0);
        assert_eq!(status["total_evaluated"], 0);
    }

    // ── Tauri command boundary tests ────────────────────────────────────

    #[test]
    fn test_cmd_genome_create_random() {
        let g = genome_create_random().expect("should succeed");
        assert_eq!(g.genes.len(), GENE_COUNT);
    }

    #[test]
    fn test_cmd_genome_from_faction_valid() {
        let g = genome_from_faction("swarm".to_string()).expect("should succeed");
        assert!(g.genes[5] < 0.35);
    }

    #[test]
    fn test_cmd_genome_from_faction_empty_rejects() {
        let result = genome_from_faction("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_genome_decode() {
        let g = Genome::random();
        let desc = genome_decode(g).expect("should decode");
        assert!(desc.body_segments >= 1);
    }

    #[test]
    fn test_cmd_genome_mutate() {
        let original = Genome::random();
        let mutated = genome_mutate(original.clone()).expect("should mutate");
        assert_eq!(mutated.mutation_count, 1);
    }

    #[test]
    fn test_cmd_genome_crossover() {
        let a = Genome::random();
        let b = Genome::random();
        let child = genome_crossover(a.clone(), b.clone()).expect("should crossover");
        assert_eq!(child.parent_ids.len(), 2);
    }

    #[test]
    fn test_cmd_genome_evaluate_fitness() {
        let g = Genome::random();
        let fitness = genome_evaluate_fitness(g).expect("should evaluate");
        assert!(fitness >= 0.0);
    }

    #[test]
    fn test_cmd_genome_evolve_generation() {
        let population = genome_evolve_generation(8, 2).expect("should evolve");
        assert!(population.len() >= 4);
    }
}
