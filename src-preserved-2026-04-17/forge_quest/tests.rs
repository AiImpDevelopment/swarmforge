// SPDX-License-Identifier: Elastic-2.0
//! Tests for ForgeQuest RPG + Colony systems.

use super::*;
use rusqlite::params;
use tempfile::TempDir;

fn test_engine() -> (ForgeQuestEngine, TempDir) {
    let dir = TempDir::new().expect("temp dir");
    let engine = ForgeQuestEngine::new(dir.path()).expect("engine");
    (engine, dir)
}

#[test]
fn test_xp_for_level() {
    assert_eq!(xp_for_level(1), 150);
    assert!(xp_for_level(10) > xp_for_level(9));
    assert!(xp_for_level(100) > xp_for_level(50));
}

#[test]
fn test_create_character() {
    let (engine, _dir) = test_engine();
    let c = engine.create_character("TestHero", "warrior").expect("create");
    assert_eq!(c.name, "TestHero");
    assert_eq!(c.class, CharacterClass::Warrior);
    assert_eq!(c.level, 1);
    assert_eq!(c.hp, 120);
    assert!(c.attack > c.magic);
}

#[test]
fn test_duplicate_character_rejected() {
    let (engine, _dir) = test_engine();
    engine.create_character("Hero", "mage").expect("first");
    assert!(engine.create_character("Hero2", "warrior").is_err());
}

#[test]
fn test_track_action_grants_xp() {
    let (engine, _dir) = test_engine();
    engine.create_character("Hero", "warrior").expect("create");
    // First-time action: 25 base * 1.5 class * 5.0 novelty = 187
    let result = engine.track_action("create_document").expect("track");
    assert_eq!(result.xp_earned, 187); // 25 * 1.5 * 5.0 (first-time novelty)
    assert_eq!(result.gold_earned, 75); // 10 * 1.5 * 5.0
}

#[test]
fn test_novelty_multiplier_diminishes() {
    let (engine, _dir) = test_engine();
    engine.create_character("Hero", "warrior").expect("create");
    // First action: 5x novelty
    let r1 = engine.track_action("create_document").expect("first");
    assert_eq!(r1.xp_earned, 187); // 25 * 1.5 * 5.0

    // Second action (count=1, <10): 2x novelty
    let r2 = engine.track_action("create_document").expect("second");
    assert_eq!(r2.xp_earned, 75); // 25 * 1.5 * 2.0
}

#[test]
fn test_governing_attributes() {
    let (engine, _dir) = test_engine();
    engine.create_character("Hero", "warrior").expect("create");
    let unit = engine.spawn_larva().expect("spawn");
    let attrs = engine.get_unit_attributes(&unit.id).expect("attrs");
    assert!(attrs.strength > 0.0);
    assert!(attrs.speed > 0.0);
    assert!(attrs.intelligence > 0.0);
    assert!(attrs.resilience > 0.0);
    assert!(attrs.charisma > 0.0);
}

#[test]
fn test_zones_are_valid() {
    let zones = all_zones();
    assert_eq!(zones.len(), 20);
    for zone in &zones {
        assert!(!zone.monsters.is_empty(), "Zone {} has no monsters", zone.name);
    }
}

#[test]
fn test_recipes_are_valid() {
    let recipes = all_recipes();
    assert!(recipes.len() >= 8);
    for r in &recipes {
        assert!(!r.materials.is_empty(), "Recipe {} has no materials", r.name);
    }
}

#[test]
fn test_auto_battle() {
    let (engine, _dir) = test_engine();
    engine.create_character("Fighter", "warrior").expect("create");
    let result = engine.auto_battle("beginners_meadow").expect("battle");
    assert!(result.rounds > 0);
    assert!(result.xp_earned > 0);
}

#[test]
fn test_swarm_initial_state() {
    let (engine, _dir) = test_engine();
    let swarm = engine.get_swarm().expect("get_swarm");
    assert_eq!(swarm.units.len(), 0);
    assert_eq!(swarm.buildings.len(), 8);
    assert_eq!(swarm.resources.essence, 100);
}

#[test]
fn test_spawn_larva() {
    let (engine, _dir) = test_engine();
    let unit = engine.spawn_larva().expect("spawn");
    assert_eq!(unit.unit_type, UnitType::ForgeDrone);
    assert_eq!(unit.level, 1);
    let swarm = engine.get_swarm().expect("swarm");
    assert_eq!(swarm.units.len(), 1);
    assert_eq!(swarm.resources.essence, 75);
}

#[test]
fn test_upgrade_swarm_building() {
    let (engine, _dir) = test_engine();
    let bldg = engine.upgrade_building("nest").expect("upgrade");
    assert_eq!(bldg.level, 1);
    assert_eq!(bldg.building_type, BuildingType::Nest);
}

#[test]
fn test_get_missions() {
    let (engine, _dir) = test_engine();
    let missions = engine.get_missions().expect("missions");
    assert_eq!(missions.len(), 20);
    assert!(missions.iter().all(|m| m.status == MissionStatus::Available));
}

#[test]
fn test_evolution_paths_valid() {
    let paths = all_evolution_paths();
    assert!(paths.len() >= 12);
    for p in &paths {
        assert!(p.essence_cost > 0);
        assert!(p.level_requirement > 0);
    }
}

#[test]
fn test_earn_swarm_resources() {
    let (engine, _dir) = test_engine();
    let earned = engine.earn_swarm_resources("create_document").expect("earn");
    assert_eq!(earned.essence, 10);
    assert_eq!(earned.biomass, 5);
    let swarm = engine.get_swarm().expect("swarm");
    assert_eq!(swarm.resources.essence, 110);
}

// ── OGame Colony Tests ────────────────────────────────────────────────

#[test]
fn test_ogame_production_rate() {
    let r0 = ForgeQuestEngine::ogame_production_rate(0, 30.0);
    assert_eq!(r0, 0.0);
    let r1 = ForgeQuestEngine::ogame_production_rate(1, 30.0);
    assert!((r1 - 33.0).abs() < 0.1);
    let r5 = ForgeQuestEngine::ogame_production_rate(5, 30.0);
    assert!(r5 > 200.0);
}

#[test]
fn test_ogame_upgrade_cost() {
    let c0 = ForgeQuestEngine::ogame_upgrade_cost(60.0, 0, 1.5);
    assert_eq!(c0, 60.0);
    let c1 = ForgeQuestEngine::ogame_upgrade_cost(60.0, 1, 1.5);
    assert_eq!(c1, 90.0);
    let c5 = ForgeQuestEngine::ogame_upgrade_cost(60.0, 5, 1.5);
    assert!(c5 > 400.0);
}

#[test]
fn test_planet_initial_state() {
    let (engine, _dir) = test_engine();
    let planet = engine.get_planet().expect("planet");
    assert_eq!(planet.name, "Hive Prime");
    assert_eq!(planet.buildings.len(), 88);  // 22 insect + 22 human + 22 demon + 22 undead
    assert_eq!(planet.research.len(), 80);  // 20 insect + 20 human + 20 demon + 20 undead
    assert_eq!(planet.fleet.len(), 63);     // 18 insect + 15 human + 15 demon + 15 undead
    assert!(planet.resources.biomass >= 500.0);
    assert!(planet.creep.coverage_percent >= 0.0);
}

#[test]
fn test_planet_building_types() {
    let types = [
        PlanetBuildingType::BiomassConverter,
        PlanetBuildingType::MineralDrill,
        PlanetBuildingType::CrystalSynthesizer,
        PlanetBuildingType::SporeExtractor,
        PlanetBuildingType::EnergyNest,
        PlanetBuildingType::CreepGenerator,
        PlanetBuildingType::BroodNest,
        PlanetBuildingType::EvolutionLab,
        PlanetBuildingType::Blighthaven,
        PlanetBuildingType::SporeDefense,
        PlanetBuildingType::BiomassStorage,
        PlanetBuildingType::MineralSilo,
    ];
    for bt in &types {
        assert_eq!(&PlanetBuildingType::from_str(bt.as_str()), bt);
        assert!(!bt.display_name().is_empty());
        assert!(!bt.description().is_empty());
    }
}

#[test]
fn test_tech_types() {
    let types = [
        TechType::Genetics, TechType::ArmorPlating,
        TechType::WeaponSystems, TechType::PropulsionDrive,
        TechType::SwarmIntelligence, TechType::Regeneration,
        TechType::MutationTech, TechType::CreepBiology,
        TechType::SpaceFaring, TechType::DarkMatterResearch,
    ];
    for tt in &types {
        assert_eq!(&TechType::from_str(tt.as_str()), tt);
        assert!(tt.required_lab_level() >= 1);
    }
}

#[test]
fn test_ship_types() {
    let types = [
        ShipType::BioFighter, ShipType::SporeInterceptor,
        ShipType::KrakenFrigate, ShipType::Leviathan,
        ShipType::BioTransporter, ShipType::ColonyPod,
        ShipType::Devourer, ShipType::WorldEater,
    ];
    for st in &types {
        assert_eq!(&ShipType::from_str(st.as_str()), st);
        let (atk, _, hp) = st.combat_stats();
        assert!(atk > 0);
        assert!(hp > 0);
        assert!(st.required_shipyard_level() >= 1);
    }
}

#[test]
fn test_shop_items() {
    let items = all_shop_items();
    assert!(items.len() >= 7);
    for item in &items {
        assert!(item.cost_dark_matter > 0);
        assert!(!item.name.is_empty());
    }
}

#[test]
fn test_galaxy_view() {
    let (engine, _dir) = test_engine();
    let slots = engine.get_galaxy(1, 1).expect("galaxy");
    assert_eq!(slots.len(), 15);
    let player_slot = slots.iter().find(|s| s.position == 4).expect("player slot");
    assert!(player_slot.occupied);
    assert_eq!(player_slot.planet_name.as_deref(), Some("Hive Prime"));
}

#[test]
fn test_collect_resources() {
    let (engine, _dir) = test_engine();
    let resources = engine.collect_planet_resources().expect("collect");
    assert!(resources.biomass >= 500.0);
}

#[test]
fn test_check_timers_empty() {
    let (engine, _dir) = test_engine();
    let completed = engine.check_timers().expect("timers");
    assert!(completed.is_empty());
}

#[test]
fn test_creep_status() {
    let (engine, _dir) = test_engine();
    let creep = engine.get_creep().expect("creep");
    assert_eq!(creep.coverage_percent, 0.0);
    assert_eq!(creep.spread_rate_per_hour, 0.0); // No generator built
}

#[test]
fn test_storage_capacity() {
    let cap0 = ForgeQuestEngine::storage_capacity(0);
    assert!(cap0 >= 5000.0);
    let cap5 = ForgeQuestEngine::storage_capacity(5);
    assert!(cap5 > cap0);
}

#[test]
fn test_unit_type_roundtrip() {
    let types = [
        UnitType::ForgeDrone, UnitType::ImpScout, UnitType::Viper,
        UnitType::ShadowWeaver, UnitType::Skyweaver, UnitType::Overseer,
        UnitType::Titan, UnitType::SwarmMother, UnitType::Ravager,
        UnitType::Matriarch,
    ];
    for ut in &types {
        assert_eq!(&UnitType::from_str(ut.as_str()), ut);
    }
}

#[test]
fn test_building_type_roundtrip() {
    let types = [
        BuildingType::Nest, BuildingType::EvolutionChamber,
        BuildingType::EssencePool, BuildingType::NeuralWeb,
        BuildingType::Armory, BuildingType::Sanctuary,
        BuildingType::Arcanum, BuildingType::WarCouncil,
    ];
    for bt in &types {
        assert_eq!(&BuildingType::from_str(bt.as_str()), bt);
    }
}

// ── Mutation System Tests ────────────────────────────────────────

#[test]
fn test_mutation_milestones() {
    assert_eq!(mutation_milestones_up_to(1), Vec::<u32>::new());
    assert_eq!(mutation_milestones_up_to(4), Vec::<u32>::new());
    assert_eq!(mutation_milestones_up_to(5), vec![5]);
    assert_eq!(mutation_milestones_up_to(10), vec![5, 10]);
    assert_eq!(mutation_milestones_up_to(14), vec![5, 10]);
    assert_eq!(mutation_milestones_up_to(15), vec![5, 10, 15]);
    assert_eq!(mutation_milestones_up_to(25), vec![5, 10, 15, 20, 25]);
}

#[test]
fn test_all_mutations_catalog_valid() {
    let catalog = all_mutations();
    // Every unit type should have at least 3 mutations (level 5)
    let unit_types = [
        UnitType::ForgeDrone, UnitType::ImpScout, UnitType::Viper,
        UnitType::ShadowWeaver, UnitType::Skyweaver, UnitType::Overseer,
        UnitType::Titan, UnitType::SwarmMother, UnitType::Ravager,
        UnitType::Matriarch,
    ];
    for ut in &unit_types {
        let count = catalog.iter().filter(|m| m.unit_type == *ut).count();
        assert!(
            count >= 3,
            "{:?} has only {} mutations, need at least 3",
            ut, count
        );
    }
    // Mutation IDs must be unique
    let mut ids: Vec<&str> = catalog.iter().map(|m| m.id.as_str()).collect();
    ids.sort();
    let unique_count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique_count, "Duplicate mutation IDs found");
}

#[test]
fn test_mutation_type_roundtrip() {
    let types = [
        MutationType::Defensive, MutationType::Offensive,
        MutationType::Utility, MutationType::Evolution,
        MutationType::Specialization,
    ];
    for mt in &types {
        assert_eq!(&MutationType::from_str(mt.as_str()), mt);
    }
}

#[test]
fn test_available_mutations_for_drone_at_5() {
    let (engine, _dir) = test_engine();
    let muts = engine.get_available_mutations("forge_drone", 5).expect("avail");
    assert_eq!(muts.len(), 3);
    let ids: Vec<&str> = muts.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains(&"drone_armor_5"));
    assert!(ids.contains(&"drone_blade_5"));
    assert!(ids.contains(&"drone_neural_5"));
}

#[test]
fn test_available_mutations_at_nonexistent_level() {
    let (engine, _dir) = test_engine();
    let muts = engine.get_available_mutations("forge_drone", 7).expect("avail");
    assert!(muts.is_empty(), "No mutations at non-milestone levels");
}

#[test]
fn test_get_unit_mutations_no_pending_at_level_1() {
    let (engine, _dir) = test_engine();
    let unit = engine.spawn_larva().expect("spawn");
    let um = engine.get_unit_mutations(&unit.id).expect("mutations");
    assert!(um.applied_mutations.is_empty());
    assert!(um.pending_choices.is_empty(), "Level 1 unit has no mutation milestone");
    assert_eq!(um.unit_level, 1);
}

#[test]
fn test_apply_mutation_requires_level() {
    let (engine, _dir) = test_engine();
    let unit = engine.spawn_larva().expect("spawn");
    // Unit is level 1, mutation needs level 5
    let result = engine.apply_mutation(&unit.id, "drone_armor_5");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("level 5"), "Error should mention level requirement");
}

#[test]
fn test_apply_mutation_wrong_type_rejected() {
    let (engine, _dir) = test_engine();
    let unit = engine.spawn_larva().expect("spawn");
    // Set unit to level 5 manually for testing
    {
        let conn = engine.conn.lock().expect("lock");
        conn.execute(
            "UPDATE swarm_units SET level = 5 WHERE id = ?1",
            params![unit.id],
        ).expect("set level");
    }
    // Try to apply a scout mutation to a drone
    let result = engine.apply_mutation(&unit.id, "scout_dodge_5");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("ImpScout"), "Error should mention type mismatch");
}

#[test]
fn test_apply_mutation_success() {
    let (engine, _dir) = test_engine();
    let unit = engine.spawn_larva().expect("spawn");
    let (orig_hp, orig_def) = (unit.hp, unit.defense);
    // Set level to 5
    {
        let conn = engine.conn.lock().expect("lock");
        conn.execute(
            "UPDATE swarm_units SET level = 5 WHERE id = ?1",
            params![unit.id],
        ).expect("set level");
    }
    let updated = engine.apply_mutation(&unit.id, "drone_armor_5").expect("apply");
    assert!(updated.hp > orig_hp, "HP should increase from Reinforced Carapace");
    assert!(updated.defense > orig_def, "DEF should increase");

    // Verify mutation is recorded
    let um = engine.get_unit_mutations(&unit.id).expect("mutations");
    assert_eq!(um.applied_mutations.len(), 1);
    assert_eq!(um.applied_mutations[0].mutation_id, "drone_armor_5");
    assert_eq!(um.applied_mutations[0].applied_at_level, 5);
}

#[test]
fn test_apply_mutation_duplicate_milestone_rejected() {
    let (engine, _dir) = test_engine();
    let unit = engine.spawn_larva().expect("spawn");
    {
        let conn = engine.conn.lock().expect("lock");
        conn.execute(
            "UPDATE swarm_units SET level = 5 WHERE id = ?1",
            params![unit.id],
        ).expect("set level");
    }
    engine.apply_mutation(&unit.id, "drone_armor_5").expect("first");
    // Try applying another level-5 mutation
    let result = engine.apply_mutation(&unit.id, "drone_blade_5");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("level 5"), "Should reject duplicate milestone");
}

#[test]
fn test_pending_choices_appear_at_milestone() {
    let (engine, _dir) = test_engine();
    let unit = engine.spawn_larva().expect("spawn");
    {
        let conn = engine.conn.lock().expect("lock");
        conn.execute(
            "UPDATE swarm_units SET level = 5 WHERE id = ?1",
            params![unit.id],
        ).expect("set level");
    }
    let um = engine.get_unit_mutations(&unit.id).expect("mutations");
    assert_eq!(um.pending_choices.len(), 3, "Should have 3 mutation choices at level 5");

    // Apply one, then check pending clears for level 5
    engine.apply_mutation(&unit.id, "drone_neural_5").expect("apply");
    let um2 = engine.get_unit_mutations(&unit.id).expect("mutations2");
    assert!(um2.pending_choices.is_empty(), "No pending after choosing at level 5 (next milestone is 10)");
}

#[test]
fn test_pending_choices_cascade_unclaimed() {
    let (engine, _dir) = test_engine();
    let unit = engine.spawn_larva().expect("spawn");
    // Set to level 10 without claiming level 5 mutation
    {
        let conn = engine.conn.lock().expect("lock");
        conn.execute(
            "UPDATE swarm_units SET level = 10 WHERE id = ?1",
            params![unit.id],
        ).expect("set level");
    }
    let um = engine.get_unit_mutations(&unit.id).expect("mutations");
    // Should show level 5 choices first (oldest unclaimed milestone)
    assert_eq!(um.pending_choices.len(), 3);
    assert!(
        um.pending_choices.iter().all(|m| m.level_required == 5),
        "Should show level 5 choices first"
    );
}

#[test]
fn test_mutation_tree_structure() {
    let (engine, _dir) = test_engine();
    let tree = engine.get_mutation_tree("forge_drone").expect("tree");
    assert!(tree.len() >= 3, "Drone should have at least 3 milestone tiers");
    // First tier should be level 5 mutations
    assert_eq!(tree[0].len(), 3, "Each milestone should have 3 choices");
    assert!(tree[0].iter().all(|m| m.level_required == 5));
    // Second tier should be level 10
    assert_eq!(tree[1].len(), 3);
    assert!(tree[1].iter().all(|m| m.level_required == 10));
}

#[test]
fn test_mutation_nonexistent_unit_rejected() {
    let (engine, _dir) = test_engine();
    let result = engine.get_unit_mutations("nonexistent_unit_42");
    assert!(result.is_err());
}

#[test]
fn test_mutation_nonexistent_mutation_rejected() {
    let (engine, _dir) = test_engine();
    let unit = engine.spawn_larva().expect("spawn");
    {
        let conn = engine.conn.lock().expect("lock");
        conn.execute(
            "UPDATE swarm_units SET level = 5 WHERE id = ?1",
            params![unit.id],
        ).expect("set level");
    }
    let result = engine.apply_mutation(&unit.id, "totally_fake_mutation");
    assert!(result.is_err());
}

#[test]
fn test_specialization_applies_large_bonus() {
    let (engine, _dir) = test_engine();
    let unit = engine.spawn_larva().expect("spawn");
    let orig_hp = unit.hp;
    // Need mutations at 5 and 10 first, then specialization at 15
    {
        let conn = engine.conn.lock().expect("lock");
        conn.execute(
            "UPDATE swarm_units SET level = 15 WHERE id = ?1",
            params![unit.id],
        ).expect("set level");
    }
    engine.apply_mutation(&unit.id, "drone_armor_5").expect("mut5");
    engine.apply_mutation(&unit.id, "drone_acid_10").expect("mut10");
    let updated = engine.apply_mutation(&unit.id, "drone_elite_15").expect("mut15");
    // Elite Drone adds 15 HP on top of previous mutations
    assert!(updated.hp > orig_hp + 15, "Specialization should give significant HP boost");
    // Original ability + 3 mutations chained with " + "
    assert!(
        updated.special_ability.matches(" + ").count() >= 2,
        "Should chain abilities with +, got: {}",
        updated.special_ability,
    );
}

// -----------------------------------------------------------------------
// Faction & new unit tests
// -----------------------------------------------------------------------

#[test]
fn test_faction_enum_roundtrip() {
    assert_eq!(Faction::from_str("insects"), Faction::Insects);
    assert_eq!(Faction::from_str("demons"), Faction::Demons);
    assert_eq!(Faction::from_str("undead"), Faction::Undead);
    assert_eq!(Faction::from_str("humans"), Faction::Humans);
    assert_eq!(Faction::from_str("unknown"), Faction::Insects); // default
    assert_eq!(Faction::Insects.as_str(), "insects");
    assert_eq!(Faction::Demons.as_str(), "demons");
    assert_eq!(Faction::Undead.as_str(), "undead");
    assert_eq!(Faction::Humans.as_str(), "humans");
}

#[test]
fn test_demon_units_have_correct_faction() {
    let demons = [
        UnitType::DemonImp, UnitType::Hellspawn, UnitType::Succubus,
        UnitType::Kultist, UnitType::FlameImp, UnitType::Infernal,
        UnitType::WarpFiend, UnitType::PitLord, UnitType::ChaosKnight,
        UnitType::Demonologist, UnitType::DoomGuard, UnitType::Balrog,
        UnitType::ShadowDemon, UnitType::Archfiend, UnitType::HellfireGolem,
        UnitType::VoidWalker, UnitType::DemonPrince, UnitType::AbyssLord,
        UnitType::Baalzephon, UnitType::DarkArchon,
    ];
    assert_eq!(demons.len(), 20, "Demon faction must have exactly 20 units");
    for unit in &demons {
        assert_eq!(unit.faction(), Faction::Demons, "{:?} should be Demon faction", unit);
    }
}

#[test]
fn test_undead_units_have_correct_faction() {
    let undead = [
        UnitType::Ghoul, UnitType::SkeletonWarrior, UnitType::ZombieHorde,
        UnitType::PlagueDoctor, UnitType::GraveDigger, UnitType::Banshee,
        UnitType::DeathKnight, UnitType::Wraith, UnitType::CryptGuard,
        UnitType::SoulReaper, UnitType::BoneGolem, UnitType::Necromancer,
        UnitType::Revenant, UnitType::Abomination, UnitType::VampireLord,
        UnitType::LichKing, UnitType::DreadLord, UnitType::BoneHydra,
        UnitType::PhantomLegion, UnitType::DeathEmperor,
    ];
    assert_eq!(undead.len(), 20, "Undead faction must have exactly 20 units");
    for unit in &undead {
        assert_eq!(unit.faction(), Faction::Undead, "{:?} should be Undead faction", unit);
    }
}

#[test]
fn test_insect_units_remain_insect_faction() {
    let insects = [
        UnitType::ForgeDrone, UnitType::ImpScout, UnitType::Viper,
        UnitType::ShadowWeaver, UnitType::Matriarch, UnitType::Dominatrix,
    ];
    for unit in &insects {
        assert_eq!(unit.faction(), Faction::Insects, "{:?} should be Insect faction", unit);
    }
}

#[test]
fn test_new_unit_tiers_are_valid() {
    let t1_demons = [
        UnitType::DemonImp, UnitType::Hellspawn, UnitType::Succubus,
        UnitType::Kultist, UnitType::FlameImp,
    ];
    let t4_demons = [
        UnitType::VoidWalker, UnitType::DemonPrince, UnitType::AbyssLord,
        UnitType::Baalzephon, UnitType::DarkArchon,
    ];
    let t1_undead = [
        UnitType::Ghoul, UnitType::SkeletonWarrior, UnitType::ZombieHorde,
        UnitType::PlagueDoctor, UnitType::GraveDigger,
    ];
    let t4_undead = [
        UnitType::LichKing, UnitType::DreadLord, UnitType::BoneHydra,
        UnitType::PhantomLegion, UnitType::DeathEmperor,
    ];
    for u in &t1_demons { assert_eq!(u.tier(), 1, "{:?} should be tier 1", u); }
    for u in &t4_demons { assert_eq!(u.tier(), 4, "{:?} should be tier 4", u); }
    for u in &t1_undead { assert_eq!(u.tier(), 1, "{:?} should be tier 1", u); }
    for u in &t4_undead { assert_eq!(u.tier(), 4, "{:?} should be tier 4", u); }
}

#[test]
fn test_new_unit_stats_scale_with_tier() {
    // T4 units must have strictly higher HP than T1 units within each faction
    let (t1_hp, _, _) = UnitType::DemonImp.base_stats();
    let (t4_hp, _, _) = UnitType::Baalzephon.base_stats();
    assert!(t4_hp > t1_hp * 3, "T4 Demon HP should far exceed T1");

    let (t1u_hp, _, _) = UnitType::Ghoul.base_stats();
    let (t4u_hp, _, _) = UnitType::DeathEmperor.base_stats();
    assert!(t4u_hp > t1u_hp * 3, "T4 Undead HP should far exceed T1");
}

#[test]
fn test_new_unit_as_str_from_str_roundtrip() {
    let units = [
        UnitType::DemonImp, UnitType::Baalzephon, UnitType::DarkArchon,
        UnitType::Ghoul, UnitType::DeathEmperor, UnitType::BoneHydra,
        UnitType::ChaosKnight, UnitType::Necromancer, UnitType::VampireLord,
        UnitType::Peasant, UnitType::KingChampion, UnitType::DragonKnight,
    ];
    for unit in &units {
        let s = unit.as_str();
        let roundtripped = UnitType::from_str(s);
        assert_eq!(*unit, roundtripped, "Roundtrip failed for {:?} ('{}')", unit, s);
    }
}

#[test]
fn test_human_units_have_correct_faction() {
    let humans = [
        UnitType::Peasant, UnitType::Footman, UnitType::Rifleman,
        UnitType::Priest, UnitType::Militia, UnitType::Knight,
        UnitType::Sorceress, UnitType::CaptainOfTheGuard, UnitType::Crossbowman,
        UnitType::BattleMage, UnitType::Paladin, UnitType::GryphonRider,
        UnitType::SiegeEngineer, UnitType::SpellBreaker, UnitType::MortarTeam,
        UnitType::KingChampion, UnitType::Archmage, UnitType::GrandMarshal,
        UnitType::DragonKnight, UnitType::HighInquisitor,
    ];
    assert_eq!(humans.len(), 20, "Human faction must have exactly 20 units");
    for unit in &humans {
        assert_eq!(unit.faction(), Faction::Humans, "{:?} should be Human faction", unit);
    }
}

#[test]
fn test_human_unit_tiers_are_valid() {
    let t1_humans = [
        UnitType::Peasant, UnitType::Footman, UnitType::Rifleman,
        UnitType::Priest, UnitType::Militia,
    ];
    let t4_humans = [
        UnitType::KingChampion, UnitType::Archmage, UnitType::GrandMarshal,
        UnitType::DragonKnight, UnitType::HighInquisitor,
    ];
    for u in &t1_humans { assert_eq!(u.tier(), 1, "{:?} should be tier 1", u); }
    for u in &t4_humans { assert_eq!(u.tier(), 4, "{:?} should be tier 4", u); }
}

#[test]
fn test_human_unit_stats_scale_with_tier() {
    let (t1_hp, _, _) = UnitType::Peasant.base_stats();
    let (t4_hp, _, _) = UnitType::DragonKnight.base_stats();
    assert!(t4_hp > t1_hp * 3, "T4 Human HP should far exceed T1");
}

#[test]
fn test_human_building_type_roundtrip() {
    let buildings = [
        PlanetBuildingType::TownHall, PlanetBuildingType::Keep,
        PlanetBuildingType::Castle, PlanetBuildingType::HumanBarracks,
        PlanetBuildingType::LumberMill, PlanetBuildingType::HumanBlacksmith,
        PlanetBuildingType::ArcaneSanctum, PlanetBuildingType::Workshop,
        PlanetBuildingType::GryphonAviary, PlanetBuildingType::AltarOfKings,
        PlanetBuildingType::ScoutTower, PlanetBuildingType::GuardTower,
        PlanetBuildingType::CannonTower, PlanetBuildingType::ArcaneTower,
        PlanetBuildingType::Farm, PlanetBuildingType::Marketplace,
        PlanetBuildingType::Church, PlanetBuildingType::Academy,
        PlanetBuildingType::SiegeWorks, PlanetBuildingType::MageTower,
        PlanetBuildingType::Harbor, PlanetBuildingType::FortressWall,
    ];
    assert_eq!(buildings.len(), 22, "Human faction must have exactly 22 buildings");
    for b in &buildings {
        let s = b.as_str();
        let roundtripped = PlanetBuildingType::from_str(s);
        assert_eq!(*b, roundtripped, "Roundtrip failed for {:?} ('{}')", b, s);
    }
}

#[test]
fn test_human_tech_type_roundtrip() {
    let techs = [
        TechType::IronForging, TechType::SteelForging, TechType::MithrilForging,
        TechType::IronPlating, TechType::SteelPlating, TechType::MithrilPlating,
        TechType::LongRifles, TechType::Rifling, TechType::Masonry,
        TechType::AdvancedMasonry, TechType::HumanFortification,
        TechType::AnimalHusbandry, TechType::CloudTechnology,
        TechType::ArcaneTraining, TechType::HolyLightTech,
        TechType::BlizzardResearch, TechType::Telescope,
        TechType::CombustionEngine, TechType::Logistics, TechType::Diplomacy,
    ];
    assert_eq!(techs.len(), 20, "Human faction must have exactly 20 technologies");
    for t in &techs {
        let s = t.as_str();
        let roundtripped = TechType::from_str(s);
        assert_eq!(*t, roundtripped, "Roundtrip failed for {:?} ('{}')", t, s);
    }
}

#[test]
fn test_human_ship_type_roundtrip() {
    let ships = [
        ShipType::ScoutFighter, ShipType::AssaultFighter, ShipType::StrikeCruiser,
        ShipType::HumanBattleship, ShipType::BattleCruiser, ShipType::StrategicBomber,
        ShipType::FleetDestroyer, ShipType::OrbitalCannon, ShipType::SalvageVessel,
        ShipType::SurveyShip, ShipType::LightFreighter, ShipType::HeavyFreighter,
        ShipType::SalvageTug, ShipType::SpyDrone, ShipType::ColonyTransport,
    ];
    assert_eq!(ships.len(), 15, "Human faction must have exactly 15 ships");
    for s in &ships {
        let str_val = s.as_str();
        let roundtripped = ShipType::from_str(str_val);
        assert_eq!(*s, roundtripped, "Roundtrip failed for {:?} ('{}')", s, str_val);
    }
}

// -- Dark Matter Earnings --

#[test]
fn test_dm_source_roundtrip() {
    let all = [
        DmSource::DocumentWritten,
        DmSource::CodeCommitted,
        DmSource::SpreadsheetCreated,
        DmSource::EmailSent,
        DmSource::TaskCompleted,
        DmSource::ActiveUsage,
        DmSource::TestsPassed,
        DmSource::BuildSucceeded,
        DmSource::MilestoneReached,
        DmSource::DailyLogin,
        DmSource::WeeklyChallenge,
    ];
    for src in &all {
        let s = src.as_str();
        let back = DmSource::from_str(s);
        assert!(back.is_some(), "from_str failed for '{}'", s);
        assert_eq!(*src, back.expect("as str should succeed"));
    }
}

#[test]
fn test_dm_source_amounts() {
    assert_eq!(DmSource::DocumentWritten.dm_amount(), 2);
    assert_eq!(DmSource::CodeCommitted.dm_amount(), 3);
    assert_eq!(DmSource::SpreadsheetCreated.dm_amount(), 2);
    assert_eq!(DmSource::EmailSent.dm_amount(), 1);
    assert_eq!(DmSource::TaskCompleted.dm_amount(), 1);
    assert_eq!(DmSource::ActiveUsage.dm_amount(), 1);
    assert_eq!(DmSource::TestsPassed.dm_amount(), 2);
    assert_eq!(DmSource::BuildSucceeded.dm_amount(), 2);
    assert_eq!(DmSource::MilestoneReached.dm_amount(), 10);
    assert_eq!(DmSource::DailyLogin.dm_amount(), 5);
    assert_eq!(DmSource::WeeklyChallenge.dm_amount(), 25);
}

#[test]
fn test_dm_invalid_source() {
    assert!(DmSource::from_str("invalid_source").is_none());
    assert!(DmSource::from_str("").is_none());
}

#[test]
fn test_dm_earn_and_history() {
    let (engine, _dir) = test_engine();

    // Earn from a document
    let total = engine
        .earn_dark_matter_from_source(&DmSource::DocumentWritten)
        .expect("earn doc");
    assert!(total >= 2);

    // Earn from a commit
    let total2 = engine
        .earn_dark_matter_from_source(&DmSource::CodeCommitted)
        .expect("earn commit");
    assert!(total2 >= total + 3);

    // Check history
    let history = engine.get_dark_matter_history(10).expect("history");
    assert_eq!(history.len(), 2);
    // Most recent first
    assert_eq!(history[0].source, DmSource::CodeCommitted);
    assert_eq!(history[0].amount, 3);
    assert_eq!(history[1].source, DmSource::DocumentWritten);
    assert_eq!(history[1].amount, 2);
}

#[test]
fn test_dm_history_empty() {
    let (engine, _dir) = test_engine();
    let history = engine.get_dark_matter_history(10).expect("empty history");
    assert!(history.is_empty());
}

#[test]
fn test_dm_weekly_challenge_high_value() {
    let (engine, _dir) = test_engine();
    let total = engine
        .earn_dark_matter_from_source(&DmSource::WeeklyChallenge)
        .expect("weekly");
    assert!(total >= 25);
}
