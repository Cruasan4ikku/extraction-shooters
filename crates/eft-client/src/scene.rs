use super::*;

pub(super) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let floor_material = materials.add(Color::srgb(0.22, 0.25, 0.23));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            ARENA_HALF_SIZE * 2.0,
            0.2,
            ARENA_HALF_SIZE * 2.0,
        ))),
        MeshMaterial3d(floor_material),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));

    let wall_material = materials.add(Color::srgb(0.32, 0.34, 0.36));
    let wall_height = 3.0;
    let wall_thickness = 0.25;
    let wall_specs = [
        (
            Vec3::new(0.0, wall_height / 2.0, -ARENA_HALF_SIZE),
            Vec3::new(ARENA_HALF_SIZE * 2.0, wall_height, wall_thickness),
        ),
        (
            Vec3::new(0.0, wall_height / 2.0, ARENA_HALF_SIZE),
            Vec3::new(ARENA_HALF_SIZE * 2.0, wall_height, wall_thickness),
        ),
        (
            Vec3::new(-ARENA_HALF_SIZE, wall_height / 2.0, 0.0),
            Vec3::new(wall_thickness, wall_height, ARENA_HALF_SIZE * 2.0),
        ),
        (
            Vec3::new(ARENA_HALF_SIZE, wall_height / 2.0, 0.0),
            Vec3::new(wall_thickness, wall_height, ARENA_HALF_SIZE * 2.0),
        ),
    ];

    for (position, size) in wall_specs {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_translation(position),
            ArenaCollider {
                center: position,
                half_extents: size / 2.0,
            },
        ));
    }

    let obstacle_material = materials.add(Color::srgb(0.42, 0.32, 0.22));
    let obstacle_material_alt = materials.add(Color::srgb(0.28, 0.34, 0.36));
    for (index, (center, size)) in MAP_COVER_SPECS.into_iter().enumerate() {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(if index % 2 == 0 {
                obstacle_material.clone()
            } else {
                obstacle_material_alt.clone()
            }),
            Transform::from_translation(center),
            ArenaCollider {
                center,
                half_extents: size / 2.0,
            },
        ));
    }

    let container_detail_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.2, 0.19),
        metallic: 0.45,
        perceptual_roughness: 0.72,
        ..default()
    });
    let container_rib_mesh = meshes.add(Cuboid::new(0.08, 1.56, 0.06));
    for x in [-1.55, -0.78, 0.0, 0.78, 1.55] {
        commands.spawn((
            Mesh3d(container_rib_mesh.clone()),
            MeshMaterial3d(container_detail_material.clone()),
            Transform::from_xyz(x, 0.9, -3.04),
        ));
    }
    let container_rail_mesh = meshes.add(Cuboid::new(3.9, 0.1, 0.08));
    for y in [0.12, 1.68] {
        commands.spawn((
            Mesh3d(container_rail_mesh.clone()),
            MeshMaterial3d(container_detail_material.clone()),
            Transform::from_xyz(0.0, y, -3.04),
        ));
    }

    let route_mark_material = materials.add(Color::srgb(0.62, 0.49, 0.22));
    let route_mark_mesh = meshes.add(Cuboid::new(0.12, 0.02, 0.42));
    for x in [-3.1, 3.1] {
        for z in [-4.0, -5.0, -6.0, -7.0, -8.0, -9.0] {
            commands.spawn((
                Mesh3d(route_mark_mesh.clone()),
                MeshMaterial3d(route_mark_material.clone()),
                Transform::from_xyz(x, 0.015, z),
            ));
        }
    }

    let loot_mesh = meshes.add(Cuboid::new(0.4, 0.4, 0.4));
    let loot_material = materials.add(Color::srgb(0.75, 0.62, 0.22));
    let loot_specs = [
        (
            Vec3::new(-8.0, 0.25, -5.0),
            inventory_item("battery", 0, 0, 1, 1, 300),
            "battery",
        ),
        (
            Vec3::new(0.0, 0.25, -6.0),
            inventory_item("electronics", 0, 0, 1, 1, 180),
            "electronics",
        ),
        (
            Vec3::new(8.0, 0.25, -5.0),
            inventory_item("ration", 0, 0, 1, 1, 250),
            "ration",
        ),
    ];
    for (position, item, key) in loot_specs {
        commands.spawn((
            Mesh3d(loot_mesh.clone()),
            MeshMaterial3d(loot_material.clone()),
            Transform::from_translation(position),
            Visibility::Visible,
            RaidLoot {
                item,
                key,
                available: true,
            },
        ));
    }

    for (index, position) in EXTRACTION_POSITIONS.into_iter().enumerate() {
        let (label, color) = if index == 0 {
            ("WEST EXIT", Color::srgb(0.16, 0.55, 0.32))
        } else {
            ("EAST EXIT", Color::srgb(0.16, 0.46, 0.62))
        };
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.4, 0.12, 2.4))),
            MeshMaterial3d(materials.add(color)),
            Transform::from_translation(position),
            Visibility::Visible,
            ExtractionPoint { radius: 2.2, label },
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.18, 1.2, 0.18))),
            MeshMaterial3d(materials.add(color)),
            Transform::from_xyz(position.x, 0.72, position.z),
            Visibility::Visible,
        ));
    }

    let enemy_material = materials.add(Color::srgb(0.52, 0.12, 0.1));
    let enemy_size = Vec3::new(0.8, 1.8, 0.6);
    for (index, (center, patrol_points)) in [
        (Vec3::new(-8.0, 0.9, 3.0), patrol_route(-8.0, true)),
        (Vec3::new(8.0, 0.9, -5.0), patrol_route(8.0, false)),
    ]
    .into_iter()
    .enumerate()
    {
        let facing_target = Vec3::new(0.0, center.y, 5.0);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(enemy_size.x, enemy_size.y, enemy_size.z))),
            MeshMaterial3d(enemy_material.clone()),
            Transform::from_translation(center).looking_at(facing_target, Vec3::Y),
            Visibility::Hidden,
            ArenaCollider {
                center,
                half_extents: enemy_size / 2.0,
            },
            Enemy {
                health: ENEMY_HEALTH,
                attack_timer: Timer::from_seconds(ENEMY_ATTACK_INTERVAL, TimerMode::Repeating),
                detection: 0.0,
                last_known_player: None,
                memory_remaining: 0.0,
                shot_random_state: 0xA076_1D64_78BD_642F ^ index as u64,
                home_position: center,
                patrol_points,
                patrol_index: 0,
            },
        ));
    }

    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    spawn_shelter(&mut commands, &mut meshes, &mut materials);

    let weapon_metal = materials.add(Color::srgb(0.16, 0.18, 0.2));
    let weapon_grip = materials.add(Color::srgb(0.08, 0.09, 0.1));
    let receiver = meshes.add(Cuboid::new(0.12, 0.14, 0.42));
    let barrel = meshes.add(Cuboid::new(0.045, 0.045, 0.3));
    let grip = meshes.add(Cuboid::new(0.08, 0.18, 0.12));
    commands
        .spawn((
            Camera3d::default(),
            Transform::from_translation(SHELTER_BED_SPAWN)
                .with_rotation(Quat::from_rotation_y(SHELTER_BED_YAW)),
            PlayerCamera {
                yaw: SHELTER_BED_YAW,
                pitch: 0.0,
            },
        ))
        .with_children(|camera| {
            camera.spawn((
                Mesh3d(receiver),
                MeshMaterial3d(weapon_metal.clone()),
                Transform::from_xyz(0.28, -0.24, -0.52),
                RaidWeaponVisual,
                Visibility::Hidden,
            ));
            camera.spawn((
                Mesh3d(barrel),
                MeshMaterial3d(weapon_metal),
                Transform::from_xyz(0.28, -0.2, -0.84),
                RaidWeaponVisual,
                Visibility::Hidden,
            ));
            camera.spawn((
                Mesh3d(grip),
                MeshMaterial3d(weapon_grip),
                Transform::from_xyz(0.28, -0.38, -0.48),
                RaidWeaponVisual,
                Visibility::Hidden,
            ));
        });

    spawn_hud(&mut commands);
    spawn_inventory_ui(&mut commands);
    spawn_stamina_ui(&mut commands);
    spawn_game_menu(&mut commands);
    spawn_settings_ui(&mut commands);
}

fn spawn_shelter(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.19, 0.12),
        perceptual_roughness: 0.94,
        ..default()
    });
    let wood_trim = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.1, 0.06),
        perceptual_roughness: 0.9,
        ..default()
    });
    let floor = materials.add(Color::srgb(0.26, 0.23, 0.18));
    let fabric = materials.add(Color::srgb(0.36, 0.38, 0.32));
    let metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.39, 0.43, 0.42),
        metallic: 0.25,
        perceptual_roughness: 0.55,
        ..default()
    });
    let ceramic = materials.add(Color::srgb(0.62, 0.63, 0.58));
    let center = SHELTER_CENTER;

    shelter_block(
        commands,
        meshes,
        floor.clone(),
        center + Vec3::new(0.0, -0.1, 0.0),
        Vec3::new(12.0, 0.2, 10.0),
    );
    for (position, size) in [
        (center + Vec3::new(0.0, 1.5, 5.0), Vec3::new(12.0, 3.0, 0.2)),
        (
            center + Vec3::new(-6.0, 1.5, 0.0),
            Vec3::new(0.2, 3.0, 10.0),
        ),
        (center + Vec3::new(6.0, 1.5, 0.0), Vec3::new(0.2, 3.0, 10.0)),
        (
            center + Vec3::new(-3.5, 1.5, -5.0),
            Vec3::new(5.0, 3.0, 0.2),
        ),
        (center + Vec3::new(3.5, 1.5, -5.0), Vec3::new(5.0, 3.0, 0.2)),
        (
            center + Vec3::new(0.0, 2.9, -5.0),
            Vec3::new(2.0, 0.2, 0.24),
        ),
    ] {
        shelter_block(commands, meshes, wood.clone(), position, size);
    }

    shelter_block(
        commands,
        meshes,
        wood.clone(),
        center + Vec3::new(0.0, 3.12, 0.0),
        Vec3::new(12.3, 0.2, 10.3),
    );
    shelter_block(
        commands,
        meshes,
        wood.clone(),
        center + Vec3::new(0.0, -0.08, -6.0),
        Vec3::new(2.8, 0.16, 2.0),
    );

    shelter_block(
        commands,
        meshes,
        wood_trim.clone(),
        center + Vec3::new(-4.2, 0.22, -2.2),
        Vec3::new(2.2, 0.35, 1.5),
    );
    shelter_block(
        commands,
        meshes,
        fabric.clone(),
        center + Vec3::new(-4.2, 0.48, -2.2),
        Vec3::new(2.0, 0.2, 1.4),
    );
    shelter_block(
        commands,
        meshes,
        fabric.clone(),
        center + Vec3::new(-4.2, 0.63, -2.72),
        Vec3::new(0.75, 0.12, 0.42),
    );

    shelter_obstacle(
        commands,
        meshes,
        metal.clone(),
        center + Vec3::new(-4.25, 0.9, 0.95),
        Vec3::new(1.25, 1.8, 1.2),
    );
    shelter_block(
        commands,
        meshes,
        ceramic.clone(),
        center + Vec3::new(-3.59, 0.9, 0.95),
        Vec3::new(0.08, 1.56, 1.0),
    );
    shelter_block(
        commands,
        meshes,
        wood_trim.clone(),
        center + Vec3::new(-3.52, 0.9, 0.6),
        Vec3::new(0.08, 0.36, 0.08),
    );

    shelter_obstacle(
        commands,
        meshes,
        wood.clone(),
        center + Vec3::new(0.25, 0.82, 1.6),
        Vec3::new(1.9, 0.14, 1.15),
    );
    for x in [-0.5, 1.0] {
        for z in [1.15, 2.05] {
            shelter_block(
                commands,
                meshes,
                wood_trim.clone(),
                center + Vec3::new(x, 0.4, z),
                Vec3::new(0.12, 0.75, 0.12),
            );
        }
    }

    for (position, size) in [
        (center + Vec3::new(2.6, 0.32, 3.4), Vec3::new(1.2, 0.6, 1.0)),
        (
            center + Vec3::new(3.7, 0.28, 3.3),
            Vec3::new(0.9, 0.52, 0.8),
        ),
    ] {
        shelter_obstacle(commands, meshes, wood_trim.clone(), position, size);
    }

    shelter_block(
        commands,
        meshes,
        wood.clone(),
        center + Vec3::new(4.5, 1.0, -1.7),
        Vec3::new(3.0, 2.0, 0.12),
    );
    shelter_block(
        commands,
        meshes,
        wood.clone(),
        center + Vec3::new(5.85, 1.0, -0.1),
        Vec3::new(0.12, 2.0, 3.2),
    );
    shelter_obstacle(
        commands,
        meshes,
        ceramic.clone(),
        center + Vec3::new(4.3, 0.28, -1.2),
        Vec3::new(0.85, 0.48, 0.95),
    );
    shelter_block(
        commands,
        meshes,
        ceramic,
        center + Vec3::new(4.3, 0.72, -1.55),
        Vec3::new(0.82, 0.42, 0.35),
    );

    commands.spawn((
        PointLight {
            intensity: 500.0,
            range: 14.0,
            color: Color::srgb(1.0, 0.82, 0.62),
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(center + Vec3::new(0.0, 2.55, 0.0)),
    ));
}

fn shelter_block(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    position: Vec3,
    size: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
        MeshMaterial3d(material),
        Transform::from_translation(position),
    ));
}

fn shelter_obstacle(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    position: Vec3,
    size: Vec3,
) {
    shelter_block(commands, meshes, material, position, size);
    commands.spawn(ArenaCollider {
        center: position,
        half_extents: size / 2.0,
    });
}

fn spawn_hud(commands: &mut Commands) {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            position_type: PositionType::Absolute,
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                Node {
                    width: Val::Px(5.0),
                    height: Val::Px(5.0),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
            ));
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.06, 0.78)),
            BorderColor(UI_BORDER),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Visibility::Hidden,
                HudStats,
            ));
        });
}

fn spawn_game_menu(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.04, 0.62)),
            Visibility::Hidden,
            GameMenuOverlay,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(90.0),
                    max_width: Val::Px(420.0),
                    padding: UiRect::all(Val::Px(28.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.075, 0.08, 0.97)),
                BorderColor(UI_BORDER),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(3.0),
                        margin: UiRect::bottom(Val::Px(14.0)),
                        ..default()
                    },
                    BackgroundColor(UI_ACCENT),
                ));
                panel.spawn((
                    Text::new("EXTRACTION"),
                    TextFont {
                        font_size: 36.0,
                        ..default()
                    },
                    TextColor(UI_ACCENT),
                    GameMenuTitle,
                ));
                panel.spawn((
                    Text::new(""),
                    Node {
                        margin: UiRect::vertical(Val::Px(16.0)),
                        ..default()
                    },
                    TextFont {
                        font_size: 17.0,
                        ..default()
                    },
                    TextColor(UI_TEXT_MUTED),
                    GameMenuStatus,
                ));

                for (label, action) in [
                    ("START RAID", GameMenuAction::StartRaid),
                    ("OPEN STASH", GameMenuAction::OpenStash),
                    ("SETTINGS", GameMenuAction::OpenSettings),
                    ("RETRY SYNC", GameMenuAction::RetryProfile),
                    ("RESUME RAID", GameMenuAction::Resume),
                    ("RETURN TO STASH", GameMenuAction::ReturnToStash),
                    ("EXIT", GameMenuAction::Exit),
                ] {
                    panel
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                max_width: Val::Px(300.0),
                                height: Val::Px(48.0),
                                margin: UiRect::vertical(Val::Px(4.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                display: Display::None,
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(UI_BUTTON),
                            BorderColor(UI_BORDER),
                            Visibility::Hidden,
                            Interaction::None,
                            GameMenuButton(action),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(label),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(UI_TEXT),
                            ));
                        });
                }
            });
        });
}

fn settings_control_button(width: f32, action: SettingsAction) -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Px(width),
            height: Val::Px(36.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(UI_BUTTON),
        BorderColor(UI_BORDER),
        Interaction::None,
        SettingsControl(action),
    )
}

fn spawn_settings_ui(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.04, 0.82)),
            Visibility::Hidden,
            SettingsOverlay,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(90.0),
                    max_width: Val::Px(520.0),
                    padding: UiRect::all(Val::Px(24.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(8.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(UI_PANEL),
                BorderColor(UI_BORDER),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(3.0),
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(UI_ACCENT),
                ));
                panel.spawn((
                    Text::new("SETTINGS"),
                    TextFont {
                        font_size: 30.0,
                        ..default()
                    },
                    TextColor(UI_ACCENT),
                ));

                for (label, field, decrease, increase) in [
                    (
                        "DISPLAY RESOLUTION",
                        SettingsField::Resolution,
                        SettingsAction::ResolutionDown,
                        SettingsAction::ResolutionUp,
                    ),
                    (
                        "MASTER VOLUME",
                        SettingsField::MasterVolume,
                        SettingsAction::VolumeDown,
                        SettingsAction::VolumeUp,
                    ),
                    (
                        "MOUSE SENSITIVITY",
                        SettingsField::MouseSensitivity,
                        SettingsAction::SensitivityDown,
                        SettingsAction::SensitivityUp,
                    ),
                ] {
                    panel
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(48.0),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::horizontal(Val::Px(8.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(UI_PANEL_RAISED),
                            BorderColor(UI_BORDER),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                Node {
                                    width: Val::Percent(42.0),
                                    ..default()
                                },
                                Text::new(label),
                                TextFont {
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(UI_TEXT_MUTED),
                            ));
                            row.spawn((
                                Node {
                                    width: Val::Px(100.0),
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                Text::new(""),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(UI_ACCENT),
                                SettingsValueText(field),
                            ));
                            for (symbol, action) in [("-", decrease), ("+", increase)] {
                                row.spawn(settings_control_button(36.0, action))
                                    .with_children(|button| {
                                        button.spawn((
                                            Text::new(symbol),
                                            TextFont {
                                                font_size: 18.0,
                                                ..default()
                                            },
                                            TextColor(UI_TEXT),
                                        ));
                                    });
                            }
                        });
                }

                panel.spawn((
                    Text::new(""),
                    Node {
                        width: Val::Percent(100.0),
                        margin: UiRect::vertical(Val::Px(4.0)),
                        ..default()
                    },
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(UI_TEXT_MUTED),
                    SettingsNoticeText,
                ));
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    })
                    .with_children(|actions| {
                        for (label, width, action) in [
                            ("RESET TO DEFAULTS", 170.0, SettingsAction::Reset),
                            ("BACK", 100.0, SettingsAction::Back),
                        ] {
                            actions
                                .spawn(settings_control_button(width, action))
                                .with_children(|button| {
                                    button.spawn((
                                        Text::new(label),
                                        TextFont {
                                            font_size: 13.0,
                                            ..default()
                                        },
                                        TextColor(UI_TEXT),
                                    ));
                                });
                        }
                    });
            });
        });
}

fn spawn_inventory_ui(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.04, 0.82)),
            Visibility::Hidden,
            InventoryOverlay,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(INVENTORY_PANEL_WIDTH),
                    height: Val::Px(INVENTORY_PANEL_HEIGHT),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(UI_PANEL),
                BorderColor(UI_BORDER),
            ))
            .with_children(|panel| {
                panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(INVENTORY_HEADER_HEIGHT),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::horizontal(Val::Px(12.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(UI_PANEL_HEADER),
                        BorderColor(UI_BORDER),
                    ))
                    .with_children(|header| {
                        header.spawn((
                            Text::new("INVENTORY"),
                            TextFont {
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(UI_ACCENT),
                        ));
                        header.spawn((
                            Text::new("I - close | U - use medkit | R - rotate"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(UI_TEXT_MUTED),
                        ));
                    });

                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(INVENTORY_CONTENT_HEIGHT),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|content| {
                        content
                            .spawn((
                                Node {
                                    width: Val::Px(INVENTORY_BODY_PANEL_WIDTH),
                                    height: Val::Px(INVENTORY_CONTENT_HEIGHT),
                                    margin: UiRect::right(Val::Px(INVENTORY_PANEL_GAP)),
                                    padding: UiRect::all(Val::Px(4.0)),
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(UI_PANEL_RAISED),
                                BorderColor(UI_BORDER),
                            ))
                            .with_children(|body| {
                                body.spawn((
                                    Text::new("BODY"),
                                    TextFont {
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(UI_ACCENT),
                                ));
                                body.spawn(Node {
                                    width: Val::Px(112.0),
                                    height: Val::Px(120.0),
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                })
                                .with_children(|figure| {
                                    figure
                                        .spawn(Node {
                                            width: Val::Px(112.0),
                                            height: Val::Px(22.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        })
                                        .with_children(|row| {
                                            row.spawn(body_segment(BodyPart::Head, 22.0, 20.0));
                                        });
                                    figure
                                        .spawn(Node {
                                            width: Val::Px(112.0),
                                            height: Val::Px(36.0),
                                            flex_direction: FlexDirection::Row,
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        })
                                        .with_children(|row| {
                                            row.spawn(body_segment(BodyPart::LeftArm, 18.0, 34.0));
                                            row.spawn(body_segment(BodyPart::Thorax, 32.0, 34.0));
                                            row.spawn(body_segment(BodyPart::RightArm, 18.0, 34.0));
                                        });
                                    figure
                                        .spawn(Node {
                                            width: Val::Px(112.0),
                                            height: Val::Px(26.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        })
                                        .with_children(|row| {
                                            row.spawn(body_segment(BodyPart::Abdomen, 30.0, 24.0));
                                        });
                                    figure
                                        .spawn(Node {
                                            width: Val::Px(112.0),
                                            height: Val::Px(36.0),
                                            flex_direction: FlexDirection::Row,
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        })
                                        .with_children(|row| {
                                            row.spawn(body_segment(BodyPart::LeftLeg, 21.0, 34.0));
                                            row.spawn(body_segment(BodyPart::RightLeg, 21.0, 34.0));
                                        });
                                });
                                body.spawn((
                                    Text::new("Hover a body part"),
                                    TextFont {
                                        font_size: 9.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.78, 0.82, 0.85)),
                                    BodyHealthHoverText,
                                ));
                                body.spawn((
                                    Text::new("HP 440 / 440"),
                                    TextFont {
                                        font_size: 11.0,
                                        ..default()
                                    },
                                    TextColor(UI_TEXT),
                                    InventoryHealthText,
                                ));
                                body.spawn((
                                    Node {
                                        width: Val::Px(108.0),
                                        height: Val::Px(8.0),
                                        ..default()
                                    },
                                    BackgroundColor(UI_BAR_TRACK),
                                ))
                                .with_children(|bar| {
                                    bar.spawn((
                                        Node {
                                            width: Val::Percent(100.0),
                                            height: Val::Percent(100.0),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.25, 0.72, 0.38)),
                                        InventoryHealthBarFill,
                                    ));
                                });
                                body.spawn((
                                    Text::new("ARMOR NONE"),
                                    TextFont {
                                        font_size: 10.0,
                                        ..default()
                                    },
                                    TextColor(UI_ACCENT),
                                    InventoryArmorText,
                                ));
                                body.spawn((
                                    Node {
                                        width: Val::Px(108.0),
                                        height: Val::Px(6.0),
                                        ..default()
                                    },
                                    BackgroundColor(UI_BAR_TRACK),
                                ))
                                .with_children(|bar| {
                                    bar.spawn((
                                        Node {
                                            width: Val::Percent(0.0),
                                            height: Val::Percent(100.0),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.74, 0.59, 0.24)),
                                        InventoryArmorBarFill,
                                    ));
                                });
                            });

                        content
                            .spawn(Node {
                                width: Val::Px(INVENTORY_GRID_PIXEL_WIDTH),
                                height: Val::Px(INVENTORY_CONTENT_HEIGHT),
                                justify_content: JustifyContent::Center,
                                ..default()
                            })
                            .with_children(|grid_frame| {
                                grid_frame
                                    .spawn(Node {
                                        width: Val::Px(INVENTORY_GRID_PIXEL_WIDTH),
                                        height: Val::Px(INVENTORY_GRID_PIXEL_HEIGHT),
                                        flex_direction: FlexDirection::Column,
                                        ..default()
                                    })
                                    .with_children(|grid| {
                                        for y in 0..INVENTORY_GRID_HEIGHT {
                                            grid.spawn(Node {
                                                width: Val::Px(INVENTORY_GRID_PIXEL_WIDTH),
                                                height: Val::Px(INVENTORY_SLOT_SIZE),
                                                flex_direction: FlexDirection::Row,
                                                ..default()
                                            })
                                            .with_children(|row| {
                                                for x in 0..INVENTORY_GRID_WIDTH {
                                                    row.spawn((
                                                        Node {
                                                            width: Val::Px(INVENTORY_SLOT_SIZE),
                                                            height: Val::Px(INVENTORY_SLOT_SIZE),
                                                            border: UiRect::all(Val::Px(1.0)),
                                                            justify_content: JustifyContent::Center,
                                                            align_items: AlignItems::Center,
                                                            ..default()
                                                        },
                                                        BackgroundColor(UI_SLOT),
                                                        BorderColor(UI_SLOT_BORDER),
                                                        Text::new(""),
                                                        TextFont {
                                                            font_size: 9.0,
                                                            ..default()
                                                        },
                                                        TextColor(Color::WHITE),
                                                        InventoryCell { x, y },
                                                    ));
                                                }
                                            });
                                        }
                                    });
                            });
                    });

                panel.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(INVENTORY_FOOTER_HEIGHT),
                        padding: UiRect::all(Val::Px(8.0)),
                        ..default()
                    },
                    Text::new(""),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(UI_TEXT_MUTED),
                    InventoryStatusText,
                ));
            });
        });
}

fn body_segment(part: BodyPart, width: f32, height: f32) -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Px(width),
            height: Val::Px(height),
            margin: UiRect::all(Val::Px(1.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.25, 0.65, 0.37)),
        BorderColor(UI_BORDER),
        Interaction::None,
        BodyPartSegment(part),
    )
}

fn spawn_stamina_ui(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),
                bottom: Val::Px(20.0),
                width: Val::Px(220.0),
                height: Val::Px(42.0),
                padding: UiRect::all(Val::Px(7.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.06, 0.82)),
            BorderColor(UI_BORDER),
            Visibility::Hidden,
            StaminaHudRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("STAMINA 100%"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(UI_TEXT),
                StaminaHudText,
            ));
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(UI_BAR_TRACK),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.35, 0.78, 0.42)),
                    StaminaHudBarFill,
                ));
            });
        });
}
