// SPDX-License-Identifier: Elastic-2.0
//! ForgeQuest RPG types -- character, items, equipment, skills, zones, quests.

use serde::{Deserialize, Serialize};

/// Health declaration for this module.
const _MODULE_HEALTH: (&str, &str) = ("forge_quest::types", "RPG Types");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CharacterClass {
    Warrior,    // Bonus from documents (strength)
    Mage,       // Bonus from AI queries (magic)
    Ranger,     // Bonus from workflows (speed)
    Blacksmith, // Bonus from spreadsheets (crafting)
    Bard,       // Bonus from social media (charisma)
    Scholar,    // Bonus from notes/research (wisdom)
}

impl CharacterClass {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Warrior => "warrior",
            Self::Mage => "mage",
            Self::Ranger => "ranger",
            Self::Blacksmith => "blacksmith",
            Self::Bard => "bard",
            Self::Scholar => "scholar",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "warrior" => Self::Warrior,
            "mage" => Self::Mage,
            "ranger" => Self::Ranger,
            "blacksmith" => Self::Blacksmith,
            "bard" => Self::Bard,
            "scholar" => Self::Scholar,
            _ => Self::Warrior,
        }
    }

    /// Class-specific stat bonus multiplier for matching actions.
    pub(crate) fn bonus_multiplier(&self, action: &str) -> f64 {
        match (self, action) {
            (Self::Warrior, "create_document") => 1.5,
            (Self::Mage, "ai_query") => 1.5,
            (Self::Ranger, "run_workflow") => 1.5,
            (Self::Blacksmith, "create_spreadsheet") => 1.5,
            (Self::Bard, "social_post") => 1.5,
            (Self::Scholar, "create_note") => 1.5,
            _ => 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Mythic,
}

impl ItemRarity {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Uncommon => "uncommon",
            Self::Rare => "rare",
            Self::Epic => "epic",
            Self::Legendary => "legendary",
            Self::Mythic => "mythic",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "common" => Self::Common,
            "uncommon" => Self::Uncommon,
            "rare" => Self::Rare,
            "epic" => Self::Epic,
            "legendary" => Self::Legendary,
            "mythic" => Self::Mythic,
            _ => Self::Common,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WeaponType {
    Sword,
    Staff,
    Bow,
    Hammer,
    Lute,
    Tome,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArmorSlot {
    Head,
    Chest,
    Legs,
    Boots,
    Gloves,
    Shield,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    Weapon(WeaponType),
    Armor(ArmorSlot),
    Accessory,
    Material,
    Potion,
    QuestItem,
}
impl ItemType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Weapon(_) => "weapon",
            Self::Armor(_) => "armor",
            Self::Accessory => "accessory",
            Self::Material => "material",
            Self::Potion => "potion",
            Self::QuestItem => "quest_item",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStats {
    pub attack: i32,
    pub defense: i32,
    pub magic: i32,
    pub hp_bonus: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub item_type: String,
    pub rarity: ItemRarity,
    pub stats: ItemStats,
    pub level_req: u32,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct Equipment {
    pub weapon: Option<Item>,
    pub head: Option<Item>,
    pub chest: Option<Item>,
    pub legs: Option<Item>,
    pub boots: Option<Item>,
    pub gloves: Option<Item>,
    pub accessory1: Option<Item>,
    pub accessory2: Option<Item>,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SkillBranch {
    Combat,
    Defense,
    Magic,
    Crafting,
    Leadership,
    Wisdom,
}
impl SkillBranch {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Combat => "combat",
            Self::Defense => "defense",
            Self::Magic => "magic",
            Self::Crafting => "crafting",
            Self::Leadership => "leadership",
            Self::Wisdom => "wisdom",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "combat" => Self::Combat,
            "defense" => Self::Defense,
            "magic" => Self::Magic,
            "crafting" => Self::Crafting,
            "leadership" => Self::Leadership,
            "wisdom" => Self::Wisdom,
            _ => Self::Combat,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: u32,
    pub points_invested: u32,
    pub max_points: u32,
    pub prerequisite: Option<String>,
    pub effect: String,
    pub branch: SkillBranch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestCharacter {
    pub name: String,
    pub class: CharacterClass,
    pub level: u32,
    pub xp: u64,
    pub hp: u32,
    pub max_hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub magic: u32,
    pub gold: u64,
    pub inventory: Vec<Item>,
    pub equipped: Equipment,
    pub skill_points: u32,
    pub skills: Vec<Skill>,
    pub quests_completed: u32,
    pub monsters_slain: u64,
    pub current_zone: String,
    pub guild: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monster {
    pub name: String,
    pub level: u32,
    pub hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub xp_reward: u64,
    pub gold_reward: u64,
    pub loot_table: Vec<(String, f32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QuestObjective {
    CreateDocuments(u32),
    RunWorkflows(u32),
    AiQueries(u32),
    SlayMonsters(u32),
    CraftItems(u32),
    ReachLevel(u32),
    CompleteStreak(u32),
    UseModules(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub objective: String,
    pub objective_target: u32,
    pub objective_progress: u32,
    pub reward_xp: u64,
    pub reward_gold: u64,
    pub reward_items: Vec<String>,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftingRecipe {
    pub id: String,
    pub name: String,
    pub result_item_id: String,
    pub materials: Vec<(String, u32)>,
    pub required_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub description: String,
    pub level_min: u32,
    pub level_max: u32,
    pub monsters: Vec<Monster>,
    pub boss: Option<Monster>,
    pub unlock_condition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpgReward {
    pub xp: u64,
    pub gold: u64,
    pub material: Option<String>,
    pub monster_fight: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleResult {
    pub victory: bool,
    pub monster_name: String,
    pub monster_level: u32,
    pub damage_dealt: u32,
    pub damage_taken: u32,
    pub xp_earned: u64,
    pub gold_earned: u64,
    pub loot: Vec<Item>,
    pub rounds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub xp_earned: u64,
    pub gold_earned: u64,
    pub material_gained: Option<String>,
    pub level_up: bool,
    pub new_level: u32,
    pub battle: Option<BattleResult>,
    pub quest_completed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub name: String,
    pub class: CharacterClass,
    pub level: u32,
    pub xp: u64,
    pub monsters_slain: u64,
    pub quests_completed: u32,
}
