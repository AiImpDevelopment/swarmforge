// SPDX-License-Identifier: Elastic-2.0
//! ForgeQuestEngine -- SQLite-backed game engine for both RPG and Colony layers.

use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::error::ImpForgeError;

use super::types::*;
use super::evosys::{GoverningAttributes, calculate_governing, novelty_multiplier};
use super::swarm_types::*;
use super::mutations::*;
use super::colony_types::*;
use super::static_data::*;

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("forge_quest::engine", "Game Engine");

pub struct ForgeQuestEngine {
    pub(crate) conn: Mutex<Connection>,
}

impl ForgeQuestEngine {
    pub fn new(data_dir: &std::path::Path) -> Result<Self, ImpForgeError> {
        std::fs::create_dir_all(data_dir).map_err(|e| {
            ImpForgeError::filesystem("QUEST_DIR", format!("Cannot create quest data dir: {e}"))
        })?;

        let db_path = data_dir.join("forge_quest.db");
        let conn = Connection::open(&db_path).map_err(|e| {
            ImpForgeError::internal("QUEST_DB_OPEN", format!("Cannot open quest DB: {e}"))
        })?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;",
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_DB_PRAGMA", format!("Pragma failed: {e}"))
        })?;

        let engine = Self {
            conn: Mutex::new(conn),
        };
        engine.init_tables()?;
        Ok(engine)
    }

    fn init_tables(&self) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS quest_character (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                name TEXT NOT NULL,
                class TEXT NOT NULL DEFAULT 'warrior',
                level INTEGER NOT NULL DEFAULT 1,
                xp INTEGER NOT NULL DEFAULT 0,
                hp INTEGER NOT NULL DEFAULT 100,
                max_hp INTEGER NOT NULL DEFAULT 100,
                attack INTEGER NOT NULL DEFAULT 10,
                defense INTEGER NOT NULL DEFAULT 5,
                magic INTEGER NOT NULL DEFAULT 5,
                gold INTEGER NOT NULL DEFAULT 0,
                skill_points INTEGER NOT NULL DEFAULT 0,
                quests_completed INTEGER NOT NULL DEFAULT 0,
                monsters_slain INTEGER NOT NULL DEFAULT 0,
                current_zone TEXT NOT NULL DEFAULT 'beginners_meadow',
                guild TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS quest_inventory (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                item_type TEXT NOT NULL,
                rarity TEXT NOT NULL DEFAULT 'common',
                attack INTEGER NOT NULL DEFAULT 0,
                defense INTEGER NOT NULL DEFAULT 0,
                magic INTEGER NOT NULL DEFAULT 0,
                hp_bonus INTEGER NOT NULL DEFAULT 0,
                level_req INTEGER NOT NULL DEFAULT 1,
                description TEXT NOT NULL DEFAULT '',
                equipped_slot TEXT
            );

            CREATE TABLE IF NOT EXISTS quest_skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                tier INTEGER NOT NULL DEFAULT 1,
                points_invested INTEGER NOT NULL DEFAULT 0,
                max_points INTEGER NOT NULL DEFAULT 5,
                prerequisite TEXT,
                effect TEXT NOT NULL,
                branch TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS quest_quests (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                objective TEXT NOT NULL,
                objective_target INTEGER NOT NULL DEFAULT 1,
                objective_progress INTEGER NOT NULL DEFAULT 0,
                reward_xp INTEGER NOT NULL DEFAULT 0,
                reward_gold INTEGER NOT NULL DEFAULT 0,
                reward_items TEXT NOT NULL DEFAULT '[]',
                completed INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS quest_battle_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                monster_name TEXT NOT NULL,
                monster_level INTEGER NOT NULL,
                victory INTEGER NOT NULL,
                xp_earned INTEGER NOT NULL DEFAULT 0,
                gold_earned INTEGER NOT NULL DEFAULT 0,
                fought_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            -- EvoSys action history for novelty multiplier
            CREATE TABLE IF NOT EXISTS quest_action_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                action TEXT NOT NULL,
                performed_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_action_log_action ON quest_action_log(action);

            -- Forge Swarm tables
            CREATE TABLE IF NOT EXISTS swarm_units (
                id TEXT PRIMARY KEY,
                unit_type TEXT NOT NULL,
                name TEXT NOT NULL,
                level INTEGER NOT NULL DEFAULT 1,
                hp INTEGER NOT NULL DEFAULT 30,
                attack INTEGER NOT NULL DEFAULT 5,
                defense INTEGER NOT NULL DEFAULT 3,
                special_ability TEXT NOT NULL DEFAULT '',
                assigned_task TEXT,
                efficiency REAL NOT NULL DEFAULT 0.5,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS swarm_buildings (
                id TEXT PRIMARY KEY,
                building_type TEXT NOT NULL UNIQUE,
                level INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS swarm_resources (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                essence INTEGER NOT NULL DEFAULT 100,
                minerals INTEGER NOT NULL DEFAULT 0,
                vespene INTEGER NOT NULL DEFAULT 0,
                biomass INTEGER NOT NULL DEFAULT 0,
                dark_matter INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS swarm_missions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                required_unit_types TEXT NOT NULL DEFAULT '[]',
                required_unit_count INTEGER NOT NULL DEFAULT 1,
                assigned_units TEXT NOT NULL DEFAULT '[]',
                duration_minutes INTEGER NOT NULL DEFAULT 5,
                reward_essence INTEGER NOT NULL DEFAULT 0,
                reward_minerals INTEGER NOT NULL DEFAULT 0,
                reward_vespene INTEGER NOT NULL DEFAULT 0,
                reward_biomass INTEGER NOT NULL DEFAULT 0,
                reward_dark_matter INTEGER NOT NULL DEFAULT 0,
                reward_items TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'available',
                started_at TEXT
            );",
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_DB_INIT", format!("Table creation failed: {e}"))
        })?;

        // OGame-style colony tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS planet_resources (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                biomass REAL NOT NULL DEFAULT 500.0,
                minerals REAL NOT NULL DEFAULT 500.0,
                crystal REAL NOT NULL DEFAULT 0.0,
                spore_gas REAL NOT NULL DEFAULT 0.0,
                dark_matter INTEGER NOT NULL DEFAULT 0,
                last_collected TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS planet_buildings (
                building_type TEXT PRIMARY KEY,
                level INTEGER NOT NULL DEFAULT 0,
                upgrading INTEGER NOT NULL DEFAULT 0,
                upgrade_finish TEXT
            );

            CREATE TABLE IF NOT EXISTS planet_research (
                tech_type TEXT PRIMARY KEY,
                level INTEGER NOT NULL DEFAULT 0,
                researching INTEGER NOT NULL DEFAULT 0,
                research_finish TEXT
            );

            CREATE TABLE IF NOT EXISTS planet_fleet (
                ship_type TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS planet_creep (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                coverage_percent REAL NOT NULL DEFAULT 0.0,
                flora_corrupted REAL NOT NULL DEFAULT 0.0,
                fauna_consumed REAL NOT NULL DEFAULT 0.0,
                last_updated TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS planet_shop_active (
                item_id TEXT PRIMARY KEY,
                activated_at TEXT NOT NULL,
                expires_at TEXT
            );

            CREATE TABLE IF NOT EXISTS planet_achievements (
                achievement_id TEXT PRIMARY KEY,
                earned_at TEXT NOT NULL DEFAULT (datetime('now')),
                dark_matter_awarded INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS swarm_mutations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                unit_id TEXT NOT NULL,
                mutation_id TEXT NOT NULL,
                applied_at_level INTEGER NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (unit_id) REFERENCES swarm_units(id) ON DELETE CASCADE,
                UNIQUE(unit_id, mutation_id)
            );

            CREATE TABLE IF NOT EXISTS planet_login_streak (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                current_streak INTEGER NOT NULL DEFAULT 0,
                last_login TEXT NOT NULL DEFAULT (datetime('now')),
                total_logins INTEGER NOT NULL DEFAULT 0
            );",
        )
        .map_err(|e| {
            ImpForgeError::internal("PLANET_DB_INIT", format!("Planet table creation failed: {e}"))
        })?;

        // Seed swarm resources row if missing
        conn.execute(
            "INSERT OR IGNORE INTO swarm_resources (id, essence) VALUES (1, 100)",
            [],
        )
        .map_err(|e| {
            ImpForgeError::internal("SWARM_SEED", format!("Swarm resources seed failed: {e}"))
        })?;

        // Seed planet resources
        conn.execute(
            "INSERT OR IGNORE INTO planet_resources (id) VALUES (1)",
            [],
        )
        .map_err(|e| {
            ImpForgeError::internal("PLANET_SEED", format!("Planet resources seed failed: {e}"))
        })?;

        // Seed planet creep
        conn.execute(
            "INSERT OR IGNORE INTO planet_creep (id) VALUES (1)",
            [],
        )
        .map_err(|e| {
            ImpForgeError::internal("CREEP_SEED", format!("Creep seed failed: {e}"))
        })?;

        // Seed planet login streak
        conn.execute(
            "INSERT OR IGNORE INTO planet_login_streak (id) VALUES (1)",
            [],
        )
        .map_err(|e| {
            ImpForgeError::internal("LOGIN_SEED", format!("Login streak seed failed: {e}"))
        })?;

        // Seed default buildings and missions
        self.seed_swarm_buildings(&conn)?;
        self.seed_swarm_missions(&conn)?;
        self.seed_planet_buildings(&conn)?;
        self.seed_planet_research(&conn)?;
        self.seed_planet_fleet(&conn)?;

        Ok(())
    }

    // -- Character management -------------------------------------------------

    pub fn create_character(
        &self,
        name: &str,
        class: &str,
    ) -> Result<QuestCharacter, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check if character already exists
        let exists: bool = conn
            .query_row("SELECT COUNT(*) FROM quest_character", [], |r| r.get::<_, i64>(0))
            .map(|c| c > 0)
            .unwrap_or(false);

        if exists {
            return Err(ImpForgeError::validation(
                "QUEST_CHAR_EXISTS",
                "Character already exists. Use quest_get_character instead.",
            ));
        }

        let class_enum = CharacterClass::from_str(class);
        let (hp, atk, def, mag) = match &class_enum {
            CharacterClass::Warrior => (120, 14, 8, 3),
            CharacterClass::Mage => (80, 5, 4, 16),
            CharacterClass::Ranger => (100, 11, 5, 8),
            CharacterClass::Blacksmith => (110, 12, 10, 3),
            CharacterClass::Bard => (90, 7, 5, 12),
            CharacterClass::Scholar => (85, 6, 4, 14),
        };

        conn.execute(
            "INSERT INTO quest_character (id, name, class, hp, max_hp, attack, defense, magic)
             VALUES (1, ?1, ?2, ?3, ?3, ?4, ?5, ?6)",
            params![name, class_enum.as_str(), hp, atk, def, mag],
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_CHAR_CREATE", format!("Create failed: {e}"))
        })?;

        // Seed starter quests
        self.seed_quests(&conn)?;
        // Seed default skills
        self.seed_skills(&conn)?;

        drop(conn);
        self.get_character()
    }

    pub fn get_character(&self) -> Result<QuestCharacter, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let char_row = conn
            .query_row("SELECT * FROM quest_character WHERE id = 1", [], |row| {
                Ok((
                    row.get::<_, String>(1)?,  // name
                    row.get::<_, String>(2)?,  // class
                    row.get::<_, u32>(3)?,     // level
                    row.get::<_, u64>(4)?,     // xp
                    row.get::<_, u32>(5)?,     // hp
                    row.get::<_, u32>(6)?,     // max_hp
                    row.get::<_, u32>(7)?,     // attack
                    row.get::<_, u32>(8)?,     // defense
                    row.get::<_, u32>(9)?,     // magic
                    row.get::<_, u64>(10)?,    // gold
                    row.get::<_, u32>(11)?,    // skill_points
                    row.get::<_, u32>(12)?,    // quests_completed
                    row.get::<_, u64>(13)?,    // monsters_slain
                    row.get::<_, String>(14)?, // current_zone
                    row.get::<_, Option<String>>(15)?, // guild
                ))
            })
            .map_err(|_| {
                ImpForgeError::validation(
                    "QUEST_NO_CHAR",
                    "No character found. Create one with quest_create_character.",
                )
            })?;

        let inventory = self.load_inventory(&conn)?;
        let equipped = self.build_equipment(&conn)?;
        let skills = self.load_skills(&conn)?;

        Ok(QuestCharacter {
            name: char_row.0,
            class: CharacterClass::from_str(&char_row.1),
            level: char_row.2,
            xp: char_row.3,
            hp: char_row.4,
            max_hp: char_row.5,
            attack: char_row.6,
            defense: char_row.7,
            magic: char_row.8,
            gold: char_row.9,
            inventory,
            equipped,
            skill_points: char_row.10,
            skills,
            quests_completed: char_row.11,
            monsters_slain: char_row.12,
            current_zone: char_row.13,
            guild: char_row.14,
        })
    }

    // -- Action tracking (productivity -> RPG) --------------------------------

    pub fn track_action(&self, action: &str) -> Result<ActionResult, ImpForgeError> {
        let reward = map_action_to_rpg(action);

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Load current character stats for class bonus
        let (class_str, level, xp, gold, skill_pts, current_zone): (String, u32, u64, u64, u32, String) = conn
            .query_row(
                "SELECT class, level, xp, gold, skill_points, current_zone FROM quest_character WHERE id = 1",
                [],
                |row| Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?,
                    row.get(3)?, row.get(4)?, row.get(5)?,
                )),
            )
            .map_err(|_| {
                ImpForgeError::validation("QUEST_NO_CHAR", "No character exists yet.")
            })?;

        let class = CharacterClass::from_str(&class_str);
        let class_mult = class.bonus_multiplier(action);

        // EvoSys: query action history count for novelty multiplier
        let action_count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM quest_action_log WHERE action = ?1",
                params![action],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let novelty_mult = novelty_multiplier(action_count);

        // Log this action for future novelty tracking
        let _ = conn.execute(
            "INSERT INTO quest_action_log (action) VALUES (?1)",
            params![action],
        );

        let multiplier = class_mult * novelty_mult;

        let xp_earned = (reward.xp as f64 * multiplier) as u64;
        let gold_earned = (reward.gold as f64 * multiplier) as u64;
        let new_xp = xp + xp_earned;
        let new_gold = gold + gold_earned;

        // Level-up check
        let old_level = level;
        let mut current_level = level;
        let mut accumulated_sp = skill_pts;
        loop {
            let needed = xp_for_level(current_level + 1);
            if new_xp >= needed {
                current_level += 1;
                accumulated_sp += 2; // 2 skill points per level
            } else {
                break;
            }
            if current_level >= 100 {
                break;
            }
        }

        let level_up = current_level > old_level;

        // Update character
        conn.execute(
            "UPDATE quest_character SET xp = ?1, gold = ?2, level = ?3, skill_points = ?4 WHERE id = 1",
            params![new_xp, new_gold, current_level, accumulated_sp],
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_UPDATE", format!("Update failed: {e}"))
        })?;

        // If leveled up, increase stats
        if level_up {
            conn.execute(
                "UPDATE quest_character SET
                    max_hp = max_hp + ?1,
                    hp = min(hp + ?1, max_hp + ?1),
                    attack = attack + ?2,
                    defense = defense + ?3,
                    magic = magic + ?4
                 WHERE id = 1",
                params![
                    5 * (current_level - old_level),
                    2 * (current_level - old_level),
                    (current_level - old_level),
                    (current_level - old_level),
                ],
            )
            .map_err(|e| {
                ImpForgeError::internal("QUEST_LEVELUP", format!("Level-up update failed: {e}"))
            })?;
        }

        // Grant material if any
        if let Some(ref mat_name) = reward.material {
            self.grant_material(&conn, mat_name)?;
        }

        // Auto-battle if the action triggers it
        let battle = if reward.monster_fight {
            Some(self.run_auto_battle(&conn, &current_zone, current_level)?)
        } else {
            None
        };

        // Update quest progress
        let quest_completed = self.update_quest_progress(&conn, action)?;

        // Also earn planet resources from every productivity action
        self.earn_planet_resources_from_action(&conn, action);

        // Also earn swarm resources from every action (drop the lock first)
        drop(conn);
        let _ = self.earn_swarm_resources(action);

        Ok(ActionResult {
            xp_earned,
            gold_earned,
            material_gained: reward.material,
            level_up,
            new_level: current_level,
            battle,
            quest_completed,
        })
    }

    // -- Auto-battle ----------------------------------------------------------

    pub fn auto_battle(&self, zone_id: &str) -> Result<BattleResult, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let level: u32 = conn
            .query_row("SELECT level FROM quest_character WHERE id = 1", [], |r| {
                r.get(0)
            })
            .map_err(|_| {
                ImpForgeError::validation("QUEST_NO_CHAR", "No character exists yet.")
            })?;

        self.run_auto_battle(&conn, zone_id, level)
    }

    fn run_auto_battle(
        &self,
        conn: &Connection,
        zone_id: &str,
        char_level: u32,
    ) -> Result<BattleResult, ImpForgeError> {
        let zones = all_zones();
        let zone = zones
            .iter()
            .find(|z| z.id == zone_id)
            .unwrap_or_else(|| &zones[0]);

        // Pick a monster from the zone (deterministic based on timestamp to avoid
        // needing the `rand` crate -- keeps the dependency list clean)
        let now = Utc::now().timestamp_millis() as usize;
        let monster = if zone.monsters.is_empty() {
            zone.boss.as_ref().unwrap_or(&zone.monsters[0]).clone()
        } else {
            let idx = now % zone.monsters.len();
            zone.monsters[idx].clone()
        };

        // Load character combat stats
        let (hp, atk, def, mag): (u32, u32, u32, u32) = conn
            .query_row(
                "SELECT hp, attack, defense, magic FROM quest_character WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|e| {
                ImpForgeError::internal("QUEST_BATTLE", format!("Load stats failed: {e}"))
            })?;

        // Add equipment bonuses
        let eq_atk: i32 = conn
            .query_row(
                "SELECT COALESCE(SUM(attack), 0) FROM quest_inventory WHERE equipped_slot IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let eq_def: i32 = conn
            .query_row(
                "SELECT COALESCE(SUM(defense), 0) FROM quest_inventory WHERE equipped_slot IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let eq_mag: i32 = conn
            .query_row(
                "SELECT COALESCE(SUM(magic), 0) FROM quest_inventory WHERE equipped_slot IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let total_atk = (atk as i32 + eq_atk).max(1) as u32;
        let total_def = (def as i32 + eq_def).max(0) as u32;
        let total_mag = (mag as i32 + eq_mag).max(0) as u32;
        let effective_power = total_atk + total_mag / 2;

        // Simple turn-based combat
        let mut char_hp = hp as i32;
        let mut mon_hp = monster.hp as i32;
        let mut rounds: u32 = 0;
        let mut total_damage_dealt: u32 = 0;
        let mut total_damage_taken: u32 = 0;

        while char_hp > 0 && mon_hp > 0 && rounds < 50 {
            rounds += 1;

            // Character attacks monster
            let char_dmg = (effective_power as i32 - monster.defense as i32 / 2).max(1);
            mon_hp -= char_dmg;
            total_damage_dealt += char_dmg as u32;

            if mon_hp <= 0 {
                break;
            }

            // Monster attacks character
            let mon_dmg = (monster.attack as i32 - total_def as i32 / 2).max(1);
            char_hp -= mon_dmg;
            total_damage_taken += mon_dmg as u32;
        }

        let victory = mon_hp <= 0;
        let xp_earned = if victory { monster.xp_reward } else { monster.xp_reward / 4 };
        let gold_earned = if victory { monster.gold_reward } else { 0 };

        // Update character HP and stats
        let new_hp = char_hp.max(1) as u32;
        conn.execute(
            "UPDATE quest_character SET hp = ?1 WHERE id = 1",
            params![new_hp],
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_HP", format!("HP update failed: {e}"))
        })?;

        if victory {
            conn.execute(
                "UPDATE quest_character SET
                    xp = xp + ?1,
                    gold = gold + ?2,
                    monsters_slain = monsters_slain + 1
                 WHERE id = 1",
                params![xp_earned, gold_earned],
            )
            .map_err(|e| {
                ImpForgeError::internal("QUEST_VICTORY", format!("Victory update failed: {e}"))
            })?;
        }

        // Log the battle
        conn.execute(
            "INSERT INTO quest_battle_log (monster_name, monster_level, victory, xp_earned, gold_earned)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![monster.name, monster.level, victory as i32, xp_earned, gold_earned],
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_BLOG", format!("Battle log failed: {e}"))
        })?;

        // Generate loot on victory
        let loot = if victory {
            self.generate_loot(conn, &monster, char_level)?
        } else {
            Vec::new()
        };

        Ok(BattleResult {
            victory,
            monster_name: monster.name,
            monster_level: monster.level,
            damage_dealt: total_damage_dealt,
            damage_taken: total_damage_taken,
            xp_earned,
            gold_earned,
            loot,
            rounds,
        })
    }

    // -- Crafting --------------------------------------------------------------

    pub fn craft_item(&self, recipe_id: &str) -> Result<Item, ImpForgeError> {
        let recipes = all_recipes();
        let recipe = recipes
            .iter()
            .find(|r| r.id == recipe_id)
            .ok_or_else(|| {
                ImpForgeError::validation("QUEST_NO_RECIPE", format!("Unknown recipe: {recipe_id}"))
            })?;

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check level requirement
        let level: u32 = conn
            .query_row("SELECT level FROM quest_character WHERE id = 1", [], |r| {
                r.get(0)
            })
            .map_err(|_| {
                ImpForgeError::validation("QUEST_NO_CHAR", "No character exists yet.")
            })?;

        if level < recipe.required_level {
            return Err(ImpForgeError::validation(
                "QUEST_LOW_LEVEL",
                format!("Requires level {}. You are level {}.", recipe.required_level, level),
            ));
        }

        // Check materials
        for (mat_id, needed) in &recipe.materials {
            let count: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM quest_inventory WHERE id LIKE ?1 AND item_type = 'material'",
                    params![format!("{mat_id}%")],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            if count < *needed {
                return Err(ImpForgeError::validation(
                    "QUEST_NO_MATERIALS",
                    format!("Need {needed}x {mat_id}, have {count}."),
                ));
            }
        }

        // Consume materials
        for (mat_id, needed) in &recipe.materials {
            let ids: Vec<String> = {
                let mut stmt = conn
                    .prepare("SELECT id FROM quest_inventory WHERE id LIKE ?1 AND item_type = 'material' LIMIT ?2")
                    .map_err(|e| ImpForgeError::internal("QUEST_CRAFT", format!("{e}")))?;
                let rows = stmt
                    .query_map(params![format!("{mat_id}%"), needed], |r| r.get(0))
                    .map_err(|e| ImpForgeError::internal("QUEST_CRAFT", format!("{e}")))?;
                rows.filter_map(|r| r.ok()).collect()
            };
            for id in ids {
                conn.execute("DELETE FROM quest_inventory WHERE id = ?1", params![id])
                    .map_err(|e| {
                        ImpForgeError::internal("QUEST_CRAFT_DEL", format!("{e}"))
                    })?;
            }
        }

        // Create the crafted item
        let item = generate_item_from_recipe(recipe, level);
        conn.execute(
            "INSERT INTO quest_inventory (id, name, item_type, rarity, attack, defense, magic, hp_bonus, level_req, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                item.id,
                item.name,
                item.item_type,
                item.rarity.as_str(),
                item.stats.attack,
                item.stats.defense,
                item.stats.magic,
                item.stats.hp_bonus,
                item.level_req,
                item.description,
            ],
        )
        .map_err(|e| {
            ImpForgeError::internal("QUEST_CRAFT_INSERT", format!("{e}"))
        })?;

        Ok(item)
    }

    // -- Equipment management -------------------------------------------------

    pub fn equip_item(&self, item_id: &str, slot: &str) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Unequip current item in that slot
        conn.execute(
            "UPDATE quest_inventory SET equipped_slot = NULL WHERE equipped_slot = ?1",
            params![slot],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_UNEQUIP", format!("{e}")))?;

        // Equip the new item
        let rows = conn
            .execute(
                "UPDATE quest_inventory SET equipped_slot = ?1 WHERE id = ?2 AND item_type != 'material'",
                params![slot, item_id],
            )
            .map_err(|e| ImpForgeError::internal("QUEST_EQUIP", format!("{e}")))?;

        if rows == 0 {
            return Err(ImpForgeError::validation(
                "QUEST_ITEM_NOT_FOUND",
                format!("Item '{item_id}' not found or is a material."),
            ));
        }

        Ok(())
    }

    pub fn unequip(&self, slot: &str) -> Result<(), ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.execute(
            "UPDATE quest_inventory SET equipped_slot = NULL WHERE equipped_slot = ?1",
            params![slot],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_UNEQUIP", format!("{e}")))?;

        Ok(())
    }

    // -- Skills ---------------------------------------------------------------

    pub fn invest_skill(&self, skill_id: &str) -> Result<Skill, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check skill points
        let sp: u32 = conn
            .query_row("SELECT skill_points FROM quest_character WHERE id = 1", [], |r| {
                r.get(0)
            })
            .map_err(|_| {
                ImpForgeError::validation("QUEST_NO_CHAR", "No character exists yet.")
            })?;

        if sp == 0 {
            return Err(ImpForgeError::validation(
                "QUEST_NO_SP",
                "No skill points available.",
            ));
        }

        // Check the skill exists and is not maxed
        let (pts, max_pts): (u32, u32) = conn
            .query_row(
                "SELECT points_invested, max_points FROM quest_skills WHERE id = ?1",
                params![skill_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "QUEST_SKILL_NOT_FOUND",
                    format!("Skill '{skill_id}' not found."),
                )
            })?;

        if pts >= max_pts {
            return Err(ImpForgeError::validation(
                "QUEST_SKILL_MAX",
                format!("Skill '{skill_id}' is already maxed ({max_pts}/{max_pts})."),
            ));
        }

        // Invest
        conn.execute(
            "UPDATE quest_skills SET points_invested = points_invested + 1 WHERE id = ?1",
            params![skill_id],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_SKILL", format!("{e}")))?;

        conn.execute(
            "UPDATE quest_character SET skill_points = skill_points - 1 WHERE id = 1",
            [],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_SKILL_SP", format!("{e}")))?;

        // Return updated skill
        conn.query_row(
            "SELECT id, name, description, tier, points_invested, max_points, prerequisite, effect, branch
             FROM quest_skills WHERE id = ?1",
            params![skill_id],
            |r| {
                Ok(Skill {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    tier: r.get(3)?,
                    points_invested: r.get(4)?,
                    max_points: r.get(5)?,
                    prerequisite: r.get(6)?,
                    effect: r.get(7)?,
                    branch: SkillBranch::from_str(&r.get::<_, String>(8)?),
                })
            },
        )
        .map_err(|e| ImpForgeError::internal("QUEST_SKILL_READ", format!("{e}")))
    }

    // -- Data loaders ---------------------------------------------------------

    fn load_inventory(&self, conn: &Connection) -> Result<Vec<Item>, ImpForgeError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, item_type, rarity, attack, defense, magic, hp_bonus, level_req, description
                 FROM quest_inventory ORDER BY rarity DESC, name",
            )
            .map_err(|e| ImpForgeError::internal("QUEST_INV", format!("{e}")))?;

        let items = stmt
            .query_map([], |r| {
                Ok(Item {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    item_type: r.get(2)?,
                    rarity: ItemRarity::from_str(&r.get::<_, String>(3)?),
                    stats: ItemStats {
                        attack: r.get(4)?,
                        defense: r.get(5)?,
                        magic: r.get(6)?,
                        hp_bonus: r.get(7)?,
                    },
                    level_req: r.get(8)?,
                    description: r.get(9)?,
                })
            })
            .map_err(|e| ImpForgeError::internal("QUEST_INV_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    fn build_equipment(&self, conn: &Connection) -> Result<Equipment, ImpForgeError> {
        let mut eq = Equipment::default();

        let mut stmt = conn
            .prepare(
                "SELECT id, name, item_type, rarity, attack, defense, magic, hp_bonus, level_req, description, equipped_slot
                 FROM quest_inventory WHERE equipped_slot IS NOT NULL",
            )
            .map_err(|e| ImpForgeError::internal("QUEST_EQ", format!("{e}")))?;

        let items: Vec<(Item, String)> = stmt
            .query_map([], |r| {
                Ok((
                    Item {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        item_type: r.get(2)?,
                        rarity: ItemRarity::from_str(&r.get::<_, String>(3)?),
                        stats: ItemStats {
                            attack: r.get(4)?,
                            defense: r.get(5)?,
                            magic: r.get(6)?,
                            hp_bonus: r.get(7)?,
                        },
                        level_req: r.get(8)?,
                        description: r.get(9)?,
                    },
                    r.get::<_, String>(10)?,
                ))
            })
            .map_err(|e| ImpForgeError::internal("QUEST_EQ_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        for (item, slot) in items {
            match slot.as_str() {
                "weapon" => eq.weapon = Some(item),
                "head" => eq.head = Some(item),
                "chest" => eq.chest = Some(item),
                "legs" => eq.legs = Some(item),
                "boots" => eq.boots = Some(item),
                "gloves" => eq.gloves = Some(item),
                "accessory1" => eq.accessory1 = Some(item),
                "accessory2" => eq.accessory2 = Some(item),
                _ => {}
            }
        }

        Ok(eq)
    }

    fn load_skills(&self, conn: &Connection) -> Result<Vec<Skill>, ImpForgeError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, tier, points_invested, max_points, prerequisite, effect, branch
                 FROM quest_skills ORDER BY branch, tier, name",
            )
            .map_err(|e| ImpForgeError::internal("QUEST_SKILLS", format!("{e}")))?;

        let skills = stmt
            .query_map([], |r| {
                Ok(Skill {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    tier: r.get(3)?,
                    points_invested: r.get(4)?,
                    max_points: r.get(5)?,
                    prerequisite: r.get(6)?,
                    effect: r.get(7)?,
                    branch: SkillBranch::from_str(&r.get::<_, String>(8)?),
                })
            })
            .map_err(|e| ImpForgeError::internal("QUEST_SKILLS_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(skills)
    }

    pub fn get_quests(&self) -> Result<Vec<Quest>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("QUEST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, objective, objective_target, objective_progress,
                        reward_xp, reward_gold, reward_items, completed
                 FROM quest_quests ORDER BY completed ASC, reward_xp DESC",
            )
            .map_err(|e| ImpForgeError::internal("QUEST_QUESTS", format!("{e}")))?;

        let quests = stmt
            .query_map([], |r| {
                let items_json: String = r.get(8)?;
                let items: Vec<String> =
                    serde_json::from_str(&items_json).unwrap_or_default();
                Ok(Quest {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    objective: r.get(3)?,
                    objective_target: r.get(4)?,
                    objective_progress: r.get(5)?,
                    reward_xp: r.get(6)?,
                    reward_gold: r.get(7)?,
                    reward_items: items,
                    completed: r.get::<_, i32>(9)? != 0,
                })
            })
            .map_err(|e| ImpForgeError::internal("QUEST_QUESTS_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(quests)
    }

    pub fn get_leaderboard(&self) -> Result<Vec<LeaderboardEntry>, ImpForgeError> {
        // Single-player leaderboard: show the player's character as a single entry.
        // In a future version this could sync with a server for multiplayer rankings.
        let char = self.get_character()?;
        Ok(vec![LeaderboardEntry {
            name: char.name,
            class: char.class,
            level: char.level,
            xp: char.xp,
            monsters_slain: char.monsters_slain,
            quests_completed: char.quests_completed,
        }])
    }

    // -- Internal helpers -----------------------------------------------------

    fn grant_material(&self, conn: &Connection, mat_name: &str) -> Result<(), ImpForgeError> {
        let id = format!("mat_{}_{}", mat_name.to_lowercase().replace(' ', "_"), Utc::now().timestamp_millis());
        conn.execute(
            "INSERT INTO quest_inventory (id, name, item_type, rarity, description)
             VALUES (?1, ?2, 'material', 'common', ?3)",
            params![id, mat_name, format!("A {mat_name} gathered from your labors.")],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_MAT", format!("{e}")))?;
        Ok(())
    }

    fn generate_loot(
        &self,
        conn: &Connection,
        monster: &Monster,
        _char_level: u32,
    ) -> Result<Vec<Item>, ImpForgeError> {
        let now = Utc::now().timestamp_millis();
        let mut loot = Vec::new();

        for (item_name, drop_chance) in &monster.loot_table {
            // Deterministic pseudo-random: hash the timestamp with the item name
            let hash = now.wrapping_mul(item_name.len() as i64 + 31) % 100;
            if (hash as f32) < (*drop_chance * 100.0) {
                let rarity = if *drop_chance < 0.05 {
                    ItemRarity::Epic
                } else if *drop_chance < 0.15 {
                    ItemRarity::Rare
                } else if *drop_chance < 0.30 {
                    ItemRarity::Uncommon
                } else {
                    ItemRarity::Common
                };

                let item = Item {
                    id: format!("loot_{}_{}", item_name.to_lowercase().replace(' ', "_"), now),
                    name: item_name.clone(),
                    item_type: "material".to_string(),
                    rarity: rarity.clone(),
                    stats: ItemStats { attack: 0, defense: 0, magic: 0, hp_bonus: 0 },
                    level_req: 1,
                    description: format!("Dropped by {}", monster.name),
                };

                conn.execute(
                    "INSERT INTO quest_inventory (id, name, item_type, rarity, description)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![item.id, item.name, item.item_type, rarity.as_str(), item.description],
                )
                .map_err(|e| ImpForgeError::internal("QUEST_LOOT", format!("{e}")))?;

                loot.push(item);
            }
        }

        Ok(loot)
    }

    fn update_quest_progress(
        &self,
        conn: &Connection,
        action: &str,
    ) -> Result<Option<String>, ImpForgeError> {
        let objective_type = match action {
            "create_document" | "create_note" => "create_documents",
            "run_workflow" => "run_workflows",
            "ai_query" => "ai_queries",
            "create_spreadsheet" => "craft_items",
            _ => return Ok(None),
        };

        // Increment matching active quests
        conn.execute(
            "UPDATE quest_quests SET objective_progress = objective_progress + 1
             WHERE objective = ?1 AND completed = 0",
            params![objective_type],
        )
        .map_err(|e| ImpForgeError::internal("QUEST_PROGRESS", format!("{e}")))?;

        // Check if any quest just completed
        let completed: Option<(String, u64, u64)> = conn
            .query_row(
                "SELECT id, reward_xp, reward_gold FROM quest_quests
                 WHERE completed = 0 AND objective_progress >= objective_target
                 LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();

        if let Some((quest_id, rxp, rgold)) = completed {
            conn.execute(
                "UPDATE quest_quests SET completed = 1 WHERE id = ?1",
                params![quest_id],
            )
            .map_err(|e| ImpForgeError::internal("QUEST_COMPLETE", format!("{e}")))?;

            conn.execute(
                "UPDATE quest_character SET xp = xp + ?1, gold = gold + ?2, quests_completed = quests_completed + 1 WHERE id = 1",
                params![rxp, rgold],
            )
            .map_err(|e| ImpForgeError::internal("QUEST_REWARD", format!("{e}")))?;

            return Ok(Some(quest_id));
        }

        Ok(None)
    }

    fn seed_quests(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let quests = vec![
            ("q_first_doc", "The Scribe's Trial", "Write your first document to earn your quill.", "create_documents", 1u32, 50u64, 25u64),
            ("q_docs_5", "Manuscript Master", "Create 5 documents to prove your scholarly might.", "create_documents", 5, 150, 75),
            ("q_first_workflow", "The Automaton's Apprentice", "Execute your first workflow.", "run_workflows", 1, 75, 40),
            ("q_workflows_3", "Clockwork Commander", "Run 3 workflows to master automation.", "run_workflows", 3, 200, 100),
            ("q_ai_10", "The Oracle's Student", "Make 10 AI queries to learn the arcane arts.", "ai_queries", 10, 100, 50),
            ("q_ai_50", "Spellweaver", "Cast 50 AI spells (queries) to ascend.", "ai_queries", 50, 300, 150),
            ("q_craft_3", "Apprentice Forgemaster", "Craft 3 items at the forge.", "craft_items", 3, 120, 60),
            ("q_slay_10", "Monster Hunter", "Slay 10 monsters in battle.", "slay_monsters", 10, 200, 100),
            ("q_slay_50", "Legendary Slayer", "Defeat 50 monsters across the realm.", "slay_monsters", 50, 500, 250),
            ("q_modules_5", "Jack of All Trades", "Use 5 different ImpForge modules.", "use_modules", 5, 150, 75),
        ];

        for (id, name, desc, obj, target, rxp, rgold) in quests {
            conn.execute(
                "INSERT OR IGNORE INTO quest_quests (id, name, description, objective, objective_target, reward_xp, reward_gold)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, name, desc, obj, target, rxp, rgold],
            )
            .map_err(|e| ImpForgeError::internal("QUEST_SEED", format!("{e}")))?;
        }

        Ok(())
    }

    fn seed_skills(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let skills = vec![
            // Combat branch
            ("sk_power_strike", "Power Strike", "Increases base attack damage.", 1u32, 5u32, None::<&str>, "+10% attack per point", "combat"),
            ("sk_crit_chance", "Critical Eye", "Chance to deal double damage.", 2, 5, Some("sk_power_strike"), "+4% crit chance per point", "combat"),
            ("sk_berserker", "Berserker Rage", "More damage when HP is low.", 3, 3, Some("sk_crit_chance"), "+20% damage below 30% HP per point", "combat"),
            // Defense branch
            ("sk_iron_skin", "Iron Skin", "Reduces incoming damage.", 1, 5, None, "+5% damage reduction per point", "defense"),
            ("sk_hp_boost", "Vitality", "Increases maximum HP.", 2, 5, Some("sk_iron_skin"), "+15 max HP per point", "defense"),
            ("sk_regen", "Regeneration", "Recover HP after each battle.", 3, 3, Some("sk_hp_boost"), "+5 HP regen per point", "defense"),
            // Magic branch
            ("sk_arcane_power", "Arcane Power", "Boosts magic damage.", 1, 5, None, "+10% magic per point", "magic"),
            ("sk_mana_flow", "Mana Flow", "AI queries grant bonus XP.", 2, 5, Some("sk_arcane_power"), "+5% AI XP bonus per point", "magic"),
            ("sk_spell_mastery", "Spell Mastery", "Chance for double rewards from AI actions.", 3, 3, Some("sk_mana_flow"), "+8% double reward chance per point", "magic"),
            // Crafting branch
            ("sk_efficient", "Efficient Crafting", "Chance to save materials.", 1, 5, None, "+5% material save chance per point", "crafting"),
            ("sk_quality", "Master Quality", "Crafted items have higher stats.", 2, 5, Some("sk_efficient"), "+10% crafted item stats per point", "crafting"),
            ("sk_rare_craft", "Rare Discovery", "Chance to craft at higher rarity.", 3, 3, Some("sk_quality"), "+5% rarity upgrade chance per point", "crafting"),
            // Leadership branch
            ("sk_gold_find", "Gold Finder", "Monsters drop more gold.", 1, 5, None, "+10% gold from battles per point", "leadership"),
            ("sk_team_spirit", "Team Spirit", "Team contributions grant bonus XP.", 2, 5, Some("sk_gold_find"), "+10% team XP bonus per point", "leadership"),
            ("sk_commander", "Commander", "All stat bonuses increased.", 3, 3, Some("sk_team_spirit"), "+3% all stats per point", "leadership"),
            // Wisdom branch
            ("sk_quick_study", "Quick Study", "All actions grant bonus XP.", 1, 5, None, "+5% XP from all actions per point", "wisdom"),
            ("sk_treasure_sense", "Treasure Sense", "Better loot drop rates.", 2, 5, Some("sk_quick_study"), "+5% loot chance per point", "wisdom"),
            ("sk_enlightenment", "Enlightenment", "Massive XP bonus for diverse module usage.", 3, 3, Some("sk_treasure_sense"), "+15% XP when switching modules per point", "wisdom"),
        ];

        for (id, name, desc, tier, max_pts, prereq, effect, branch) in skills {
            conn.execute(
                "INSERT OR IGNORE INTO quest_skills (id, name, description, tier, max_points, prerequisite, effect, branch)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![id, name, desc, tier, max_pts, prereq, effect, branch],
            )
            .map_err(|e| ImpForgeError::internal("QUEST_SKILL_SEED", format!("{e}")))?;
        }

        Ok(())
    }

    // =========================================================================
    // Forge Swarm — Colony-building meta-game
    // =========================================================================

    fn seed_swarm_buildings(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let buildings = [
            "nest", "evolution_chamber", "essence_pool", "neural_web",
            "armory", "sanctuary", "arcanum", "war_council",
        ];
        for bt in &buildings {
            conn.execute(
                "INSERT OR IGNORE INTO swarm_buildings (id, building_type, level) VALUES (?1, ?1, 0)",
                params![bt],
            )
            .map_err(|e| ImpForgeError::internal("SWARM_BLDG_SEED", format!("{e}")))?;
        }
        Ok(())
    }

    fn seed_swarm_missions(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        #[allow(clippy::type_complexity)]
        let missions: Vec<(&str, &str, &str, &str, u32, u32, u64, u64, u64, u64, u64, &str)> = vec![
            ("m_gather", "Gather Essence", "Send a Drone to collect raw Essence from the forge.", "forge_drone", 1, 5, 50, 0, 0, 0, 0, "[]"),
            ("m_scout_web", "Scout the Web", "A Skyweaver scouts the internet for useful data.", "skyweaver", 1, 10, 30, 0, 10, 5, 0, "[\"Web Scroll\"]"),
            ("m_defend", "Defend the Hive", "Shadow Weavers patrol the perimeter for threats.", "shadow_weaver", 2, 15, 40, 0, 0, 0, 0, "[\"Security Report\"]"),
            ("m_raid_mine", "Raid the Data Mine", "Vipers infiltrate a rich data deposit.", "viper", 3, 20, 60, 200, 0, 0, 0, "[]"),
            ("m_arcane", "Arcane Research", "A Titan delves into deep reasoning and analysis.", "titan", 1, 30, 100, 0, 100, 0, 0, "[\"Arcane Tome\"]"),
            ("m_breed", "Breed New Larva", "The Swarm Mother produces new offspring for the hive.", "swarm_mother", 1, 60, 20, 0, 0, 20, 0, "[\"Larva Egg\",\"Larva Egg\",\"Larva Egg\"]"),
            ("m_boss", "Boss Challenge", "Assemble an elite squad to face a fearsome foe.", "any", 5, 45, 500, 50, 50, 50, 10, "[\"Legendary Token\"]"),
            ("m_neural", "Neural Expansion", "Overseers map the neural pathways of the hive mind.", "overseer", 2, 30, 80, 0, 0, 30, 0, "[\"Neural Fragment\"]"),
            ("m_dark", "Dark Matter Harvest", "A Ravager ventures into the void to harvest dark matter.", "ravager", 1, 40, 60, 0, 0, 0, 50, "[]"),
            ("m_final", "The Final Evolution", "The ultimate test. Matriarch leads the Titans to ascend.", "matriarch", 6, 120, 2000, 200, 200, 200, 100, "[\"Mythic Core\"]"),
            // Expanded missions (10)
            ("m_terraform", "Terraform New World", "Deploy drones and a skyweaver to colonize a barren planet.", "forge_drone", 3, 45, 300, 200, 0, 0, 0, "[\"Colony Blueprint\"]"),
            ("m_psi_recon", "Psionic Reconnaissance", "Overseers scan for enemy hive signals.", "overseer", 2, 20, 100, 0, 0, 0, 0, "[\"Intel Report\"]"),
            ("m_deep_harvest", "Deep Space Harvest", "A LeechHauler ventures into deep space for resources.", "any", 1, 60, 500, 200, 0, 0, 0, "[]"),
            ("m_genetic", "Genetic Breakthrough", "The Matriarch unlocks a new DNA sequence.", "matriarch", 1, 90, 200, 0, 100, 100, 50, "[\"DNA Sequence\"]"),
            ("m_infest", "Infest Enemy Colony", "Infestors infiltrate and convert enemy units.", "any", 2, 30, 150, 100, 50, 0, 0, "[\"Captured Unit\"]"),
            ("m_nydus", "Build Nydus Tunnel", "Establish a new Nydus transport route.", "any", 3, 45, 200, 150, 0, 50, 0, "[\"Tunnel Map\"]"),
            ("m_orbital", "Orbital Strike", "Fleet ships bombard enemy structures from orbit.", "any", 3, 30, 300, 0, 100, 0, 20, "[\"Victory Medal\"]"),
            ("m_mass_evolve", "Mass Evolution Event", "A grand evolution ceremony for the swarm.", "any", 5, 120, 1000, 100, 100, 100, 50, "[\"Evolution Catalyst\"]"),
            ("m_dark_harvest", "Harvest Dark Matter", "A Titan ventures into the void rift for dark matter.", "titan", 1, 60, 100, 0, 0, 0, 100, "[]"),
            ("m_convergence", "The Grand Convergence", "All forces unite for the ultimate mission.", "any", 8, 180, 5000, 500, 500, 500, 200, "[\"Mythic Core\",\"Legendary Token\"]"),
        ];

        for (id, name, desc, req_types, req_count, dur, ess, min, ves, bio, dm, items) in &missions {
            conn.execute(
                "INSERT OR IGNORE INTO swarm_missions
                 (id, name, description, required_unit_types, required_unit_count,
                  duration_minutes, reward_essence, reward_minerals, reward_vespene,
                  reward_biomass, reward_dark_matter, reward_items)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    id, name, desc,
                    format!("[\"{req_types}\"]"),
                    req_count, dur, ess, min, ves, bio, dm, items
                ],
            )
            .map_err(|e| ImpForgeError::internal("SWARM_MISSION_SEED", format!("{e}")))?;
        }
        Ok(())
    }

    // -- Swarm state ----------------------------------------------------------

    pub fn get_swarm(&self) -> Result<SwarmState, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let units = self.load_swarm_units(&conn)?;
        let buildings = self.load_swarm_buildings(&conn)?;
        let resources = self.load_swarm_resources(&conn)?;

        // Calculate max units from Nest level
        let nest_level = buildings.iter()
            .find(|b| b.building_type == BuildingType::Nest)
            .map(|b| b.level)
            .unwrap_or(0);
        let max_units = 10 + nest_level * 5;

        // Calculate max essence from EssencePool level
        let pool_level = buildings.iter()
            .find(|b| b.building_type == BuildingType::EssencePool)
            .map(|b| b.level)
            .unwrap_or(0);
        let max_essence = 1000 + (pool_level as u64) * 1000;

        let evolution_paths = all_evolution_paths();

        Ok(SwarmState {
            units,
            buildings,
            resources,
            max_units,
            max_essence,
            evolution_paths,
        })
    }

    fn load_swarm_units(&self, conn: &Connection) -> Result<Vec<SwarmUnit>, ImpForgeError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, unit_type, name, level, hp, attack, defense,
                        special_ability, assigned_task, efficiency
                 FROM swarm_units ORDER BY level DESC, name",
            )
            .map_err(|e| ImpForgeError::internal("SWARM_UNITS", format!("{e}")))?;

        let units = stmt
            .query_map([], |r| {
                let ut = UnitType::from_str(&r.get::<_, String>(1)?);
                Ok(SwarmUnit {
                    id: r.get(0)?,
                    unit_type: ut,
                    name: r.get(2)?,
                    level: r.get(3)?,
                    hp: r.get(4)?,
                    attack: r.get(5)?,
                    defense: r.get(6)?,
                    special_ability: r.get(7)?,
                    assigned_task: r.get(8)?,
                    efficiency: r.get(9)?,
                })
            })
            .map_err(|e| ImpForgeError::internal("SWARM_UNITS_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(units)
    }

    fn load_swarm_buildings(&self, conn: &Connection) -> Result<Vec<Building>, ImpForgeError> {
        let mut stmt = conn
            .prepare("SELECT id, building_type, level FROM swarm_buildings ORDER BY building_type")
            .map_err(|e| ImpForgeError::internal("SWARM_BLDG", format!("{e}")))?;

        let buildings = stmt
            .query_map([], |r| {
                let bt = BuildingType::from_str(&r.get::<_, String>(1)?);
                let level: u32 = r.get(2)?;
                Ok(Building {
                    id: r.get(0)?,
                    building_type: bt.clone(),
                    level,
                    max_level: bt.max_level(),
                    bonus: bt.bonus_description(level),
                    upgrade_cost: bt.base_upgrade_cost() * (level as u64 + 1),
                })
            })
            .map_err(|e| ImpForgeError::internal("SWARM_BLDG_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(buildings)
    }

    fn load_swarm_resources(&self, conn: &Connection) -> Result<SwarmResources, ImpForgeError> {
        conn.query_row(
            "SELECT essence, minerals, vespene, biomass, dark_matter FROM swarm_resources WHERE id = 1",
            [],
            |r| {
                Ok(SwarmResources {
                    essence: r.get(0)?,
                    minerals: r.get(1)?,
                    vespene: r.get(2)?,
                    biomass: r.get(3)?,
                    dark_matter: r.get(4)?,
                })
            },
        )
        .map_err(|e| ImpForgeError::internal("SWARM_RES", format!("{e}")))
    }

    // -- Spawn & Evolve -------------------------------------------------------

    pub fn spawn_larva(&self) -> Result<SwarmUnit, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check unit cap
        let unit_count: u32 = conn
            .query_row("SELECT COUNT(*) FROM swarm_units", [], |r| r.get(0))
            .unwrap_or(0);

        let nest_level: u32 = conn
            .query_row(
                "SELECT level FROM swarm_buildings WHERE building_type = 'nest'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let max_units = 10 + nest_level * 5;
        if unit_count >= max_units {
            return Err(ImpForgeError::validation(
                "SWARM_CAP",
                format!("Unit cap reached ({unit_count}/{max_units}). Upgrade your Nest."),
            ));
        }

        // Spawning a Larva costs 25 Essence (first one free if essence >= 25)
        let essence: u64 = conn
            .query_row("SELECT essence FROM swarm_resources WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);

        let spawn_cost: u64 = 25;
        if essence < spawn_cost {
            return Err(ImpForgeError::validation(
                "SWARM_NO_ESSENCE",
                format!("Need {spawn_cost} Essence to spawn a Larva. Have {essence}."),
            ));
        }

        conn.execute(
            "UPDATE swarm_resources SET essence = essence - ?1 WHERE id = 1",
            params![spawn_cost],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_SPEND", format!("{e}")))?;

        // Create the larva as a ForgeDrone (Tier 1 default)
        let now = Utc::now().timestamp_millis();
        let id = format!("larva_{now}");
        let name = format!("Larva #{}", unit_count + 1);
        let (hp, atk, def) = UnitType::ForgeDrone.base_stats();

        let unit = SwarmUnit {
            id: id.clone(),
            unit_type: UnitType::ForgeDrone,
            name: name.clone(),
            level: 1,
            hp,
            attack: atk,
            defense: def,
            special_ability: UnitType::ForgeDrone.special_ability().to_string(),
            assigned_task: None,
            efficiency: 0.5,
        };

        conn.execute(
            "INSERT INTO swarm_units (id, unit_type, name, level, hp, attack, defense, special_ability, efficiency)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                unit.id, unit.unit_type.as_str(), unit.name,
                unit.level, unit.hp, unit.attack, unit.defense,
                unit.special_ability, unit.efficiency,
            ],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_SPAWN", format!("{e}")))?;

        Ok(unit)
    }

    pub fn evolve_unit(&self, unit_id: &str, target_type: &str) -> Result<SwarmUnit, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Load the unit
        let (current_type_str, level, efficiency): (String, u32, f32) = conn
            .query_row(
                "SELECT unit_type, level, efficiency FROM swarm_units WHERE id = ?1",
                params![unit_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| {
                ImpForgeError::validation("SWARM_NO_UNIT", format!("Unit '{unit_id}' not found."))
            })?;

        let target = UnitType::from_str(target_type);

        // Find the evolution path
        let paths = all_evolution_paths();
        let path = paths.iter()
            .find(|p| p.from == current_type_str && p.to == target.as_str())
            .ok_or_else(|| {
                ImpForgeError::validation(
                    "SWARM_NO_PATH",
                    format!("No evolution path from '{}' to '{}'.", current_type_str, target.as_str()),
                )
            })?;

        // Check level requirement
        if level < path.level_requirement {
            return Err(ImpForgeError::validation(
                "SWARM_LOW_LEVEL",
                format!("Unit needs level {} (currently {})", path.level_requirement, level),
            ));
        }

        // Check evolution chamber level (must be >= target tier)
        let chamber_level: u32 = conn
            .query_row(
                "SELECT level FROM swarm_buildings WHERE building_type = 'evolution_chamber'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if chamber_level < target.tier() {
            return Err(ImpForgeError::validation(
                "SWARM_CHAMBER",
                format!(
                    "Evolution Chamber level {} required (have {}). Upgrade it first.",
                    target.tier(), chamber_level
                ),
            ));
        }

        // Matriarch uniqueness check
        if target == UnitType::Matriarch {
            let existing: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM swarm_units WHERE unit_type = 'matriarch'",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if existing > 0 {
                return Err(ImpForgeError::validation(
                    "SWARM_MATRIARCH_UNIQUE",
                    "Only one Matriarch may exist in the swarm.",
                ));
            }
        }

        // Check and spend Essence
        let essence: u64 = conn
            .query_row("SELECT essence FROM swarm_resources WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);

        if essence < path.essence_cost {
            return Err(ImpForgeError::validation(
                "SWARM_NO_ESSENCE",
                format!("Need {} Essence (have {}).", path.essence_cost, essence),
            ));
        }

        conn.execute(
            "UPDATE swarm_resources SET essence = essence - ?1 WHERE id = 1",
            params![path.essence_cost],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_EVOLVE_PAY", format!("{e}")))?;

        // Evolve the unit
        let (new_hp, new_atk, new_def) = target.base_stats();
        // Carry over efficiency bonus
        let evolved_efficiency = (efficiency + 0.1).min(2.0);

        conn.execute(
            "UPDATE swarm_units SET unit_type = ?1, hp = ?2, attack = ?3, defense = ?4,
             special_ability = ?5, efficiency = ?6 WHERE id = ?7",
            params![
                target.as_str(), new_hp, new_atk, new_def,
                target.special_ability(), evolved_efficiency, unit_id,
            ],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_EVOLVE", format!("{e}")))?;

        // Return the evolved unit
        let unit = SwarmUnit {
            id: unit_id.to_string(),
            unit_type: target.clone(),
            name: format!("{} (evolved)", target.emoji()),
            level,
            hp: new_hp,
            attack: new_atk,
            defense: new_def,
            special_ability: target.special_ability().to_string(),
            assigned_task: None,
            efficiency: evolved_efficiency,
        };

        Ok(unit)
    }

    // -- Buildings ------------------------------------------------------------

    pub fn upgrade_building(&self, building_type: &str) -> Result<Building, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let bt = BuildingType::from_str(building_type);

        let current_level: u32 = conn
            .query_row(
                "SELECT level FROM swarm_buildings WHERE building_type = ?1",
                params![bt.as_str()],
                |r| r.get(0),
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "SWARM_NO_BLDG",
                    format!("Building '{}' not found.", building_type),
                )
            })?;

        if current_level >= bt.max_level() {
            return Err(ImpForgeError::validation(
                "SWARM_BLDG_MAX",
                format!("'{}' is already at max level {}.", building_type, bt.max_level()),
            ));
        }

        let cost = bt.base_upgrade_cost() * (current_level as u64 + 1);
        let essence: u64 = conn
            .query_row("SELECT essence FROM swarm_resources WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);

        if essence < cost {
            return Err(ImpForgeError::validation(
                "SWARM_NO_ESSENCE",
                format!("Need {} Essence to upgrade (have {}).", cost, essence),
            ));
        }

        conn.execute(
            "UPDATE swarm_resources SET essence = essence - ?1 WHERE id = 1",
            params![cost],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_BLDG_PAY", format!("{e}")))?;

        let new_level = current_level + 1;
        conn.execute(
            "UPDATE swarm_buildings SET level = ?1 WHERE building_type = ?2",
            params![new_level, bt.as_str()],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_BLDG_UP", format!("{e}")))?;

        Ok(Building {
            id: bt.as_str().to_string(),
            building_type: bt.clone(),
            level: new_level,
            max_level: bt.max_level(),
            bonus: bt.bonus_description(new_level),
            upgrade_cost: bt.base_upgrade_cost() * (new_level as u64 + 1),
        })
    }

    // -- Missions -------------------------------------------------------------

    pub fn get_missions(&self) -> Result<Vec<SwarmMission>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;
        self.load_swarm_missions(&conn)
    }

    fn load_swarm_missions(&self, conn: &Connection) -> Result<Vec<SwarmMission>, ImpForgeError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, required_unit_types, required_unit_count,
                        assigned_units, duration_minutes, reward_essence, reward_minerals,
                        reward_vespene, reward_biomass, reward_dark_matter, reward_items,
                        status, started_at
                 FROM swarm_missions ORDER BY status, duration_minutes",
            )
            .map_err(|e| ImpForgeError::internal("SWARM_MISSIONS", format!("{e}")))?;

        let missions = stmt
            .query_map([], |r| {
                let req_types_json: String = r.get(3)?;
                let assigned_json: String = r.get(5)?;
                let items_json: String = r.get(12)?;
                Ok(SwarmMission {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    required_unit_types: serde_json::from_str(&req_types_json).unwrap_or_default(),
                    required_unit_count: r.get(4)?,
                    assigned_units: serde_json::from_str(&assigned_json).unwrap_or_default(),
                    duration_minutes: r.get(6)?,
                    reward: SwarmResources {
                        essence: r.get(7)?,
                        minerals: r.get(8)?,
                        vespene: r.get(9)?,
                        biomass: r.get(10)?,
                        dark_matter: r.get(11)?,
                    },
                    reward_items: serde_json::from_str(&items_json).unwrap_or_default(),
                    status: MissionStatus::from_str(&r.get::<_, String>(13)?),
                    started_at: r.get(14)?,
                })
            })
            .map_err(|e| ImpForgeError::internal("SWARM_MISSIONS_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(missions)
    }

    pub fn assign_mission(
        &self,
        mission_id: &str,
        unit_ids: Vec<String>,
    ) -> Result<SwarmMission, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check mission exists and is available
        let status_str: String = conn
            .query_row(
                "SELECT status FROM swarm_missions WHERE id = ?1",
                params![mission_id],
                |r| r.get(0),
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "SWARM_NO_MISSION",
                    format!("Mission '{}' not found.", mission_id),
                )
            })?;

        if status_str != MissionStatus::Available.as_str() {
            return Err(ImpForgeError::validation(
                "SWARM_MISSION_BUSY",
                format!("Mission '{}' is not available (status: {}).", mission_id, status_str),
            ));
        }

        // Verify all units exist and are not already assigned
        for uid in &unit_ids {
            let task: Option<String> = conn
                .query_row(
                    "SELECT assigned_task FROM swarm_units WHERE id = ?1",
                    params![uid],
                    |r| r.get(0),
                )
                .map_err(|_| {
                    ImpForgeError::validation(
                        "SWARM_NO_UNIT",
                        format!("Unit '{}' not found.", uid),
                    )
                })?;

            if task.is_some() {
                return Err(ImpForgeError::validation(
                    "SWARM_UNIT_BUSY",
                    format!("Unit '{}' is already on a task.", uid),
                ));
            }
        }

        // Assign units to the mission
        let assigned_json = serde_json::to_string(&unit_ids).unwrap_or_else(|_| "[]".to_string());
        let now = Utc::now().to_rfc3339();

        conn.execute(
            &format!(
                "UPDATE swarm_missions SET status = '{}', assigned_units = ?1, started_at = ?2 WHERE id = ?3",
                MissionStatus::InProgress.as_str()
            ),
            params![assigned_json, now, mission_id],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_ASSIGN", format!("{e}")))?;

        // Mark units as assigned
        for uid in &unit_ids {
            conn.execute(
                "UPDATE swarm_units SET assigned_task = ?1 WHERE id = ?2",
                params![mission_id, uid],
            )
            .map_err(|e| ImpForgeError::internal("SWARM_UNIT_ASSIGN", format!("{e}")))?;
        }

        // Return updated mission
        let missions = self.load_swarm_missions(&conn)?;
        missions
            .into_iter()
            .find(|m| m.id == mission_id)
            .ok_or_else(|| {
                ImpForgeError::internal("SWARM_MISSION_LOST", "Mission disappeared after assign.")
            })
    }

    pub fn collect_mission(&self, mission_id: &str) -> Result<MissionReward, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Load mission
        #[allow(clippy::type_complexity)]
        let (status_str, started_at_opt, duration, name,
             r_ess, r_min, r_ves, r_bio, r_dm, items_json, assigned_json): (
            String, Option<String>, u32, String,
            u64, u64, u64, u64, u64, String, String,
        ) = conn
            .query_row(
                "SELECT status, started_at, duration_minutes, name,
                        reward_essence, reward_minerals, reward_vespene,
                        reward_biomass, reward_dark_matter, reward_items, assigned_units
                 FROM swarm_missions WHERE id = ?1",
                params![mission_id],
                |r| Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?,
                    r.get(4)?, r.get(5)?, r.get(6)?,
                    r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?,
                )),
            )
            .map_err(|_| {
                ImpForgeError::validation(
                    "SWARM_NO_MISSION",
                    format!("Mission '{}' not found.", mission_id),
                )
            })?;

        if status_str != MissionStatus::InProgress.as_str() {
            return Err(ImpForgeError::validation(
                "SWARM_NOT_ACTIVE",
                format!("Mission '{}' is not in progress.", mission_id),
            ));
        }

        // Check if enough time has passed
        if let Some(ref started) = started_at_opt {
            if let Ok(start_time) = chrono::DateTime::parse_from_rfc3339(started) {
                let elapsed = Utc::now().signed_duration_since(start_time.with_timezone(&Utc));
                let needed = chrono::Duration::minutes(duration as i64);
                if elapsed < needed {
                    let remaining = needed - elapsed;
                    return Err(ImpForgeError::validation(
                        "SWARM_NOT_DONE",
                        format!(
                            "Mission not complete. {} minutes remaining.",
                            remaining.num_minutes().max(1)
                        ),
                    ));
                }
            }
        }

        // Grant resources
        conn.execute(
            "UPDATE swarm_resources SET
                essence = essence + ?1,
                minerals = minerals + ?2,
                vespene = vespene + ?3,
                biomass = biomass + ?4,
                dark_matter = dark_matter + ?5
             WHERE id = 1",
            params![r_ess, r_min, r_ves, r_bio, r_dm],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_REWARD", format!("{e}")))?;

        // Free up assigned units and grant XP to them
        let assigned_ids: Vec<String> =
            serde_json::from_str(&assigned_json).unwrap_or_default();
        for uid in &assigned_ids {
            conn.execute(
                "UPDATE swarm_units SET assigned_task = NULL,
                    level = level + 1,
                    efficiency = MIN(2.0, efficiency + 0.05)
                 WHERE id = ?1",
                params![uid],
            )
            .map_err(|e| ImpForgeError::internal("SWARM_UNIT_FREE", format!("{e}")))?;
        }

        // Reset mission to available
        conn.execute(
            &format!(
                "UPDATE swarm_missions SET status = '{}', assigned_units = '[]', started_at = NULL WHERE id = ?1",
                MissionStatus::Available.as_str()
            ),
            params![mission_id],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_MISSION_RESET", format!("{e}")))?;

        let items: Vec<String> = serde_json::from_str(&items_json).unwrap_or_default();

        Ok(MissionReward {
            resources: SwarmResources {
                essence: r_ess,
                minerals: r_min,
                vespene: r_ves,
                biomass: r_bio,
                dark_matter: r_dm,
            },
            items,
            xp_earned: r_ess / 2, // Bonus RPG XP from missions
            mission_name: name,
        })
    }

    pub fn swarm_auto_assign(&self) -> Result<Vec<SwarmMission>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check if WarCouncil is built (level >= 1)
        let wc_level: u32 = conn
            .query_row(
                "SELECT level FROM swarm_buildings WHERE building_type = 'war_council'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if wc_level < 1 {
            return Err(ImpForgeError::validation(
                "SWARM_NO_WC",
                "Build a War Council (level 1+) to unlock auto-assign.",
            ));
        }

        // Get available missions sorted by reward value
        let mut avail_stmt = conn
            .prepare(
                "SELECT id, required_unit_types, required_unit_count
                 FROM swarm_missions WHERE status = 'available'
                 ORDER BY (reward_essence + reward_minerals + reward_vespene + reward_biomass + reward_dark_matter * 5) DESC",
            )
            .map_err(|e| ImpForgeError::internal("SWARM_AUTO", format!("{e}")))?;

        let missions: Vec<(String, String, u32)> = avail_stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| ImpForgeError::internal("SWARM_AUTO_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        // Get idle units (no assigned_task)
        let mut idle_stmt = conn
            .prepare(
                "SELECT id, unit_type FROM swarm_units WHERE assigned_task IS NULL ORDER BY level DESC",
            )
            .map_err(|e| ImpForgeError::internal("SWARM_AUTO_IDLE", format!("{e}")))?;

        let mut idle_units: Vec<(String, String)> = idle_stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| ImpForgeError::internal("SWARM_AUTO_Q2", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        let mut assigned_missions = Vec::new();
        let now = Utc::now().to_rfc3339();

        for (mid, req_types_json, req_count) in &missions {
            let req_types: Vec<String> =
                serde_json::from_str(req_types_json).unwrap_or_default();
            let count = *req_count as usize;

            if idle_units.len() < count {
                continue;
            }

            // Try to match required types
            let mut selected = Vec::new();
            let is_any = req_types.iter().any(|t| t == "any");

            for i in (0..idle_units.len()).rev() {
                if selected.len() >= count {
                    break;
                }
                let (ref uid, ref utype) = idle_units[i];
                if is_any || req_types.iter().any(|t| t == utype) {
                    selected.push(uid.clone());
                    idle_units.remove(i);
                }
            }

            if selected.len() < count {
                // Put units back (they were removed speculatively)
                // Actually we only removed matching ones, so just continue
                continue;
            }

            // Assign this mission
            let assigned_json = serde_json::to_string(&selected).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "UPDATE swarm_missions SET status = 'in_progress', assigned_units = ?1, started_at = ?2 WHERE id = ?3",
                params![assigned_json, now, mid],
            )
            .map_err(|e| ImpForgeError::internal("SWARM_AUTO_ASSIGN", format!("{e}")))?;

            for uid in &selected {
                conn.execute(
                    "UPDATE swarm_units SET assigned_task = ?1 WHERE id = ?2",
                    params![mid, uid],
                )
                .map_err(|e| ImpForgeError::internal("SWARM_AUTO_UNIT", format!("{e}")))?;
            }

            assigned_missions.push(mid.clone());
        }

        // Return updated missions
        let all = self.load_swarm_missions(&conn)?;
        Ok(all
            .into_iter()
            .filter(|m| assigned_missions.contains(&m.id))
            .collect())
    }

    // -- EvoSys: Governing Attributes ------------------------------------------

    /// Calculate the governing attributes for a swarm unit by its ID.
    pub fn get_unit_attributes(&self, unit_id: &str) -> Result<GoverningAttributes, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let unit: SwarmUnit = conn
            .query_row(
                "SELECT id, unit_type, name, level, hp, attack, defense, special_ability, assigned_task, efficiency
                 FROM swarm_units WHERE id = ?1",
                params![unit_id],
                |row| {
                    Ok(SwarmUnit {
                        id: row.get(0)?,
                        unit_type: UnitType::from_str(&row.get::<_, String>(1)?),
                        name: row.get(2)?,
                        level: row.get(3)?,
                        hp: row.get(4)?,
                        attack: row.get(5)?,
                        defense: row.get(6)?,
                        special_ability: row.get(7)?,
                        assigned_task: row.get(8)?,
                        efficiency: row.get(9)?,
                    })
                },
            )
            .map_err(|_| {
                ImpForgeError::validation("SWARM_UNIT_NOT_FOUND", format!("Unit {unit_id} not found"))
            })?;

        Ok(calculate_governing(&unit))
    }

    // -- Resource earning from productivity -----------------------------------

    pub fn earn_swarm_resources(&self, action: &str) -> Result<SwarmResources, ImpForgeError> {
        let earned = swarm_resources_for_action(action);

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("SWARM_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Check if any ForgeDrone exists for bonus
        let drone_count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM swarm_units WHERE unit_type = 'forge_drone'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // Matriarch bonus
        let matriarch_bonus: f64 = conn
            .query_row(
                "SELECT COUNT(*) FROM swarm_units WHERE unit_type = 'matriarch'",
                [],
                |r| r.get::<_, u32>(0),
            )
            .map(|c| if c > 0 { 1.2 } else { 1.0 })
            .unwrap_or(1.0);

        let drone_bonus = 1.0 + (drone_count as f64 * 0.1); // +10% per drone
        let total_bonus = drone_bonus * matriarch_bonus;

        let actual = SwarmResources {
            essence: (earned.essence as f64 * total_bonus) as u64,
            minerals: (earned.minerals as f64 * total_bonus) as u64,
            vespene: (earned.vespene as f64 * total_bonus) as u64,
            biomass: (earned.biomass as f64 * total_bonus) as u64,
            dark_matter: (earned.dark_matter as f64 * total_bonus) as u64,
        };

        conn.execute(
            "UPDATE swarm_resources SET
                essence = essence + ?1,
                minerals = minerals + ?2,
                vespene = vespene + ?3,
                biomass = biomass + ?4,
                dark_matter = dark_matter + ?5
             WHERE id = 1",
            params![actual.essence, actual.minerals, actual.vespene, actual.biomass, actual.dark_matter],
        )
        .map_err(|e| ImpForgeError::internal("SWARM_EARN", format!("{e}")))?;

        Ok(actual)
    }

    // =========================================================================
    // OGame-style Colony System
    // =========================================================================

    fn seed_planet_buildings(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let types = [
            "biomass_converter", "mineral_drill", "crystal_synthesizer",
            "spore_extractor", "energy_nest", "creep_generator",
            "brood_nest", "evolution_lab", "blighthaven",
            "spore_defense", "biomass_storage", "mineral_silo",
            "spawn_pool", "hydralisk_den", "ultralisk_cavern",
            "nydus_network", "spore_launcher", "bio_reactor",
            "genetic_archive", "creep_tumor", "psionic_link",
            "observation_spire",
            // Human faction buildings
            "town_hall", "keep", "castle", "human_barracks",
            "lumber_mill", "human_blacksmith", "arcane_sanctum",
            "workshop", "gryphon_aviary", "altar_of_kings",
            "scout_tower", "guard_tower", "cannon_tower", "arcane_tower",
            "farm", "marketplace", "church", "academy",
            "siege_works", "mage_tower", "harbor", "fortress_wall",
            // Demon faction buildings
            "infernal_pit", "hellfire_forge", "soul_well", "brimstone_refinery",
            "dark_altar", "demon_gate", "torture_chamber", "lava_foundry",
            "imp_barracks", "succubus_den", "hellhound_kennel", "infernal_tower",
            "chaos_spire", "wrath_engine", "blood_pool", "summoning_circle",
            "hellfire_wall", "shadow_market", "doom_spire", "corruption_node",
            "abyssal_shipyard", "throne_of_agony",
            // Undead faction buildings
            "necropolis", "undead_crypt", "graveyard", "plague_cauldron",
            "altar_of_darkness", "undead_slaughterhouse", "temple_of_the_damned",
            "bone_forge", "ziggurat", "spirit_tower", "nerubian_tower",
            "tomb_of_relics", "undead_boneyard", "sacrificial_pit",
            "necrosis_spreader", "ossuary", "plague_lab", "bone_wall",
            "soul_cage", "spectral_market", "ghost_shipyard", "citadel_of_undeath",
        ];
        for bt in &types {
            conn.execute(
                "INSERT OR IGNORE INTO planet_buildings (building_type, level) VALUES (?1, 0)",
                params![bt],
            )
            .map_err(|e| ImpForgeError::internal("PLANET_BLDG_SEED", format!("{e}")))?;
        }
        Ok(())
    }

    fn seed_planet_research(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let types = [
            "genetics", "armor_plating", "weapon_systems", "propulsion_drive",
            "swarm_intelligence", "regeneration", "mutation_tech",
            "creep_biology", "space_faring", "dark_matter_research",
            "bio_plasma", "adaptive_armor", "neural_network",
            "tunnel_digestion", "warp_drive", "psionic_scream",
            "symbiosis", "mass_evolution", "orbital_bombardment",
            "hive_mind_link",
            // Human faction technologies
            "iron_forging", "steel_forging", "mithril_forging",
            "iron_plating", "steel_plating", "mithril_plating",
            "long_rifles", "rifling", "masonry", "advanced_masonry",
            "human_fortification", "animal_husbandry", "cloud_technology",
            "arcane_training", "holy_light_tech", "blizzard_research",
            "telescope", "combustion_engine", "logistics", "diplomacy",
            // Demon faction technologies
            "hellfire_weapons_i", "hellfire_weapons_ii", "hellfire_weapons_iii",
            "demon_hide_i", "demon_hide_ii", "demon_hide_iii",
            "infernal_speed", "soul_absorption", "chaos_magic", "hellfire_mastery",
            "demon_wings", "torture_expertise", "portal_network", "brimstone_extraction",
            "corruption_spread", "fear_aura", "fel_engineering", "blood_pact",
            "abyssal_summoning", "apocalypse_protocol",
            // Undead faction technologies
            "bone_weapons_i", "bone_weapons_ii", "bone_weapons_iii",
            "unholy_armor_i", "unholy_armor_ii", "unholy_armor_iii",
            "ghoul_frenzy", "disease_cloud", "necromancy_tech", "frost_magic",
            "skeletal_mastery", "plague_research", "spectral_binding", "corpse_explosion",
            "dark_ritual", "necrosis_expansion", "bone_armor", "soul_harvest",
            "lich_ascension", "world_eater_protocol",
        ];
        for tt in &types {
            conn.execute(
                "INSERT OR IGNORE INTO planet_research (tech_type, level) VALUES (?1, 0)",
                params![tt],
            )
            .map_err(|e| ImpForgeError::internal("PLANET_RESEARCH_SEED", format!("{e}")))?;
        }
        Ok(())
    }

    fn seed_planet_fleet(&self, conn: &Connection) -> Result<(), ImpForgeError> {
        let types = [
            "bio_fighter", "spore_interceptor", "kraken_frigate", "leviathan",
            "bio_transporter", "colony_pod", "devourer", "world_eater",
            "leech_hauler", "spore_carrier", "hive_ship", "void_kraken",
            "mycetic_spore", "neural_parasite", "narwhal", "drone_ship",
            "razorfiend", "hierophant",
            // Human faction fleet
            "scout_fighter", "assault_fighter", "strike_cruiser",
            "human_battleship", "battle_cruiser", "strategic_bomber",
            "fleet_destroyer", "orbital_cannon", "salvage_vessel",
            "survey_ship", "light_freighter", "heavy_freighter",
            "salvage_tug", "spy_drone", "colony_transport",
            // Demon faction ships
            "fire_imp", "fiend_raider", "hell_chariot", "infernal_dreadnought",
            "baalfire_cruiser", "hellfire_rainer", "pit_lord_vessel", "abyssal_maw",
            "soul_harvester", "shadow_stalker", "imp_barge", "abyssal_barge",
            "slag_dredger", "eye_of_perdition", "hellgate_opener",
            // Undead faction ships
            "specter", "banshee_ship", "death_frigate", "phantom_galleon",
            "lich_cruiser", "plague_bringer", "dread_revenant", "undead_world_eater",
            "corpse_collector", "haunt", "wraith_skiff", "bone_galleon",
            "bone_picker", "shade", "crypt_ship",
        ];
        for st in &types {
            conn.execute(
                "INSERT OR IGNORE INTO planet_fleet (ship_type, count) VALUES (?1, 0)",
                params![st],
            )
            .map_err(|e| ImpForgeError::internal("PLANET_FLEET_SEED", format!("{e}")))?;
        }
        Ok(())
    }

    // -- OGame production formulas --------------------------------------------

    /// OGame-style production: base_rate * level * 1.1^level
    pub(crate) fn ogame_production_rate(level: u32, base_rate: f64) -> f64 {
        if level == 0 { return 0.0; }
        base_rate * level as f64 * 1.1_f64.powi(level as i32)
    }

    /// OGame-style upgrade cost: base * factor^level
    pub(crate) fn ogame_upgrade_cost(base: f64, level: u32, factor: f64) -> f64 {
        base * factor.powi(level as i32)
    }

    /// Build time in seconds: (cost_bio + cost_min) / (2500 * (1 + brood_nest_level)) * 3600
    fn ogame_build_time(cost_bio: f64, cost_min: f64, brood_nest_level: u32) -> u64 {
        let hours = (cost_bio + cost_min) / (2500.0 * (1.0 + brood_nest_level as f64));
        (hours * 3600.0).max(30.0) as u64
    }

    pub(crate) fn storage_capacity(storage_level: u32) -> f64 {
        5000.0 * (2.5 * (1.0 + storage_level as f64)).floor()
    }

    // -- Planet state ---------------------------------------------------------

    pub fn get_planet(&self) -> Result<Planet, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let resources = self.calculate_planet_resources(&conn)?;
        let buildings = self.load_planet_buildings(&conn)?;
        let research = self.load_planet_research(&conn)?;
        let fleet = self.load_planet_fleet(&conn)?;
        let creep = self.load_creep_status(&conn)?;

        // Storage caps from BiomassStorage and MineralSilo
        let bio_storage_level = buildings.iter()
            .find(|b| b.building_type == PlanetBuildingType::BiomassStorage)
            .map(|b| b.level)
            .unwrap_or(0);
        let min_storage_level = buildings.iter()
            .find(|b| b.building_type == PlanetBuildingType::MineralSilo)
            .map(|b| b.level)
            .unwrap_or(0);

        Ok(Planet {
            name: "Hive Prime".to_string(),
            resources,
            buildings,
            research,
            fleet,
            creep,
            storage_biomass_cap: Self::storage_capacity(bio_storage_level),
            storage_minerals_cap: Self::storage_capacity(min_storage_level),
            storage_crystal_cap: Self::storage_capacity(0), // No crystal storage building yet
            storage_spore_gas_cap: Self::storage_capacity(0),
        })
    }

    fn calculate_planet_resources(&self, conn: &Connection) -> Result<PlanetResources, ImpForgeError> {
        let (biomass, minerals, crystal, spore_gas, dm, last_collected): (f64, f64, f64, f64, u64, String) =
            conn.query_row(
                "SELECT biomass, minerals, crystal, spore_gas, dark_matter, last_collected
                 FROM planet_resources WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .map_err(|e| ImpForgeError::internal("PLANET_RES", format!("{e}")))?;

        // Calculate production rates from building levels
        let buildings = self.load_planet_building_levels(conn)?;
        let swarm_intel_level = self.get_research_level(conn, "swarm_intelligence");
        let swarm_bonus = 1.0 + swarm_intel_level as f64 * 0.05;

        // Creep bonus
        let creep_coverage: f64 = conn
            .query_row("SELECT coverage_percent FROM planet_creep WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0.0);
        let creep_bonus = if creep_coverage >= 100.0 { 1.5 }
            else if creep_coverage >= 50.0 { 1.2 }
            else { 1.0 };

        let bio_rate = Self::ogame_production_rate(buildings.get("biomass_converter").copied().unwrap_or(0), 30.0) * swarm_bonus * creep_bonus;
        let min_rate = Self::ogame_production_rate(buildings.get("mineral_drill").copied().unwrap_or(0), 20.0) * swarm_bonus * creep_bonus;
        let cry_rate = Self::ogame_production_rate(buildings.get("crystal_synthesizer").copied().unwrap_or(0), 10.0) * swarm_bonus * creep_bonus;
        let gas_rate = Self::ogame_production_rate(buildings.get("spore_extractor").copied().unwrap_or(0), 10.0) * swarm_bonus * creep_bonus;

        // Energy calculation
        let mut energy_prod: i64 = 0;
        let mut energy_cons: i64 = 0;
        for (bt_str, level) in &buildings {
            let bt = PlanetBuildingType::from_str(bt_str);
            let delta = bt.energy_per_level() * (*level as i64);
            if delta > 0 { energy_prod += delta; }
            else { energy_cons += delta.abs(); }
        }

        // Calculate elapsed time and accumulate resources
        let elapsed_hours = if let Ok(last) = chrono::NaiveDateTime::parse_from_str(&last_collected, "%Y-%m-%d %H:%M:%S") {
            let now = Utc::now().naive_utc();
            let diff = now.signed_duration_since(last);
            (diff.num_seconds() as f64 / 3600.0).max(0.0)
        } else {
            0.0
        };

        // Biomass bonus from creep consuming flora
        let flora_bonus = 1.0 + (creep_coverage / 100.0) * 0.5;

        let new_biomass = (biomass + bio_rate * elapsed_hours * flora_bonus).min(Self::storage_capacity(
            buildings.get("biomass_storage").copied().unwrap_or(0)));
        let new_minerals = (minerals + min_rate * elapsed_hours).min(Self::storage_capacity(
            buildings.get("mineral_silo").copied().unwrap_or(0)));
        let new_crystal = (crystal + cry_rate * elapsed_hours).min(Self::storage_capacity(0));
        let new_spore_gas = (spore_gas + gas_rate * elapsed_hours).min(Self::storage_capacity(0));

        // Update stored resources with new values
        conn.execute(
            "UPDATE planet_resources SET biomass = ?1, minerals = ?2, crystal = ?3,
             spore_gas = ?4, last_collected = datetime('now') WHERE id = 1",
            params![new_biomass, new_minerals, new_crystal, new_spore_gas],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_RES_UPDATE", format!("{e}")))?;

        Ok(PlanetResources {
            biomass: new_biomass,
            minerals: new_minerals,
            crystal: new_crystal,
            spore_gas: new_spore_gas,
            energy: energy_prod - energy_cons,
            dark_matter: dm,
            biomass_per_hour: bio_rate * flora_bonus,
            minerals_per_hour: min_rate,
            crystal_per_hour: cry_rate,
            spore_gas_per_hour: gas_rate,
            energy_production: energy_prod,
            energy_consumption: energy_cons,
        })
    }

    fn load_planet_building_levels(&self, conn: &Connection) -> Result<std::collections::HashMap<String, u32>, ImpForgeError> {
        let mut stmt = conn
            .prepare("SELECT building_type, level FROM planet_buildings")
            .map_err(|e| ImpForgeError::internal("PLANET_BLDG_LEVELS", format!("{e}")))?;

        let map: std::collections::HashMap<String, u32> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)))
            .map_err(|e| ImpForgeError::internal("PLANET_BLDG_LEVELS_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(map)
    }

    fn get_research_level(&self, conn: &Connection, tech: &str) -> u32 {
        conn.query_row(
            "SELECT level FROM planet_research WHERE tech_type = ?1",
            params![tech],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }

    fn load_planet_buildings(&self, conn: &Connection) -> Result<Vec<PlanetBuilding>, ImpForgeError> {
        let brood_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'brood_nest'", [], |r| r.get(0))
            .unwrap_or(0);

        let mut stmt = conn
            .prepare("SELECT building_type, level, upgrading, upgrade_finish FROM planet_buildings ORDER BY building_type")
            .map_err(|e| ImpForgeError::internal("PLANET_BLDG", format!("{e}")))?;

        let buildings = stmt
            .query_map([], |r| {
                let bt_str: String = r.get(0)?;
                let level: u32 = r.get(1)?;
                let upgrading: bool = r.get::<_, i32>(2)? != 0;
                let finish: Option<String> = r.get(3)?;
                Ok((bt_str, level, upgrading, finish))
            })
            .map_err(|e| ImpForgeError::internal("PLANET_BLDG_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .map(|(bt_str, level, upgrading, finish)| {
                let bt = PlanetBuildingType::from_str(&bt_str);
                let (base_bio, base_min, base_cry, base_gas, factor) = bt.base_costs();
                let cost_bio = Self::ogame_upgrade_cost(base_bio, level, factor);
                let cost_min = Self::ogame_upgrade_cost(base_min, level, factor);
                let cost_cry = Self::ogame_upgrade_cost(base_cry, level, factor);
                let cost_gas = Self::ogame_upgrade_cost(base_gas, level, factor);
                let build_time = Self::ogame_build_time(cost_bio, cost_min, brood_level);

                PlanetBuilding {
                    display_name: bt.display_name().to_string(),
                    description: bt.description().to_string(),
                    building_type: bt,
                    level,
                    upgrading,
                    upgrade_finish: finish,
                    cost_biomass: cost_bio,
                    cost_minerals: cost_min,
                    cost_crystal: cost_cry,
                    cost_spore_gas: cost_gas,
                    build_time_seconds: build_time,
                }
            })
            .collect();

        Ok(buildings)
    }

    fn load_planet_research(&self, conn: &Connection) -> Result<Vec<Research>, ImpForgeError> {
        let mut stmt = conn
            .prepare("SELECT tech_type, level, researching, research_finish FROM planet_research ORDER BY tech_type")
            .map_err(|e| ImpForgeError::internal("PLANET_RESEARCH", format!("{e}")))?;

        let research = stmt
            .query_map([], |r| {
                let tt_str: String = r.get(0)?;
                let level: u32 = r.get(1)?;
                let researching: bool = r.get::<_, i32>(2)? != 0;
                let finish: Option<String> = r.get(3)?;
                Ok((tt_str, level, researching, finish))
            })
            .map_err(|e| ImpForgeError::internal("PLANET_RESEARCH_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .map(|(tt_str, level, researching, finish)| {
                let tt = TechType::from_str(&tt_str);
                let (base_bio, base_min, base_cry, base_gas, factor) = tt.base_costs();
                let cost_bio = Self::ogame_upgrade_cost(base_bio, level, factor);
                let cost_min = Self::ogame_upgrade_cost(base_min, level, factor);
                let cost_cry = Self::ogame_upgrade_cost(base_cry, level, factor);
                let cost_gas = Self::ogame_upgrade_cost(base_gas, level, factor);
                let research_time = (((cost_bio + cost_min) / 1000.0) * 3600.0).max(60.0) as u64;

                Research {
                    display_name: tt.display_name().to_string(),
                    description: tt.description().to_string(),
                    tech_type: tt.clone(),
                    level,
                    researching,
                    research_finish: finish,
                    cost_biomass: cost_bio,
                    cost_minerals: cost_min,
                    cost_crystal: cost_cry,
                    cost_spore_gas: cost_gas,
                    research_time_seconds: research_time,
                    required_lab_level: tt.required_lab_level(),
                }
            })
            .collect();

        Ok(research)
    }

    fn load_planet_fleet(&self, conn: &Connection) -> Result<Vec<Ship>, ImpForgeError> {
        let mut stmt = conn
            .prepare("SELECT ship_type, count FROM planet_fleet ORDER BY ship_type")
            .map_err(|e| ImpForgeError::internal("PLANET_FLEET", format!("{e}")))?;

        let fleet = stmt
            .query_map([], |r| {
                let st_str: String = r.get(0)?;
                let count: u32 = r.get(1)?;
                Ok((st_str, count))
            })
            .map_err(|e| ImpForgeError::internal("PLANET_FLEET_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .map(|(st_str, count)| {
                let st = ShipType::from_str(&st_str);
                let (atk, shields, hp) = st.combat_stats();
                Ship {
                    display_name: st.display_name().to_string(),
                    description: st.description().to_string(),
                    ship_type: st,
                    count,
                    attack: atk,
                    shields,
                    hp,
                }
            })
            .collect();

        Ok(fleet)
    }

    fn load_creep_status(&self, conn: &Connection) -> Result<CreepStatus, ImpForgeError> {
        let (coverage, flora, fauna): (f64, f64, f64) = conn
            .query_row(
                "SELECT coverage_percent, flora_corrupted, fauna_consumed FROM planet_creep WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap_or((0.0, 0.0, 0.0));

        let gen_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'creep_generator'", [], |r| r.get(0))
            .unwrap_or(0);
        let creep_bio_level = self.get_research_level(conn, "creep_biology");

        let spread_rate = (gen_level as f32 * 0.5 + creep_bio_level as f32 * 0.3).max(0.0);
        let biomass_bonus = coverage as f32 / 100.0 * 50.0; // up to +50%

        Ok(CreepStatus {
            coverage_percent: coverage as f32,
            spread_rate_per_hour: spread_rate,
            flora_corrupted: flora as f32,
            fauna_consumed: fauna as f32,
            biomass_bonus,
        })
    }

    // -- Planet upgrades ------------------------------------------------------

    pub fn upgrade_planet_building(&self, building_type_str: &str) -> Result<PlanetBuilding, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let bt = PlanetBuildingType::from_str(building_type_str);

        // Check if already upgrading something
        let upgrading_count: u32 = conn
            .query_row("SELECT COUNT(*) FROM planet_buildings WHERE upgrading = 1", [], |r| r.get(0))
            .unwrap_or(0);
        if upgrading_count > 0 {
            return Err(ImpForgeError::validation(
                "PLANET_BUSY", "Already upgrading a building. Wait for it to finish.",
            ));
        }

        let level: u32 = conn
            .query_row(
                "SELECT level FROM planet_buildings WHERE building_type = ?1",
                params![bt.as_str()],
                |r| r.get(0),
            )
            .map_err(|_| ImpForgeError::validation("PLANET_NO_BLDG", format!("Building '{}' not found.", building_type_str)))?;

        // Blighthaven requires SpaceFaring research
        if bt == PlanetBuildingType::Blighthaven && level == 0 {
            let sf_level = self.get_research_level(&conn, "space_faring");
            if sf_level < 1 {
                return Err(ImpForgeError::validation(
                    "PLANET_NEED_RESEARCH", "Blighthaven requires Space Faring research level 1.",
                ));
            }
        }

        // Calculate costs
        let (base_bio, base_min, base_cry, base_gas, factor) = bt.base_costs();
        let cost_bio = Self::ogame_upgrade_cost(base_bio, level, factor);
        let cost_min = Self::ogame_upgrade_cost(base_min, level, factor);
        let cost_cry = Self::ogame_upgrade_cost(base_cry, level, factor);
        let cost_gas = Self::ogame_upgrade_cost(base_gas, level, factor);

        // Check resources (collect first)
        drop(conn);
        let resources = {
            let conn2 = self.conn.lock().map_err(|e| {
                ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
            })?;
            self.calculate_planet_resources(&conn2)?
        };

        if resources.biomass < cost_bio || resources.minerals < cost_min
            || resources.crystal < cost_cry || resources.spore_gas < cost_gas {
            return Err(ImpForgeError::validation(
                "PLANET_NO_RES",
                format!("Insufficient resources. Need: {:.0}B {:.0}M {:.0}C {:.0}G",
                    cost_bio, cost_min, cost_cry, cost_gas),
            ));
        }

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Deduct resources
        conn.execute(
            "UPDATE planet_resources SET biomass = biomass - ?1, minerals = minerals - ?2,
             crystal = crystal - ?3, spore_gas = spore_gas - ?4 WHERE id = 1",
            params![cost_bio, cost_min, cost_cry, cost_gas],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_DEDUCT", format!("{e}")))?;

        // Calculate build time
        let brood_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'brood_nest'", [], |r| r.get(0))
            .unwrap_or(0);
        let build_secs = Self::ogame_build_time(cost_bio, cost_min, brood_level);
        let finish = Utc::now() + chrono::Duration::seconds(build_secs as i64);

        conn.execute(
            "UPDATE planet_buildings SET upgrading = 1, upgrade_finish = ?1 WHERE building_type = ?2",
            params![finish.to_rfc3339(), bt.as_str()],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_UPGRADE", format!("{e}")))?;

        let new_cost_bio = Self::ogame_upgrade_cost(base_bio, level + 1, factor);
        let new_cost_min = Self::ogame_upgrade_cost(base_min, level + 1, factor);
        let new_cost_cry = Self::ogame_upgrade_cost(base_cry, level + 1, factor);
        let new_cost_gas = Self::ogame_upgrade_cost(base_gas, level + 1, factor);

        Ok(PlanetBuilding {
            display_name: bt.display_name().to_string(),
            description: bt.description().to_string(),
            building_type: bt,
            level,
            upgrading: true,
            upgrade_finish: Some(finish.to_rfc3339()),
            cost_biomass: new_cost_bio,
            cost_minerals: new_cost_min,
            cost_crystal: new_cost_cry,
            cost_spore_gas: new_cost_gas,
            build_time_seconds: build_secs,
        })
    }

    pub fn collect_planet_resources(&self) -> Result<PlanetResources, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;
        self.calculate_planet_resources(&conn)
    }

    pub fn start_research(&self, tech_str: &str) -> Result<Research, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let tt = TechType::from_str(tech_str);

        // Check if already researching
        let researching_count: u32 = conn
            .query_row("SELECT COUNT(*) FROM planet_research WHERE researching = 1", [], |r| r.get(0))
            .unwrap_or(0);
        if researching_count > 0 {
            return Err(ImpForgeError::validation(
                "PLANET_RESEARCH_BUSY", "Already researching. Wait for it to finish.",
            ));
        }

        // Check Evolution Lab level
        let lab_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'evolution_lab'", [], |r| r.get(0))
            .unwrap_or(0);
        if lab_level < tt.required_lab_level() {
            return Err(ImpForgeError::validation(
                "PLANET_LAB_LOW",
                format!("Requires Evolution Lab level {} (have {}).", tt.required_lab_level(), lab_level),
            ));
        }

        let level: u32 = conn
            .query_row(
                "SELECT level FROM planet_research WHERE tech_type = ?1",
                params![tt.as_str()],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let (base_bio, base_min, base_cry, base_gas, factor) = tt.base_costs();
        let cost_bio = Self::ogame_upgrade_cost(base_bio, level, factor);
        let cost_min = Self::ogame_upgrade_cost(base_min, level, factor);
        let cost_cry = Self::ogame_upgrade_cost(base_cry, level, factor);
        let cost_gas = Self::ogame_upgrade_cost(base_gas, level, factor);

        // Check resources
        let (bio, min, cry, gas): (f64, f64, f64, f64) = conn
            .query_row(
                "SELECT biomass, minerals, crystal, spore_gas FROM planet_resources WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|e| ImpForgeError::internal("PLANET_RES_CHECK", format!("{e}")))?;

        if bio < cost_bio || min < cost_min || cry < cost_cry || gas < cost_gas {
            return Err(ImpForgeError::validation("PLANET_NO_RES", "Insufficient resources for research."));
        }

        // Deduct
        conn.execute(
            "UPDATE planet_resources SET biomass = biomass - ?1, minerals = minerals - ?2,
             crystal = crystal - ?3, spore_gas = spore_gas - ?4 WHERE id = 1",
            params![cost_bio, cost_min, cost_cry, cost_gas],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_DEDUCT_R", format!("{e}")))?;

        let research_secs = (((cost_bio + cost_min) / 1000.0) * 3600.0).max(60.0) as i64;
        let finish = Utc::now() + chrono::Duration::seconds(research_secs);

        conn.execute(
            "UPDATE planet_research SET researching = 1, research_finish = ?1 WHERE tech_type = ?2",
            params![finish.to_rfc3339(), tt.as_str()],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_RESEARCH_START", format!("{e}")))?;

        let new_cost_bio = Self::ogame_upgrade_cost(base_bio, level + 1, factor);
        let new_cost_min = Self::ogame_upgrade_cost(base_min, level + 1, factor);

        Ok(Research {
            display_name: tt.display_name().to_string(),
            description: tt.description().to_string(),
            tech_type: tt.clone(),
            level,
            researching: true,
            research_finish: Some(finish.to_rfc3339()),
            cost_biomass: Self::ogame_upgrade_cost(base_bio, level + 1, factor),
            cost_minerals: new_cost_min,
            cost_crystal: Self::ogame_upgrade_cost(base_cry, level + 1, factor),
            cost_spore_gas: Self::ogame_upgrade_cost(base_gas, level + 1, factor),
            research_time_seconds: (((new_cost_bio + new_cost_min) / 1000.0) * 3600.0).max(60.0) as u64,
            required_lab_level: tt.required_lab_level(),
        })
    }

    pub fn build_ships(&self, ship_type_str: &str, count: u32) -> Result<Ship, ImpForgeError> {
        if count == 0 {
            return Err(ImpForgeError::validation("PLANET_ZERO", "Must build at least 1 ship."));
        }

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let st = ShipType::from_str(ship_type_str);

        // Check Blighthaven level
        let shipyard_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'blighthaven'", [], |r| r.get(0))
            .unwrap_or(0);
        if shipyard_level < st.required_shipyard_level() {
            return Err(ImpForgeError::validation(
                "PLANET_SHIPYARD_LOW",
                format!("Requires Blighthaven level {} (have {}).", st.required_shipyard_level(), shipyard_level),
            ));
        }

        // WorldEater requires SpaceFaring level 20 and creep 75%
        if st == ShipType::WorldEater {
            let sf = self.get_research_level(&conn, "space_faring");
            if sf < 20 {
                return Err(ImpForgeError::validation("PLANET_WE_RESEARCH", "World Eater requires Space Faring level 20."));
            }
            let creep: f64 = conn
                .query_row("SELECT coverage_percent FROM planet_creep WHERE id = 1", [], |r| r.get(0))
                .unwrap_or(0.0);
            if creep < 75.0 {
                return Err(ImpForgeError::validation("PLANET_WE_CREEP", "World Eater requires 75% creep coverage."));
            }
        }

        let (cost_bio, cost_min, cost_cry, cost_gas) = st.unit_cost();
        let total_bio = cost_bio * count as f64;
        let total_min = cost_min * count as f64;
        let total_cry = cost_cry * count as f64;
        let total_gas = cost_gas * count as f64;

        let (bio, min, cry, gas): (f64, f64, f64, f64) = conn
            .query_row(
                "SELECT biomass, minerals, crystal, spore_gas FROM planet_resources WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|e| ImpForgeError::internal("PLANET_RES_SHIP", format!("{e}")))?;

        if bio < total_bio || min < total_min || cry < total_cry || gas < total_gas {
            return Err(ImpForgeError::validation("PLANET_NO_RES", "Insufficient resources to build ships."));
        }

        conn.execute(
            "UPDATE planet_resources SET biomass = biomass - ?1, minerals = minerals - ?2,
             crystal = crystal - ?3, spore_gas = spore_gas - ?4 WHERE id = 1",
            params![total_bio, total_min, total_cry, total_gas],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_DEDUCT_S", format!("{e}")))?;

        conn.execute(
            "UPDATE planet_fleet SET count = count + ?1 WHERE ship_type = ?2",
            params![count, st.as_str()],
        )
        .map_err(|e| ImpForgeError::internal("PLANET_BUILD_SHIP", format!("{e}")))?;

        let new_count: u32 = conn
            .query_row("SELECT count FROM planet_fleet WHERE ship_type = ?1", params![st.as_str()], |r| r.get(0))
            .unwrap_or(count);

        let (atk, shields, hp) = st.combat_stats();
        Ok(Ship {
            display_name: st.display_name().to_string(),
            description: st.description().to_string(),
            ship_type: st,
            count: new_count,
            attack: atk,
            shields,
            hp,
        })
    }

    pub fn get_creep(&self) -> Result<CreepStatus, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Update creep spread based on elapsed time
        let (coverage, last_updated): (f64, String) = conn
            .query_row(
                "SELECT coverage_percent, last_updated FROM planet_creep WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((0.0, Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()));

        let gen_level: u32 = conn
            .query_row("SELECT level FROM planet_buildings WHERE building_type = 'creep_generator'", [], |r| r.get(0))
            .unwrap_or(0);
        let creep_bio_level = self.get_research_level(&conn, "creep_biology");
        let spread_rate = gen_level as f64 * 0.5 + creep_bio_level as f64 * 0.3;

        let elapsed_hours = if let Ok(last) = chrono::NaiveDateTime::parse_from_str(&last_updated, "%Y-%m-%d %H:%M:%S") {
            let now = Utc::now().naive_utc();
            (now.signed_duration_since(last).num_seconds() as f64 / 3600.0).max(0.0)
        } else { 0.0 };

        let new_coverage = (coverage + spread_rate * elapsed_hours).min(100.0);
        let new_flora = (new_coverage * 0.8).min(100.0);
        let new_fauna = (new_coverage * 0.6).min(100.0);

        conn.execute(
            "UPDATE planet_creep SET coverage_percent = ?1, flora_corrupted = ?2,
             fauna_consumed = ?3, last_updated = datetime('now') WHERE id = 1",
            params![new_coverage, new_flora, new_fauna],
        )
        .map_err(|e| ImpForgeError::internal("CREEP_UPDATE", format!("{e}")))?;

        Ok(CreepStatus {
            coverage_percent: new_coverage as f32,
            spread_rate_per_hour: spread_rate as f32,
            flora_corrupted: new_flora as f32,
            fauna_consumed: new_fauna as f32,
            biomass_bonus: new_coverage as f32 / 100.0 * 50.0,
        })
    }
    pub fn get_shop_items(&self) -> Result<Vec<ShopItem>, ImpForgeError> {
        Ok(all_shop_items())
    }

    pub fn buy_shop_item(&self, item_id: &str) -> Result<ShopItem, ImpForgeError> {
        let items = all_shop_items();
        let item = items.iter().find(|i| i.id == item_id).ok_or_else(|| {
            ImpForgeError::validation("SHOP_NOT_FOUND", format!("Shop item '{}' not found.", item_id))
        })?;

        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let dm: u64 = conn
            .query_row("SELECT dark_matter FROM planet_resources WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);

        if dm < item.cost_dark_matter {
            return Err(ImpForgeError::validation(
                "SHOP_NO_DM",
                format!("Need {} Dark Matter (have {}).", item.cost_dark_matter, dm),
            ));
        }

        conn.execute(
            "UPDATE planet_resources SET dark_matter = dark_matter - ?1 WHERE id = 1",
            params![item.cost_dark_matter],
        )
        .map_err(|e| ImpForgeError::internal("SHOP_BUY", format!("{e}")))?;

        let expires = item.duration_hours.map(|h| {
            (Utc::now() + chrono::Duration::hours(h as i64)).to_rfc3339()
        });

        conn.execute(
            "INSERT OR REPLACE INTO planet_shop_active (item_id, activated_at, expires_at) VALUES (?1, ?2, ?3)",
            params![item.id, Utc::now().to_rfc3339(), expires],
        )
        .map_err(|e| ImpForgeError::internal("SHOP_ACTIVATE", format!("{e}")))?;

        Ok(item.clone())
    }

    pub fn get_galaxy(&self, _galaxy: u32, _system: u32) -> Result<Vec<PlanetSlot>, ImpForgeError> {
        // Single-player galaxy view -- show the player's planet plus procedural neighbors
        let mut slots = Vec::with_capacity(15);
        for pos in 1..=15 {
            if pos == 4 {
                slots.push(PlanetSlot {
                    position: pos,
                    occupied: true,
                    planet_name: Some("Hive Prime".to_string()),
                    player_name: Some("You".to_string()),
                    planet_type: Some("terran".to_string()),
                });
            } else {
                // Deterministic procedural occupation
                let occupied = (pos * 7 + _galaxy * 13 + _system * 31) % 5 < 2;
                slots.push(PlanetSlot {
                    position: pos,
                    occupied,
                    planet_name: if occupied { Some(format!("Planet {}-{}-{}", _galaxy, _system, pos)) } else { None },
                    player_name: if occupied { Some(format!("NPC_{}", pos * _system + _galaxy)) } else { None },
                    planet_type: if occupied {
                        Some(match pos % 4 { 0 => "desert", 1 => "ice", 2 => "gas", _ => "terran" }.to_string())
                    } else { None },
                });
            }
        }
        Ok(slots)
    }

    pub fn check_timers(&self) -> Result<Vec<CompletedTimer>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let now = Utc::now();
        let mut completed = Vec::new();

        // Check building timers
        let mut stmt = conn
            .prepare("SELECT building_type, upgrade_finish FROM planet_buildings WHERE upgrading = 1 AND upgrade_finish IS NOT NULL")
            .map_err(|e| ImpForgeError::internal("TIMER_CHECK", format!("{e}")))?;

        let building_timers: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| ImpForgeError::internal("TIMER_CHECK_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        for (bt_str, finish_str) in building_timers {
            if let Ok(finish_time) = chrono::DateTime::parse_from_rfc3339(&finish_str) {
                if now >= finish_time.with_timezone(&Utc) {
                    // Complete the upgrade
                    conn.execute(
                        "UPDATE planet_buildings SET level = level + 1, upgrading = 0, upgrade_finish = NULL WHERE building_type = ?1",
                        params![bt_str],
                    )
                    .map_err(|e| ImpForgeError::internal("TIMER_COMPLETE_B", format!("{e}")))?;

                    let bt = PlanetBuildingType::from_str(&bt_str);
                    completed.push(CompletedTimer {
                        timer_type: "building".to_string(),
                        item_name: bt.display_name().to_string(),
                        completed_at: now.to_rfc3339(),
                    });
                }
            }
        }

        // Check research timers
        let mut stmt2 = conn
            .prepare("SELECT tech_type, research_finish FROM planet_research WHERE researching = 1 AND research_finish IS NOT NULL")
            .map_err(|e| ImpForgeError::internal("TIMER_CHECK_R", format!("{e}")))?;

        let research_timers: Vec<(String, String)> = stmt2
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| ImpForgeError::internal("TIMER_CHECK_R_Q", format!("{e}")))?
            .filter_map(|r| r.ok())
            .collect();

        for (tt_str, finish_str) in research_timers {
            if let Ok(finish_time) = chrono::DateTime::parse_from_rfc3339(&finish_str) {
                if now >= finish_time.with_timezone(&Utc) {
                    conn.execute(
                        "UPDATE planet_research SET level = level + 1, researching = 0, research_finish = NULL WHERE tech_type = ?1",
                        params![tt_str],
                    )
                    .map_err(|e| ImpForgeError::internal("TIMER_COMPLETE_R", format!("{e}")))?;

                    let tt = TechType::from_str(&tt_str);
                    completed.push(CompletedTimer {
                        timer_type: "research".to_string(),
                        item_name: tt.display_name().to_string(),
                        completed_at: now.to_rfc3339(),
                    });
                }
            }
        }

        Ok(completed)
    }

    /// Award dark matter from achievements, daily login, etc.
    pub fn award_dark_matter(&self, amount: u64, reason: &str) -> Result<u64, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("PLANET_LOCK", format!("Lock poisoned: {e}"))
        })?;

        conn.execute(
            "UPDATE planet_resources SET dark_matter = dark_matter + ?1 WHERE id = 1",
            params![amount],
        )
        .map_err(|e| ImpForgeError::internal("DM_AWARD", format!("{e}")))?;

        // Track achievement
        conn.execute(
            "INSERT OR IGNORE INTO planet_achievements (achievement_id, dark_matter_awarded) VALUES (?1, ?2)",
            params![reason, amount],
        )
        .map_err(|e| ImpForgeError::internal("DM_TRACK", format!("{e}")))?;

        let total: u64 = conn
            .query_row("SELECT dark_matter FROM planet_resources WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);

        Ok(total)
    }

    // ── Dark Matter Earnings (productivity → game currency) ──────────────

    /// Earn Dark Matter from a productivity activity.  Records the earning
    /// in `dm_earnings` and credits `planet_resources`.  Returns the new total.
    pub fn earn_dark_matter_from_source(
        &self,
        source: &DmSource,
    ) -> Result<u32, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DM_EARN_LOCK", format!("Lock poisoned: {e}"))
        })?;

        let amount = source.dm_amount();

        // Ensure earnings table exists
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dm_earnings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source TEXT NOT NULL,
                amount INTEGER NOT NULL,
                earned_at TEXT NOT NULL
            )"
        )
        .map_err(|e| ImpForgeError::internal("DM_TABLE", format!("{e}")))?;

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO dm_earnings (source, amount, earned_at) VALUES (?1, ?2, ?3)",
            params![source.as_str(), amount, now],
        )
        .map_err(|e| ImpForgeError::internal("DM_INSERT", format!("{e}")))?;

        // Credit planet resources
        conn.execute(
            "UPDATE planet_resources SET dark_matter = dark_matter + ?1 WHERE id = 1",
            params![amount as u64],
        )
        .map_err(|e| ImpForgeError::internal("DM_CREDIT", format!("{e}")))?;

        let total: u32 = conn
            .query_row(
                "SELECT dark_matter FROM planet_resources WHERE id = 1",
                [],
                |r| r.get::<_, u64>(0),
            )
            .map(|v| v as u32)
            .unwrap_or(0);

        Ok(total)
    }

    /// Get recent Dark Matter earning history.
    pub fn get_dark_matter_history(
        &self,
        limit: u32,
    ) -> Result<Vec<DarkMatterEarnings>, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("DM_HIST_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Ensure table exists (safe to call repeatedly)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dm_earnings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source TEXT NOT NULL,
                amount INTEGER NOT NULL,
                earned_at TEXT NOT NULL
            )"
        )
        .map_err(|e| ImpForgeError::internal("DM_TABLE", format!("{e}")))?;

        let mut stmt = conn
            .prepare(
                "SELECT source, amount, earned_at FROM dm_earnings
                 ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| ImpForgeError::internal("DM_HIST_PREP", format!("{e}")))?;

        let rows = stmt
            .query_map(params![limit], |r| {
                let src_str: String = r.get(0)?;
                let amount: u32 = r.get(1)?;
                let ts: String = r.get(2)?;
                Ok((src_str, amount, ts))
            })
            .map_err(|e| ImpForgeError::internal("DM_HIST_QUERY", format!("{e}")))?;

        let mut history = Vec::new();
        for row in rows {
            let (src_str, amount, ts) =
                row.map_err(|e| ImpForgeError::internal("DM_HIST_ROW", format!("{e}")))?;
            let source = DmSource::from_str(&src_str).unwrap_or(DmSource::ActiveUsage);
            history.push(DarkMatterEarnings {
                source,
                amount,
                timestamp: ts,
            });
        }

        Ok(history)
    }

    /// Called from track_action to also give planet resources from productivity
    fn earn_planet_resources_from_action(&self, conn: &Connection, action: &str) {
        let (bio, min, cry, gas, dm) = match action {
            "create_document" => (15.0, 0.0, 0.0, 0.0, 0u64),
            "run_workflow" => (10.0, 5.0, 0.0, 2.0, 0),
            "ai_query" => (5.0, 0.0, 3.0, 0.0, 0),
            "create_spreadsheet" => (8.0, 10.0, 0.0, 0.0, 0),
            "social_post" => (12.0, 0.0, 0.0, 0.0, 0),
            "create_note" => (10.0, 0.0, 0.0, 0.0, 0),
            "create_slide" => (8.0, 5.0, 0.0, 0.0, 0),
            "complete_quest" => (30.0, 10.0, 5.0, 5.0, 5),
            _ => (3.0, 0.0, 0.0, 0.0, 0),
        };

        let _ = conn.execute(
            "UPDATE planet_resources SET biomass = biomass + ?1, minerals = minerals + ?2,
             crystal = crystal + ?3, spore_gas = spore_gas + ?4, dark_matter = dark_matter + ?5
             WHERE id = 1",
            params![bio, min, cry, gas, dm],
        );
    }

    // ── Mutation System ──────────────────────────────────────────────────

    /// Load all applied mutations for a specific unit from the database.
    fn load_unit_applied_mutations(
        &self,
        conn: &Connection,
        unit_id: &str,
    ) -> Result<Vec<AppliedMutation>, ImpForgeError> {
        let mut stmt = conn
            .prepare(
                "SELECT mutation_id, applied_at_level FROM swarm_mutations
                 WHERE unit_id = ?1 ORDER BY applied_at_level ASC",
            )
            .map_err(|e| ImpForgeError::internal("MUT_LOAD", format!("{e}")))?;

        let mutations = stmt
            .query_map(params![unit_id], |r| {
                Ok(AppliedMutation {
                    mutation_id: r.get(0)?,
                    applied_at_level: r.get(1)?,
                })
            })
            .map_err(|e| ImpForgeError::internal("MUT_QUERY", format!("{e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ImpForgeError::internal("MUT_COLLECT", format!("{e}")))?;

        Ok(mutations)
    }

    /// Get a unit's mutation state: applied mutations and any pending choices.
    pub fn get_unit_mutations(&self, unit_id: &str) -> Result<UnitMutations, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("MUT_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Load unit info
        let (unit_type_str, level): (String, u32) = conn
            .query_row(
                "SELECT unit_type, level FROM swarm_units WHERE id = ?1",
                params![unit_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| {
                ImpForgeError::validation("MUT_NO_UNIT", format!("Unit '{unit_id}' not found."))
            })?;

        let unit_type = UnitType::from_str(&unit_type_str);
        let applied = self.load_unit_applied_mutations(&conn, unit_id)?;

        // Figure out which milestones are still unclaimed
        let milestones = mutation_milestones_up_to(level);
        let applied_levels: Vec<u32> = applied.iter().map(|a| a.applied_at_level).collect();

        // Find the first unclaimed milestone
        let pending_level = milestones
            .iter()
            .find(|m| !applied_levels.contains(m))
            .copied();

        let pending_choices = if let Some(pl) = pending_level {
            self.available_mutations_for(&unit_type, pl)
        } else {
            Vec::new()
        };

        Ok(UnitMutations {
            unit_id: unit_id.to_string(),
            unit_type,
            unit_level: level,
            applied_mutations: applied,
            pending_choices,
        })
    }

    /// Get the 3 mutation choices for a unit type at a specific milestone level.
    fn available_mutations_for(&self, unit_type: &UnitType, level: u32) -> Vec<Mutation> {
        all_mutations()
            .into_iter()
            .filter(|m| m.unit_type == *unit_type && m.level_required == level)
            .collect()
    }

    /// Public version: get available mutations for any unit_type + level combo.
    pub fn get_available_mutations(
        &self,
        unit_type_str: &str,
        level: u32,
    ) -> Result<Vec<Mutation>, ImpForgeError> {
        let ut = UnitType::from_str(unit_type_str);
        Ok(self.available_mutations_for(&ut, level))
    }

    /// Apply a chosen mutation to a unit, permanently updating its stats.
    pub fn apply_mutation(
        &self,
        unit_id: &str,
        mutation_id: &str,
    ) -> Result<SwarmUnit, ImpForgeError> {
        let conn = self.conn.lock().map_err(|e| {
            ImpForgeError::internal("MUT_LOCK", format!("Lock poisoned: {e}"))
        })?;

        // Load unit
        let (unit_type_str, level, hp, attack, defense, efficiency): (String, u32, u32, u32, u32, f32) =
            conn.query_row(
                "SELECT unit_type, level, hp, attack, defense, efficiency
                 FROM swarm_units WHERE id = ?1",
                params![unit_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .map_err(|_| {
                ImpForgeError::validation("MUT_NO_UNIT", format!("Unit '{unit_id}' not found."))
            })?;

        let unit_type = UnitType::from_str(&unit_type_str);

        // Find the mutation in the catalog
        let mutation = all_mutations()
            .into_iter()
            .find(|m| m.id == mutation_id)
            .ok_or_else(|| {
                ImpForgeError::validation(
                    "MUT_NOT_FOUND",
                    format!("Mutation '{mutation_id}' does not exist."),
                )
            })?;

        // Validate: mutation must match unit type
        if mutation.unit_type != unit_type {
            return Err(ImpForgeError::validation(
                "MUT_WRONG_TYPE",
                format!(
                    "Mutation '{}' is for {:?}, not {:?}.",
                    mutation_id, mutation.unit_type, unit_type
                ),
            ));
        }

        // Validate: unit must be at or past the required level
        if level < mutation.level_required {
            return Err(ImpForgeError::validation(
                "MUT_LOW_LEVEL",
                format!(
                    "Unit needs level {} for this mutation (currently {}).",
                    mutation.level_required, level
                ),
            ));
        }

        // Validate: the milestone must not already have a mutation applied
        let applied = self.load_unit_applied_mutations(&conn, unit_id)?;
        let applied_levels: Vec<u32> = applied.iter().map(|a| a.applied_at_level).collect();

        if applied_levels.contains(&mutation.level_required) {
            return Err(ImpForgeError::validation(
                "MUT_ALREADY_APPLIED",
                format!(
                    "Unit already has a mutation at level {}.",
                    mutation.level_required
                ),
            ));
        }

        // Check this specific mutation has not been applied (belt-and-suspenders)
        if applied.iter().any(|a| a.mutation_id == mutation_id) {
            return Err(ImpForgeError::validation(
                "MUT_DUPLICATE",
                format!("Mutation '{mutation_id}' already applied to this unit."),
            ));
        }

        // Apply stat changes (clamp to at least 1 for hp/atk/def)
        let new_hp = (hp as i32 + mutation.stat_changes.hp_bonus).max(1) as u32;
        let new_atk = (attack as i32 + mutation.stat_changes.attack_bonus).max(1) as u32;
        let new_def = (defense as i32 + mutation.stat_changes.defense_bonus).max(1) as u32;
        let new_eff = (efficiency + mutation.stat_changes.production_bonus).clamp(0.0, 5.0);

        // Build new special_ability by appending mutation ability
        let existing_ability: String = conn
            .query_row(
                "SELECT special_ability FROM swarm_units WHERE id = ?1",
                params![unit_id],
                |r| r.get(0),
            )
            .unwrap_or_default();

        let new_ability = if let Some(ref ab) = mutation.special_ability {
            if existing_ability.is_empty() {
                ab.clone()
            } else {
                format!("{existing_ability} + {ab}")
            }
        } else {
            existing_ability
        };

        // Update unit in DB
        conn.execute(
            "UPDATE swarm_units SET hp = ?1, attack = ?2, defense = ?3,
             efficiency = ?4, special_ability = ?5 WHERE id = ?6",
            params![new_hp, new_atk, new_def, new_eff, new_ability, unit_id],
        )
        .map_err(|e| ImpForgeError::internal("MUT_UPDATE", format!("{e}")))?;

        // Record the mutation
        conn.execute(
            "INSERT INTO swarm_mutations (unit_id, mutation_id, applied_at_level)
             VALUES (?1, ?2, ?3)",
            params![unit_id, mutation_id, mutation.level_required],
        )
        .map_err(|e| ImpForgeError::internal("MUT_INSERT", format!("{e}")))?;

        // Return updated unit
        let name: String = conn
            .query_row("SELECT name FROM swarm_units WHERE id = ?1", params![unit_id], |r| r.get(0))
            .unwrap_or_default();
        let assigned_task: Option<String> = conn
            .query_row(
                "SELECT assigned_task FROM swarm_units WHERE id = ?1",
                params![unit_id],
                |r| r.get(0),
            )
            .unwrap_or(None);

        Ok(SwarmUnit {
            id: unit_id.to_string(),
            unit_type,
            name,
            level,
            hp: new_hp,
            attack: new_atk,
            defense: new_def,
            special_ability: new_ability,
            assigned_task,
            efficiency: new_eff,
        })
    }

    /// Get the full mutation tree for a unit type — all mutations grouped by milestone level.
    pub fn get_mutation_tree(
        &self,
        unit_type_str: &str,
    ) -> Result<Vec<Vec<Mutation>>, ImpForgeError> {
        let ut = UnitType::from_str(unit_type_str);
        let catalog = all_mutations();

        // Collect all mutations for this unit type, grouped by level
        let mut level_map: std::collections::BTreeMap<u32, Vec<Mutation>> =
            std::collections::BTreeMap::new();

        for m in catalog {
            if m.unit_type == ut {
                level_map.entry(m.level_required).or_default().push(m);
            }
        }

        Ok(level_map.into_values().collect())
    }
}
