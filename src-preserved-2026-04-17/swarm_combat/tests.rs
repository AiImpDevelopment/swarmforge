use super::*;

// -- Damage formula --

    #[test]
    fn test_neutral_damage() {
        // Fire vs Scale = 1.0x
        let dmg = calculate_damage(100.0, 100.0, DamageType::Fire, ArmorType::Scale);
        // 100 * 1.0 * 100 / (100 + 100) = 50
        assert!((dmg - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_super_effective_damage() {
        // Fire vs Chitin = 1.5x
        let dmg = calculate_damage(100.0, 100.0, DamageType::Fire, ArmorType::Chitin);
        // 100 * 1.5 * 100 / 200 = 75
        assert!((dmg - 75.0).abs() < 0.01);
    }

    #[test]
    fn test_resisted_damage() {
        // Fire vs Hellforged = 0.25x
        let dmg = calculate_damage(100.0, 100.0, DamageType::Fire, ArmorType::Hellforged);
        // 100 * 0.25 * 100 / 200 = 12.5
        assert!((dmg - 12.5).abs() < 0.01);
    }

    #[test]
    fn test_zero_armor_means_full_damage() {
        let dmg = calculate_damage(200.0, 0.0, DamageType::Blunt, ArmorType::Crystal);
        // 200 * 1.5 * 100 / 100 = 300
        assert!((dmg - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_minimum_chip_damage() {
        // Very high armor, very low attack should still do at least 1
        let dmg = calculate_damage(1.0, 999999.0, DamageType::Blunt, ArmorType::Ethereal);
        assert!(dmg >= 1.0);
    }

    // -- Type multiplier matrix --

    #[test]
    fn test_type_matrix_symmetry_check() {
        // Verify Fire vs Chitin = 1.5, Chitin-side check
        assert!((type_multiplier(&DamageType::Fire, &ArmorType::Chitin) - 1.5).abs() < 0.001);
        // Electricity vs Crystal = 1.5
        assert!((type_multiplier(&DamageType::Electricity, &ArmorType::Crystal) - 1.5).abs() < 0.001);
        // Stab vs Void = 0.25
        assert!((type_multiplier(&DamageType::Stab, &ArmorType::Void) - 0.25).abs() < 0.001);
    }

    // -- Travel time --

    #[test]
    fn test_same_system_travel() {
        let fleet = vec![("bio_fighter".to_string(), 10)];
        let time = calculate_travel_time((1, 1, 3), (1, 1, 7), &fleet, 1.0);
        // distance = |3-7|*2500 + 5000 = 15000
        // bio_fighter speed = 12500
        // time = ceil(15000 / 12500) = 2
        assert_eq!(time, 2);
    }

    #[test]
    fn test_cross_system_travel() {
        let fleet = vec![("leviathan".to_string(), 1)];
        let time = calculate_travel_time((1, 2, 1), (1, 5, 1), &fleet, 1.0);
        // distance = |2-5|*19500 + 25000 = 83500
        // leviathan speed = 5000
        // time = ceil(83500/5000) = 17
        assert_eq!(time, 17);
    }

    #[test]
    fn test_cross_galaxy_travel() {
        let fleet = vec![("bio_fighter".to_string(), 5)];
        let time = calculate_travel_time((1, 1, 1), (3, 1, 1), &fleet, 1.0);
        // distance = |1-3|*200000 + 100000 = 500000
        // bio_fighter speed = 12500
        // time = ceil(500000/12500) = 40
        assert_eq!(time, 40);
    }

    #[test]
    fn test_slowest_ship_determines_speed() {
        // Mix fast (bio_fighter 12500) and slow (colony_pod 2500)
        let fleet = vec![
            ("bio_fighter".to_string(), 10),
            ("colony_pod".to_string(), 1),
        ];
        let time = calculate_travel_time((1, 1, 1), (1, 1, 5), &fleet, 1.0);
        // distance = 4*2500 + 5000 = 15000
        // slowest = colony_pod @ 2500
        // time = ceil(15000 / 2500) = 6
        assert_eq!(time, 6);
    }

    // -- Battle simulation --

    #[test]
    fn test_overwhelming_attacker_wins() {
        let attacker = vec![("devourer".to_string(), 50)];
        let defender = vec![("bio_fighter".to_string(), 5)];
        let result = simulate_battle(&attacker, &defender);
        assert!(result.attacker_won);
        assert!(result.rounds >= 1);
        assert!(result.loot.total() > 0.0);
    }

    #[test]
    fn test_overwhelming_defender_wins() {
        let attacker = vec![("mycetic_spore".to_string(), 2)];
        let defender = vec![("void_kraken".to_string(), 10)];
        let result = simulate_battle(&attacker, &defender);
        assert!(!result.attacker_won);
        // Attacker lost all mycetic spores
        assert!(!result.attacker_losses.is_empty());
    }

    #[test]
    fn test_battle_max_6_rounds() {
        let attacker = vec![("bio_fighter".to_string(), 1)];
        let defender = vec![("bio_fighter".to_string(), 1)];
        let result = simulate_battle(&attacker, &defender);
        assert!(result.rounds <= 6);
    }

    // -- Persistence --

    #[test]
    fn test_engine_dispatch_and_recall() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let engine = SwarmCombatEngine::new(tmp.path()).expect("engine init");

        let mission = engine
            .dispatch_fleet(
                (1, 1, 1),
                (1, 2, 3),
                vec![("bio_fighter".to_string(), 10)],
                "attack",
                Resources::default(),
                1.0,
            )
            .expect("dispatch");

        assert_eq!(mission.status, FleetStatus::Outbound);

        // Recall
        let recalled = engine.recall_fleet(&mission.id).expect("recall");
        assert_eq!(recalled.status, FleetStatus::Returning);

        // Cannot recall again
        let err = engine.recall_fleet(&mission.id);
        assert!(err.is_err());
    }

    #[test]
    fn test_engine_list_fleets() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let engine = SwarmCombatEngine::new(tmp.path()).expect("engine init");

        engine
            .dispatch_fleet(
                (1, 1, 1),
                (1, 1, 5),
                vec![("bio_fighter".to_string(), 5)],
                "transport",
                Resources { biomass: 100.0, ..Default::default() },
                1.0,
            )
            .expect("dispatch");

        let list = engine.list_fleets().expect("list");
        assert_eq!(list.len(), 1);
        assert!(list[0].cargo.biomass > 0.0);
    }

    #[test]
    fn test_battle_report_persist() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let engine = SwarmCombatEngine::new(tmp.path()).expect("engine init");

        let result = simulate_battle(
            &[("kraken_frigate".to_string(), 20)],
            &[("bio_fighter".to_string(), 10)],
        );

        engine.save_battle_report(&result, None).expect("save report");

        let loaded = engine.get_battle_report(&result.id).expect("load report");
        assert_eq!(loaded.id, result.id);
        assert_eq!(loaded.rounds, result.rounds);
        assert_eq!(loaded.attacker_won, result.attacker_won);
    }

    // -- Faction Terrain --

    #[test]
    fn test_terrain_effect_insects() {
        let eff = terrain_get_effect(&FactionTerrain::ChitinousResin);
        assert!((eff.move_speed_bonus - 0.25).abs() < f64::EPSILON);
        assert!((eff.attack_speed_bonus - 0.10).abs() < f64::EPSILON);
        assert!(eff.vision_granted);
        assert!((eff.armor_bonus).abs() < f64::EPSILON);
    }

    #[test]
    fn test_terrain_effect_demons() {
        let eff = terrain_get_effect(&FactionTerrain::HellfireCorruption);
        assert!((eff.ability_power_bonus - 0.15).abs() < f64::EPSILON);
        assert!(eff.mana_regen_bonus > 0.0);
        assert!((eff.damage_per_sec_to_enemies - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn test_terrain_effect_undead() {
        let eff = terrain_get_effect(&FactionTerrain::Necrosis);
        assert!((eff.hp_regen_per_sec - 0.02).abs() < f64::EPSILON);
        assert!((eff.auto_raise_chance - 0.30).abs() < f64::EPSILON);
    }

    #[test]
    fn test_terrain_effect_humans() {
        let eff = terrain_get_effect(&FactionTerrain::HumanSettlement);
        assert!((eff.armor_bonus - 0.10).abs() < f64::EPSILON);
        assert!((eff.resource_bonus - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_terrain_roundtrip() {
        let all = [
            FactionTerrain::ChitinousResin,
            FactionTerrain::HellfireCorruption,
            FactionTerrain::Necrosis,
            FactionTerrain::HumanSettlement,
            FactionTerrain::Neutral,
            FactionTerrain::Contested,
            FactionTerrain::Nexus,
        ];
        for t in &all {
            let s = t.as_str();
            let back = FactionTerrain::from_str(s);
            assert_eq!(*t, back, "Roundtrip failed for {:?}", t);
        }
    }

    #[test]
    fn test_terrain_spread_strengthens() {
        let mut tiles = vec![TerrainTile {
            x: 0,
            y: 0,
            terrain_type: FactionTerrain::ChitinousResin,
            strength: 0.5,
            spread_rate: 1.0,
            owner_faction: "insects".to_string(),
        }];
        terrain_spread_tick(&mut tiles, 3600.0); // 1 hour
        assert!(tiles[0].strength > 0.5);
        assert!(tiles[0].strength <= 1.0);
    }

    #[test]
    fn test_terrain_cross_faction_damage() {
        let tile = TerrainTile {
            x: 0,
            y: 0,
            terrain_type: FactionTerrain::HellfireCorruption,
            strength: 1.0,
            spread_rate: 0.5,
            owner_faction: "demons".to_string(),
        };

        // Demons take no damage on their own terrain
        let own = terrain_cross_faction_damage("demons", &tile);
        assert!((own).abs() < f64::EPSILON);

        // Non-demons take 1%/sec scaled by strength
        let enemy = terrain_cross_faction_damage("insects", &tile);
        assert!((enemy - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn test_terrain_contested_detection() {
        let tiles = vec![
            TerrainTile {
                x: 0, y: 0,
                terrain_type: FactionTerrain::ChitinousResin,
                strength: 1.0, spread_rate: 0.5,
                owner_faction: "insects".to_string(),
            },
            TerrainTile {
                x: 1, y: 0,
                terrain_type: FactionTerrain::Neutral,
                strength: 0.0, spread_rate: 0.0,
                owner_faction: "neutral".to_string(),
            },
            TerrainTile {
                x: 2, y: 0,
                terrain_type: FactionTerrain::HellfireCorruption,
                strength: 1.0, spread_rate: 0.5,
                owner_faction: "demons".to_string(),
            },
        ];

        let contested = terrain_check_contested(&tiles);
        // The middle tile (1,0) borders both insects and demons
        assert!(contested.contains(&(1, 0)));
        // Edge tiles only border one faction each
        assert!(!contested.contains(&(0, 0)));
        assert!(!contested.contains(&(2, 0)));
    }

    // -- Defense structures --

    #[test]
    fn test_defense_type_from_str_valid() {
        assert_eq!(DefenseType::from_str("missile_launcher"), Some(DefenseType::MissileLauncher));
        assert_eq!(DefenseType::from_str("plasma_turret"), Some(DefenseType::PlasmaTurret));
        assert_eq!(DefenseType::from_str("large_shield_dome"), Some(DefenseType::LargeShieldDome));
        assert_eq!(DefenseType::from_str("interplanetary_missile"), Some(DefenseType::InterplanetaryMissile));
    }

    #[test]
    fn test_defense_type_from_str_invalid() {
        assert_eq!(DefenseType::from_str("unknown_thing"), None);
        assert_eq!(DefenseType::from_str(""), None);
    }

    #[test]
    fn test_defense_stats_missile_launcher() {
        let stats = defense_stats(&DefenseType::MissileLauncher);
        assert_eq!(stats.hp, 200);
        assert_eq!(stats.shield, 20);
        assert_eq!(stats.damage, 80);
        assert_eq!(stats.cost_biomass, 2000);
        assert_eq!(stats.cost_minerals, 0);
        assert_eq!(stats.cost_spore_gas, 0);
    }

    #[test]
    fn test_defense_stats_plasma_turret() {
        let stats = defense_stats(&DefenseType::PlasmaTurret);
        assert_eq!(stats.hp, 10000);
        assert_eq!(stats.shield, 300);
        assert_eq!(stats.damage, 3000);
        assert_eq!(stats.cost_biomass, 50000);
        assert_eq!(stats.cost_minerals, 50000);
        assert_eq!(stats.cost_spore_gas, 30000);
    }

    #[test]
    fn test_defense_stats_shield_domes() {
        let small = defense_stats(&DefenseType::SmallShieldDome);
        assert_eq!(small.shield, 2000);
        assert_eq!(small.damage, 1);

        let large = defense_stats(&DefenseType::LargeShieldDome);
        assert_eq!(large.shield, 10000);
        assert_eq!(large.damage, 1);
    }

    #[test]
    fn test_defense_cost_single() {
        let cost = defense_cost(&DefenseType::GaussCannon, 1);
        assert_eq!(cost.biomass, 20000);
        assert_eq!(cost.minerals, 15000);
        assert_eq!(cost.spore_gas, 2000);
        assert_eq!(cost.total_units, 1);
    }

    #[test]
    fn test_defense_cost_multiple() {
        let cost = defense_cost(&DefenseType::MissileLauncher, 100);
        assert_eq!(cost.biomass, 200_000);
        assert_eq!(cost.minerals, 0);
        assert_eq!(cost.spore_gas, 0);
        assert_eq!(cost.total_units, 100);
    }

    #[test]
    fn test_defense_all_types_count() {
        assert_eq!(DefenseType::all().len(), 10);
    }

    #[test]
    fn test_defense_type_roundtrip() {
        for dt in DefenseType::all() {
            let s = dt.as_str();
            let parsed = DefenseType::from_str(s);
            assert_eq!(parsed, Some(*dt), "Roundtrip failed for {s}");
        }
    }

    #[test]
    fn test_defense_vs_fleet_simulation() {
        let defenses = vec![
            (DefenseType::PlasmaTurret, 10),
            (DefenseType::GaussCannon, 20),
            (DefenseType::LargeShieldDome, 1),
        ];
        let attacker = vec![("bio_fighter".to_string(), 5)];
        let result = defense_vs_fleet(&defenses, &attacker);
        // Heavy defenses should crush a small fighter wing
        assert!(!result.attacker_won);
        assert!(result.rounds >= 1);
    }

    #[test]
    fn test_defense_build_persistence() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let engine = SwarmCombatEngine::new(tmp.path()).expect("engine init");

        engine
            .build_defense("colony-1", &DefenseType::MissileLauncher, 50)
            .expect("build defense");

        let defenses = engine.get_colony_defenses("colony-1").expect("get defenses");
        assert_eq!(defenses.len(), 1);
        assert_eq!(defenses[0].0, "missile_launcher");
        assert_eq!(defenses[0].1, 50);

        // Build more of the same type -- should add to existing count
        engine
            .build_defense("colony-1", &DefenseType::MissileLauncher, 25)
            .expect("build more");

        let defenses = engine.get_colony_defenses("colony-1").expect("get updated");
        assert_eq!(defenses[0].1, 75); // 50 + 25
    }
