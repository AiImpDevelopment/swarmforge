    use super::*;
    use tempfile::TempDir;


    fn test_engine() -> (SwarmAdvancedEngine, TempDir) {
        let dir = TempDir::new().expect("tempdir");
        let engine = SwarmAdvancedEngine::new(dir.path()).expect("engine init");
        (engine, dir)
    }

    // -----------------------------------------------------------------------
    // Human Faction Commander Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_commander_authenticate_correct() {
        let (engine, _dir) = test_engine();
        let state = engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        assert!(state.authenticated);
        assert!(!state.npc_ai_active);
        assert!(state.standing_orders.auto_build);
        assert!(state.standing_orders.auto_defend);
    }

    #[test]
    fn test_commander_wrong_passphrase() {
        let (engine, _dir) = test_engine();
        let err = engine.commander_authenticate("wrong-password").unwrap_err();
        assert_eq!(err.code, "INVALID_PASSPHRASE");
    }

    #[test]
    fn test_commander_release_enables_npc() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        engine.commander_release().expect("commander release should succeed");
        let state = engine.commander_npc_status().expect("commander npc status should succeed");
        assert!(state.npc_ai_active);
    }

    #[test]
    fn test_commander_set_strategy() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let strat = engine.commander_set_strategy("aggressive").expect("commander set strategy should succeed");
        assert!(matches!(strat, FactionStrategy::Aggressive));
    }

    #[test]
    fn test_commander_set_strategy_invalid() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let err = engine.commander_set_strategy("nonexistent").unwrap_err();
        assert_eq!(err.code, "INVALID_STRATEGY");
    }

    #[test]
    fn test_commander_requires_auth_for_strategy() {
        let (engine, _dir) = test_engine();
        // Not authenticated — should fail
        let err = engine.commander_set_strategy("aggressive").unwrap_err();
        assert_eq!(err.code, "COMMANDER_NOT_AUTH");
    }

    #[test]
    fn test_commander_set_orders() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let mut orders = StandingOrders::default();
        orders.auto_expand = false;
        orders.min_army_per_colony = 500;
        engine.commander_set_orders(orders).expect("commander set orders should succeed");
        let state = engine.commander_npc_status().expect("commander npc status should succeed");
        assert!(!state.standing_orders.auto_expand);
        assert_eq!(state.standing_orders.min_army_per_colony, 500);
    }

    #[test]
    fn test_commander_issue_directive() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let directive = engine.commander_issue_directive(
            DirectiveType::RaidColony { target_coord: "[1:042:07]".into() }
        ).expect("test commander issue directive should succeed");
        assert_eq!(directive.status, "queued");
        assert!(!directive.id.is_empty());
    }

    #[test]
    fn test_commander_colony_status() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let status = engine.commander_colony_status("colony_1").expect("commander colony status should succeed");
        assert_eq!(status["faction"], "humans");
    }

    #[test]
    fn test_commander_war_games() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let result = engine.commander_war_games("humans", "insects", 1000).expect("commander war games should succeed");
        assert_eq!(result.total_battles, 1000);
        assert_eq!(result.faction_a_wins + result.faction_b_wins + result.draws, 1000);
    }

    #[test]
    fn test_commander_war_games_invalid_count() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let err = engine.commander_war_games("a", "b", 0).unwrap_err();
        assert_eq!(err.code, "CMD_SIM_COUNT");
    }

    #[test]
    fn test_commander_ai_decisions() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let boards = engine.commander_ai_decisions().expect("commander ai decisions should succeed");
        assert_eq!(boards.len(), 3);
    }

    #[test]
    fn test_commander_default_npc_active() {
        let state = HumanCommanderState::default();
        assert!(!state.authenticated);
        assert!(state.npc_ai_active); // NPC runs by default
    }

    #[test]
    fn test_standing_orders_default_hostile() {
        let orders = StandingOrders::default();
        assert!(orders.auto_defend);
        assert!(orders.auto_build);
        assert_eq!(orders.diplomatic_stances.get("insects"), Some(&"hostile".to_string()));
        assert_eq!(orders.diplomatic_stances.get("demons"), Some(&"hostile".to_string()));
        assert_eq!(orders.diplomatic_stances.get("undead"), Some(&"hostile".to_string()));
    }

    // -----------------------------------------------------------------------
    // Standalone Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_standalone_config_detect() {
        // Without IMPFORGE_EMBEDDED set, should be standalone
        std::env::remove_var("IMPFORGE_EMBEDDED");
        let config = StandaloneConfig::detect();
        assert_eq!(config.launch_mode, LaunchMode::Standalone);
        assert!(!config.impforge_integration);
        assert!(!config.dm_from_productivity);
        assert_eq!(config.build_type, "standalone");
    }

    #[test]
    fn test_standalone_features_standalone() {
        std::env::remove_var("IMPFORGE_EMBEDDED");
        let config = StandaloneConfig::detect();
        let features = FeatureAvailability::from_config(&config);
        assert!(!features.dark_matter_from_productivity);
        assert!(!features.office_suite_integration);
        assert!(features.standalone_game);
        assert!(features.multiplayer);
        assert!(features.achievements);
        assert!(!features.cloud_save);
    }

    #[test]
    fn test_standalone_features_embedded() {
        std::env::set_var("IMPFORGE_EMBEDDED", "1");
        let config = StandaloneConfig::detect();
        let features = FeatureAvailability::from_config(&config);
        assert!(features.dark_matter_from_productivity);
        assert!(features.office_suite_integration);
        assert!(features.ide_integration);
        assert!(features.workflow_integration);
        assert!(features.standalone_game);
        // Clean up
        std::env::remove_var("IMPFORGE_EMBEDDED");
    }

    // -----------------------------------------------------------------------
    // Fleet Save Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_fleet_save_success() {
        let (engine, _dir) = test_engine();
        let save = engine
            .fleet_save("fleet_1", "[1:100:5]", FleetSavePurpose::AvoidAttack, 42)
            .expect("fleet save should succeed");
        assert_eq!(save.fleet_id, "fleet_1");
        assert_eq!(save.destination, "[1:100:5]");
        assert_eq!(save.purpose, FleetSavePurpose::AvoidAttack);
        assert_eq!(save.ship_count, 42);
    }

    #[test]
    fn test_fleet_save_empty_id() {
        let (engine, _dir) = test_engine();
        let err = engine
            .fleet_save("", "[1:1:1]", FleetSavePurpose::AvoidAttack, 1)
            .unwrap_err();
        assert_eq!(err.code, "FLEET_SAVE_EMPTY_ID");
    }

    #[test]
    fn test_fleet_save_empty_destination() {
        let (engine, _dir) = test_engine();
        let err = engine
            .fleet_save("fleet_1", "", FleetSavePurpose::Deployment, 1)
            .unwrap_err();
        assert_eq!(err.code, "FLEET_SAVE_EMPTY_DEST");
    }

    // -----------------------------------------------------------------------
    // Vacation Mode Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_vacation_mode_activate() {
        let (engine, _dir) = test_engine();
        let mode = engine.vacation_mode_toggle("colony_1", true).expect("vacation mode toggle should succeed");
        assert!(mode.active);
        assert!(mode.production_paused);
        assert!(mode.attack_immune);
        assert!(mode.cant_attack);
        assert!(mode.started_at.is_some());
    }

    #[test]
    fn test_vacation_mode_deactivate() {
        let (engine, _dir) = test_engine();
        engine.vacation_mode_toggle("colony_1", true).expect("vacation mode toggle should succeed");
        let mode = engine.vacation_mode_toggle("colony_1", false).expect("vacation mode toggle should succeed");
        assert!(!mode.active);
        assert!(!mode.production_paused);
    }

    #[test]
    fn test_vacation_mode_empty_colony() {
        let (engine, _dir) = test_engine();
        let err = engine.vacation_mode_toggle("", true).unwrap_err();
        assert_eq!(err.code, "VACATION_EMPTY_COLONY");
    }

    // -----------------------------------------------------------------------
    // Noob Protection Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_noob_protection_defender_weak() {
        let (engine, _dir) = test_engine();
        let protection = engine.check_noob_protection(100_000, 10_000).expect("check noob protection should succeed");
        assert!(protection.protected); // defender below threshold
    }

    #[test]
    fn test_noob_protection_power_range() {
        let (engine, _dir) = test_engine();
        // Both above threshold, but attacker is 10x stronger (>5x range)
        let protection = engine.check_noob_protection(500_000, 50_001).expect("check noob protection should succeed");
        assert!(protection.protected); // out of 5x range
    }

    #[test]
    fn test_noob_protection_fair_fight() {
        let (engine, _dir) = test_engine();
        // Both above threshold, within 5x range
        let protection = engine.check_noob_protection(100_000, 60_000).expect("check noob protection should succeed");
        assert!(!protection.protected);
    }

    // -----------------------------------------------------------------------
    // Debris Field Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_create_debris_field() {
        let (engine, _dir) = test_engine();
        let debris = engine.create_debris_field("[1:100:5]", 100_000.0, 50_000.0).expect("create debris field should succeed");
        assert_eq!(debris.coord, "[1:100:5]");
        // 30% of destroyed costs
        assert!((debris.resources_primary - 30_000.0).abs() < 0.01);
        assert!((debris.resources_secondary - 15_000.0).abs() < 0.01);
        assert_eq!(debris.expires_hours, 24);
    }

    #[test]
    fn test_create_debris_field_empty_coord() {
        let (engine, _dir) = test_engine();
        let err = engine.create_debris_field("", 100.0, 50.0).unwrap_err();
        assert_eq!(err.code, "DEBRIS_EMPTY_COORD");
    }

    #[test]
    fn test_collect_debris_full() {
        let (engine, _dir) = test_engine();
        let debris = engine.create_debris_field("[1:1:1]", 10_000.0, 5_000.0).expect("create debris field should succeed");
        // 10 recyclers * 20,000 capacity = 200,000 (more than enough)
        let collected = engine.collect_debris("colony_1", &debris.id, 10).expect("collect debris should succeed");
        assert!((collected.primary_collected - 3_000.0).abs() < 0.01); // 30% of 10k
        assert!((collected.secondary_collected - 1_500.0).abs() < 0.01); // 30% of 5k
    }

    #[test]
    fn test_collect_debris_partial() {
        let (engine, _dir) = test_engine();
        let debris = engine.create_debris_field("[2:2:2]", 1_000_000.0, 500_000.0).expect("create debris field should succeed");
        // 1 recycler * 20,000 capacity vs 300k+150k=450k debris
        let collected = engine.collect_debris("colony_1", &debris.id, 1).expect("collect debris should succeed");
        let total_debris = 300_000.0 + 150_000.0;
        let ratio = 20_000.0 / total_debris;
        assert!((collected.primary_collected - 300_000.0 * ratio).abs() < 1.0);
    }

    #[test]
    fn test_collect_debris_no_recyclers() {
        let (engine, _dir) = test_engine();
        let debris = engine.create_debris_field("[3:3:3]", 100.0, 50.0).expect("create debris field should succeed");
        let err = engine.collect_debris("colony_1", &debris.id, 0).unwrap_err();
        assert_eq!(err.code, "DEBRIS_NO_RECYCLERS");
    }

    // -----------------------------------------------------------------------
    // Moon Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_moon_status_none() {
        let (engine, _dir) = test_engine();
        let moon = engine.moon_status("[1:1:1]").expect("moon status should succeed");
        assert!(moon.is_none());
    }

    #[test]
    fn test_moon_creation_empty_coord() {
        let (engine, _dir) = test_engine();
        let err = engine.check_moon_creation("", 100_000.0).unwrap_err();
        assert_eq!(err.code, "MOON_EMPTY_COORD");
    }

    // -----------------------------------------------------------------------
    // Phalanx Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_phalanx_scan_no_level() {
        let (engine, _dir) = test_engine();
        let err = engine.phalanx_scan("[1:1:1]", "[1:2:1]", 0).unwrap_err();
        assert_eq!(err.code, "PHALANX_NO_SENSOR");
    }

    // -----------------------------------------------------------------------
    // Jump Gate Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_jump_gate_same_moon() {
        let (engine, _dir) = test_engine();
        let err = engine.jump_gate_transfer("[1:1:1]", "[1:1:1]", 10).unwrap_err();
        assert_eq!(err.code, "JUMP_SAME_MOON");
    }

    // -----------------------------------------------------------------------
    // Default state tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_commander_default_state() {
        let state = HumanCommanderState::default();
        assert!(!state.authenticated);
        assert!(state.npc_ai_active);
        assert!(state.colonies.is_empty());
    }

    #[test]
    fn test_vacation_mode_default() {
        let mode = VacationMode::default();
        assert!(!mode.active);
        assert_eq!(mode.min_duration_hours, 48);
        assert_eq!(mode.max_duration_days, 30);
        assert!(!mode.production_paused);
    }

    #[test]
    fn test_noob_protection_default() {
        let prot = NoobProtection::default();
        assert!(prot.protected);
        assert_eq!(prot.power_threshold, 50_000);
        assert!((prot.attack_range - 5.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Kill Rate Limit Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_kill_rate_under_limit_allowed() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        // No attacks recorded yet -- kill rate is 0%, well under 20%
        let allowed = engine.commander_check_kill_rate(5000).expect("commander check kill rate should succeed");
        assert!(allowed);
    }

    #[test]
    fn test_kill_rate_over_limit_blocked() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        // Record a single attack that killed 100 units (100% kill rate)
        engine.commander_record_attack("colony_alpha", 100).expect("commander record attack should succeed");
        // Now check -- 100/1 = 100% > 20%, should be blocked
        let allowed = engine.commander_check_kill_rate(5000).expect("commander check kill rate should succeed");
        assert!(!allowed);
    }

    #[test]
    fn test_kill_rate_weak_target_penalty() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        // Record many attacks with low kills to get rate near 20%
        for _ in 0..5 {
            engine.commander_record_attack("colony_beta", 1).expect("commander record attack should succeed");
        }
        // kill rate = 5/5 = 1.0 which is way over -- but lets test the
        // weak-target penalty is added on top
        let allowed = engine.commander_check_kill_rate(500).expect("commander check kill rate should succeed");
        // rate(1.0) + weakness(0.05) = 1.05 > 0.20 -- blocked
        assert!(!allowed);
    }

    // -----------------------------------------------------------------------
    // Alertness Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_alertness_increases_on_attack() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let before = engine.commander_alertness().expect("commander alertness should succeed");
        assert!((before - 0.0).abs() < f64::EPSILON);

        engine.commander_record_attack("settlement_1", 10).expect("commander record attack should succeed");
        let after = engine.commander_alertness().expect("commander alertness should succeed");
        // Default increase_per_attack = 0.10
        assert!((after - 0.10).abs() < f64::EPSILON);
    }

    #[test]
    fn test_alertness_repeated_attack_multiplier() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        // First attack on settlement_x: +0.10 * 1.0 = 0.10
        engine.commander_record_attack("settlement_x", 5).expect("commander record attack should succeed");
        // Second attack on same settlement: +0.10 * 1.5 = 0.15
        engine.commander_record_attack("settlement_x", 5).expect("commander record attack should succeed");
        let level = engine.commander_alertness().expect("commander alertness should succeed");
        assert!((level - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_alertness_decay() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        // Raise alertness
        engine.commander_record_attack("settlement_z", 10).expect("commander record attack should succeed");
        let raised = engine.commander_alertness().expect("commander alertness should succeed");
        assert!((raised - 0.10).abs() < f64::EPSILON);

        // Decay 1 hour: 0.10 - (0.05 * 1.0) = 0.05
        let after_decay = engine.commander_decay_alertness(1.0).expect("commander decay alertness should succeed");
        assert!((after_decay - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_alertness_does_not_go_negative() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        // Decay when alertness is 0 -- should stay at 0
        let level = engine.commander_decay_alertness(100.0).expect("commander decay alertness should succeed");
        assert!((level - 0.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Trade Evaluation Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_trade_minimum_accepted() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let offer = engine.commander_evaluate_trade("player_1", 500.0, 100.0).expect("commander evaluate trade should succeed");
        assert!(offer.accepted);
        assert_eq!(offer.protection_hours, 24); // 500 / (100*5) * 24 = 24
        assert_eq!(offer.player_id, "player_1");
    }

    #[test]
    fn test_trade_maximum_capped() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let offer = engine.commander_evaluate_trade("player_2", 50_000.0, 100.0).expect("commander evaluate trade should succeed");
        assert!(offer.accepted);
        assert_eq!(offer.protection_hours, 168); // capped at 1 week
    }

    #[test]
    fn test_trade_below_minimum_rejected() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let offer = engine.commander_evaluate_trade("player_3", 200.0, 100.0).expect("commander evaluate trade should succeed");
        assert!(!offer.accepted);
        assert_eq!(offer.protection_hours, 0);
    }

    #[test]
    fn test_trade_middle_value() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        // 2500 resources, 100/hr production: 2500 / (100*5) * 24 = 120 hours
        let offer = engine.commander_evaluate_trade("player_4", 2_500.0, 100.0).expect("commander evaluate trade should succeed");
        assert!(offer.accepted);
        assert_eq!(offer.protection_hours, 120);
    }

    #[test]
    fn test_trade_empty_player_rejected() {
        let (engine, _dir) = test_engine();
        engine.commander_authenticate(COMMANDER_PASSPHRASE).expect("commander authenticate should succeed");
        let err = engine.commander_evaluate_trade("", 50_000.0, 100.0).unwrap_err();
        assert_eq!(err.code, "TRADE_EMPTY_PLAYER");
    }

    // -----------------------------------------------------------------------
    // Auto-Play APM Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_auto_play_config_defaults() {
        let (engine, _dir) = test_engine();
        let config = engine.commander_auto_play_config().expect("commander auto play config should succeed");
        assert_eq!(config.max_actions_per_5s, 22);
        assert_eq!(config.sustained_apm, 250);
        assert_eq!(config.burst_apm, 500);
        assert_eq!(config.reaction_delay_ms, 200);
        assert!((config.offline_efficiency - 0.8).abs() < f64::EPSILON);
        assert!((config.subscription_bonus - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_auto_play_tick_respects_apm_limit() {
        let (engine, _dir) = test_engine();
        // Auto-play does not require auth (runs when developer is offline)
        let actions = engine.commander_auto_play_tick(5.0).expect("commander auto play tick should succeed");
        // 250 APM = ~4.17 per second, 5s = ~21 raw actions
        // Window cap: 22 per 5s, scaled by window_ratio = 1.0 => max 22
        // Effective: min(21, 22) * 0.8 * 1.0 = ~17
        assert!(!actions.is_empty());
        assert!(actions.len() <= 22); // Never exceeds AlphaStar limit
    }

    #[test]
    fn test_auto_play_tick_zero_delta() {
        let (engine, _dir) = test_engine();
        let actions = engine.commander_auto_play_tick(0.0).expect("commander auto play tick should succeed");
        assert!(actions.is_empty());
    }

    #[test]
    fn test_auto_play_tick_small_delta() {
        let (engine, _dir) = test_engine();
        // Very small tick -- should still produce at least 1 action
        let actions = engine.commander_auto_play_tick(0.01).expect("commander auto play tick should succeed");
        assert!(!actions.is_empty());
        // But should be very few actions
        assert!(actions.len() <= 5);
    }

    // -----------------------------------------------------------------------
    // Display Mode Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_display_mode_parsing() {
        assert_eq!(DisplayMode::from_str_lossy("fullscreen"), DisplayMode::Fullscreen);
        assert_eq!(DisplayMode::from_str_lossy("companion_window"), DisplayMode::CompanionWindow);
        assert_eq!(DisplayMode::from_str_lossy("companion"), DisplayMode::CompanionWindow);
        assert_eq!(DisplayMode::from_str_lossy("sidebar"), DisplayMode::Sidebar);
        assert_eq!(DisplayMode::from_str_lossy("deactivated"), DisplayMode::Deactivated);
        assert_eq!(DisplayMode::from_str_lossy("hidden"), DisplayMode::Deactivated);
        assert_eq!(DisplayMode::from_str_lossy("off"), DisplayMode::Deactivated);
    }

    #[test]
    fn test_display_mode_unknown_defaults_to_companion() {
        assert_eq!(DisplayMode::from_str_lossy("nonsense"), DisplayMode::CompanionWindow);
        assert_eq!(DisplayMode::from_str_lossy(""), DisplayMode::CompanionWindow);
    }

    #[test]
    fn test_display_mode_via_engine() {
        let (engine, _dir) = test_engine();
        let mode = engine.swarmforge_set_display_mode("sidebar");
        assert_eq!(mode, DisplayMode::Sidebar);
    }

    // -----------------------------------------------------------------------
    // Default state includes new fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_commander_default_has_balance_fields() {
        let state = HumanCommanderState::default();
        assert!((state.kill_rate_limit.max_kill_rate - 0.20).abs() < f64::EPSILON);
        assert!(!state.kill_rate_limit.is_limited);
        assert!((state.alertness.level - 0.0).abs() < f64::EPSILON);
        assert!((state.alertness.decay_per_hour - 0.05).abs() < f64::EPSILON);
        assert_eq!(state.auto_play.max_actions_per_5s, 22);
    }
