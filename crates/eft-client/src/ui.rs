use super::*;

pub(super) fn toggle_inventory(
    keyboard: Res<ButtonInput<KeyCode>>,
    raid: Res<RaidState>,
    sync: Res<BackendSync>,
    settings: Res<GameSettingsState>,
    mut inventory: ResMut<InventoryState>,
    mut windows: Query<&mut Window>,
    mut overlays: Query<&mut Visibility, With<InventoryOverlay>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyI)
        || settings.open
        || matches!(raid.phase, RaidPhase::Paused | RaidPhase::Debrief)
        || (!inventory.open
            && raid.phase == RaidPhase::Stash
            && (!sync.ready || !sync.save_enabled))
    {
        return;
    }

    inventory.open = !inventory.open;
    inventory.dragging = None;
    inventory.notice = if inventory.open {
        "Drag items; U uses a medkit; R rotates a dragged item.".to_owned()
    } else {
        "Inventory closed.".to_owned()
    };

    for mut window in &mut windows {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    }
    for mut visibility in &mut overlays {
        *visibility = if inventory.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(super) fn handle_game_menu(
    mut buttons: Query<
        (&Interaction, &GameMenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut menu_commands: ResMut<GameMenuCommands>,
    mut inventory: ResMut<InventoryState>,
    mut sync: ResMut<BackendSync>,
    mut settings: ResMut<GameSettingsState>,
    mut windows: Query<&mut Window>,
    mut inventory_overlays: Query<&mut Visibility, With<InventoryOverlay>>,
    mut exit: EventWriter<AppExit>,
) {
    for (interaction, button, mut background) in &mut buttons {
        *background = match *interaction {
            Interaction::Pressed => UI_BUTTON_PRESSED.into(),
            Interaction::Hovered => UI_BUTTON_HOVERED.into(),
            Interaction::None => UI_BUTTON.into(),
        };
        if *interaction != Interaction::Pressed {
            continue;
        }

        match button.0 {
            GameMenuAction::StartRaid => menu_commands.start_raid = true,
            GameMenuAction::OpenStash => {
                if !sync.ready || !sync.save_enabled {
                    continue;
                }
                inventory.open = true;
                inventory.dragging = None;
                inventory.notice =
                    "Drag items; U uses a medkit; R rotates a dragged item.".to_owned();
                for mut window in &mut windows {
                    window.cursor_options.grab_mode = CursorGrabMode::None;
                    window.cursor_options.visible = true;
                }
                for mut visibility in &mut inventory_overlays {
                    *visibility = Visibility::Visible;
                }
            }
            GameMenuAction::OpenSettings => settings.open = true,
            GameMenuAction::RetryProfile => {
                if sync.loading || !sync.retry_available {
                    continue;
                }
                if sync.ready {
                    queue_inventory_save(&mut sync, &inventory.layout);
                } else {
                    match sync.requests.send(BackendRequest::Load) {
                        Ok(()) => {
                            sync.loading = true;
                            sync.retry_available = false;
                            sync.status = "Retrying profile load...".to_owned();
                        }
                        Err(error) => {
                            sync.status = format!("Could not queue profile retry: {error}");
                        }
                    }
                }
            }
            GameMenuAction::Resume => menu_commands.resume = true,
            GameMenuAction::ReturnToStash => menu_commands.return_to_stash = true,
            GameMenuAction::Exit => {
                exit.send(AppExit::Success);
            }
        }
    }
}

pub(super) fn handle_settings_controls(
    mut buttons: Query<
        (&Interaction, &SettingsControl, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut settings: ResMut<GameSettingsState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut global_volume: ResMut<GlobalVolume>,
    mut windows: Query<&mut Window>,
) {
    if settings.open && keyboard.just_pressed(KeyCode::Escape) {
        settings.open = false;
    }

    for (interaction, control, mut background) in &mut buttons {
        *background = match *interaction {
            Interaction::Pressed => UI_BUTTON_PRESSED.into(),
            Interaction::Hovered => UI_BUTTON_HOVERED.into(),
            Interaction::None => UI_BUTTON.into(),
        };
        if *interaction != Interaction::Pressed || !settings.open {
            continue;
        }

        match control.0 {
            SettingsAction::Back => {
                settings.open = false;
                continue;
            }
            SettingsAction::ResolutionDown => settings.values.cycle_resolution(-1),
            SettingsAction::ResolutionUp => settings.values.cycle_resolution(1),
            SettingsAction::VolumeDown => settings.values.adjust_master_volume(-1),
            SettingsAction::VolumeUp => settings.values.adjust_master_volume(1),
            SettingsAction::SensitivityDown => settings.values.adjust_mouse_sensitivity(-1),
            SettingsAction::SensitivityUp => settings.values.adjust_mouse_sensitivity(1),
            SettingsAction::Reset => settings.reset_to_defaults(),
        }

        if !matches!(control.0, SettingsAction::Reset) {
            settings.save();
        }
        global_volume.volume = Volume::new(settings.values.master_volume);
        let (width, height) = settings.values.resolution.dimensions();
        for mut window in &mut windows {
            window.resolution.set(width, height);
        }
    }
}

pub(super) fn update_settings_ui(
    settings: Res<GameSettingsState>,
    mut overlays: Query<&mut Visibility, With<SettingsOverlay>>,
    mut values: Query<(&SettingsValueText, &mut Text), Without<SettingsNoticeText>>,
    mut notices: Query<&mut Text, With<SettingsNoticeText>>,
) {
    for mut visibility in &mut overlays {
        *visibility = if settings.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (field, mut text) in &mut values {
        text.0 = match field.0 {
            SettingsField::Resolution => settings.values.resolution.label().to_owned(),
            SettingsField::MasterVolume => {
                format!("{:.0}%", settings.values.master_volume * 100.0)
            }
            SettingsField::MouseSensitivity => format!(
                "{:.2}x",
                settings.values.mouse_sensitivity / DEFAULT_MOUSE_SENSITIVITY
            ),
        };
    }
    for mut text in &mut notices {
        text.0.clone_from(&settings.notice);
    }
}

pub(super) fn handle_inventory_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    mut inventory: ResMut<InventoryState>,
    mut vitals: ResMut<PlayerVitals>,
) {
    if !inventory.open {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        if let Some(item_id) = inventory.dragging.clone() {
            if let Some(layout) = try_rotate_inventory_item(&inventory.layout, &item_id) {
                inventory.layout = layout;
                inventory.notice = format!("Rotated {item_id}.");
            } else {
                inventory.notice =
                    "Cannot rotate here: the item would overlap or leave the grid.".to_owned();
            }
        } else {
            inventory.notice = "Pick up an item before rotating it.".to_owned();
        }
    }

    if keyboard.just_pressed(KeyCode::KeyU) {
        if vitals.health.is_dead() {
            inventory.notice = "Cannot use a medkit after a fatal wound.".to_owned();
        } else if vitals.health.is_fully_healed() {
            inventory.notice = "Health is already full.".to_owned();
        } else if let Some((layout, health)) =
            try_use_medkit(&inventory.layout, &vitals.health, MEDKIT_HEAL_AMOUNT)
        {
            inventory.layout = layout;
            vitals.health = health;
            inventory.notice = format!(
                "Used a medkit. Health: {} / {}.",
                vitals.health.total_current(),
                vitals.health.total_maximum()
            );
        } else {
            inventory.notice = "No medkit in the inventory.".to_owned();
        }
    }

    let Some(window) = windows.iter().next() else {
        return;
    };

    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let Some((x, y)) = inventory_cell_at(window) {
            inventory.dragging = inventory
                .layout
                .items
                .iter()
                .find(|item| {
                    x >= item.x
                        && x < item.x + item.width
                        && y >= item.y
                        && y < item.y + item.height
                })
                .map(|item| item.id.clone());
            if let Some(item) = &inventory.dragging {
                inventory.notice = format!("Dragging {item}.");
            }
        }
    }

    if mouse_buttons.just_released(MouseButton::Left) {
        if let Some(item_id) = inventory.dragging.take() {
            if let Some((x, y)) = inventory_cell_at(window) {
                if let Some(layout) = try_move_inventory_item(&inventory.layout, &item_id, x, y) {
                    inventory.layout = layout;
                    inventory.notice = format!("Moved {item_id}.");
                } else {
                    inventory.notice =
                        "Invalid placement: items cannot overlap or leave the grid.".to_owned();
                }
            } else {
                inventory.notice = "Move cancelled: drop inside the grid.".to_owned();
            }
        }
    }
}

pub(super) fn update_game_menu(
    raid: Res<RaidState>,
    inventory: Res<InventoryState>,
    sync: Res<BackendSync>,
    settings: Res<GameSettingsState>,
    mut overlays: Query<&mut Visibility, (With<GameMenuOverlay>, Without<GameMenuButton>)>,
    mut titles: Query<&mut Text, With<GameMenuTitle>>,
    mut statuses: Query<&mut Text, (With<GameMenuStatus>, Without<GameMenuTitle>)>,
    mut buttons: Query<(&GameMenuButton, &mut Visibility, &mut Node), Without<GameMenuOverlay>>,
) {
    let menu_visible = matches!(raid.phase, RaidPhase::Paused | RaidPhase::Debrief)
        || (raid.phase == RaidPhase::Stash && raid.menu_open);
    let menu_visible = menu_visible && !inventory.open && !settings.open;
    for mut visibility in &mut overlays {
        *visibility = if menu_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    let profile_status = if sync.loading {
        "PROFILE LOADING"
    } else if sync.ready && sync.save_enabled {
        "PROFILE READY"
    } else if sync.ready && sync.retry_available {
        "PROFILE SAVE FAILED"
    } else if sync.ready {
        "SAVING PROFILE"
    } else if sync.retry_available {
        "PROFILE LOAD FAILED"
    } else {
        "PROFILE NOT READY"
    };
    let (title, status) = match raid.phase {
        RaidPhase::Stash => (
            "STASH",
            format!(
                "{profile_status}\n{}\n{}\n\n{}",
                sync.status,
                raid_loadout_summary(&inventory.layout),
                raid.status
            ),
        ),
        RaidPhase::Active => ("", String::new()),
        RaidPhase::Paused => (
            "PAUSED",
            "The raid is paused.\nYour last saved stash remains safe if you exit now.".to_owned(),
        ),
        RaidPhase::Debrief => ("RAID DEBRIEF", raid.status.clone()),
    };

    for mut text in &mut titles {
        text.0 = title.to_owned();
    }
    for mut text in &mut statuses {
        text.0 = status.clone();
    }
    for (button, mut visibility, mut node) in &mut buttons {
        let visible = match button.0 {
            GameMenuAction::StartRaid | GameMenuAction::OpenStash => {
                raid.phase == RaidPhase::Stash && sync.ready && sync.save_enabled
            }
            GameMenuAction::OpenSettings => raid.phase != RaidPhase::Active,
            GameMenuAction::RetryProfile => {
                raid.phase == RaidPhase::Stash && sync.retry_available && !sync.loading
            }
            GameMenuAction::Resume => raid.phase == RaidPhase::Paused,
            GameMenuAction::ReturnToStash => raid.phase == RaidPhase::Debrief,
            GameMenuAction::Exit => true,
        };
        let show_button = menu_visible && visible;
        *visibility = if show_button {
            node.display = Display::Flex;
            Visibility::Visible
        } else {
            node.display = Display::None;
            Visibility::Hidden
        };
    }
}

pub(super) fn raid_loadout_summary(layout: &InventoryLayout) -> String {
    let carried_items = [
        ("medkit", "Medkit"),
        ("ammo", "Ammo"),
        ("armor", "Armor vest"),
    ]
    .into_iter()
    .filter_map(|(id, label)| {
        layout
            .items
            .iter()
            .any(|item| item.id == id)
            .then_some(label)
    })
    .collect::<Vec<_>>();
    let carried_items = if carried_items.is_empty() {
        "None from stash".to_owned()
    } else {
        carried_items.join(", ")
    };

    format!(
        "STARTING KIT\nWeapon: Starter Rifle\nFrom stash: {carried_items}\nExtraction secures found loot; KIA loses carried items."
    )
}

pub(super) fn item_label(id: &str) -> &'static str {
    match id {
        "medkit" => "MED",
        "armor" => "ARM",
        "ammo" => "MAG",
        "bandage" => "BAND",
        "ration" => "FOOD",
        "water" => "WTR",
        id if id.starts_with("ration-") => "FOOD",
        id if id.starts_with("battery-") => "BAT",
        id if id.starts_with("electronics-") => "ELEC",
        _ => "",
    }
}

pub(super) fn item_color(id: &str) -> Color {
    match id {
        "medkit" => Color::srgb(0.56, 0.18, 0.17),
        "armor" => Color::srgb(0.24, 0.38, 0.5),
        "ammo" => Color::srgb(0.38, 0.42, 0.45),
        "bandage" => Color::srgb(0.4, 0.43, 0.32),
        "ration" => Color::srgb(0.28, 0.4, 0.27),
        "water" => Color::srgb(0.2, 0.38, 0.48),
        id if id.starts_with("ration-") => Color::srgb(0.28, 0.4, 0.27),
        id if id.starts_with("battery-") => Color::srgb(0.75, 0.62, 0.22),
        id if id.starts_with("electronics-") => Color::srgb(0.3, 0.48, 0.68),
        _ => Color::srgb(0.3, 0.32, 0.34),
    }
}

pub(super) fn update_hud(
    weapon: Res<WeaponState>,
    vitals: Res<PlayerVitals>,
    raid: Res<RaidState>,
    inventory: Res<InventoryState>,
    feedback: Res<CombatFeedback>,
    player: Query<&Transform, With<PlayerCamera>>,
    extraction_points: Query<(&Transform, &ExtractionPoint)>,
    mut weapon_visuals: Query<&mut Visibility, (With<RaidWeaponVisual>, Without<HudStats>)>,
    mut hud: Query<(&mut Text, &mut Visibility), (With<HudStats>, Without<RaidWeaponVisual>)>,
) {
    if !weapon.is_changed()
        && !vitals.is_changed()
        && !raid.is_changed()
        && !inventory.is_changed()
        && !feedback.is_changed()
    {
        return;
    }

    let shelter_roaming = raid.phase == RaidPhase::Stash && !raid.menu_open && !inventory.open;
    for mut visibility in &mut weapon_visuals {
        *visibility = if raid.phase == RaidPhase::Active && !inventory.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (mut text, mut visibility) in &mut hud {
        if shelter_roaming {
            text.0 =
                "SHELTER\nWASD: MOVE | CLICK + MOUSE: LOOK\nI: OPEN STASH | ESC: MENU".to_owned();
            *visibility = Visibility::Visible;
            continue;
        }
        *visibility = if raid.phase == RaidPhase::Active && !inventory.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let armor_status = vitals.armor.as_ref().map_or_else(
            || "NONE".to_owned(),
            |armor| {
                format!(
                    "{}/{}",
                    armor.durability, armor.definition.maximum_durability
                )
            },
        );
        let feedback_line = if feedback.remaining > 0.0 {
            format!("{}\n", feedback.message)
        } else {
            String::new()
        };
        let extraction_hint = if raid.phase == RaidPhase::Active {
            player.iter().next().and_then(|player| {
                extraction_points
                    .iter()
                    .find(|(transform, point)| {
                        transform.translation.distance(player.translation) <= point.radius
                    })
                    .map(|(_, point)| format!("EXTRACTION AVAILABLE: {} [F]", point.label))
            })
        } else {
            None
        }
        .map_or_else(String::new, |hint| format!("{hint}\n"));
        text.0 = format!(
            "{}{}{}HP {} / {}\nARMOR {}\nAMMO {} | RESERVE {}",
            feedback_line,
            raid.status,
            extraction_hint,
            vitals.health.total_current(),
            vitals.health.total_maximum(),
            armor_status,
            weapon.magazine,
            weapon.reserve,
        );
    }
}
