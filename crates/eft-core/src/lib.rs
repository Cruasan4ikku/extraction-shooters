use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod gameplay;
mod save;

pub use gameplay::{
    apply_damage, apply_hit, heal, try_use_medkit, ArmorDamage, ArmorDefinition, ArmorState,
    BodyPart, DamageType, GameDataError, HealthDamage, HealthState, HitResult, ItemDefinition,
    ItemKind, RaidEvent, RaidEventKind, WeaponDefinition, MEDKIT_HEAL_AMOUNT, PLAYER_MAX_HEALTH,
};
pub use save::{InventorySave, InventorySaveError, INVENTORY_SAVE_FORMAT_VERSION};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryLayout {
    pub width: u32,
    pub height: u32,
    pub items: Vec<ItemPlacement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemPlacement {
    pub id: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub weight_grams: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durability: Option<u32>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InventoryError {
    #[error("grid width and height must be greater than zero")]
    InvalidGridDimensions,
    #[error("item ID must not be empty")]
    EmptyItemId,
    #[error("duplicate item ID: {0}")]
    DuplicateItemId(String),
    #[error("item dimensions must be greater than zero: {0}")]
    InvalidItemDimensions(String),
    #[error("item is outside the inventory grid: {0}")]
    OutOfBounds(String),
    #[error("items overlap: {0} and {1}")]
    Overlap(String, String),
    #[error("total inventory weight overflowed")]
    WeightOverflow,
}

pub fn validate_inventory(layout: &InventoryLayout) -> Result<u64, InventoryError> {
    if layout.width == 0 || layout.height == 0 {
        return Err(InventoryError::InvalidGridDimensions);
    }

    let mut ids = HashSet::with_capacity(layout.items.len());
    let mut total_weight_grams = 0_u64;
    let mut bounds = Vec::with_capacity(layout.items.len());

    for item in &layout.items {
        if item.id.trim().is_empty() {
            return Err(InventoryError::EmptyItemId);
        }
        if !ids.insert(item.id.as_str()) {
            return Err(InventoryError::DuplicateItemId(item.id.clone()));
        }
        if item.width == 0 || item.height == 0 {
            return Err(InventoryError::InvalidItemDimensions(item.id.clone()));
        }

        let right = item
            .x
            .checked_add(item.width)
            .filter(|right| *right <= layout.width);
        let bottom = item
            .y
            .checked_add(item.height)
            .filter(|bottom| *bottom <= layout.height);
        let (Some(right), Some(bottom)) = (right, bottom) else {
            return Err(InventoryError::OutOfBounds(item.id.clone()));
        };

        total_weight_grams = total_weight_grams
            .checked_add(u64::from(item.weight_grams))
            .ok_or(InventoryError::WeightOverflow)?;
        bounds.push((item, right, bottom));
    }

    for index in 0..bounds.len() {
        let (item, right, bottom) = bounds[index];
        for (other, other_right, other_bottom) in bounds.iter().skip(index + 1) {
            let overlaps = item.x < *other_right
                && other.x < right
                && item.y < *other_bottom
                && other.y < bottom;
            if overlaps {
                return Err(InventoryError::Overlap(item.id.clone(), other.id.clone()));
            }
        }
    }

    Ok(total_weight_grams)
}

pub fn try_move_inventory_item(
    layout: &InventoryLayout,
    item_id: &str,
    x: u32,
    y: u32,
) -> Option<InventoryLayout> {
    let mut candidate = layout.clone();
    let item = candidate.items.iter_mut().find(|item| item.id == item_id)?;
    item.x = x;
    item.y = y;
    validate_inventory(&candidate).ok()?;
    Some(candidate)
}

pub fn try_add_inventory_item(
    layout: &InventoryLayout,
    item: &ItemPlacement,
) -> Option<InventoryLayout> {
    if layout.width == 0
        || layout.height == 0
        || item.width == 0
        || item.height == 0
        || item.width > layout.width
        || item.height > layout.height
    {
        return None;
    }

    for y in 0..=layout.height - item.height {
        for x in 0..=layout.width - item.width {
            let mut candidate = layout.clone();
            let mut placed_item = item.clone();
            placed_item.x = x;
            placed_item.y = y;
            candidate.items.push(placed_item);
            if validate_inventory(&candidate).is_ok() {
                return Some(candidate);
            }
        }
    }

    None
}

pub fn merge_raid_inventory(
    stash: &InventoryLayout,
    carried: &InventoryLayout,
) -> Option<InventoryLayout> {
    let mut merged = stash.clone();
    for item in &carried.items {
        merged = try_add_inventory_item(&merged, item)?;
    }
    Some(merged)
}

pub fn next_raid_number(
    previous: u64,
    stash: &InventoryLayout,
    loot_templates: &[ItemPlacement],
) -> Option<u64> {
    let mut candidate = previous.checked_add(1)?;
    loop {
        let collides = loot_templates.iter().any(|item| {
            let candidate_id = format!("{}-{candidate}", item.id);
            stash.items.iter().any(|stored| stored.id == candidate_id)
        });
        if !collides {
            return Some(candidate);
        }
        candidate = candidate.checked_add(1)?;
    }
}

pub fn prepare_raid_loadout(
    stash: &InventoryLayout,
    loot_templates: &[ItemPlacement],
    raid_number: u64,
) -> Option<(InventoryLayout, InventoryLayout)> {
    let mut remaining_stash = stash.clone();
    let mut loadout = InventoryLayout {
        width: stash.width,
        height: stash.height,
        items: Vec::new(),
    };
    for item_id in ["medkit", "ammo", "armor"] {
        if let Some(index) = remaining_stash
            .items
            .iter()
            .position(|item| item.id == item_id)
        {
            let item = remaining_stash.items.remove(index);
            loadout = try_add_inventory_item(&loadout, &item)?;
        }
    }

    let mut projected_stash = remaining_stash.clone();
    for item in loadout.items.iter().chain(loot_templates) {
        let mut returning_item = item.clone();
        if loot_templates.iter().any(|loot| loot.id == item.id) {
            returning_item.id = format!("{}-{raid_number}", item.id);
        }
        projected_stash = try_add_inventory_item(&projected_stash, &returning_item)?;
    }

    Some((remaining_stash, loadout))
}

pub fn try_rotate_inventory_item(
    layout: &InventoryLayout,
    item_id: &str,
) -> Option<InventoryLayout> {
    let mut candidate = layout.clone();
    let item = candidate.items.iter_mut().find(|item| item.id == item_id)?;
    std::mem::swap(&mut item.width, &mut item.height);
    validate_inventory(&candidate).ok()?;
    Some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, x: u32, y: u32, width: u32, height: u32) -> ItemPlacement {
        ItemPlacement {
            id: id.to_owned(),
            x,
            y,
            width,
            height,
            weight_grams: 125,
            durability: None,
        }
    }

    fn layout(items: Vec<ItemPlacement>) -> InventoryLayout {
        InventoryLayout {
            width: 4,
            height: 3,
            items,
        }
    }

    #[test]
    fn accepts_non_overlapping_items_and_sums_weight() {
        let inventory = layout(vec![item("a", 0, 0, 2, 1), item("b", 2, 0, 2, 2)]);

        assert_eq!(validate_inventory(&inventory), Ok(250));
    }

    #[test]
    fn rejects_overlapping_items() {
        let inventory = layout(vec![item("a", 0, 0, 2, 2), item("b", 1, 1, 2, 2)]);

        assert_eq!(
            validate_inventory(&inventory),
            Err(InventoryError::Overlap("a".to_owned(), "b".to_owned()))
        );
    }

    #[test]
    fn rejects_items_outside_grid_and_zero_sized_items() {
        assert_eq!(
            validate_inventory(&layout(vec![item("outside", 3, 0, 2, 1)])),
            Err(InventoryError::OutOfBounds("outside".to_owned()))
        );
        assert_eq!(
            validate_inventory(&layout(vec![item("zero", 0, 0, 0, 1)])),
            Err(InventoryError::InvalidItemDimensions("zero".to_owned()))
        );
    }

    #[test]
    fn rejects_duplicate_ids_and_invalid_grid_dimensions() {
        assert_eq!(
            validate_inventory(&layout(vec![
                item("same", 0, 0, 1, 1),
                item("same", 1, 0, 1, 1)
            ])),
            Err(InventoryError::DuplicateItemId("same".to_owned()))
        );

        let invalid_grid = InventoryLayout {
            width: 0,
            height: 3,
            items: vec![],
        };
        assert_eq!(
            validate_inventory(&invalid_grid),
            Err(InventoryError::InvalidGridDimensions)
        );
    }

    #[test]
    fn inventory_rules_move_rotate_merge_and_reserve_raid_items_without_bevy() {
        let stash = InventoryLayout {
            width: 4,
            height: 3,
            items: vec![
                ItemPlacement {
                    id: "medkit".to_owned(),
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    weight_grams: 100,
                    durability: None,
                },
                ItemPlacement {
                    id: "ammo".to_owned(),
                    x: 1,
                    y: 0,
                    width: 1,
                    height: 1,
                    weight_grams: 50,
                    durability: None,
                },
            ],
        };
        let moved = try_move_inventory_item(&stash, "medkit", 3, 2).unwrap();
        assert_eq!((moved.items[0].x, moved.items[0].y), (3, 2));
        assert!(try_rotate_inventory_item(&stash, "medkit").is_some());

        let loot = vec![ItemPlacement {
            id: "battery".to_owned(),
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            weight_grams: 300,
            durability: None,
        }];
        let (remaining, loadout) = prepare_raid_loadout(&stash, &loot, 1).unwrap();
        assert!(remaining.items.is_empty());
        assert_eq!(loadout.items.len(), 2);

        let carried = InventoryLayout {
            width: 4,
            height: 3,
            items: vec![ItemPlacement {
                id: "battery-1".to_owned(),
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                weight_grams: 300,
                durability: None,
            }],
        };
        assert_eq!(
            merge_raid_inventory(&remaining, &carried)
                .unwrap()
                .items
                .len(),
            1
        );
        assert_eq!(next_raid_number(0, &carried, &loot), Some(2));
    }
}
