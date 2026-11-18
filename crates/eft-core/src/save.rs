use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{try_add_inventory_item, InventoryLayout, ItemPlacement};

pub const INVENTORY_SAVE_FORMAT_VERSION: u32 = 2;
const STARTER_ARMOR_DURABILITY: u32 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InventorySave {
    pub format_version: u32,
    pub inventory: InventoryLayout,
}

impl InventorySave {
    pub fn new(inventory: InventoryLayout) -> Self {
        Self {
            format_version: INVENTORY_SAVE_FORMAT_VERSION,
            inventory,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, InventorySaveError> {
        if self.format_version != INVENTORY_SAVE_FORMAT_VERSION {
            return Err(InventorySaveError::UnsupportedVersion(self.format_version));
        }
        serde_json::to_vec_pretty(self).map_err(InventorySaveError::Json)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, InventorySaveError> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(InventorySaveError::Json)?;
        if let Some(version) = value.get("format_version") {
            let version = version
                .as_u64()
                .and_then(|version| u32::try_from(version).ok())
                .ok_or(InventorySaveError::InvalidVersion)?;
            match version {
                INVENTORY_SAVE_FORMAT_VERSION => {
                    serde_json::from_value(value).map_err(InventorySaveError::Json)
                }
                1 => {
                    let mut save: Self =
                        serde_json::from_value(value).map_err(InventorySaveError::Json)?;
                    save.inventory = migrate_starter_armor(save.inventory);
                    save.format_version = INVENTORY_SAVE_FORMAT_VERSION;
                    Ok(save)
                }
                _ => Err(InventorySaveError::UnsupportedVersion(version)),
            }
        } else {
            let inventory: InventoryLayout =
                serde_json::from_value(value).map_err(InventorySaveError::Json)?;
            Ok(Self::new(migrate_starter_armor(inventory)))
        }
    }
}

fn migrate_starter_armor(mut inventory: InventoryLayout) -> InventoryLayout {
    if let Some(armor) = inventory.items.iter_mut().find(|item| item.id == "armor") {
        armor.durability.get_or_insert(STARTER_ARMOR_DURABILITY);
        return inventory;
    }

    let armor = ItemPlacement {
        id: "armor".to_owned(),
        x: 0,
        y: 0,
        width: 2,
        height: 2,
        weight_grams: 4_000,
        durability: Some(STARTER_ARMOR_DURABILITY),
    };
    match try_add_inventory_item(&inventory, &armor) {
        Some(migrated) => migrated,
        None => inventory,
    }
}

#[derive(Debug, Error)]
pub enum InventorySaveError {
    #[error("invalid inventory save JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("inventory save format version is invalid")]
    InvalidVersion,
    #[error("unsupported inventory save format version: {0}")]
    UnsupportedVersion(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory() -> InventoryLayout {
        InventoryLayout {
            width: 4,
            height: 3,
            items: Vec::new(),
        }
    }

    #[test]
    fn versioned_save_round_trips() {
        let save = InventorySave::new(inventory());
        let encoded = save.encode().unwrap();

        assert_eq!(InventorySave::decode(&encoded).unwrap(), save);
    }

    #[test]
    fn legacy_inventory_json_migrates_to_current_version() {
        let legacy = serde_json::to_vec(&inventory()).unwrap();

        let migrated = InventorySave::decode(&legacy).unwrap();
        assert_eq!(migrated.format_version, INVENTORY_SAVE_FORMAT_VERSION);
        assert!(migrated.inventory.items.iter().any(|item| {
            item.id == "armor" && item.durability == Some(STARTER_ARMOR_DURABILITY)
        }));
    }

    #[test]
    fn version_one_saves_migrate_once_to_current_version() {
        let version_one = serde_json::json!({
            "format_version": 1,
            "inventory": inventory()
        });
        let encoded = serde_json::to_vec(&version_one).unwrap();

        let migrated = InventorySave::decode(&encoded).unwrap();
        assert_eq!(migrated.format_version, INVENTORY_SAVE_FORMAT_VERSION);
        assert!(migrated
            .inventory
            .items
            .iter()
            .any(|item| item.id == "armor"));
    }

    #[test]
    fn armor_durability_survives_versioned_saves() {
        let mut inventory = inventory();
        inventory.items.push(crate::ItemPlacement {
            id: "armor".to_owned(),
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            weight_grams: 100,
            durability: Some(37),
        });
        let encoded = InventorySave::new(inventory.clone()).encode().unwrap();

        assert_eq!(
            InventorySave::decode(&encoded).unwrap().inventory,
            inventory
        );
    }

    #[test]
    fn unknown_save_versions_are_rejected() {
        let future = br#"{"format_version":3,"inventory":{"width":2,"height":1,"items":[]}}"#;

        assert!(matches!(
            InventorySave::decode(future),
            Err(InventorySaveError::UnsupportedVersion(3))
        ));
    }
}
