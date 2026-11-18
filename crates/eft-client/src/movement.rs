use super::*;

pub(super) fn capture_cursor(
    mut windows: Query<&mut Window>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    inventory: Res<InventoryState>,
    raid: Res<RaidState>,
) {
    let Some(mut window) = windows.iter_mut().next() else {
        return;
    };

    let shelter_roaming = raid.phase == RaidPhase::Stash && !raid.menu_open;
    if (raid.phase != RaidPhase::Active && !shelter_roaming)
        || inventory.open
        || raid.menu_open
        || keyboard.just_pressed(KeyCode::Escape)
    {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
        return;
    }

    if mouse_buttons.just_pressed(MouseButton::Left) {
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
        window.cursor_options.visible = false;
    }
}

pub(super) fn look(
    mut mouse_motion: EventReader<MouseMotion>,
    windows: Query<&Window>,
    mut cameras: Query<(&mut Transform, &mut PlayerCamera)>,
    recoil: Res<WeaponRecoil>,
    raid: Res<RaidState>,
    inventory: Res<InventoryState>,
    settings: Res<GameSettingsState>,
) {
    let shelter_roaming = raid.phase == RaidPhase::Stash && !raid.menu_open;
    if (raid.phase != RaidPhase::Active && !shelter_roaming) || inventory.open {
        return;
    }

    let Some(window) = windows.iter().next() else {
        return;
    };
    if window.cursor_options.grab_mode != CursorGrabMode::Locked {
        return;
    }

    let delta = mouse_motion
        .read()
        .fold(Vec2::ZERO, |delta, motion| delta + motion.delta);
    let Some((mut transform, mut camera)) = cameras.iter_mut().next() else {
        return;
    };
    camera.yaw -= delta.x * settings.values.mouse_sensitivity;
    camera.pitch = (camera.pitch - delta.y * settings.values.mouse_sensitivity).clamp(-1.54, 1.54);
    transform.rotation =
        Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch + recoil.angle, 0.0);
}

pub(super) fn move_player(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut cameras: Query<(&mut Transform, &PlayerCamera)>,
    colliders: Query<&ArenaCollider>,
    inventory: Res<InventoryState>,
    raid: Res<RaidState>,
    mut vitals: ResMut<PlayerVitals>,
    mut signature: ResMut<PlayerSignature>,
) {
    let shelter_roaming = raid.phase == RaidPhase::Stash && !raid.menu_open;
    if (raid.phase != RaidPhase::Active && !shelter_roaming) || raid.menu_open {
        signature.moving = false;
        signature.sprinting = false;
        signature.crouching = false;
        return;
    }

    if inventory.open {
        signature.moving = false;
        signature.sprinting = false;
        signature.crouching = false;
        vitals.stamina = advance_stamina(vitals.stamina, false, time.delta_secs());
        return;
    }

    let Some((mut transform, camera)) = cameras.iter_mut().next() else {
        return;
    };
    let crouching =
        keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    signature.crouching = crouching;
    transform.translation.y = if crouching { 1.0 } else { 1.65 };

    let yaw_rotation = Quat::from_rotation_y(camera.yaw);
    let forward = yaw_rotation * Vec3::NEG_Z;
    let right = yaw_rotation * Vec3::X;
    let mut direction = Vec3::ZERO;

    if keyboard.pressed(KeyCode::KeyW) {
        direction += forward;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        direction -= forward;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        direction += right;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        direction -= right;
    }

    if direction != Vec3::ZERO {
        let sprint_key =
            keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        let sprinting = sprint_key && !crouching && vitals.stamina > 0.0;
        signature.moving = true;
        signature.sprinting = sprinting;
        vitals.stamina = advance_stamina(vitals.stamina, sprinting, time.delta_secs());
        let speed = if crouching {
            CROUCH_SPEED
        } else if sprinting {
            SPRINT_SPEED
        } else {
            MOVE_SPEED
        };
        let movement = direction.normalize() * speed * time.delta_secs();

        let next_position =
            move_on_clear_axes(transform.translation, movement, colliders.iter().copied());
        transform.translation = if shelter_roaming {
            constrain_shelter_position(next_position)
        } else {
            next_position
        };
    } else {
        signature.moving = false;
        signature.sprinting = false;
        vitals.stamina = advance_stamina(vitals.stamina, false, time.delta_secs());
    }
}

pub(super) fn constrain_shelter_position(mut position: Vec3) -> Vec3 {
    let center = SHELTER_CENTER;
    let front_boundary = center.z - 4.4;
    position.x = position.x.clamp(-5.4, 5.4);
    position.z = position.z.min(center.z + 4.4);

    if position.z < front_boundary {
        if position.x.abs() > 0.75 {
            position.z = front_boundary;
        } else {
            position.z = position.z.max(center.z - 7.2);
        }
    } else {
        position.z = position.z.max(front_boundary);
    }

    position
}

pub(super) fn advance_stamina(stamina: f32, sprinting: bool, delta_secs: f32) -> f32 {
    let change = if sprinting {
        -STAMINA_DRAIN_PER_SECOND
    } else {
        STAMINA_REGEN_PER_SECOND
    } * delta_secs;
    (stamina + change).clamp(0.0, MAX_STAMINA)
}

pub(super) fn footstep_interval(signature: &PlayerSignature) -> Option<f32> {
    if !signature.moving {
        None
    } else if signature.sprinting {
        Some(0.28)
    } else if signature.crouching {
        Some(0.62)
    } else {
        Some(0.42)
    }
}

pub(super) fn play_player_footsteps(
    time: Res<Time>,
    raid: Res<RaidState>,
    inventory: Res<InventoryState>,
    signature: Res<PlayerSignature>,
    asset_server: Option<Res<AssetServer>>,
    mut commands: Commands,
    mut step_timer: Local<f32>,
    mut step_index: Local<usize>,
) {
    if raid.phase != RaidPhase::Active || inventory.open {
        *step_timer = 0.0;
        return;
    }

    let Some(interval) = footstep_interval(&signature) else {
        *step_timer = 0.0;
        return;
    };

    *step_timer = (*step_timer - time.delta_secs()).max(0.0);
    if *step_timer == 0.0 {
        let volume = if signature.sprinting {
            0.8
        } else if signature.crouching {
            0.35
        } else {
            0.55
        };
        play_sound(
            &mut commands,
            asset_server.as_deref(),
            footstep_audio_path(*step_index),
            PlaybackSettings::DESPAWN.with_volume(Volume::new(volume)),
        );
        *step_index = (*step_index + 1) % AUDIO_FOOTSTEPS.len();
        *step_timer = interval;
    }
}

pub(super) fn move_on_clear_axes(
    position: Vec3,
    movement: Vec3,
    obstacles: impl Iterator<Item = ArenaCollider>,
) -> Vec3 {
    let obstacles = obstacles.collect::<Vec<_>>();
    let next_x = position + Vec3::X * movement.x;
    let mut result = position;
    if !is_blocked(next_x, obstacles.iter().copied()) {
        result.x = next_x.x;
    }

    let next_z = result + Vec3::Z * movement.z;
    if !is_blocked(next_z, obstacles.iter().copied()) {
        result.z = next_z.z;
    }
    result
}

pub(super) fn is_blocked(position: Vec3, colliders: impl Iterator<Item = ArenaCollider>) -> bool {
    colliders.into_iter().any(|collider| {
        let nearest_x = position.x.clamp(
            collider.center.x - collider.half_extents.x,
            collider.center.x + collider.half_extents.x,
        );
        let nearest_z = position.z.clamp(
            collider.center.z - collider.half_extents.z,
            collider.center.z + collider.half_extents.z,
        );
        let offset_x = position.x - nearest_x;
        let offset_z = position.z - nearest_z;

        offset_x * offset_x + offset_z * offset_z < PLAYER_RADIUS * PLAYER_RADIUS
    })
}
