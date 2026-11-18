use super::*;

pub(super) fn update_enemies(
    time: Res<Time>,
    inventory: Res<InventoryState>,
    mut raid: ResMut<RaidState>,
    mut signature: ResMut<PlayerSignature>,
    player_camera: Query<&Transform, (With<PlayerCamera>, Without<Enemy>)>,
    mut enemies: Query<
        (&mut Transform, &mut ArenaCollider, &mut Enemy),
        (With<Enemy>, Without<PlayerCamera>),
    >,
    obstacles: Query<&ArenaCollider, Without<Enemy>>,
    mut vitals: ResMut<PlayerVitals>,
    mut feedback: ResMut<CombatFeedback>,
    asset_server: Option<Res<AssetServer>>,
    mut commands: Commands,
) {
    let delta_secs = time.delta_secs();
    signature.gunshot_remaining = (signature.gunshot_remaining - delta_secs).max(0.0);
    if raid.phase != RaidPhase::Active || inventory.open || vitals.health.is_dead() {
        return;
    }

    let Some(player_transform) = player_camera.iter().next() else {
        return;
    };
    let player_position = player_transform.translation;
    let player_aim_point = player_position - Vec3::Y * PLAYER_AIM_HEIGHT;

    for (mut transform, mut collider, mut enemy) in &mut enemies {
        if enemy.health == 0 {
            continue;
        }

        let was_alerted = enemy.detection > 0.0;
        let was_spotted = enemy.detection >= 1.0;
        let to_player = player_position - transform.translation;
        let distance = Vec3::new(to_player.x, 0.0, to_player.z).length();
        let enemy_forward = transform.rotation * Vec3::NEG_Z;
        let can_see = player_in_vision(
            transform.translation,
            enemy_forward,
            player_position,
            signature.crouching,
            obstacles.iter().copied(),
        );

        if can_see {
            enemy.detection = (enemy.detection
                + detection_rate(distance, signature.sprinting, signature.crouching) * delta_secs)
                .min(1.0);
            enemy.last_known_player = Some(player_position);
            enemy.memory_remaining = ENEMY_TARGET_MEMORY;
        }

        let movement_hearing_range = if signature.sprinting {
            16.0
        } else if signature.crouching {
            3.0
        } else {
            8.0
        };
        let heard_player = signature.gunshot_remaining > 0.0
            && distance <= ENEMY_GUNSHOT_HEARING_RANGE
            || signature.moving && distance <= movement_hearing_range;
        if heard_player {
            enemy.last_known_player = Some(player_position);
            enemy.memory_remaining = if signature.gunshot_remaining > 0.0 {
                ENEMY_GUNSHOT_MEMORY
            } else {
                ENEMY_TARGET_MEMORY
            };
            enemy.detection = enemy.detection.max(if signature.gunshot_remaining > 0.0 {
                0.4
            } else {
                0.15
            });
        } else if !can_see {
            enemy.memory_remaining = (enemy.memory_remaining - delta_secs).max(0.0);
        }

        if enemy.memory_remaining == 0.0 {
            enemy.last_known_player = None;
            enemy.detection = (enemy.detection - ENEMY_DETECTION_DECAY * delta_secs).max(0.0);
        }

        if !was_alerted && enemy.detection > 0.0 {
            feedback.show(if signature.gunshot_remaining > 0.0 {
                "ENEMY ALERTED BY GUNFIRE"
            } else if heard_player {
                "ENEMY HEARD MOVEMENT"
            } else {
                "ENEMY AWARENESS RISING"
            });
        }
        if !was_spotted && enemy.detection >= 1.0 {
            feedback.show("ENEMY SPOTTED YOU");
        }

        if let Some(last_known_player) = enemy.last_known_player {
            let mut to_known_player = last_known_player - transform.translation;
            to_known_player.y = 0.0;
            let distance_to_known_player = to_known_player.length();
            if distance_to_known_player > f32::EPSILON {
                transform.rotation =
                    Quat::from_rotation_y((-to_known_player.x).atan2(-to_known_player.z));
            }

            let spotted = enemy.detection >= 1.0;
            let stop_distance = if spotted && can_see {
                ENEMY_STOP_DISTANCE
            } else {
                0.8
            };
            if distance_to_known_player > stop_distance {
                let movement = to_known_player.normalize_or_zero() * ENEMY_MOVE_SPEED * delta_secs;
                transform.translation =
                    move_on_clear_axes(transform.translation, movement, obstacles.iter().copied());
                collider.center = transform.translation;
            }
        } else if !enemy.patrol_points.is_empty() {
            let target = enemy.patrol_points[enemy.patrol_index];
            let mut to_target = target - transform.translation;
            to_target.y = 0.0;
            if to_target.length() < 0.4 {
                enemy.patrol_index = (enemy.patrol_index + 1) % enemy.patrol_points.len();
            } else {
                transform.rotation = Quat::from_rotation_y((-to_target.x).atan2(-to_target.z));
                let movement = to_target.normalize_or_zero() * ENEMY_MOVE_SPEED * delta_secs;
                transform.translation =
                    move_on_clear_axes(transform.translation, movement, obstacles.iter().copied());
                collider.center = transform.translation;
            }
        }

        let shot_origin = transform.translation + Vec3::Y * ENEMY_MUZZLE_HEIGHT;
        let shot_distance = shot_origin.distance(player_aim_point);
        let can_attack = enemy.detection >= 1.0
            && can_see
            && shot_distance <= ENEMY_ATTACK_RANGE
            && has_line_of_sight(shot_origin, player_aim_point, obstacles.iter().copied());

        if can_attack {
            enemy.attack_timer.tick(time.delta());
            let shot_fired = enemy.attack_timer.just_finished();
            if shot_fired {
                play_sound(
                    &mut commands,
                    asset_server.as_deref(),
                    AUDIO_ENEMY_GUNSHOT,
                    PlaybackSettings::DESPAWN.with_volume(Volume::new(0.75)),
                );
            }
            if shot_fired
                && bot_shot_hits(
                    shot_origin,
                    player_position,
                    shot_distance,
                    signature.moving,
                    signature.crouching,
                    obstacles.iter().copied(),
                    &mut enemy.shot_random_state,
                )
            {
                let zone = roll_hit_zone(&mut enemy.shot_random_state);
                let hit = {
                    let PlayerVitals { health, armor, .. } = &mut *vitals;
                    apply_hit(
                        health,
                        armor.as_mut(),
                        zone,
                        ENEMY_DAMAGE,
                        DamageType::Ballistic,
                    )
                };
                if let Some(hit) = hit {
                    let impact_sound = if hit.armor_durability_lost > 0 {
                        AUDIO_IMPACT_ARMOR
                    } else {
                        AUDIO_IMPACT_FLESH
                    };
                    play_sound(
                        &mut commands,
                        asset_server.as_deref(),
                        impact_sound,
                        PlaybackSettings::DESPAWN.with_volume(Volume::new(0.55)),
                    );
                    if hit.armor_durability_lost > 0 {
                        raid.record_event(RaidEventKind::ArmorDamaged {
                            durability_lost: hit.armor_durability_lost,
                            remaining_durability: vitals
                                .armor
                                .as_ref()
                                .map_or(0, |armor| armor.durability),
                        });
                    }
                    raid.record_event(RaidEventKind::DamageReceived {
                        zone: hit.zone,
                        amount: hit.health_damage,
                        damage_type: DamageType::Ballistic,
                    });
                    feedback.show(damage_feedback(hit));
                }
            }
        } else {
            enemy.attack_timer.reset();
        }
    }
}

pub(super) fn update_raid_flow(
    keyboard: Res<ButtonInput<KeyCode>>,
    sync: Res<BackendSync>,
    settings: Res<GameSettingsState>,
    mut raid: ResMut<RaidState>,
    mut menu_commands: ResMut<GameMenuCommands>,
    mut inventory: ResMut<InventoryState>,
    mut vitals: ResMut<PlayerVitals>,
    mut weapon: ResMut<WeaponState>,
    mut recoil: ResMut<WeaponRecoil>,
    mut stats: ResMut<CombatStats>,
    mut signature: ResMut<PlayerSignature>,
    mut player: Query<(&mut Transform, &mut PlayerCamera), (With<PlayerCamera>, Without<Enemy>)>,
    mut enemies: Query<
        (
            &mut Transform,
            &mut ArenaCollider,
            &mut Enemy,
            &mut Visibility,
        ),
        (With<Enemy>, Without<PlayerCamera>),
    >,
    mut loot: Query<
        (Entity, &Transform, &mut RaidLoot, &mut Visibility),
        (With<RaidLoot>, Without<Enemy>, Without<PlayerCamera>),
    >,
    extraction_points: Query<
        (&Transform, &ExtractionPoint),
        (With<ExtractionPoint>, Without<PlayerCamera>, Without<Enemy>),
    >,
) {
    let start_requested = menu_commands.start_raid;
    let resume_requested = menu_commands.resume;
    let return_requested = menu_commands.return_to_stash;
    let pause_requested =
        keyboard.just_pressed(KeyCode::Escape) && !inventory.open && !settings.open;
    menu_commands.start_raid = false;
    menu_commands.resume = false;
    menu_commands.return_to_stash = false;

    if raid.phase == RaidPhase::Stash && pause_requested {
        raid.menu_open = !raid.menu_open;
        return;
    }

    if raid.phase == RaidPhase::Active && pause_requested {
        raid.phase = RaidPhase::Paused;
        raid.status = "Raid paused.".to_owned();
        return;
    }

    if raid.phase == RaidPhase::Paused {
        if resume_requested {
            raid.phase = RaidPhase::Active;
            raid.status = "RAID | E: loot | F: extract at WEST or EAST EXIT".to_owned();
        }
        return;
    }

    if raid.phase == RaidPhase::Stash && start_requested {
        if inventory.open {
            raid.status = "Close the inventory before starting a raid.".to_owned();
            return;
        }
        if !sync.ready || !sync.save_enabled {
            raid.status = format!(
                "Backend is not ready for a persistent raid: {}",
                sync.status
            );
            return;
        }

        let loot_templates = loot
            .iter_mut()
            .map(|(_, _, pickup, _)| pickup.item.clone())
            .collect::<Vec<_>>();
        let Some(raid_number) =
            next_raid_number(raid.raid_number, &inventory.layout, &loot_templates)
        else {
            raid.status = "Could not allocate a unique raid ID.".to_owned();
            return;
        };
        let Some((stash_after_loadout, loadout)) =
            prepare_raid_loadout(&inventory.layout, &loot_templates, raid_number)
        else {
            raid.status =
                "Not enough stash space to bring back the raid loot; make room and try again."
                    .to_owned();
            return;
        };
        let armor = match armor_from_inventory(&loadout) {
            Ok(armor) => armor,
            Err(error) => {
                raid.status = format!("Cannot start raid: invalid armor state ({error}).");
                return;
            }
        };
        let Some((mut player_transform, mut player_camera)) = player.iter_mut().next() else {
            raid.status = "Cannot start raid: player camera is missing.".to_owned();
            return;
        };

        inventory.layout = loadout;
        inventory.dragging = None;
        raid.stash_after_loadout = Some(stash_after_loadout);
        raid.raid_number = raid_number;
        raid.loot_collected = 0;
        raid.events.clear();
        raid.record_event(RaidEventKind::RaidStarted);
        raid.menu_open = false;
        raid.phase = RaidPhase::Active;
        raid.status = "RAID | E: loot | F: extract at WEST or EAST EXIT".to_owned();

        player_camera.yaw = 0.0;
        player_camera.pitch = 0.0;
        player_transform.translation = Vec3::new(0.0, 1.65, 5.0);
        player_transform.rotation = Quat::IDENTITY;
        vitals.health = HealthState::standard();
        vitals.armor = armor;
        vitals.stamina = MAX_STAMINA;
        weapon.magazine = weapon.definition.magazine_capacity;
        weapon.reserve = weapon.definition.magazine_capacity * 3;
        recoil.angle = 0.0;
        *signature = PlayerSignature::default();
        stats.enemies_killed = 0;
        stats.total_enemies = 2;

        for (mut transform, mut collider, mut enemy, mut visibility) in &mut enemies {
            transform.translation = enemy.home_position;
            transform.rotation = Transform::from_translation(enemy.home_position)
                .looking_at(Vec3::new(0.0, enemy.home_position.y, 5.0), Vec3::Y)
                .rotation;
            collider.center = enemy.home_position;
            enemy.health = ENEMY_HEALTH;
            enemy.attack_timer.reset();
            enemy.detection = 0.0;
            enemy.last_known_player = None;
            enemy.memory_remaining = 0.0;
            enemy.patrol_index = 0;
            *visibility = Visibility::Visible;
        }
        for (_, _, mut pickup, mut visibility) in &mut loot {
            pickup.available = true;
            *visibility = Visibility::Visible;
        }
        return;
    }

    if inventory.open {
        return;
    }

    if raid.phase == RaidPhase::Active && vitals.health.is_dead() {
        let Some(stash) = raid.stash_after_loadout.take() else {
            raid.status = "Raid state error: stash snapshot is missing.".to_owned();
            return;
        };
        let lost_items = inventory.layout.items.len();
        let fatal_wound = fatal_wound_label(&vitals.health);
        raid.record_event(RaidEventKind::PlayerDied);
        inventory.layout = stash;
        inventory.dragging = None;
        vitals.armor = None;
        raid.phase = RaidPhase::Debrief;
        let current_health = vitals.health.total_current();
        let maximum_health = vitals.health.total_maximum();
        let lost_health = maximum_health.saturating_sub(current_health);
        raid.status = format!(
            "KIA | {fatal_wound}\nLost {lost_items} carried items.\nLost raid loot: {}\nHealth remaining: {current_health}/{maximum_health} (lost {lost_health}).",
            collected_loot_summary(&raid.events)
        );
        return;
    }

    if raid.phase == RaidPhase::Active && keyboard.just_pressed(KeyCode::KeyE) {
        let Some(player_position) = player
            .iter_mut()
            .next()
            .map(|(transform, _)| transform.translation)
        else {
            raid.status = "Cannot pick up loot: player camera is missing.".to_owned();
            return;
        };
        let nearest = loot
            .iter_mut()
            .filter(|(_, _, pickup, _)| pickup.available)
            .map(|(entity, transform, _, _)| {
                (entity, transform.translation.distance(player_position))
            })
            .filter(|(_, distance)| *distance <= 2.5)
            .min_by(|left, right| left.1.total_cmp(&right.1));

        if let Some((entity, _)) = nearest {
            if let Ok((_, _, mut pickup, mut visibility)) = loot.get_mut(entity) {
                let mut item = pickup.item.clone();
                item.id = format!("{}-{}", pickup.key, raid.raid_number);
                if let Some(layout) = try_add_inventory_item(&inventory.layout, &item) {
                    inventory.layout = layout;
                    pickup.available = false;
                    *visibility = Visibility::Hidden;
                    raid.loot_collected += 1;
                    raid.record_event(RaidEventKind::LootCollected { item_id: item.id });
                    raid.status =
                        format!("Picked up {}. Extract at WEST or EAST EXIT.", pickup.key);
                } else {
                    raid.status = format!("No room for {} in the raid inventory.", pickup.key);
                }
            }
        } else {
            raid.status = "No loot nearby. Move close and press E.".to_owned();
        }
    }

    if raid.phase == RaidPhase::Active && keyboard.just_pressed(KeyCode::KeyF) {
        let Some(player_position) = player
            .iter_mut()
            .next()
            .map(|(transform, _)| transform.translation)
        else {
            raid.status = "Cannot extract: player camera is missing.".to_owned();
            return;
        };
        let nearby_extraction = extraction_points.iter().find_map(|(transform, point)| {
            (transform.translation.distance(player_position) <= point.radius).then_some(point.label)
        });
        let Some(extraction_label) = nearby_extraction else {
            raid.status = "Reach WEST or EAST EXIT and press F.".to_owned();
            return;
        };

        let Some(stash) = raid.stash_after_loadout.as_ref() else {
            raid.status = "Raid state error: stash snapshot is missing.".to_owned();
            return;
        };
        let mut carried = inventory.layout.clone();
        if let Some(armor) = vitals.armor.as_ref() {
            let Some(armor_item) = carried.items.iter_mut().find(|item| item.id == "armor") else {
                raid.status =
                    "Could not secure the equipped armor; raid remains active.".to_owned();
                return;
            };
            armor_item.durability = Some(armor.durability);
        }
        let Some(merged_stash) = merge_raid_inventory(stash, &carried) else {
            raid.status = "Could not secure the loot in the stash; raid remains active.".to_owned();
            return;
        };

        let current_health = vitals.health.total_current();
        let maximum_health = vitals.health.total_maximum();
        let lost_health = maximum_health.saturating_sub(current_health);
        let armor_status = vitals.armor.as_ref().map_or_else(
            || "not equipped".to_owned(),
            |armor| {
                format!(
                    "{} / {}",
                    armor.durability, armor.definition.maximum_durability
                )
            },
        );
        let loot_summary = collected_loot_summary(&raid.events);
        inventory.layout = merged_stash;
        inventory.dragging = None;
        raid.stash_after_loadout = None;
        vitals.armor = None;
        raid.phase = RaidPhase::Debrief;
        let secured_item_count = raid.loot_collected;
        raid.record_event(RaidEventKind::PlayerExtracted { secured_item_count });
        raid.status = format!(
            "EXTRACTED VIA {extraction_label}\nSecured: {loot_summary}\nHealth: {current_health}/{maximum_health} (lost {lost_health}).\nArmor durability: {armor_status}."
        );
        return;
    }

    if raid.phase == RaidPhase::Debrief && return_requested {
        let Some((mut player_transform, mut player_camera)) = player.iter_mut().next() else {
            raid.status = "Cannot return to shelter: player camera is missing.".to_owned();
            return;
        };
        player_transform.translation = SHELTER_BED_SPAWN;
        player_camera.yaw = SHELTER_BED_YAW;
        player_camera.pitch = 0.0;
        player_transform.rotation = Quat::from_rotation_y(SHELTER_BED_YAW);
        raid.menu_open = false;
        raid.phase = RaidPhase::Stash;
        raid.loot_collected = 0;
        vitals.health = HealthState::standard();
        vitals.stamina = MAX_STAMINA;
        for (_, _, _, mut visibility) in &mut enemies {
            *visibility = Visibility::Hidden;
        }
        raid.status = "Profile ready. Prepare your gear and start a raid.".to_owned();
    }
}

pub(super) fn bot_shot_hits(
    origin: Vec3,
    player_position: Vec3,
    distance: f32,
    player_moving: bool,
    player_crouching: bool,
    obstacles: impl Iterator<Item = ArenaCollider>,
    random_state: &mut u64,
) -> bool {
    let target_center = player_position - Vec3::Y * if player_crouching { 0.5 } else { 0.8 };
    let target_half_height = if player_crouching { 0.45 } else { 0.8 };
    let target_collider = ArenaCollider {
        center: target_center,
        half_extents: Vec3::new(PLAYER_RADIUS, target_half_height, PLAYER_RADIUS),
    };
    let spread = ENEMY_BASE_SPREAD
        + distance * ENEMY_DISTANCE_SPREAD
        + if player_moving {
            ENEMY_MOVEMENT_SPREAD
        } else {
            0.0
        };
    let direction = spread_direction(
        (target_center - origin).normalize_or_zero(),
        spread,
        random_state,
    );

    closest_target(
        origin,
        direction,
        obstacles
            .map(|obstacle| ((), obstacle, false))
            .chain(std::iter::once(((), target_collider, true))),
    )
    .is_some()
}

pub(super) fn player_in_vision(
    enemy_position: Vec3,
    enemy_forward: Vec3,
    player_position: Vec3,
    crouching: bool,
    obstacles: impl Iterator<Item = ArenaCollider>,
) -> bool {
    let to_player = player_position - enemy_position;
    let horizontal_to_player = Vec3::new(to_player.x, 0.0, to_player.z);
    let distance = horizontal_to_player.length();
    if distance > ENEMY_VISION_RANGE || distance <= f32::EPSILON {
        return false;
    }

    let horizontal_forward = Vec3::new(enemy_forward.x, 0.0, enemy_forward.z).normalize_or_zero();
    if horizontal_forward.dot(horizontal_to_player / distance) < ENEMY_VISION_FOV_COS {
        return false;
    }

    let eye_position = enemy_position + Vec3::Y * 1.35;
    let aim_point = player_position - Vec3::Y * if crouching { 0.35 } else { PLAYER_AIM_HEIGHT };
    has_line_of_sight(eye_position, aim_point, obstacles)
}

pub(super) fn detection_rate(distance: f32, sprinting: bool, crouching: bool) -> f32 {
    let proximity = (1.0 - distance / ENEMY_VISION_RANGE).clamp(0.0, 1.0);
    let exposure = if sprinting {
        1.35
    } else if crouching {
        0.45
    } else {
        1.0
    };
    (0.2 + proximity * 0.8) * exposure
}

pub(super) fn has_line_of_sight(
    origin: Vec3,
    target: Vec3,
    obstacles: impl Iterator<Item = ArenaCollider>,
) -> bool {
    let to_target = target - origin;
    let distance = to_target.length();
    if distance <= f32::EPSILON {
        return true;
    }

    let direction = to_target / distance;
    obstacles.into_iter().all(
        |obstacle| match ray_aabb_distance(origin, direction, obstacle) {
            Some(hit_distance) => hit_distance >= distance - PLAYER_RADIUS,
            None => true,
        },
    )
}
