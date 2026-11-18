use super::*;

fn wall() -> ArenaCollider {
    ArenaCollider {
        center: Vec3::ZERO,
        half_extents: Vec3::new(0.25, 1.5, 3.0),
    }
}

#[test]
fn blocks_positions_inside_or_close_to_a_wall() {
    let collider = wall();

    assert!(is_blocked(
        Vec3::new(0.0, 1.65, 0.0),
        std::iter::once(collider)
    ));
    assert!(is_blocked(
        Vec3::new(-0.5, 1.65, 0.0),
        std::iter::once(collider)
    ));
    assert!(!is_blocked(
        Vec3::new(-0.7, 1.65, 0.0),
        std::iter::once(collider)
    ));
}

#[test]
fn footstep_cadence_matches_player_movement_speed() {
    let mut signature = PlayerSignature::default();
    assert_eq!(footstep_interval(&signature), None);

    signature.moving = true;
    let walking_interval = footstep_interval(&signature).unwrap();
    signature.sprinting = true;
    let sprinting_interval = footstep_interval(&signature).unwrap();
    signature.sprinting = false;
    signature.crouching = true;
    let crouching_interval = footstep_interval(&signature).unwrap();

    assert!(sprinting_interval < walking_interval);
    assert!(walking_interval < crouching_interval);
}

#[test]
fn footsteps_cycle_through_recorded_variants() {
    let first = footstep_audio_path(0);
    assert_ne!(first, footstep_audio_path(1));
    assert_eq!(first, footstep_audio_path(AUDIO_FOOTSTEPS.len()));
}

#[test]
fn asset_root_resolves_sound_files_from_the_project_layout() {
    let asset_root = resolve_asset_root().unwrap();

    assert!(asset_root.join(AUDIO_PLAYER_GUNSHOT).is_file());
    assert!(asset_root.join(AUDIO_ENEMY_GUNSHOT).is_file());
    assert!(asset_root.join(AUDIO_AMBIENCE).is_file());
    assert!(asset_root.join(AUDIO_IMPACT_FLESH).is_file());
    assert!(asset_root.join(AUDIO_IMPACT_ARMOR).is_file());
    assert!(AUDIO_FOOTSTEPS
        .iter()
        .all(|path| asset_root.join(path).is_file()));
}

#[test]
fn greybox_map_has_two_clear_routes_and_accessible_extractions() {
    let obstacles = MAP_COVER_SPECS
        .into_iter()
        .map(|(center, size)| ArenaCollider {
            center,
            half_extents: size / 2.0,
        })
        .collect::<Vec<_>>();
    let west_route = [
        Vec3::new(0.0, 1.65, 5.0),
        Vec3::new(-3.0, 1.65, 5.0),
        Vec3::new(-8.0, 1.65, 3.0),
        Vec3::new(-8.0, 1.65, -5.0),
        Vec3::new(-9.0, 1.65, -9.0),
    ];
    let east_route = [
        Vec3::new(0.0, 1.65, 5.0),
        Vec3::new(3.0, 1.65, 5.0),
        Vec3::new(8.0, 1.65, 3.0),
        Vec3::new(8.0, 1.65, -5.0),
        Vec3::new(9.0, 1.65, -9.0),
    ];
    let route_is_clear = |route: &[Vec3]| {
        route.windows(2).all(|pair| {
            let delta = pair[1] - pair[0];
            let steps = (delta.length() / 0.2).ceil() as usize;
            (0..=steps).all(|step| {
                let position = pair[0].lerp(pair[1], step as f32 / steps as f32);
                !is_blocked(position, obstacles.iter().copied())
            })
        })
    };

    assert!(is_blocked(
        Vec3::new(0.0, 1.65, -1.0),
        obstacles.iter().copied()
    ));
    assert!(route_is_clear(&west_route));
    assert!(route_is_clear(&east_route));
    assert_eq!(EXTRACTION_POSITIONS.len(), 2);
    assert_ne!(EXTRACTION_POSITIONS[0], EXTRACTION_POSITIONS[1]);

    for x in [-8.0, 8.0] {
        for point in patrol_route(x, x < 0.0) {
            assert!(
                !is_blocked(point, obstacles.iter().copied()),
                "patrol point {point:?} intersects cover"
            );
        }
    }
}

#[test]
fn raycast_ignores_targets_behind_obstacles() {
    let origin = Vec3::new(0.0, 1.0, 5.0);
    let direction = Vec3::NEG_Z;
    let target = ArenaCollider {
        center: Vec3::new(0.0, 1.0, -2.0),
        half_extents: Vec3::splat(0.5),
    };
    let wall = ArenaCollider {
        center: Vec3::new(0.0, 1.0, 1.0),
        half_extents: Vec3::new(1.0, 1.0, 0.1),
    };

    assert_eq!(
        closest_target(
            origin,
            direction,
            [(1_u8, target, true), (2_u8, wall, false)].into_iter()
        ),
        None
    );
    assert_eq!(
        closest_target(origin, direction, [(1_u8, target, true)].into_iter()),
        Some(1)
    );
}

#[test]
fn weapon_spread_is_reproducible_and_stays_near_the_aim_direction() {
    let forward = Vec3::NEG_Z;
    let spread = 0.04;
    let mut first_state = 12345;
    let mut second_state = 12345;
    let first = spread_direction(forward, spread, &mut first_state);
    let second = spread_direction(forward, spread, &mut second_state);

    assert_eq!(first, second);
    assert!(forward.dot(first) > (spread + 0.001).cos());
    assert_ne!(first, forward);
}

#[test]
fn enemy_vision_requires_facing_and_clear_sight() {
    let enemy_position = Vec3::ZERO;
    let player_position = Vec3::new(0.0, 1.65, -5.0);
    let wall = ArenaCollider {
        center: Vec3::new(0.0, 1.0, -2.5),
        half_extents: Vec3::new(1.0, 1.0, 0.2),
    };

    assert!(player_in_vision(
        enemy_position,
        Vec3::NEG_Z,
        player_position,
        false,
        std::iter::empty()
    ));
    assert!(!player_in_vision(
        enemy_position,
        Vec3::Z,
        player_position,
        false,
        std::iter::empty()
    ));
    assert!(!player_in_vision(
        enemy_position,
        Vec3::NEG_Z,
        player_position,
        false,
        std::iter::once(wall)
    ));
}

#[test]
fn bot_shots_can_miss_and_are_blocked_by_cover() {
    let origin = Vec3::new(0.0, 1.35, 5.0);
    let player_position = Vec3::new(0.0, 1.65, 0.0);
    let cover = ArenaCollider {
        center: Vec3::new(0.0, 1.0, 2.5),
        half_extents: Vec3::new(1.0, 1.0, 0.2),
    };
    let mut blocked_shot_seed = 42;

    let mut hits = 0;
    for seed in 1_u64..=32 {
        let mut random_state = seed;
        if bot_shot_hits(
            Vec3::new(0.0, 1.35, 9.0),
            player_position,
            9.0,
            true,
            true,
            std::iter::empty(),
            &mut random_state,
        ) {
            hits += 1;
        }
    }
    assert!(hits > 0 && hits < 32);
    assert!(!bot_shot_hits(
        origin,
        player_position,
        5.0,
        false,
        false,
        std::iter::once(cover),
        &mut blocked_shot_seed
    ));

    let mut stationary_hits = 0;
    let mut moving_hits = 0;
    for seed in 1_u64..=128 {
        let mut stationary_state = seed;
        stationary_hits += u32::from(bot_shot_hits(
            Vec3::new(0.0, 1.35, 6.0),
            Vec3::new(0.0, 1.65, 0.0),
            6.0,
            false,
            false,
            std::iter::empty(),
            &mut stationary_state,
        ));

        let mut moving_state = seed;
        moving_hits += u32::from(bot_shot_hits(
            Vec3::new(0.0, 1.35, 6.0),
            Vec3::new(0.0, 1.65, 0.0),
            6.0,
            true,
            false,
            std::iter::empty(),
            &mut moving_state,
        ));
    }
    assert!(
        stationary_hits > 64 && stationary_hits < 128 && moving_hits > 0,
        "stationary hit count {stationary_hits}, moving hit count {moving_hits}"
    );
    assert!(moving_hits < stationary_hits);
}

#[test]
fn bots_detect_sprinting_faster_than_crouching() {
    let distance = 6.0;

    assert!(detection_rate(distance, true, false) > detection_rate(distance, false, false));
    assert!(detection_rate(distance, false, false) > detection_rate(distance, false, true));
    assert!(detection_rate(6.0, false, false) < 1.0);
}

#[test]
fn reload_transfers_only_the_ammo_needed_and_reports_empty_reserve() {
    assert_eq!(
        reload_magazine(4, 20, 12),
        (12, 12, ReloadOutcome::Loaded(8))
    );
    assert_eq!(
        reload_magazine(12, 20, 12),
        (12, 20, ReloadOutcome::MagazineFull)
    );
    assert_eq!(
        reload_magazine(4, 0, 12),
        (4, 0, ReloadOutcome::NoReserveAmmo)
    );
}

#[test]
fn damage_feedback_names_the_hit_zone_and_severity() {
    let mut health = HealthState::standard();
    let hit = apply_hit(
        &mut health,
        None,
        BodyPart::Thorax,
        20,
        DamageType::Ballistic,
    )
    .unwrap();

    assert_eq!(damage_feedback(hit), "WOUNDED: THORAX | -20 HP");

    let mut health = HealthState::standard();
    let mut armor = Some(ArmorState::with_durability(starter_armor_definition(), 100).unwrap());
    let armored_hit = apply_hit(
        &mut health,
        armor.as_mut(),
        BodyPart::Thorax,
        20,
        DamageType::Ballistic,
    )
    .unwrap();
    let feedback = damage_feedback(armored_hit);
    assert!(feedback.contains("armor absorbed"));
    assert!(feedback.contains("durability)"));
}

#[test]
fn firing_consumes_rounds_and_reload_uses_reserve_ammo() {
    let definition = starter_weapon_definition();
    let mut app = App::new();
    app.insert_resource(Time::<()>::default())
        .insert_resource(ButtonInput::<MouseButton>::default())
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(WeaponState {
            definition,
            magazine: 2,
            reserve: 3,
            random_state: 1,
        })
        .insert_resource(WeaponRecoil::default())
        .insert_resource(PlayerSignature::default())
        .insert_resource(CombatStats {
            enemies_killed: 0,
            total_enemies: 1,
        })
        .insert_resource(CombatFeedback::default())
        .insert_resource(InventoryState::default())
        .insert_resource({
            let mut raid = RaidState::default();
            raid.phase = RaidPhase::Active;
            raid
        })
        .add_systems(Update, fire_weapon);
    let mut window = Window::default();
    window.cursor_options.grab_mode = CursorGrabMode::Locked;
    app.world_mut().spawn(window);
    app.world_mut().spawn((
        Transform::from_xyz(0.0, 1.65, 5.0),
        PlayerCamera {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));
    let enemy = app
        .world_mut()
        .spawn((
            Transform::from_xyz(0.0, 0.9, 0.0),
            ArenaCollider {
                center: Vec3::new(0.0, 0.9, 0.0),
                half_extents: Vec3::new(0.4, 0.9, 0.3),
            },
            Enemy {
                health: 50,
                attack_timer: Timer::from_seconds(ENEMY_ATTACK_INTERVAL, TimerMode::Repeating),
                detection: 0.0,
                last_known_player: None,
                memory_remaining: 0.0,
                shot_random_state: 1,
                home_position: Vec3::ZERO,
                patrol_points: Vec::new(),
                patrol_index: 0,
            },
        ))
        .id();

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();

    assert_eq!(app.world().resource::<WeaponState>().magazine, 1);
    assert_eq!(app.world().get::<Enemy>(enemy).unwrap().health, 10);
    assert_eq!(
        app.world().resource::<CombatFeedback>().message,
        "TARGET HIT: 10 HP"
    );

    app.world_mut()
        .insert_resource(ButtonInput::<MouseButton>::default());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();

    assert_eq!(app.world().resource::<WeaponState>().magazine, 4);
    assert_eq!(app.world().resource::<WeaponState>().reserve, 0);
    assert_eq!(
        app.world().resource::<CombatFeedback>().message,
        "RELOADED: +3 ROUNDS"
    );

    app.world_mut()
        .insert_resource(ButtonInput::<MouseButton>::default());
    app.world_mut()
        .insert_resource(ButtonInput::<KeyCode>::default());
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();

    assert_eq!(app.world().get::<Enemy>(enemy).unwrap().health, 0);
    assert_eq!(app.world().resource::<CombatStats>().enemies_killed, 1);
    assert_eq!(
        app.world().resource::<CombatFeedback>().message,
        "TARGET ELIMINATED"
    );

    app.world_mut().resource_mut::<WeaponState>().magazine = 0;
    app.world_mut()
        .insert_resource(ButtonInput::<MouseButton>::default());
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert_eq!(
        app.world().resource::<CombatFeedback>().message,
        "EMPTY MAGAZINE: PRESS R TO RELOAD"
    );

    app.world_mut()
        .insert_resource(ButtonInput::<MouseButton>::default());
    app.world_mut()
        .insert_resource(ButtonInput::<KeyCode>::default());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    assert_eq!(
        app.world().resource::<CombatFeedback>().message,
        "NO SPARE AMMO"
    );
}

#[test]
fn line_of_sight_is_blocked_by_arena_colliders() {
    let obstacle = ArenaCollider {
        center: Vec3::new(0.0, 1.0, 2.0),
        half_extents: Vec3::splat(0.5),
    };

    assert!(!has_line_of_sight(
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 1.0, 5.0),
        std::iter::once(obstacle)
    ));
    assert!(has_line_of_sight(
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 1.0, 5.0),
        std::iter::empty()
    ));
}

#[test]
fn enemy_shots_hit_the_player_through_cover_only_when_ray_clears_it() {
    let cover = ArenaCollider {
        center: Vec3::new(0.0, 0.6, 2.5),
        half_extents: Vec3::new(1.0, 0.6, 0.5),
    };
    let enemy_position = Vec3::new(0.0, 0.9, 0.0);
    let player_position = Vec3::new(0.0, 1.65, 5.0);

    assert!(has_line_of_sight(
        enemy_position + Vec3::Y * 1.35,
        player_position,
        std::iter::once(cover)
    ));
    assert!(!has_line_of_sight(
        enemy_position + Vec3::Y * ENEMY_MUZZLE_HEIGHT,
        player_position - Vec3::Y * PLAYER_AIM_HEIGHT,
        std::iter::once(cover)
    ));
}

#[test]
fn damage_reduces_health_and_stops_at_zero() {
    let mut health = 35;
    assert!(!apply_damage(&mut health, 10));
    assert_eq!(health, 25);
    assert!(apply_damage(&mut health, 30));
    assert_eq!(health, 0);
}

#[test]
fn hit_rolls_map_to_every_body_zone() {
    assert_eq!(body_part_from_roll(0.02), BodyPart::Head);
    assert_eq!(body_part_from_roll(0.2), BodyPart::Thorax);
    assert_eq!(body_part_from_roll(0.6), BodyPart::Abdomen);
    assert_eq!(body_part_from_roll(0.72), BodyPart::LeftArm);
    assert_eq!(body_part_from_roll(0.82), BodyPart::RightArm);
    assert_eq!(body_part_from_roll(0.9), BodyPart::LeftLeg);
    assert_eq!(body_part_from_roll(0.97), BodyPart::RightLeg);
}

#[test]
fn fatal_wound_reason_matches_the_destroyed_vital_zone() {
    let mut health = HealthState::standard();
    health.apply_damage(BodyPart::Thorax, u32::MAX);
    assert_eq!(fatal_wound_label(&health), "THORAX DESTROYED");

    health.apply_damage(BodyPart::Head, u32::MAX);
    assert_eq!(fatal_wound_label(&health), "HEAD DESTROYED");
}

#[test]
fn enemy_ai_system_initializes_without_query_conflicts() {
    let mut app = App::new();
    let mut raid = RaidState::default();
    raid.phase = RaidPhase::Active;
    app.insert_resource(Time::<()>::default())
        .insert_resource(InventoryState::default())
        .insert_resource(raid)
        .insert_resource(PlayerVitals::default())
        .insert_resource(PlayerSignature::default())
        .insert_resource(CombatFeedback::default())
        .add_systems(Update, update_enemies);
    app.world_mut().spawn((
        Transform::from_xyz(0.0, 1.65, 5.0),
        PlayerCamera {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));
    app.world_mut().spawn((
        Transform::from_xyz(-6.0, 0.9, -6.0),
        ArenaCollider {
            center: Vec3::new(-6.0, 0.9, -6.0),
            half_extents: Vec3::new(0.4, 0.9, 0.3),
        },
        Enemy {
            health: ENEMY_HEALTH,
            attack_timer: Timer::from_seconds(ENEMY_ATTACK_INTERVAL, TimerMode::Repeating),
            detection: 0.0,
            last_known_player: None,
            memory_remaining: 0.0,
            shot_random_state: 1,
            home_position: Vec3::new(-6.0, 0.9, -6.0),
            patrol_points: Vec::new(),
            patrol_index: 0,
        },
    ));

    app.update();
}

#[test]
fn gunfire_alerts_enemy_and_hud_reports_detection() {
    let mut raid = RaidState::default();
    raid.phase = RaidPhase::Active;
    let mut signature = PlayerSignature::default();
    signature.gunshot_remaining = 0.5;
    let mut app = App::new();
    app.insert_resource(Time::<()>::default())
        .insert_resource(InventoryState::default())
        .insert_resource(raid)
        .insert_resource(PlayerVitals::default())
        .insert_resource(signature)
        .insert_resource(CombatFeedback::default())
        .add_systems(Update, update_enemies);
    app.world_mut().spawn((
        Transform::from_xyz(0.0, 1.65, 5.0),
        PlayerCamera {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));
    app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.9, 0.0),
        ArenaCollider {
            center: Vec3::new(0.0, 0.9, 0.0),
            half_extents: Vec3::new(0.4, 0.9, 0.3),
        },
        Enemy {
            health: ENEMY_HEALTH,
            attack_timer: Timer::from_seconds(ENEMY_ATTACK_INTERVAL, TimerMode::Repeating),
            detection: 0.0,
            last_known_player: None,
            memory_remaining: 0.0,
            shot_random_state: 1,
            home_position: Vec3::new(0.0, 0.9, 0.0),
            patrol_points: Vec::new(),
            patrol_index: 0,
        },
    ));

    app.update();

    assert_eq!(
        app.world().resource::<CombatFeedback>().message,
        "ENEMY ALERTED BY GUNFIRE"
    );
}

#[test]
fn hud_reports_ammo_and_nearby_extraction_without_enemy_count() {
    let mut raid = RaidState::default();
    raid.phase = RaidPhase::Active;
    let mut app = App::new();
    let weapon = starter_weapon_definition();
    app.insert_resource(WeaponState {
        definition: weapon,
        magazine: 5,
        reserve: 17,
        random_state: 1,
    })
    .insert_resource(PlayerVitals::default())
    .insert_resource(raid)
    .insert_resource(InventoryState::default())
    .insert_resource(CombatFeedback::default())
    .add_systems(Update, update_hud);
    app.world_mut()
        .spawn((Text::new(""), Visibility::Hidden, HudStats));
    app.world_mut().spawn((
        Transform::from_translation(Vec3::new(-9.0, 1.65, -9.0)),
        PlayerCamera {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));
    app.world_mut().spawn((
        Transform::from_translation(EXTRACTION_POSITIONS[0]),
        ExtractionPoint {
            radius: 2.2,
            label: "WEST EXIT",
        },
    ));

    app.update();

    let mut hud = app.world_mut().query_filtered::<&Text, With<HudStats>>();
    let text = &hud.single(app.world()).0;
    assert!(text.contains("AMMO 5 | RESERVE 17"));
    assert!(text.contains("EXTRACTION AVAILABLE: WEST EXIT [F]"));
    assert!(!text.contains("ENEMIES"));
}

#[test]
fn shelter_hud_shows_controls_while_roaming() {
    let weapon = starter_weapon_definition();
    let mut app = App::new();
    app.insert_resource(WeaponState {
        definition: weapon,
        magazine: 5,
        reserve: 17,
        random_state: 1,
    })
    .insert_resource(PlayerVitals::default())
    .insert_resource(RaidState::default())
    .insert_resource(InventoryState::default())
    .insert_resource(CombatFeedback::default())
    .add_systems(Update, update_hud);
    app.world_mut()
        .spawn((Text::new(""), Visibility::Hidden, HudStats));

    app.update();

    let world = app.world_mut();
    let mut hud = world.query_filtered::<(&Text, &Visibility), With<HudStats>>();
    let (text, visibility) = hud.single(world);
    assert_eq!(*visibility, Visibility::Visible);
    assert!(text.0.contains("WASD: MOVE"));
    assert!(text.0.contains("ESC: MENU"));
}

#[test]
fn shelter_movement_is_enabled_and_stays_within_the_house() {
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_millis(100));
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::KeyW);
    let mut app = App::new();
    app.insert_resource(time)
        .insert_resource(keys)
        .insert_resource(InventoryState::default())
        .insert_resource(RaidState::default())
        .insert_resource(PlayerVitals::default())
        .insert_resource(PlayerSignature::default())
        .add_systems(Update, move_player);
    app.world_mut().spawn((
        Transform::from_translation(SHELTER_BED_SPAWN),
        PlayerCamera {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));

    app.update();

    let world = app.world_mut();
    let mut player = world.query_filtered::<&Transform, With<PlayerCamera>>();
    assert!(player.single(world).translation.z < SHELTER_BED_SPAWN.z);

    let blocked_side_exit = constrain_shelter_position(Vec3::new(4.0, 1.65, 38.0));
    assert!(blocked_side_exit.z >= SHELTER_CENTER.z - 4.4);
    let doorway_exit = constrain_shelter_position(Vec3::new(0.0, 1.65, 35.0));
    assert!(doorway_exit.z >= SHELTER_CENTER.z - 7.2);
    assert!(doorway_exit.x.abs() <= 0.75);
}

#[test]
fn inventory_move_accepts_valid_slots_and_rejects_overlaps() {
    let layout = InventoryState::default().layout;

    let moved = try_move_inventory_item(&layout, "medkit", 8, 4).unwrap();
    assert_eq!(
        moved
            .items
            .iter()
            .find(|item| item.id == "medkit")
            .unwrap()
            .x,
        8
    );

    assert!(try_move_inventory_item(&layout, "medkit", 1, 0).is_none());
    assert_eq!(layout.items[0].x, 0);
}

#[test]
fn inventory_click_mapping_accounts_for_body_panel() {
    let window_size = Vec2::new(535.0, 400.0);
    let panel_left = (window_size.x - INVENTORY_PANEL_WIDTH) / 2.0;
    let panel_top = (window_size.y - INVENTORY_PANEL_HEIGHT) / 2.0;
    let grid_left = panel_left + INVENTORY_BODY_PANEL_WIDTH + INVENTORY_PANEL_GAP;
    let grid_top = panel_top
        + INVENTORY_HEADER_HEIGHT
        + (INVENTORY_CONTENT_HEIGHT - INVENTORY_GRID_PIXEL_HEIGHT) / 2.0;

    assert_eq!(
        inventory_cell_at_position(
            window_size,
            Vec2::new(
                grid_left + INVENTORY_SLOT_SIZE / 2.0,
                grid_top + INVENTORY_SLOT_SIZE / 2.0
            )
        ),
        Some((0, 0))
    );
    assert_eq!(
        inventory_cell_at_position(window_size, Vec2::new(panel_left + 40.0, grid_top + 18.0)),
        None
    );
}

#[test]
fn inventory_rotation_swaps_dimensions_only_when_placement_is_valid() {
    let layout = InventoryLayout {
        width: 3,
        height: 2,
        items: vec![
            inventory_item("long", 0, 0, 2, 1, 100),
            inventory_item("other", 2, 0, 1, 1, 100),
        ],
    };
    let rotated = try_rotate_inventory_item(&layout, "long").unwrap();
    let item = rotated.items.iter().find(|item| item.id == "long").unwrap();
    assert_eq!((item.width, item.height), (1, 2));

    let blocked_layout = InventoryLayout {
        width: 2,
        height: 2,
        items: vec![
            inventory_item("long", 0, 0, 2, 1, 100),
            inventory_item("other", 0, 1, 1, 1, 100),
        ],
    };
    assert!(try_rotate_inventory_item(&blocked_layout, "long").is_none());
    assert_eq!(blocked_layout.items[0].width, 2);
}

#[test]
fn raid_loadout_is_reserved_and_loot_return_space_is_checked() {
    let stash = default_inventory_layout();
    let loot = vec![
        inventory_item("battery", 0, 0, 1, 1, 300),
        inventory_item("electronics", 0, 0, 1, 1, 180),
        inventory_item("ration", 0, 0, 1, 1, 250),
    ];

    let (remaining_stash, loadout) = prepare_raid_loadout(&stash, &loot, 1).unwrap();
    assert!(remaining_stash.items.iter().all(|item| item.id != "medkit"));
    assert!(remaining_stash.items.iter().all(|item| item.id != "ammo"));
    assert!(loadout.items.iter().any(|item| item.id == "medkit"));
    assert!(loadout.items.iter().any(|item| item.id == "ammo"));
}

#[test]
fn raid_start_rejects_a_stash_that_cannot_fit_returned_loot() {
    let stash = InventoryLayout {
        width: INVENTORY_GRID_WIDTH,
        height: INVENTORY_GRID_HEIGHT,
        items: (0..INVENTORY_GRID_HEIGHT)
            .flat_map(|y| {
                (0..INVENTORY_GRID_WIDTH)
                    .map(move |x| inventory_item(&format!("item-{x}-{y}"), x, y, 1, 1, 1))
            })
            .collect(),
    };
    let loot = vec![inventory_item("battery", 0, 0, 1, 1, 300)];

    assert!(prepare_raid_loadout(&stash, &loot, 1).is_none());
}

#[test]
fn extracted_inventory_merges_into_stash_without_overwriting_items() {
    let stash = default_inventory_layout();
    let carried = InventoryLayout {
        width: INVENTORY_GRID_WIDTH,
        height: INVENTORY_GRID_HEIGHT,
        items: vec![inventory_item("battery-1", 0, 0, 1, 1, 300)],
    };

    let merged = merge_raid_inventory(&stash, &carried).unwrap();
    assert_eq!(merged.items.len(), stash.items.len() + 1);
    assert!(merged.items.iter().any(|item| item.id == "battery-1"));
    assert!(validate_inventory(&merged).is_ok());
}

#[test]
fn raid_number_skips_ids_already_in_stash() {
    let mut stash = empty_inventory_layout();
    stash.items.push(inventory_item("battery-1", 0, 0, 1, 1, 1));
    let loot = vec![inventory_item("battery", 0, 0, 1, 1, 300)];

    assert_eq!(next_raid_number(0, &stash, &loot), Some(2));
}

#[test]
fn raid_can_start_loot_extract_and_return_to_stash() {
    let (requests, _request_receiver) = mpsc::channel();
    let (_message_sender, messages) = mpsc::channel();
    let keys = ButtonInput::<KeyCode>::default();
    let mut app = App::new();
    let mut menu_commands = GameMenuCommands::default();
    menu_commands.start_raid = true;
    app.insert_resource(keys)
        .insert_resource(BackendSync {
            requests,
            messages: Mutex::new(messages),
            status: "Connected.".to_owned(),
            ready: true,
            loading: false,
            save_enabled: true,
            retry_available: false,
            last_queued: Some(default_inventory_layout()),
        })
        .insert_resource(InventoryState::default())
        .insert_resource(RaidState::default())
        .insert_resource(GameSettingsState::default())
        .insert_resource(menu_commands)
        .insert_resource(PlayerVitals::default())
        .insert_resource({
            let definition = starter_weapon_definition();
            WeaponState {
                magazine: definition.magazine_capacity,
                reserve: definition.magazine_capacity * 3,
                definition,
                random_state: 1,
            }
        })
        .insert_resource(WeaponRecoil::default())
        .insert_resource(CombatStats {
            enemies_killed: 0,
            total_enemies: 2,
        })
        .insert_resource(PlayerSignature::default())
        .add_systems(Update, update_raid_flow);
    app.world_mut().spawn((
        Transform::from_xyz(0.0, 1.65, 5.0),
        PlayerCamera {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));
    app.world_mut().spawn((
        Transform::from_xyz(-6.0, 0.9, -6.0),
        ArenaCollider {
            center: Vec3::new(-6.0, 0.9, -6.0),
            half_extents: Vec3::new(0.4, 0.9, 0.3),
        },
        Visibility::Hidden,
        Enemy {
            health: ENEMY_HEALTH,
            attack_timer: Timer::from_seconds(ENEMY_ATTACK_INTERVAL, TimerMode::Repeating),
            detection: 0.0,
            last_known_player: None,
            memory_remaining: 0.0,
            shot_random_state: 1,
            home_position: Vec3::new(-6.0, 0.9, -6.0),
            patrol_points: Vec::new(),
            patrol_index: 0,
        },
    ));
    app.world_mut().spawn((
        Transform::from_xyz(0.0, 1.65, 5.0),
        Visibility::Visible,
        RaidLoot {
            item: inventory_item("battery", 0, 0, 1, 1, 300),
            key: "battery",
            available: true,
        },
    ));
    for (index, position) in EXTRACTION_POSITIONS.into_iter().enumerate() {
        app.world_mut().spawn((
            Transform::from_translation(position),
            ExtractionPoint {
                radius: 2.2,
                label: if index == 0 { "WEST EXIT" } else { "EAST EXIT" },
            },
        ));
    }

    app.update();
    assert_eq!(app.world().resource::<RaidState>().phase, RaidPhase::Active);
    assert!(app.world().resource::<PlayerVitals>().armor.is_some());
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Escape);
    app.world_mut().insert_resource(keys);
    app.update();
    assert_eq!(app.world().resource::<RaidState>().phase, RaidPhase::Paused);
    app.world_mut()
        .insert_resource(ButtonInput::<KeyCode>::default());
    app.world_mut().resource_mut::<GameMenuCommands>().resume = true;
    app.update();
    assert_eq!(app.world().resource::<RaidState>().phase, RaidPhase::Active);

    app.world_mut()
        .resource_mut::<PlayerVitals>()
        .armor
        .as_mut()
        .unwrap()
        .durability = 37;

    app.world_mut()
        .insert_resource(ButtonInput::<KeyCode>::default());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyE);
    app.update();
    assert_eq!(app.world().resource::<RaidState>().loot_collected, 1);

    app.world_mut()
        .insert_resource(ButtonInput::<KeyCode>::default());
    {
        let mut world = app.world_mut();
        let mut player = world.query_filtered::<&mut Transform, With<PlayerCamera>>();
        player.single_mut(&mut world).translation = Vec3::new(9.0, 1.65, -9.0);
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyF);
    app.update();
    assert_eq!(
        app.world().resource::<RaidState>().phase,
        RaidPhase::Debrief
    );
    assert!(app
        .world()
        .resource::<InventoryState>()
        .layout
        .items
        .iter()
        .any(|item| item.id == "battery-1"));
    assert!(app
        .world()
        .resource::<RaidState>()
        .status
        .contains("EXTRACTED VIA EAST EXIT"));
    assert_eq!(
        app.world()
            .resource::<InventoryState>()
            .layout
            .items
            .iter()
            .find(|item| item.id == "armor")
            .and_then(|item| item.durability),
        Some(37)
    );

    app.world_mut()
        .insert_resource(ButtonInput::<KeyCode>::default());
    app.world_mut()
        .resource_mut::<GameMenuCommands>()
        .return_to_stash = true;
    app.update();
    assert_eq!(app.world().resource::<RaidState>().phase, RaidPhase::Stash);
    assert!(!app.world().resource::<RaidState>().menu_open);
    let world = app.world_mut();
    let mut player = world.query_filtered::<(&Transform, &PlayerCamera), With<PlayerCamera>>();
    let (transform, camera) = player.single(&world);
    assert_eq!(transform.translation, SHELTER_BED_SPAWN);
    assert_eq!(camera.yaw, SHELTER_BED_YAW);

    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Escape);
    app.insert_resource(keys);
    app.update();
    assert!(app.world().resource::<RaidState>().menu_open);

    app.insert_resource(ButtonInput::<KeyCode>::default());
    app.update();
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Escape);
    app.insert_resource(keys);
    app.update();
    assert!(!app.world().resource::<RaidState>().menu_open);
}

#[test]
fn start_raid_menu_button_requests_a_raid() {
    let mut app = App::new();
    let (requests, _request_receiver) = mpsc::channel();
    let (_message_sender, messages) = mpsc::channel();
    app.add_event::<AppExit>()
        .insert_resource(GameMenuCommands::default())
        .insert_resource(InventoryState::default())
        .insert_resource(GameSettingsState::default())
        .insert_resource(BackendSync {
            requests,
            messages: Mutex::new(messages),
            status: "Connected.".to_owned(),
            ready: true,
            loading: false,
            save_enabled: true,
            retry_available: false,
            last_queued: None,
        })
        .add_systems(Update, handle_game_menu);
    app.world_mut().spawn((
        Button,
        Interaction::Pressed,
        GameMenuButton(GameMenuAction::StartRaid),
        BackgroundColor(Color::BLACK),
    ));

    app.update();

    assert!(app.world().resource::<GameMenuCommands>().start_raid);
}

#[test]
fn settings_controls_apply_only_while_settings_are_open() {
    let mut app = App::new();
    app.insert_resource(GameSettingsState::default())
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(GlobalVolume::new(1.0))
        .add_systems(Update, handle_settings_controls);
    let button = app
        .world_mut()
        .spawn((
            Button,
            Interaction::Pressed,
            SettingsControl(SettingsAction::VolumeDown),
            BackgroundColor(Color::BLACK),
        ))
        .id();

    app.update();
    assert_eq!(
        app.world()
            .resource::<GameSettingsState>()
            .values
            .master_volume,
        1.0
    );

    *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::None;
    app.update();
    app.world_mut().resource_mut::<GameSettingsState>().open = true;
    *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
    app.update();

    assert_eq!(
        app.world()
            .resource::<GameSettingsState>()
            .values
            .master_volume,
        0.9
    );
}

#[test]
fn settings_ui_system_initializes_without_query_conflicts() {
    let mut app = App::new();
    app.insert_resource(GameSettingsState::default())
        .add_systems(Update, update_settings_ui);
    app.world_mut().spawn((Visibility::Hidden, SettingsOverlay));
    let value_text = app
        .world_mut()
        .spawn((Text::new(""), SettingsValueText(SettingsField::Resolution)));
    let value_text = value_text.id();
    let notice_text = app.world_mut().spawn((Text::new(""), SettingsNoticeText));
    let notice_text = notice_text.id();

    app.update();

    assert_eq!(app.world().get::<Text>(value_text).unwrap().0, "1280x720");
    assert!(!app.world().get::<Text>(notice_text).unwrap().0.is_empty());
}

#[test]
fn retry_sync_retries_profile_load_or_save() {
    let (requests, request_receiver) = mpsc::channel();
    let (_message_sender, messages) = mpsc::channel();
    let mut app = App::new();
    app.add_event::<AppExit>()
        .insert_resource(GameMenuCommands::default())
        .insert_resource(InventoryState::default())
        .insert_resource(GameSettingsState::default())
        .insert_resource(BackendSync {
            requests,
            messages: Mutex::new(messages),
            status: "Profile load failed.".to_owned(),
            ready: false,
            loading: false,
            save_enabled: false,
            retry_available: true,
            last_queued: None,
        })
        .add_systems(Update, handle_game_menu);
    let button = app
        .world_mut()
        .spawn((
            Button,
            Interaction::Pressed,
            GameMenuButton(GameMenuAction::RetryProfile),
            BackgroundColor(Color::BLACK),
        ))
        .id();

    app.update();
    assert!(matches!(
        request_receiver.try_recv(),
        Ok(BackendRequest::Load)
    ));
    assert!(app.world().resource::<BackendSync>().loading);

    *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::None;
    app.update();
    {
        let mut sync = app.world_mut().resource_mut::<BackendSync>();
        sync.ready = true;
        sync.loading = false;
        sync.retry_available = true;
    }
    *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
    app.update();

    assert!(matches!(
        request_receiver.try_recv(),
        Ok(BackendRequest::Save(_))
    ));
    assert!(!app.world().resource::<BackendSync>().retry_available);
}

#[test]
fn paused_raid_does_not_save_raid_inventory_to_stash() {
    let (requests, request_receiver) = mpsc::channel();
    let (_message_sender, messages) = mpsc::channel();
    let mut raid = RaidState::default();
    raid.phase = RaidPhase::Paused;
    let mut app = App::new();
    app.insert_resource(InventoryState::default())
        .insert_resource(raid)
        .insert_resource(BackendSync {
            requests,
            messages: Mutex::new(messages),
            status: "Connected.".to_owned(),
            ready: true,
            loading: false,
            save_enabled: true,
            retry_available: false,
            last_queued: Some(empty_inventory_layout()),
        })
        .add_systems(Update, sync_inventory_changes);

    app.update();

    assert!(request_receiver.try_recv().is_err());
}

#[test]
fn game_menu_switches_between_stash_and_debrief_actions() {
    let (requests, _request_receiver) = mpsc::channel();
    let (_message_sender, messages) = mpsc::channel();
    let mut app = App::new();
    let mut raid = RaidState::default();
    raid.menu_open = true;
    app.insert_resource(raid)
        .insert_resource(InventoryState::default())
        .insert_resource(GameSettingsState::default())
        .insert_resource(BackendSync {
            requests,
            messages: Mutex::new(messages),
            status: "Connected.".to_owned(),
            ready: true,
            loading: false,
            save_enabled: true,
            retry_available: false,
            last_queued: None,
        })
        .add_systems(Update, update_game_menu);
    let overlay = app
        .world_mut()
        .spawn((Visibility::Hidden, GameMenuOverlay))
        .id();
    let start_button = app
        .world_mut()
        .spawn((
            Node::default(),
            Visibility::Hidden,
            GameMenuButton(GameMenuAction::StartRaid),
        ))
        .id();
    let return_button = app
        .world_mut()
        .spawn((
            Node::default(),
            Visibility::Hidden,
            GameMenuButton(GameMenuAction::ReturnToStash),
        ))
        .id();
    let retry_button = app
        .world_mut()
        .spawn((
            Node::default(),
            Visibility::Hidden,
            GameMenuButton(GameMenuAction::RetryProfile),
        ))
        .id();
    let resume_button = app
        .world_mut()
        .spawn((
            Node::default(),
            Visibility::Hidden,
            GameMenuButton(GameMenuAction::Resume),
        ))
        .id();

    app.update();
    assert_eq!(
        app.world().get::<Visibility>(overlay),
        Some(&Visibility::Visible)
    );
    assert_eq!(
        app.world().get::<Visibility>(start_button),
        Some(&Visibility::Visible)
    );
    assert_eq!(
        app.world().get::<Node>(start_button).unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world().get::<Visibility>(return_button),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        app.world().get::<Visibility>(retry_button),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        app.world().get::<Node>(retry_button).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Visibility>(resume_button),
        Some(&Visibility::Hidden)
    );

    {
        let mut sync = app.world_mut().resource_mut::<BackendSync>();
        sync.ready = false;
        sync.loading = true;
    }
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(start_button),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        app.world().get::<Visibility>(retry_button),
        Some(&Visibility::Hidden)
    );
    {
        let mut sync = app.world_mut().resource_mut::<BackendSync>();
        sync.loading = false;
        sync.retry_available = true;
    }
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(retry_button),
        Some(&Visibility::Visible)
    );

    app.world_mut().resource_mut::<RaidState>().phase = RaidPhase::Debrief;
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(start_button),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        app.world().get::<Visibility>(return_button),
        Some(&Visibility::Visible)
    );

    app.world_mut().resource_mut::<RaidState>().phase = RaidPhase::Paused;
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(resume_button),
        Some(&Visibility::Visible)
    );
}

#[test]
fn cursor_is_released_when_stash_menu_opens() {
    let mut window = Window::default();
    window.cursor_options.grab_mode = CursorGrabMode::Locked;
    window.cursor_options.visible = false;

    let mut app = App::new();
    let mut raid = RaidState::default();
    raid.menu_open = true;
    app.insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(ButtonInput::<MouseButton>::default())
        .insert_resource(InventoryState::default())
        .insert_resource(raid)
        .add_systems(Update, capture_cursor);
    let window_entity = app.world_mut().spawn(window).id();

    app.update();

    let window = app.world().get::<Window>(window_entity).unwrap();
    assert_eq!(window.cursor_options.grab_mode, CursorGrabMode::None);
    assert!(window.cursor_options.visible);
}

#[test]
fn using_medkit_consumes_it_and_heals_the_most_injured_zone() {
    let layout = default_inventory_layout();
    let mut health = HealthState::standard();
    health.apply_damage(BodyPart::Thorax, 30);
    let (healed_layout, health) = try_use_medkit(&layout, &health, MEDKIT_HEAL_AMOUNT).unwrap();

    assert_eq!(health.zones[&BodyPart::Thorax].current, 85);
    assert!(!healed_layout.items.iter().any(|item| item.id == "medkit"));
    assert!(try_use_medkit(&healed_layout, &health, MEDKIT_HEAL_AMOUNT).is_none());
    assert!(layout.items.iter().any(|item| item.id == "medkit"));
}

#[test]
fn armor_is_loaded_from_inventory_with_saved_durability() {
    let mut layout = default_inventory_layout();
    let armor_item = layout
        .items
        .iter_mut()
        .find(|item| item.id == "armor")
        .unwrap();
    armor_item.durability = Some(37);

    assert_eq!(
        armor_from_inventory(&layout).unwrap().unwrap().durability,
        37
    );
    assert_eq!(
        armor_from_inventory(&empty_inventory_layout()).unwrap(),
        None
    );
}

#[test]
fn stamina_drains_for_sprinting_and_regenerates_at_rest() {
    assert_eq!(advance_stamina(100.0, true, 1.0), 75.0);
    assert_eq!(advance_stamina(50.0, false, 1.0), 68.0);
    assert_eq!(advance_stamina(1.0, true, 2.0), 0.0);
    assert_eq!(advance_stamina(99.0, false, 1.0), MAX_STAMINA);
}

#[test]
fn inventory_vitals_ui_shows_zone_health_armor_and_stamina() {
    let mut health = HealthState::standard();
    health.apply_damage(BodyPart::Thorax, 40);
    let vitals = PlayerVitals {
        health,
        armor: Some(ArmorState::with_durability(starter_armor_definition(), 40).unwrap()),
        stamina: 25.0,
    };
    let mut raid = RaidState::default();
    raid.phase = RaidPhase::Active;
    let mut app = App::new();
    app.insert_resource(vitals)
        .insert_resource(raid)
        .insert_resource(InventoryState::default())
        .add_systems(Update, update_vitals_ui);

    app.world_mut().spawn((
        BodyPartSegment(BodyPart::Thorax),
        Interaction::Hovered,
        BackgroundColor(Color::BLACK),
        BorderColor(Color::BLACK),
    ));
    let hover_text = app
        .world_mut()
        .spawn((Text::new(""), BodyHealthHoverText))
        .id();
    let health_text = app
        .world_mut()
        .spawn((Text::new(""), InventoryHealthText))
        .id();
    let health_bar = app
        .world_mut()
        .spawn((Node::default(), InventoryHealthBarFill))
        .id();
    let armor_text = app
        .world_mut()
        .spawn((Text::new(""), InventoryArmorText))
        .id();
    let armor_bar = app
        .world_mut()
        .spawn((Node::default(), InventoryArmorBarFill))
        .id();
    let stamina_root = app
        .world_mut()
        .spawn((Visibility::Hidden, StaminaHudRoot))
        .id();
    let stamina_text = app.world_mut().spawn((Text::new(""), StaminaHudText)).id();
    let stamina_bar = app
        .world_mut()
        .spawn((
            Node::default(),
            BackgroundColor(Color::BLACK),
            StaminaHudBarFill,
        ))
        .id();

    app.update();

    assert_eq!(
        app.world().get::<Text>(hover_text).unwrap().0,
        "THORAX 45 / 85"
    );
    assert_eq!(
        app.world().get::<Text>(health_text).unwrap().0,
        "HP 400 / 440"
    );
    assert_eq!(
        app.world().get::<Text>(armor_text).unwrap().0,
        "ARMOR 40 / 100"
    );
    assert_eq!(
        app.world().get::<Text>(stamina_text).unwrap().0,
        "STAMINA 25%"
    );
    assert_eq!(
        app.world().get::<Visibility>(stamina_root),
        Some(&Visibility::Visible)
    );
    assert!(matches!(
        app.world().get::<Node>(health_bar).unwrap().width,
        Val::Percent(value) if (value - (400.0 / 440.0 * 100.0)).abs() < 0.001
    ));
    assert_eq!(
        app.world().get::<Node>(armor_bar).unwrap().width,
        Val::Percent(40.0)
    );
    assert_eq!(
        app.world().get::<Node>(stamina_bar).unwrap().width,
        Val::Percent(25.0)
    );
}

#[test]
fn inventory_ui_system_initializes_without_query_conflicts() {
    let mut app = App::new();
    let (requests, _request_receiver) = mpsc::channel();
    let (_message_sender, messages) = mpsc::channel();
    app.insert_resource(InventoryState::default())
        .insert_resource(BackendSync {
            requests,
            messages: Mutex::new(messages),
            status: "Connected.".to_owned(),
            ready: true,
            loading: false,
            save_enabled: true,
            retry_available: false,
            last_queued: None,
        })
        .add_systems(Update, update_inventory_ui);

    app.update();
}
