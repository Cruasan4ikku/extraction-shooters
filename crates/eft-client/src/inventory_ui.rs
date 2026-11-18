use super::*;

pub(super) fn inventory_cell_at(window: &Window) -> Option<(u32, u32)> {
    let cursor = window.cursor_position()?;
    inventory_cell_at_position(Vec2::new(window.width(), window.height()), cursor)
}

pub(super) fn inventory_cell_at_position(window_size: Vec2, cursor: Vec2) -> Option<(u32, u32)> {
    let panel_left = (window_size.x - INVENTORY_PANEL_WIDTH) / 2.0;
    let panel_top = (window_size.y - INVENTORY_PANEL_HEIGHT) / 2.0;
    let grid_left = panel_left + INVENTORY_BODY_PANEL_WIDTH + INVENTORY_PANEL_GAP;
    let grid_top = panel_top
        + INVENTORY_HEADER_HEIGHT
        + (INVENTORY_CONTENT_HEIGHT - INVENTORY_GRID_PIXEL_HEIGHT) / 2.0;
    let local_x = cursor.x - grid_left;
    let local_y = cursor.y - grid_top;

    if local_x < 0.0
        || local_y < 0.0
        || local_x >= INVENTORY_GRID_PIXEL_WIDTH
        || local_y >= INVENTORY_GRID_PIXEL_HEIGHT
    {
        return None;
    }

    Some((
        (local_x / INVENTORY_SLOT_SIZE) as u32,
        (local_y / INVENTORY_SLOT_SIZE) as u32,
    ))
}

pub(super) fn update_inventory_ui(
    inventory: Res<InventoryState>,
    sync: Res<BackendSync>,
    mut cells: Query<
        (&InventoryCell, &mut BackgroundColor, &mut Text),
        Without<InventoryStatusText>,
    >,
    mut status: Query<&mut Text, With<InventoryStatusText>>,
) {
    let inventory_changed = inventory.is_changed();
    let sync_changed = sync.is_changed();
    if !inventory_changed && !sync_changed {
        return;
    }

    if inventory_changed {
        for (cell, mut background, mut text) in &mut cells {
            let item = inventory.layout.items.iter().find(|item| {
                cell.x >= item.x
                    && cell.x < item.x + item.width
                    && cell.y >= item.y
                    && cell.y < item.y + item.height
            });

            if let Some(item) = item {
                *background = item_color(&item.id).into();
                text.0 = if cell.x == item.x && cell.y == item.y {
                    item_label(&item.id).to_owned()
                } else {
                    String::new()
                };
            } else {
                *background = UI_SLOT.into();
                text.0.clear();
            }
        }
    }

    for mut text in &mut status {
        text.0 = format!("{}\n{}", inventory.notice, sync.status);
    }
}

pub(super) fn body_part_name(part: BodyPart) -> &'static str {
    match part {
        BodyPart::Head => "HEAD",
        BodyPart::Thorax => "THORAX",
        BodyPart::Abdomen => "ABDOMEN",
        BodyPart::LeftArm => "LEFT ARM",
        BodyPart::RightArm => "RIGHT ARM",
        BodyPart::LeftLeg => "LEFT LEG",
        BodyPart::RightLeg => "RIGHT LEG",
    }
}

pub(super) fn health_fraction(current: u32, maximum: u32) -> f32 {
    if maximum == 0 {
        0.0
    } else {
        (current as f32 / maximum as f32).clamp(0.0, 1.0)
    }
}

pub(super) fn update_vitals_ui(
    vitals: Res<PlayerVitals>,
    raid: Res<RaidState>,
    inventory: Res<InventoryState>,
    mut body_segments: Query<
        (
            &BodyPartSegment,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Without<StaminaHudBarFill>,
    >,
    mut hover_text: Query<
        &mut Text,
        (
            With<BodyHealthHoverText>,
            Without<InventoryHealthText>,
            Without<InventoryArmorText>,
            Without<StaminaHudText>,
        ),
    >,
    mut health_text: Query<
        &mut Text,
        (
            With<InventoryHealthText>,
            Without<BodyHealthHoverText>,
            Without<InventoryArmorText>,
            Without<StaminaHudText>,
        ),
    >,
    mut health_bars: Query<
        &mut Node,
        (
            With<InventoryHealthBarFill>,
            Without<InventoryArmorBarFill>,
            Without<StaminaHudBarFill>,
        ),
    >,
    mut armor_text: Query<
        &mut Text,
        (
            With<InventoryArmorText>,
            Without<BodyHealthHoverText>,
            Without<InventoryHealthText>,
            Without<StaminaHudText>,
        ),
    >,
    mut armor_bars: Query<
        &mut Node,
        (
            With<InventoryArmorBarFill>,
            Without<InventoryHealthBarFill>,
            Without<StaminaHudBarFill>,
        ),
    >,
    mut stamina_roots: Query<&mut Visibility, With<StaminaHudRoot>>,
    mut stamina_text: Query<
        &mut Text,
        (
            With<StaminaHudText>,
            Without<BodyHealthHoverText>,
            Without<InventoryHealthText>,
            Without<InventoryArmorText>,
        ),
    >,
    mut stamina_bars: Query<
        (&mut Node, &mut BackgroundColor),
        (
            With<StaminaHudBarFill>,
            Without<InventoryHealthBarFill>,
            Without<InventoryArmorBarFill>,
            Without<BodyPartSegment>,
        ),
    >,
) {
    let mut hovered_part = None;
    for (segment, interaction, mut background, mut border) in &mut body_segments {
        let zone = vitals.health.zones.get(&segment.0);
        let fraction = zone.map_or(0.0, |zone| health_fraction(zone.current, zone.maximum));
        *background = if fraction > 0.66 {
            Color::srgb(0.24, 0.62, 0.36).into()
        } else if fraction > 0.33 {
            Color::srgb(0.78, 0.58, 0.2).into()
        } else {
            Color::srgb(0.72, 0.22, 0.2).into()
        };
        let is_armored = vitals.armor.as_ref().is_some_and(|armor| {
            armor.durability > 0 && armor.definition.covered_zones.contains(&segment.0)
        });
        *border = BorderColor(if is_armored {
            Color::srgb(0.9, 0.73, 0.3)
        } else {
            Color::srgb(0.08, 0.1, 0.11)
        });
        if *interaction == Interaction::Hovered {
            hovered_part = Some(segment.0);
        }
    }

    for mut text in &mut hover_text {
        text.0 = hovered_part.map_or_else(
            || "Hover a body part".to_owned(),
            |part| {
                let zone = vitals.health.zones.get(&part);
                format!(
                    "{} {} / {}",
                    body_part_name(part),
                    zone.map_or(0, |zone| zone.current),
                    zone.map_or(0, |zone| zone.maximum)
                )
            },
        );
    }

    let current_health = vitals.health.total_current();
    let maximum_health = vitals.health.total_maximum();
    let health_percent = health_fraction(current_health, maximum_health) * 100.0;
    for mut text in &mut health_text {
        text.0 = format!("HP {current_health} / {maximum_health}");
    }
    for mut node in &mut health_bars {
        node.width = Val::Percent(health_percent);
    }

    let (armor_label, armor_percent) = vitals.armor.as_ref().map_or_else(
        || ("ARMOR NONE".to_owned(), 0.0),
        |armor| {
            (
                format!(
                    "ARMOR {} / {}",
                    armor.durability, armor.definition.maximum_durability
                ),
                health_fraction(armor.durability, armor.definition.maximum_durability) * 100.0,
            )
        },
    );
    for mut text in &mut armor_text {
        text.0 = armor_label.clone();
    }
    for mut node in &mut armor_bars {
        node.width = Val::Percent(armor_percent);
    }

    let stamina_percent = (vitals.stamina / MAX_STAMINA).clamp(0.0, 1.0) * 100.0;
    for mut visibility in &mut stamina_roots {
        *visibility = if raid.phase == RaidPhase::Active && !inventory.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut text in &mut stamina_text {
        text.0 = format!("STAMINA {:.0}%", vitals.stamina);
    }
    for (mut node, mut background) in &mut stamina_bars {
        node.width = Val::Percent(stamina_percent);
        *background = if stamina_percent > 50.0 {
            Color::srgb(0.35, 0.78, 0.42).into()
        } else if stamina_percent > 20.0 {
            Color::srgb(0.82, 0.64, 0.2).into()
        } else {
            Color::srgb(0.76, 0.24, 0.2).into()
        };
    }
}
