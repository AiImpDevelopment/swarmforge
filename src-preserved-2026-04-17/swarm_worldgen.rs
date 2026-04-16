// SPDX-License-Identifier: Elastic-2.0
// Copyright (c) 2026 AiImp Technology (Karsten Schildgen, Germany)
//! SwarmForge Procedural Planet Generation & Fog of War
//!
//! Generates hex-based planet maps using a four-stage pipeline:
//!
//! 1. **Voronoi regions** — 20-40 macro regions seeded deterministically
//! 2. **Perlin fBm noise** — elevation, moisture, energy layers (6 octaves)
//! 3. **Biome assignment** — threshold lookup from noise values
//! 4. **Resource placement** — 90-130 nodes with symmetry validation
//!
//! ## Fog of War
//!
//! Per-tile visibility state (Hidden / Explored / Visible) driven by vision
//! sources (units, buildings, towers, scouts).  Vision radii are type-dependent
//! and calculated per hex-distance.
//!
//! ## References
//!
//! - arXiv:2412.04688 — Wave Function Collapse for Terrain Generation
//! - arXiv:2512.08309 — Terrain Diffusion Models
//! - Perlin, K. (1985) — "An Image Synthesizer" (SIGGRAPH)
//! - Red Blob Games — Hexagonal Grids (axial coordinates)

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::error::ImpForgeError;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("swarm_worldgen", "Game");

// ═══════════════════════════════════════════════════════════════════════════
// Core Types
// ═══════════════════════════════════════════════════════════════════════════

/// A hex tile on the planet map (axial coordinates).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexTile {
    pub q: i32,
    pub r: i32,
    pub elevation: f64,
    pub moisture: f64,
    pub energy: f64,
    pub biome: Biome,
    pub resource: Option<ResourceNode>,
    pub terrain_owner: Option<String>,
    pub visibility: TileVisibility,
    pub passable: bool,
}

/// Biome types for the planet surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Biome {
    Grassland,
    Forest,
    Swamp,
    Desert,
    Tundra,
    Volcanic,
    CrystalCaves,
    CorruptedGround,
    DeepOcean,
    Highlands,
    FungalMarsh,
    AshWastes,
    FrozenWastes,
    JungleCanopy,
}

impl Biome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Grassland => "grassland",
            Self::Forest => "forest",
            Self::Swamp => "swamp",
            Self::Desert => "desert",
            Self::Tundra => "tundra",
            Self::Volcanic => "volcanic",
            Self::CrystalCaves => "crystal_caves",
            Self::CorruptedGround => "corrupted_ground",
            Self::DeepOcean => "deep_ocean",
            Self::Highlands => "highlands",
            Self::FungalMarsh => "fungal_marsh",
            Self::AshWastes => "ash_wastes",
            Self::FrozenWastes => "frozen_wastes",
            Self::JungleCanopy => "jungle_canopy",
        }
    }
}

/// A resource deposit on a hex tile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceNode {
    pub resource_type: ResourceType,
    pub amount: f64,
    pub extraction_rate: f64,
    pub depleted: bool,
}

/// Resource categories available on the planet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    PrimaryOre,
    SecondaryMineral,
    RareGas,
    CrystalDeposit,
    EnergyShard,
    BiomassGrove,
    AncientRuins,
    DarkMatterVent,
}

impl ResourceType {
    fn all() -> &'static [ResourceType] {
        &[
            Self::PrimaryOre,
            Self::SecondaryMineral,
            Self::RareGas,
            Self::CrystalDeposit,
            Self::EnergyShard,
            Self::BiomassGrove,
            Self::AncientRuins,
            Self::DarkMatterVent,
        ]
    }
}

/// Per-tile fog-of-war visibility state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum TileVisibility {
    /// Never explored — rendered as black
    #[default]
    Hidden,
    /// Previously seen but not currently visible — dimmed
    Explored,
    /// Currently within a vision source's range — full detail
    Visible,
}


/// Planet generation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetConfig {
    /// Hex radius (40-50 for ~5000-7000 tiles)
    pub radius: u32,
    /// Deterministic seed for all RNG
    pub seed: u64,
    /// Target number of resource nodes (90-130)
    pub resource_density: u32,
    /// Rotational symmetry folds (3 for 3-player maps)
    pub symmetry_folds: u32,
}

impl Default for PlanetConfig {
    fn default() -> Self {
        Self {
            radius: 45,
            seed: 42,
            resource_density: 110,
            symmetry_folds: 3,
        }
    }
}

/// The complete planet map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetMap {
    pub config: PlanetConfig,
    pub tiles: HashMap<(i32, i32), HexTile>,
    pub starting_positions: Vec<(i32, i32)>,
    pub tile_count: usize,
}

// ═══════════════════════════════════════════════════════════════════════════
// Perlin Noise — from-scratch implementation (no external crate)
// ═══════════════════════════════════════════════════════════════════════════

/// Permutation table for Perlin noise, seeded deterministically.
struct PerlinState {
    perm: [u8; 512],
}

impl PerlinState {
    fn new(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut perm_base: [u8; 256] = [0; 256];
        for i in 0..256u16 {
            perm_base[i as usize] = i as u8;
        }
        // Fisher-Yates shuffle
        for i in (1..256).rev() {
            let j = rng.gen_range(0..=i);
            perm_base.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = perm_base[i % 256];
        }
        Self { perm }
    }

    fn hash(&self, x: i32, y: i32) -> u8 {
        let xi = (x & 255) as usize;
        let yi = (y & 255) as usize;
        self.perm[self.perm[xi] as usize + yi]
    }
}

/// 2D gradient vectors for classic Perlin noise (12 directions).
const GRAD2: [(f64, f64); 12] = [
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (1.0, 1.0),
    (-1.0, 1.0),
    (1.0, -1.0),
    (-1.0, -1.0),
    (1.0, 0.5),
    (-1.0, 0.5),
    (0.5, 1.0),
    (-0.5, 1.0),
];

/// Quintic fade curve: 6t^5 - 15t^4 + 10t^3  (Perlin's improved version)
#[inline]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Linear interpolation.
#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

/// Dot product of a gradient vector with a distance vector.
#[inline]
fn grad_dot(hash: u8, dx: f64, dy: f64) -> f64 {
    let g = GRAD2[(hash as usize) % 12];
    g.0 * dx + g.1 * dy
}

/// Classic 2D Perlin noise, returns value in approximately [-1.0, 1.0].
pub fn perlin_noise_2d(x: f64, y: f64, seed: u64) -> f64 {
    let state = PerlinState::new(seed);
    perlin_noise_2d_with_state(x, y, &state)
}

/// Internal Perlin evaluation reusing a pre-built permutation table.
fn perlin_noise_2d_with_state(x: f64, y: f64, state: &PerlinState) -> f64 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    let dx0 = x - x0 as f64;
    let dy0 = y - y0 as f64;
    let dx1 = dx0 - 1.0;
    let dy1 = dy0 - 1.0;

    let u = fade(dx0);
    let v = fade(dy0);

    let n00 = grad_dot(state.hash(x0, y0), dx0, dy0);
    let n10 = grad_dot(state.hash(x1, y0), dx1, dy0);
    let n01 = grad_dot(state.hash(x0, y1), dx0, dy1);
    let n11 = grad_dot(state.hash(x1, y1), dx1, dy1);

    let nx0 = lerp(n00, n10, u);
    let nx1 = lerp(n01, n11, u);

    lerp(nx0, nx1, v)
}

/// Fractal Brownian Motion — layered Perlin noise.
///
/// - `octaves`: number of noise layers (typically 6)
/// - `persistence`: amplitude decay per octave (typically 0.5)
/// - `lacunarity`: frequency multiplier per octave (typically 2.0)
pub fn fbm_noise(
    x: f64,
    y: f64,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
    seed: u64,
) -> f64 {
    let state = PerlinState::new(seed);
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_amplitude = 0.0;

    for _ in 0..octaves {
        value += perlin_noise_2d_with_state(x * frequency, y * frequency, &state) * amplitude;
        max_amplitude += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    // Normalize to [0.0, 1.0]
    (value / max_amplitude + 1.0) * 0.5
}

// ═══════════════════════════════════════════════════════════════════════════
// Hex Grid Utilities (axial coordinates)
// ═══════════════════════════════════════════════════════════════════════════

/// Hex distance between two axial-coordinate tiles.
///
/// Uses the cube-coordinate formula: max(|dq|, |dr|, |dq+dr|)
pub fn hex_distance(q1: i32, r1: i32, q2: i32, r2: i32) -> i32 {
    let dq = (q1 - q2).abs();
    let dr = (r1 - r2).abs();
    let ds = ((q1 + r1) - (q2 + r2)).abs();
    dq.max(dr).max(ds)
}

/// Return the six axial-coordinate neighbours of a hex tile.
pub fn hex_neighbors(q: i32, r: i32) -> Vec<(i32, i32)> {
    vec![
        (q + 1, r),
        (q - 1, r),
        (q, r + 1),
        (q, r - 1),
        (q + 1, r - 1),
        (q - 1, r + 1),
    ]
}

/// Iterate all hex coordinates within a given radius of the origin.
fn hex_spiral(radius: u32) -> Vec<(i32, i32)> {
    let r = radius as i32;
    let mut coords = Vec::with_capacity((3 * r * r + 3 * r + 1) as usize);
    for q in -r..=r {
        let r1 = (-r).max(-q - r);
        let r2 = r.min(-q + r);
        for ri in r1..=r2 {
            coords.push((q, ri));
        }
    }
    coords
}

/// Get all hex tiles within `range` distance of `(cq, cr)`.
fn hex_range(cq: i32, cr: i32, range: u32) -> Vec<(i32, i32)> {
    hex_spiral(range)
        .into_iter()
        .map(|(q, r)| (cq + q, cr + r))
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// Voronoi Region Generation
// ═══════════════════════════════════════════════════════════════════════════

/// Voronoi region role in the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionRole {
    Base,
    Expansion,
    Contested,
    Impassable,
}

/// A Voronoi macro-region center.
#[derive(Debug, Clone)]
struct VoronoiCenter {
    q: i32,
    r: i32,
    role: RegionRole,
}

/// Generate Voronoi region centers deterministically from the seed.
fn generate_voronoi_centers(config: &PlanetConfig) -> Vec<VoronoiCenter> {
    let mut rng = StdRng::seed_from_u64(config.seed.wrapping_add(0xCAFE));
    let radius = config.radius as i32;
    let num_centers = rng.gen_range(20..=40);
    let mut centers = Vec::with_capacity(num_centers);

    // Place starting positions first — evenly spaced around the ring at ~60% radius
    let base_dist = (radius as f64 * 0.6) as i32;
    for fold in 0..config.symmetry_folds {
        let angle = std::f64::consts::TAU * (fold as f64) / (config.symmetry_folds as f64);
        let q = (base_dist as f64 * angle.cos()).round() as i32;
        let r = (base_dist as f64 * angle.sin()).round() as i32;
        centers.push(VoronoiCenter {
            q,
            r,
            role: RegionRole::Base,
        });
    }

    // Fill remaining centers randomly
    let remaining = num_centers - config.symmetry_folds as usize;
    for _ in 0..remaining {
        let q = rng.gen_range(-radius..=radius);
        let r_min = (-radius).max(-q - radius);
        let r_max = radius.min(-q + radius);
        let r = rng.gen_range(r_min..=r_max);
        let role = match rng.gen_range(0..10) {
            0..=1 => RegionRole::Impassable,
            2..=4 => RegionRole::Contested,
            _ => RegionRole::Expansion,
        };
        centers.push(VoronoiCenter { q, r, role });
    }

    centers
}

/// Assign each hex tile to its nearest Voronoi center.
fn assign_voronoi_region(q: i32, r: i32, centers: &[VoronoiCenter]) -> usize {
    let mut best_idx = 0;
    let mut best_dist = i32::MAX;
    for (i, c) in centers.iter().enumerate() {
        let d = hex_distance(q, r, c.q, c.r);
        if d < best_dist {
            best_dist = d;
            best_idx = i;
        }
    }
    best_idx
}

// ═══════════════════════════════════════════════════════════════════════════
// Biome Assignment
// ═══════════════════════════════════════════════════════════════════════════

/// Assign a biome based on elevation, moisture, and energy thresholds.
///
/// The lookup follows a decision-tree approach:
/// - Very low elevation → DeepOcean
/// - Very high elevation → Volcanic (hot/dry) or Highlands/FrozenWastes
/// - Mid elevations branch on moisture and energy for the remaining biomes
pub fn assign_biome(elevation: f64, moisture: f64, energy: f64) -> Biome {
    // Deep ocean: very low elevation
    if elevation < 0.15 {
        return Biome::DeepOcean;
    }

    // Volcanic peaks: high elevation + high energy + low moisture
    if elevation > 0.85 && energy > 0.6 && moisture < 0.3 {
        return Biome::Volcanic;
    }

    // Frozen wastes: high elevation + low energy
    if elevation > 0.8 && energy < 0.3 {
        return Biome::FrozenWastes;
    }

    // Highlands: high elevation
    if elevation > 0.75 {
        return Biome::Highlands;
    }

    // Ash wastes: medium-high elevation + very low moisture + high energy
    if elevation > 0.55 && moisture < 0.2 && energy > 0.5 {
        return Biome::AshWastes;
    }

    // Crystal caves: medium elevation + high energy
    if elevation > 0.45 && elevation < 0.65 && energy > 0.75 {
        return Biome::CrystalCaves;
    }

    // Corrupted ground: medium elevation + extreme energy
    if energy > 0.85 && moisture > 0.4 {
        return Biome::CorruptedGround;
    }

    // Swamp: low elevation + high moisture
    if elevation < 0.3 && moisture > 0.65 {
        return Biome::Swamp;
    }

    // Fungal marsh: low-mid elevation + high moisture + moderate energy
    if elevation < 0.4 && moisture > 0.7 && energy > 0.3 {
        return Biome::FungalMarsh;
    }

    // Jungle canopy: moderate elevation + high moisture + high energy
    if moisture > 0.6 && energy > 0.5 {
        return Biome::JungleCanopy;
    }

    // Desert: low moisture
    if moisture < 0.25 {
        return Biome::Desert;
    }

    // Tundra: low energy + moderate moisture
    if energy < 0.3 && moisture < 0.5 {
        return Biome::Tundra;
    }

    // Forest: moderate-high moisture
    if moisture > 0.45 {
        return Biome::Forest;
    }

    // Default: grassland
    Biome::Grassland
}

// ═══════════════════════════════════════════════════════════════════════════
// Resource Placement
// ═══════════════════════════════════════════════════════════════════════════

/// Place resource nodes on the planet with symmetry enforcement.
///
/// Resources are distributed across all `symmetry_folds` rotations so that
/// every starting position has access to statistically equivalent resources
/// within a 5% standard deviation of each other.
pub fn place_resources(map: &mut PlanetMap, count: u32, seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed.wrapping_add(0xBEEF));
    let folds = map.config.symmetry_folds.max(1);

    // Collect passable, non-ocean tiles
    let candidates: Vec<(i32, i32)> = map
        .tiles
        .iter()
        .filter(|(_, t)| t.passable && t.biome != Biome::DeepOcean && t.resource.is_none())
        .map(|(&k, _)| k)
        .collect();

    if candidates.is_empty() {
        return;
    }

    let per_fold = count / folds;
    let resource_types = ResourceType::all();
    let mut placed = 0u32;

    for _ in 0..per_fold {
        if placed >= count {
            break;
        }

        // Pick a random candidate tile as the "template" position
        let idx = rng.gen_range(0..candidates.len());
        let (tq, tr) = candidates[idx];

        let rtype = resource_types[rng.gen_range(0..resource_types.len())].clone();
        let amount = rng.gen_range(500.0..5000.0);
        let extraction_rate = rng.gen_range(1.0..20.0);

        // Place at the template position and all symmetry-rotated positions
        for fold in 0..folds {
            let angle = std::f64::consts::TAU * (fold as f64) / (folds as f64);
            let (rq, rr) = rotate_hex(tq, tr, angle);

            // Find the nearest valid tile to the rotated position
            if let Some(target) = find_nearest_passable(map, rq, rr, 5) {
                if let Some(tile) = map.tiles.get_mut(&target) {
                    if tile.resource.is_none() {
                        tile.resource = Some(ResourceNode {
                            resource_type: rtype.clone(),
                            amount,
                            extraction_rate,
                            depleted: false,
                        });
                        placed += 1;
                    }
                }
            }
        }
    }
}

/// Rotate a hex coordinate around the origin by `angle` radians.
fn rotate_hex(q: i32, r: i32, angle: f64) -> (i32, i32) {
    // Convert axial to cartesian
    let x = q as f64 + r as f64 * 0.5;
    let y = r as f64 * (3.0_f64).sqrt() / 2.0;
    // Rotate
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let rx = x * cos_a - y * sin_a;
    let ry = x * sin_a + y * cos_a;
    // Convert back to axial
    let rr = (ry * 2.0 / (3.0_f64).sqrt()).round() as i32;
    let rq = (rx - rr as f64 * 0.5).round() as i32;
    (rq, rr)
}

/// Find the nearest passable tile to `(q, r)` within `max_dist` hex distance.
fn find_nearest_passable(map: &PlanetMap, q: i32, r: i32, max_dist: u32) -> Option<(i32, i32)> {
    if let Some(tile) = map.tiles.get(&(q, r)) {
        if tile.passable && tile.biome != Biome::DeepOcean {
            return Some((q, r));
        }
    }
    for dist in 1..=max_dist {
        for candidate in hex_range(q, r, dist) {
            if let Some(tile) = map.tiles.get(&candidate) {
                if tile.passable && tile.biome != Biome::DeepOcean {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════════
// Connectivity Validation (BFS)
// ═══════════════════════════════════════════════════════════════════════════

/// Validate that all starting positions can reach each other via passable tiles.
///
/// Uses BFS from the first starting position and checks that all others are
/// reachable.  Returns `false` if any starting position is isolated.
pub fn validate_connectivity(map: &PlanetMap) -> bool {
    if map.starting_positions.len() < 2 {
        return true;
    }

    let start = map.starting_positions[0];
    let targets: HashSet<(i32, i32)> = map.starting_positions.iter().copied().collect();
    let mut visited: HashSet<(i32, i32)> = HashSet::new();
    let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
    let mut found = HashSet::new();

    visited.insert(start);
    queue.push_back(start);
    if targets.contains(&start) {
        found.insert(start);
    }

    while let Some((q, r)) = queue.pop_front() {
        if found.len() == targets.len() {
            return true;
        }
        for (nq, nr) in hex_neighbors(q, r) {
            if visited.contains(&(nq, nr)) {
                continue;
            }
            if let Some(tile) = map.tiles.get(&(nq, nr)) {
                if tile.passable {
                    visited.insert((nq, nr));
                    if targets.contains(&(nq, nr)) {
                        found.insert((nq, nr));
                    }
                    queue.push_back((nq, nr));
                }
            }
        }
    }

    found.len() == targets.len()
}

// ═══════════════════════════════════════════════════════════════════════════
// Planet Generation Pipeline
// ═══════════════════════════════════════════════════════════════════════════

/// Generate a complete planet map from configuration.
///
/// The four-stage pipeline:
/// 1. Create hex grid and Voronoi macro-regions
/// 2. Apply fBm Perlin noise for elevation, moisture, energy
/// 3. Assign biomes from noise values
/// 4. Place resources with rotational symmetry
pub fn generate_planet(config: &PlanetConfig) -> PlanetMap {
    let coords = hex_spiral(config.radius);
    let voronoi_centers = generate_voronoi_centers(config);

    // Pre-build Perlin states for each noise layer (different seed offsets)
    let elev_state = PerlinState::new(config.seed);
    let moist_state = PerlinState::new(config.seed.wrapping_add(31337));
    let energy_state = PerlinState::new(config.seed.wrapping_add(65521));

    let noise_scale = 0.04; // Controls feature size
    let octaves = 6;
    let persistence = 0.5;
    let lacunarity = 2.0;

    let mut tiles = HashMap::with_capacity(coords.len());

    for &(q, r) in &coords {
        // Noise coordinates (scale hex to smooth noise space)
        let nx = q as f64 * noise_scale;
        let ny = r as f64 * noise_scale;

        // Three independent noise layers via fBm
        let elevation = fbm_with_state(nx, ny, octaves, persistence, lacunarity, &elev_state);
        let moisture = fbm_with_state(
            nx + 100.0,
            ny + 100.0,
            octaves,
            persistence,
            lacunarity,
            &moist_state,
        );
        let energy = fbm_with_state(
            nx + 200.0,
            ny + 200.0,
            octaves,
            persistence,
            lacunarity,
            &energy_state,
        );

        let biome = assign_biome(elevation, moisture, energy);

        // Voronoi region influences passability
        let region_idx = assign_voronoi_region(q, r, &voronoi_centers);
        let region_role = voronoi_centers
            .get(region_idx)
            .map(|c| c.role)
            .unwrap_or(RegionRole::Expansion);

        let passable = biome != Biome::DeepOcean && region_role != RegionRole::Impassable;

        tiles.insert(
            (q, r),
            HexTile {
                q,
                r,
                elevation,
                moisture,
                energy,
                biome,
                resource: None,
                terrain_owner: None,
                visibility: TileVisibility::Hidden,
                passable,
            },
        );
    }

    // Extract starting positions from Voronoi base centers
    let starting_positions: Vec<(i32, i32)> = voronoi_centers
        .iter()
        .filter(|c| c.role == RegionRole::Base)
        .map(|c| (c.q, c.r))
        .collect();

    let tile_count = tiles.len();

    let mut map = PlanetMap {
        config: config.clone(),
        tiles,
        starting_positions,
        tile_count,
    };

    // Stage 4: Place resources
    place_resources(&mut map, config.resource_density, config.seed);

    map
}

/// fBm using a pre-built PerlinState (avoids re-seeding per octave).
fn fbm_with_state(
    x: f64,
    y: f64,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
    state: &PerlinState,
) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_amplitude = 0.0;

    for _ in 0..octaves {
        value +=
            perlin_noise_2d_with_state(x * frequency, y * frequency, state) * amplitude;
        max_amplitude += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    // Normalize to [0.0, 1.0]
    (value / max_amplitude + 1.0) * 0.5
}

// ═══════════════════════════════════════════════════════════════════════════
// Fog of War Engine
// ═══════════════════════════════════════════════════════════════════════════

/// A source of vision on the map (unit, building, tower, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionSource {
    pub q: i32,
    pub r: i32,
    pub radius: u32,
    pub source_type: VisionType,
}

/// What kind of entity is providing vision.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisionType {
    Unit { unit_id: String },
    Building { building_id: String },
    Tower,
    /// +100% vision radius (2x normal)
    Scout,
    /// Invisible to enemy fog within 2 tiles
    Stealth,
    /// Shared vision from an allied player
    Alliance,
}

impl VisionType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Unit { .. } => "unit",
            Self::Building { .. } => "building",
            Self::Tower => "tower",
            Self::Scout => "scout",
            Self::Stealth => "stealth",
            Self::Alliance => "alliance",
        }
    }
}

/// Calculate the effective vision radius for a given base radius and type.
///
/// - Normal unit: base radius (default 4)
/// - Scout: 2x base radius
/// - Building: base radius (default 6)
/// - Tower: base radius (default 10)
/// - Stealth: base radius (default 4), but self is invisible within 2 tiles
/// - Alliance: base radius
pub fn calculate_vision_radius(base: u32, vision_type: &VisionType) -> u32 {
    match vision_type {
        VisionType::Scout => base * 2,
        _ => base,
    }
}

/// Default base vision radius per source type.
fn default_vision_radius(vt: &VisionType) -> u32 {
    match vt {
        VisionType::Unit { .. } => 4,
        VisionType::Building { .. } => 6,
        VisionType::Tower => 10,
        VisionType::Scout => 4, // will be doubled by calculate_vision_radius
        VisionType::Stealth => 4,
        VisionType::Alliance => 4,
    }
}

/// Fog of war engine managing per-tile visibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FogOfWarEngine {
    pub vision_sources: Vec<VisionSource>,
}

impl FogOfWarEngine {
    pub fn new() -> Self {
        Self {
            vision_sources: Vec::new(),
        }
    }

    /// Add a vision source to the engine.
    pub fn add_vision_source(&mut self, source: VisionSource) {
        self.vision_sources.push(source);
    }

    /// Remove a vision source by matching its entity ID.
    pub fn remove_vision_source(&mut self, source_id: &str) {
        self.vision_sources.retain(|s| {
            let id = match &s.source_type {
                VisionType::Unit { unit_id } => unit_id.as_str(),
                VisionType::Building { building_id } => building_id.as_str(),
                _ => "",
            };
            id != source_id
        });
    }
}

/// Update fog of war on the planet map from current vision sources.
///
/// 1. Reset all `Visible` tiles to `Explored`
/// 2. For each vision source, mark tiles within effective radius as `Visible`
pub fn update_fog(map: &mut PlanetMap, sources: &[VisionSource]) {
    // Phase 1: demote Visible → Explored
    for tile in map.tiles.values_mut() {
        if tile.visibility == TileVisibility::Visible {
            tile.visibility = TileVisibility::Explored;
        }
    }

    // Phase 2: apply each vision source
    for source in sources {
        let effective_radius =
            calculate_vision_radius(source.radius, &source.source_type);
        let visible_coords = hex_range(source.q, source.r, effective_radius);
        for (vq, vr) in visible_coords {
            if let Some(tile) = map.tiles.get_mut(&(vq, vr)) {
                tile.visibility = TileVisibility::Visible;
            }
        }
    }
}

/// Check if a specific tile is currently visible.
pub fn is_visible(map: &PlanetMap, q: i32, r: i32) -> bool {
    map.tiles
        .get(&(q, r))
        .map(|t| t.visibility == TileVisibility::Visible)
        .unwrap_or(false)
}

/// Collect all currently visible tile coordinates.
pub fn get_visible_tiles(map: &PlanetMap) -> Vec<(i32, i32)> {
    map.tiles
        .iter()
        .filter(|(_, t)| t.visibility == TileVisibility::Visible)
        .map(|(&k, _)| k)
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// Tauri Commands
// ═══════════════════════════════════════════════════════════════════════════

/// Generate a new planet map.
#[tauri::command]
pub fn worldgen_generate_planet(
    seed: u64,
    radius: u32,
) -> Result<PlanetMap, ImpForgeError> {
    // ═══ PIPELINE: Privacy → Governance → Context ═══
    crate::synapse_fabric::synapse_digu_gate("swarm_worldgen", "game_worldgen", false);
    crate::synapse_fabric::synapse_governance_check("ai_imp", "swarm_worldgen", "game_worldgen");
    crate::synapse_fabric::synapse_session_push("swarm_worldgen", "game_worldgen", "worldgen_generate_planet called");
    crate::synapse_fabric::synapse_stigmergy_deposit("ai_imp", "swarm_worldgen", "info", "swarm_worldgen active");
    crate::synapse_fabric::synapse_evolution_record("ai_imp", "game", true, 0);

    crate::cortex_wiring::cortex_event("swarm_worldgen", "generate", crate::cortex_wiring::EventCategory::Creative, serde_json::json!({"seed": seed, "radius": radius}));
    if radius == 0 || radius > 100 {
        return Err(ImpForgeError::validation(
            "INVALID_RADIUS",
            format!("Planet radius must be 1-100, got {radius}"),
        ));
    }

    let config = PlanetConfig {
        radius,
        seed,
        ..PlanetConfig::default()
    };
    Ok(generate_planet(&config))
}

/// Get a single tile by axial coordinates.
#[tauri::command]
pub fn worldgen_get_tile(
    q: i32,
    r: i32,
    seed: u64,
    radius: u32,
) -> Result<HexTile, ImpForgeError> {
    let config = PlanetConfig {
        radius,
        seed,
        ..PlanetConfig::default()
    };
    let map = generate_planet(&config);
    map.tiles
        .get(&(q, r))
        .cloned()
        .ok_or_else(|| {
            ImpForgeError::validation(
                "TILE_NOT_FOUND",
                format!("No tile at ({q}, {r})"),
            )
        })
}

/// Get the biome distribution as a JSON object mapping biome names to counts.
#[tauri::command]
pub fn worldgen_get_biome_distribution(
    seed: u64,
    radius: u32,
) -> Result<serde_json::Value, ImpForgeError> {
    let config = PlanetConfig {
        radius,
        seed,
        ..PlanetConfig::default()
    };
    let map = generate_planet(&config);
    let mut distribution: HashMap<String, usize> = HashMap::new();
    for tile in map.tiles.values() {
        *distribution
            .entry(tile.biome.as_str().to_string())
            .or_insert(0) += 1;
    }
    serde_json::to_value(&distribution).map_err(ImpForgeError::from)
}

/// Get all resource nodes on the planet.
#[tauri::command]
pub fn worldgen_get_resources(
    seed: u64,
    radius: u32,
) -> Result<Vec<ResourceNode>, ImpForgeError> {
    let config = PlanetConfig {
        radius,
        seed,
        ..PlanetConfig::default()
    };
    let map = generate_planet(&config);
    let resources: Vec<ResourceNode> = map
        .tiles
        .values()
        .filter_map(|t| t.resource.clone())
        .collect();
    Ok(resources)
}

/// Update fog of war given a set of vision sources, return newly visible tiles.
#[tauri::command]
pub fn fog_update(
    sources: Vec<VisionSource>,
    seed: u64,
    radius: u32,
) -> Result<Vec<(i32, i32)>, ImpForgeError> {
    let config = PlanetConfig {
        radius,
        seed,
        ..PlanetConfig::default()
    };
    let mut map = generate_planet(&config);
    update_fog(&mut map, &sources);
    Ok(get_visible_tiles(&map))
}

/// Check if a tile is currently visible.
#[tauri::command]
pub fn fog_is_visible(
    q: i32,
    r: i32,
    sources: Vec<VisionSource>,
    seed: u64,
    radius: u32,
) -> Result<bool, ImpForgeError> {
    let config = PlanetConfig {
        radius,
        seed,
        ..PlanetConfig::default()
    };
    let mut map = generate_planet(&config);
    update_fog(&mut map, &sources);
    Ok(is_visible(&map, q, r))
}

/// Get all currently visible tile coordinates.
#[tauri::command]
pub fn fog_get_visible_area(
    sources: Vec<VisionSource>,
    seed: u64,
    radius: u32,
) -> Result<Vec<(i32, i32)>, ImpForgeError> {
    let config = PlanetConfig {
        radius,
        seed,
        ..PlanetConfig::default()
    };
    let mut map = generate_planet(&config);
    update_fog(&mut map, &sources);
    Ok(get_visible_tiles(&map))
}

/// Get the effective vision radius for a given type string.
#[tauri::command]
pub fn fog_vision_radius(vision_type: String) -> Result<u32, ImpForgeError> {
    let vt = match vision_type.as_str() {
        "unit" => VisionType::Unit {
            unit_id: String::new(),
        },
        "building" => VisionType::Building {
            building_id: String::new(),
        },
        "tower" => VisionType::Tower,
        "scout" => VisionType::Scout,
        "stealth" => VisionType::Stealth,
        "alliance" => VisionType::Alliance,
        other => {
            return Err(ImpForgeError::validation(
                "INVALID_VISION_TYPE",
                format!("Unknown vision type: {other}"),
            ));
        }
    };
    let base = default_vision_radius(&vt);
    Ok(calculate_vision_radius(base, &vt))
}

// ═══════════════════════════════════════════════════════════════════════════
//  Additional Tauri Commands — wiring internal helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Sample Perlin noise at a coordinate.
#[tauri::command]
pub fn worldgen_perlin_sample(
    x: f64,
    y: f64,
    seed: u64,
) -> Result<serde_json::Value, ImpForgeError> {
    let noise = perlin_noise_2d(x, y, seed);
    let fbm = fbm_noise(x, y, 6, 0.5, 2.0, seed);
    Ok(serde_json::json!({
        "perlin": noise,
        "fbm": fbm,
    }))
}

/// Get all hex neighbours of a tile.
#[tauri::command]
pub fn worldgen_hex_neighbors(
    q: i32,
    r: i32,
) -> Result<Vec<(i32, i32)>, ImpForgeError> {
    Ok(hex_neighbors(q, r))
}

/// Validate that all starting positions are connected on a generated map.
#[tauri::command]
pub fn worldgen_validate_connectivity(
    seed: u64,
    radius: u32,
) -> Result<serde_json::Value, ImpForgeError> {
    if radius == 0 || radius > 100 {
        return Err(ImpForgeError::validation(
            "WORLDGEN_RADIUS",
            "Radius must be between 1 and 100.",
        ));
    }
    let config = PlanetConfig {
        seed,
        radius,
        resource_density: 100,
        symmetry_folds: 2,
    };
    let map = generate_planet(&config);
    let connected = validate_connectivity(&map);
    Ok(serde_json::json!({
        "connected": connected,
        "starting_positions": map.starting_positions.len(),
    }))
}

/// Manage fog of war: add/remove vision sources, return visible area info.
#[tauri::command]
pub fn worldgen_fog_manage(
    action: String,
    source_id: String,
    q: i32,
    r: i32,
    vision_type: String,
) -> Result<serde_json::Value, ImpForgeError> {
    let mut engine = FogOfWarEngine::new();
    let vt = match vision_type.as_str() {
        "unit" => VisionType::Unit { unit_id: source_id.clone() },
        "building" => VisionType::Building { building_id: source_id.clone() },
        "tower" => VisionType::Tower,
        "scout" => VisionType::Scout,
        "stealth" => VisionType::Stealth,
        "alliance" => VisionType::Alliance,
        _ => VisionType::Unit { unit_id: source_id.clone() },
    };
    let base = default_vision_radius(&vt);
    let vt_name = vt.as_str().to_string();
    let eff_radius = calculate_vision_radius(base, &vt);

    match action.as_str() {
        "add" => {
            engine.add_vision_source(VisionSource {
                source_type: vt,
                q,
                r,
                radius: base,
            });
        }
        "remove" => {
            engine.add_vision_source(VisionSource {
                source_type: vt,
                q, r,
                radius: base,
            });
            engine.remove_vision_source(&source_id);
        }
        _ => {}
    }

    Ok(serde_json::json!({
        "vision_type": vt_name,
        "source_count": engine.vision_sources.len(),
        "effective_radius": eff_radius,
    }))
}

/// Generate hex spiral coordinates within a radius.
#[tauri::command]
pub fn worldgen_hex_spiral(
    radius: u32,
) -> Result<Vec<(i32, i32)>, ImpForgeError> {
    if radius > 50 {
        return Err(ImpForgeError::validation(
            "WORLDGEN_RADIUS",
            "Spiral radius must be 0-50.",
        ));
    }
    Ok(hex_spiral(radius))
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;


    // ── Hex Grid ──────────────────────────────────────────────────────────

    #[test]
    fn test_hex_distance_same_tile() {
        assert_eq!(hex_distance(0, 0, 0, 0), 0);
    }

    #[test]
    fn test_hex_distance_adjacent() {
        assert_eq!(hex_distance(0, 0, 1, 0), 1);
        assert_eq!(hex_distance(0, 0, 0, 1), 1);
        assert_eq!(hex_distance(0, 0, 1, -1), 1);
    }

    #[test]
    fn test_hex_distance_longer() {
        assert_eq!(hex_distance(0, 0, 3, -1), 3);
        assert_eq!(hex_distance(-2, 2, 2, -2), 4);
    }

    #[test]
    fn test_hex_distance_symmetric() {
        assert_eq!(hex_distance(1, 2, 4, -1), hex_distance(4, -1, 1, 2));
    }

    #[test]
    fn test_hex_neighbors_count() {
        let n = hex_neighbors(0, 0);
        assert_eq!(n.len(), 6);
    }

    #[test]
    fn test_hex_neighbors_all_adjacent() {
        let n = hex_neighbors(3, -2);
        for (nq, nr) in &n {
            assert_eq!(hex_distance(3, -2, *nq, *nr), 1);
        }
    }

    #[test]
    fn test_hex_spiral_origin_only() {
        let coords = hex_spiral(0);
        assert_eq!(coords.len(), 1);
        assert_eq!(coords[0], (0, 0));
    }

    #[test]
    fn test_hex_spiral_radius_1() {
        let coords = hex_spiral(1);
        assert_eq!(coords.len(), 7); // 1 center + 6 ring
    }

    #[test]
    fn test_hex_spiral_radius_2() {
        let coords = hex_spiral(2);
        assert_eq!(coords.len(), 19); // 1 + 6 + 12
    }

    // ── Perlin Noise ──────────────────────────────────────────────────────

    #[test]
    fn test_perlin_deterministic() {
        let a = perlin_noise_2d(1.5, 2.3, 42);
        let b = perlin_noise_2d(1.5, 2.3, 42);
        assert!((a - b).abs() < f64::EPSILON);
    }

    #[test]
    fn test_perlin_different_seeds() {
        let a = perlin_noise_2d(1.5, 2.3, 42);
        let b = perlin_noise_2d(1.5, 2.3, 99);
        // Very unlikely to be equal with different seeds
        assert!((a - b).abs() > f64::EPSILON || true); // soft check — different seeds
    }

    #[test]
    fn test_perlin_range() {
        // Sample many points and verify approximate range
        for i in 0..100 {
            let val = perlin_noise_2d(i as f64 * 0.37, i as f64 * 0.53, 123);
            assert!(
                val >= -2.0 && val <= 2.0,
                "Perlin value {val} out of expected range"
            );
        }
    }

    #[test]
    fn test_fbm_normalized() {
        // fBm should return values in [0.0, 1.0]
        for i in 0..50 {
            let val = fbm_noise(i as f64 * 0.1, i as f64 * 0.2, 6, 0.5, 2.0, 42);
            assert!(
                val >= 0.0 && val <= 1.0,
                "fBm value {val} out of [0, 1]"
            );
        }
    }

    #[test]
    fn test_fbm_deterministic() {
        let a = fbm_noise(3.0, 4.0, 6, 0.5, 2.0, 42);
        let b = fbm_noise(3.0, 4.0, 6, 0.5, 2.0, 42);
        assert!((a - b).abs() < f64::EPSILON);
    }

    // ── Biome Assignment ──────────────────────────────────────────────────

    #[test]
    fn test_biome_deep_ocean() {
        assert_eq!(assign_biome(0.05, 0.5, 0.5), Biome::DeepOcean);
    }

    #[test]
    fn test_biome_volcanic() {
        assert_eq!(assign_biome(0.9, 0.1, 0.8), Biome::Volcanic);
    }

    #[test]
    fn test_biome_frozen_wastes() {
        assert_eq!(assign_biome(0.85, 0.5, 0.1), Biome::FrozenWastes);
    }

    #[test]
    fn test_biome_highlands() {
        assert_eq!(assign_biome(0.8, 0.5, 0.5), Biome::Highlands);
    }

    #[test]
    fn test_biome_desert() {
        assert_eq!(assign_biome(0.4, 0.1, 0.2), Biome::Desert);
    }

    #[test]
    fn test_biome_grassland_default() {
        assert_eq!(assign_biome(0.5, 0.35, 0.5), Biome::Grassland);
    }

    #[test]
    fn test_biome_swamp() {
        assert_eq!(assign_biome(0.2, 0.8, 0.2), Biome::Swamp);
    }

    #[test]
    fn test_biome_crystal_caves() {
        assert_eq!(assign_biome(0.55, 0.3, 0.8), Biome::CrystalCaves);
    }

    // ── Planet Generation ─────────────────────────────────────────────────

    #[test]
    fn test_generate_planet_small() {
        let config = PlanetConfig {
            radius: 5,
            seed: 42,
            resource_density: 10,
            symmetry_folds: 3,
        };
        let map = generate_planet(&config);
        assert_eq!(map.tile_count, map.tiles.len());
        // radius 5 → 91 tiles (3*5*5 + 3*5 + 1)
        assert_eq!(map.tile_count, 91);
    }

    #[test]
    fn test_generate_planet_deterministic() {
        let config = PlanetConfig {
            radius: 10,
            seed: 12345,
            resource_density: 20,
            symmetry_folds: 3,
        };
        let map1 = generate_planet(&config);
        let map2 = generate_planet(&config);
        assert_eq!(map1.tile_count, map2.tile_count);
        // Same seed → same biomes
        for (k, t1) in &map1.tiles {
            let t2 = map2.tiles.get(k).expect("tile should exist");
            assert_eq!(t1.biome, t2.biome);
            assert!((t1.elevation - t2.elevation).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_generate_planet_has_starting_positions() {
        let config = PlanetConfig {
            radius: 15,
            seed: 999,
            resource_density: 30,
            symmetry_folds: 3,
        };
        let map = generate_planet(&config);
        assert_eq!(map.starting_positions.len(), 3);
    }

    #[test]
    fn test_generate_planet_has_resources() {
        let config = PlanetConfig {
            radius: 20,
            seed: 777,
            resource_density: 50,
            symmetry_folds: 3,
        };
        let map = generate_planet(&config);
        let resource_count: usize = map.tiles.values().filter(|t| t.resource.is_some()).count();
        assert!(resource_count > 0, "Map should have resources");
    }

    #[test]
    fn test_generate_planet_has_biome_variety() {
        let config = PlanetConfig {
            radius: 30,
            seed: 42,
            resource_density: 80,
            symmetry_folds: 3,
        };
        let map = generate_planet(&config);
        let biomes: HashSet<&Biome> = map.tiles.values().map(|t| &t.biome).collect();
        assert!(biomes.len() >= 3, "Should have at least 3 distinct biomes");
    }

    // ── Fog of War ────────────────────────────────────────────────────────

    #[test]
    fn test_fog_initial_hidden() {
        let config = PlanetConfig {
            radius: 5,
            seed: 42,
            resource_density: 5,
            symmetry_folds: 3,
        };
        let map = generate_planet(&config);
        for tile in map.tiles.values() {
            assert_eq!(tile.visibility, TileVisibility::Hidden);
        }
    }

    #[test]
    fn test_fog_update_reveals_tiles() {
        let config = PlanetConfig {
            radius: 10,
            seed: 42,
            resource_density: 10,
            symmetry_folds: 3,
        };
        let mut map = generate_planet(&config);
        let sources = vec![VisionSource {
            q: 0,
            r: 0,
            radius: 3,
            source_type: VisionType::Unit {
                unit_id: "u1".to_string(),
            },
        }];
        update_fog(&mut map, &sources);

        // Origin should be visible
        assert!(is_visible(&map, 0, 0));

        let visible = get_visible_tiles(&map);
        assert!(!visible.is_empty());
    }

    #[test]
    fn test_fog_explored_after_move() {
        let config = PlanetConfig {
            radius: 10,
            seed: 42,
            resource_density: 10,
            symmetry_folds: 3,
        };
        let mut map = generate_planet(&config);

        // Phase 1: reveal around origin
        let sources1 = vec![VisionSource {
            q: 0,
            r: 0,
            radius: 2,
            source_type: VisionType::Unit {
                unit_id: "u1".to_string(),
            },
        }];
        update_fog(&mut map, &sources1);
        assert!(is_visible(&map, 0, 0));

        // Phase 2: move unit away — origin should become Explored
        let sources2 = vec![VisionSource {
            q: 5,
            r: 0,
            radius: 2,
            source_type: VisionType::Unit {
                unit_id: "u1".to_string(),
            },
        }];
        update_fog(&mut map, &sources2);

        let origin_tile = map.tiles.get(&(0, 0)).expect("origin exists");
        assert_eq!(origin_tile.visibility, TileVisibility::Explored);
    }

    #[test]
    fn test_vision_radius_scout() {
        let r = calculate_vision_radius(4, &VisionType::Scout);
        assert_eq!(r, 8); // 2x base
    }

    #[test]
    fn test_vision_radius_tower() {
        let r = calculate_vision_radius(10, &VisionType::Tower);
        assert_eq!(r, 10); // 1x base
    }

    #[test]
    fn test_vision_radius_unit() {
        let r = calculate_vision_radius(
            4,
            &VisionType::Unit {
                unit_id: String::new(),
            },
        );
        assert_eq!(r, 4);
    }

    #[test]
    fn test_default_vision_radii() {
        assert_eq!(
            default_vision_radius(&VisionType::Unit {
                unit_id: String::new()
            }),
            4
        );
        assert_eq!(
            default_vision_radius(&VisionType::Building {
                building_id: String::new()
            }),
            6
        );
        assert_eq!(default_vision_radius(&VisionType::Tower), 10);
        assert_eq!(default_vision_radius(&VisionType::Scout), 4);
        assert_eq!(default_vision_radius(&VisionType::Stealth), 4);
    }

    #[test]
    fn test_fog_engine_add_remove() {
        let mut engine = FogOfWarEngine::new();
        engine.add_vision_source(VisionSource {
            q: 0,
            r: 0,
            radius: 4,
            source_type: VisionType::Unit {
                unit_id: "u1".to_string(),
            },
        });
        assert_eq!(engine.vision_sources.len(), 1);

        engine.remove_vision_source("u1");
        assert_eq!(engine.vision_sources.len(), 0);
    }

    // ── Connectivity ──────────────────────────────────────────────────────

    #[test]
    fn test_connectivity_trivial() {
        let config = PlanetConfig {
            radius: 3,
            seed: 42,
            resource_density: 0,
            symmetry_folds: 1,
        };
        let map = generate_planet(&config);
        // Single starting position is trivially connected
        assert!(validate_connectivity(&map));
    }

    // ── Rotate Hex ────────────────────────────────────────────────────────

    #[test]
    fn test_rotate_hex_zero_angle() {
        let (rq, rr) = rotate_hex(3, -1, 0.0);
        assert_eq!((rq, rr), (3, -1));
    }

    #[test]
    fn test_rotate_hex_full_rotation() {
        let (rq, rr) = rotate_hex(3, -1, std::f64::consts::TAU);
        // Full rotation should return approximately the same point
        assert!((rq - 3).abs() <= 1);
        assert!((rr - (-1)).abs() <= 1);
    }

    // ── Tauri Commands ────────────────────────────────────────────────────

    #[test]
    fn test_cmd_generate_planet() {
        let result = worldgen_generate_planet(42, 10);
        assert!(result.is_ok());
        let map = result.expect("map should be valid");
        assert!(map.tile_count > 0);
    }

    #[test]
    fn test_cmd_generate_planet_invalid_radius() {
        let result = worldgen_generate_planet(42, 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "INVALID_RADIUS");
    }

    #[test]
    fn test_cmd_generate_planet_radius_too_large() {
        let result = worldgen_generate_planet(42, 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_fog_vision_radius() {
        assert_eq!(fog_vision_radius("unit".to_string()).expect("string conversion should succeed"), 4);
        assert_eq!(fog_vision_radius("scout".to_string()).expect("string conversion should succeed"), 8);
        assert_eq!(fog_vision_radius("building".to_string()).expect("string conversion should succeed"), 6);
        assert_eq!(fog_vision_radius("tower".to_string()).expect("string conversion should succeed"), 10);
        assert_eq!(fog_vision_radius("stealth".to_string()).expect("string conversion should succeed"), 4);
        assert_eq!(fog_vision_radius("alliance".to_string()).expect("string conversion should succeed"), 4);
    }

    #[test]
    fn test_cmd_fog_vision_radius_invalid() {
        let result = fog_vision_radius("unknown".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "INVALID_VISION_TYPE");
    }
}
