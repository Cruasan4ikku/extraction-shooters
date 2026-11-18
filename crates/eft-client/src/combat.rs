use super::*;

pub(super) fn fire_weapon(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut colliders: Query<(Entity, &ArenaCollider, Option<&mut Enemy>)>,
    mut weapon: ResMut<WeaponState>,
    mut recoil: ResMut<WeaponRecoil>,
    mut signature: ResMut<PlayerSignature>,
    mut stats: ResMut<CombatStats>,
    mut feedback: ResMut<CombatFeedback>,
    asset_server: Option<Res<AssetServer>>,
    inventory: Res<InventoryState>,
    mut raid: ResMut<RaidState>,
    mut commands: Commands,
) {
    recoil.angle = (recoil.angle - WEAPON_RECOIL_RECOVERY * time.delta_secs()).max(0.0);
    if raid.phase != RaidPhase::Active || inventory.open {
        return;
    }

    let Some(window) = windows.iter().next() else {
        return;
    };
    if window.cursor_options.grab_mode != CursorGrabMode::Locked {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        let (magazine, reserve, outcome) = reload_magazine(
            weapon.magazine,
            weapon.reserve,
            weapon.definition.magazine_capacity,
        );
        weapon.magazine = magazine;
        weapon.reserve = reserve;
        match outcome {
            ReloadOutcome::Loaded(rounds) => feedback.show(format!("RELOADED: +{rounds} ROUNDS")),
            ReloadOutcome::MagazineFull => feedback.show("MAGAZINE FULL"),
            ReloadOutcome::NoReserveAmmo => feedback.show("NO SPARE AMMO"),
        }
    }

    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if weapon.magazine == 0 {
        feedback.show("EMPTY MAGAZINE: PRESS R TO RELOAD");
        return;
    }

    let Some(camera) = cameras.iter().next() else {
        return;
    };
    let origin = camera.translation;
    let forward = camera.rotation * Vec3::NEG_Z;
    let moving = keyboard.pressed(KeyCode::KeyW)
        || keyboard.pressed(KeyCode::KeyA)
        || keyboard.pressed(KeyCode::KeyS)
        || keyboard.pressed(KeyCode::KeyD);
    let crouching =
        keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let sprinting = moving
        && !crouching
        && (keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight));
    let spread = if sprinting {
        WEAPON_SPRINTING_SPREAD
    } else if crouching {
        WEAPON_CROUCHING_SPREAD
    } else if moving {
        WEAPON_MOVING_SPREAD
    } else {
        weapon.definition.base_spread_milliradians as f32 / 1000.0
    } + recoil.angle * 0.35;
    let direction = spread_direction(forward, spread, &mut weapon.random_state);
    weapon.magazine -= 1;
    recoil.angle = (recoil.angle + WEAPON_RECOIL_PER_SHOT).min(WEAPON_MAX_RECOIL);
    signature.gunshot_remaining = ENEMY_GUNSHOT_MEMORY;
    play_sound(
        &mut commands,
        asset_server.as_deref(),
        AUDIO_PLAYER_GUNSHOT,
        PlaybackSettings::DESPAWN.with_volume(Volume::new(0.85)),
    );

    let hit = closest_target(
        origin,
        direction,
        colliders.iter_mut().map(|(entity, collider, enemy)| {
            (
                entity,
                *collider,
                enemy.as_ref().is_some_and(|enemy| enemy.health > 0),
            )
        }),
    );

    if let Some(target) = hit {
        if let Ok((_, _, Some(mut enemy))) = colliders.get_mut(target) {
            play_sound(
                &mut commands,
                asset_server.as_deref(),
                AUDIO_IMPACT_FLESH,
                PlaybackSettings::DESPAWN.with_volume(Volume::new(0.55)),
            );
            let eliminated = apply_damage(&mut enemy.health, weapon.definition.damage);
            if eliminated {
                stats.enemies_killed += 1;
                raid.record_event(RaidEventKind::EnemyKilled);
                commands.entity(target).insert(Visibility::Hidden);
                feedback.show("TARGET ELIMINATED");
            } else {
                feedback.show(format!("TARGET HIT: {} HP", enemy.health));
            }
        }
    } else {
        feedback.show("SHOT MISSED");
    }
}

pub(super) fn update_combat_feedback(time: Res<Time>, mut feedback: ResMut<CombatFeedback>) {
    if feedback.remaining <= 0.0 {
        return;
    }

    feedback.remaining = (feedback.remaining - time.delta_secs()).max(0.0);
    if feedback.remaining == 0.0 {
        feedback.message.clear();
    }
}

pub(super) fn spread_direction(forward: Vec3, spread_angle: f32, random_state: &mut u64) -> Vec3 {
    let radius = next_random_unit(random_state).sqrt() * spread_angle;
    let angle = next_random_unit(random_state) * std::f32::consts::TAU;
    let world_up = if forward.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let right = forward.cross(world_up).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    (forward + right * (radius * angle.cos()) + up * (radius * angle.sin())).normalize_or_zero()
}

pub(super) fn next_random_unit(state: &mut u64) -> f32 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    *state = value;
    (value.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / 16_777_216.0
}

pub(super) fn roll_hit_zone(random_state: &mut u64) -> BodyPart {
    body_part_from_roll(next_random_unit(random_state))
}

pub(super) fn body_part_from_roll(roll: f32) -> BodyPart {
    if roll < 0.08 {
        BodyPart::Head
    } else if roll < 0.48 {
        BodyPart::Thorax
    } else if roll < 0.68 {
        BodyPart::Abdomen
    } else if roll < 0.78 {
        BodyPart::LeftArm
    } else if roll < 0.88 {
        BodyPart::RightArm
    } else if roll < 0.94 {
        BodyPart::LeftLeg
    } else {
        BodyPart::RightLeg
    }
}

pub(super) fn closest_target<T: Copy>(
    origin: Vec3,
    direction: Vec3,
    colliders: impl Iterator<Item = (T, ArenaCollider, bool)>,
) -> Option<T> {
    let mut nearest_distance = f32::INFINITY;
    let mut nearest_target = None;

    for (entity, collider, is_target) in colliders {
        if let Some(distance) = ray_aabb_distance(origin, direction, collider) {
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_target = is_target.then_some(entity);
            }
        }
    }

    nearest_target
}

pub(super) fn ray_aabb_distance(
    origin: Vec3,
    direction: Vec3,
    collider: ArenaCollider,
) -> Option<f32> {
    let minimum = collider.center - collider.half_extents;
    let maximum = collider.center + collider.half_extents;
    let mut entry = 0.0_f32;
    let mut exit = f32::INFINITY;

    for axis in 0..3 {
        let ray_origin = origin[axis];
        let ray_direction = direction[axis];
        if ray_direction.abs() <= f32::EPSILON {
            if ray_origin < minimum[axis] || ray_origin > maximum[axis] {
                return None;
            }
            continue;
        }

        let inverse_direction = ray_direction.recip();
        let mut near = (minimum[axis] - ray_origin) * inverse_direction;
        let mut far = (maximum[axis] - ray_origin) * inverse_direction;
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }

        entry = entry.max(near);
        exit = exit.min(far);
        if entry > exit {
            return None;
        }
    }

    (exit >= 0.0).then_some(entry)
}
