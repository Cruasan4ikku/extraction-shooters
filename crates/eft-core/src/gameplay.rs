use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{InventoryLayout, ItemPlacement};

pub const PLAYER_MAX_HEALTH: u32 = 440;
pub const MEDKIT_HEAL_AMOUNT: u32 = 45;
const ARMOR_REDUCTION_SCALE: u32 = 10_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Weapon,
    Ammunition,
    Armor,
    Medical,
    Consumable,
    Valuable,
    Miscellaneous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ItemDefinition {
    pub id: String,
    pub kind: ItemKind,
    pub width: u32,
    pub height: u32,
    pub weight_grams: u32,
}

impl ItemDefinition {
    pub fn validate(&self) -> Result<(), GameDataError> {
        if self.id.trim().is_empty() {
            return Err(GameDataError::EmptyDefinitionId);
        }
        if self.width == 0 || self.height == 0 {
            return Err(GameDataError::InvalidItemDimensions);
        }
        Ok(())
    }

    pub fn placement(&self, instance_id: impl Into<String>, x: u32, y: u32) -> ItemPlacement {
        ItemPlacement {
            id: instance_id.into(),
            x,
            y,
            width: self.width,
            height: self.height,
            weight_grams: self.weight_grams,
            durability: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponDefinition {
    pub id: String,
    pub damage: u32,
    pub rate_of_fire_rpm: u32,
    pub magazine_capacity: u32,
    pub muzzle_velocity_meters_per_second: u32,
    pub base_spread_milliradians: u32,
}

impl WeaponDefinition {
    pub fn validate(&self) -> Result<(), GameDataError> {
        if self.id.trim().is_empty() {
            return Err(GameDataError::EmptyDefinitionId);
        }
        if self.damage == 0
            || self.rate_of_fire_rpm == 0
            || self.magazine_capacity == 0
            || self.muzzle_velocity_meters_per_second == 0
        {
            return Err(GameDataError::InvalidWeaponStatistics);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum BodyPart {
    Head,
    Thorax,
    Abdomen,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DamageType {
    Ballistic,
    Blunt,
    Explosive,
    Environmental,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArmorDefinition {
    pub id: String,
    pub protection_class: u8,
    pub maximum_durability: u32,
    pub damage_reduction_basis_points: u16,
    pub covered_zones: Vec<BodyPart>,
}

impl ArmorDefinition {
    pub fn validate(&self) -> Result<(), GameDataError> {
        if self.id.trim().is_empty() {
            return Err(GameDataError::EmptyDefinitionId);
        }
        if self.protection_class == 0
            || self.maximum_durability == 0
            || self.damage_reduction_basis_points == 0
            || u32::from(self.damage_reduction_basis_points) > ARMOR_REDUCTION_SCALE
            || self.covered_zones.is_empty()
        {
            return Err(GameDataError::InvalidArmorStatistics);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HealthDamage {
    pub applied: u32,
    pub zone_destroyed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArmorDamage {
    pub absorbed: u32,
    pub remaining: u32,
    pub durability_lost: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HitResult {
    pub zone: BodyPart,
    pub incoming_damage: u32,
    pub armor_absorbed: u32,
    pub armor_durability_lost: u32,
    pub health_damage: u32,
    pub zone_destroyed: bool,
    pub player_dead: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BodyZoneHealth {
    pub current: u32,
    pub maximum: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HealthState {
    pub zones: BTreeMap<BodyPart, BodyZoneHealth>,
}

impl HealthState {
    pub fn standard() -> Self {
        let zones = [
            (BodyPart::Head, 35),
            (BodyPart::Thorax, 85),
            (BodyPart::Abdomen, 70),
            (BodyPart::LeftArm, 60),
            (BodyPart::RightArm, 60),
            (BodyPart::LeftLeg, 65),
            (BodyPart::RightLeg, 65),
        ]
        .into_iter()
        .map(|(part, maximum)| {
            (
                part,
                BodyZoneHealth {
                    current: maximum,
                    maximum,
                },
            )
        })
        .collect();
        Self { zones }
    }

    pub fn apply_damage(&mut self, zone: BodyPart, amount: u32) -> Option<HealthDamage> {
        let health = self.zones.get_mut(&zone)?;
        let applied = health.current.min(amount);
        health.current -= applied;
        Some(HealthDamage {
            applied,
            zone_destroyed: health.current == 0,
        })
    }

    pub fn total_current(&self) -> u32 {
        self.zones
            .values()
            .fold(0, |total, health| total.saturating_add(health.current))
    }

    pub fn total_maximum(&self) -> u32 {
        self.zones
            .values()
            .fold(0, |total, health| total.saturating_add(health.maximum))
    }

    pub fn is_fully_healed(&self) -> bool {
        self.zones
            .values()
            .all(|health| health.current >= health.maximum)
    }

    pub fn heal_most_injured(&mut self, amount: u32) -> u32 {
        let Some((_, health)) = self
            .zones
            .iter_mut()
            .filter(|(_, health)| health.current < health.maximum)
            .max_by_key(|(_, health)| health.maximum - health.current)
        else {
            return 0;
        };
        let restored = health.maximum.saturating_sub(health.current).min(amount);
        health.current += restored;
        restored
    }

    pub fn heal(&mut self, zone: BodyPart, amount: u32) -> Option<u32> {
        let health = self.zones.get_mut(&zone)?;
        let restored = health.maximum.saturating_sub(health.current).min(amount);
        health.current += restored;
        Some(restored)
    }

    pub fn is_dead(&self) -> bool {
        self.zones
            .get(&BodyPart::Head)
            .map_or(true, |health| health.current == 0)
            || self
                .zones
                .get(&BodyPart::Thorax)
                .map_or(true, |health| health.current == 0)
    }
}

impl Default for HealthState {
    fn default() -> Self {
        Self::standard()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArmorState {
    pub definition: ArmorDefinition,
    pub durability: u32,
}

impl ArmorState {
    pub fn new(definition: ArmorDefinition) -> Result<Self, GameDataError> {
        definition.validate()?;
        let durability = definition.maximum_durability;
        Self::with_durability(definition, durability)
    }

    pub fn with_durability(
        definition: ArmorDefinition,
        durability: u32,
    ) -> Result<Self, GameDataError> {
        definition.validate()?;
        if durability > definition.maximum_durability {
            return Err(GameDataError::InvalidArmorDurability);
        }
        Ok(Self {
            definition,
            durability,
        })
    }

    pub fn apply_damage(&mut self, zone: BodyPart, incoming: u32) -> ArmorDamage {
        if incoming == 0 || self.durability == 0 || !self.definition.covered_zones.contains(&zone) {
            return ArmorDamage {
                absorbed: 0,
                remaining: incoming,
                durability_lost: 0,
            };
        }

        let maximum_durability = self.definition.maximum_durability.max(1);
        let base_reduction =
            u32::from(self.definition.damage_reduction_basis_points).min(ARMOR_REDUCTION_SCALE);
        let reduction = (u64::from(base_reduction)
            * u64::from(self.durability.min(maximum_durability))
            / u64::from(maximum_durability)) as u32;
        let absorbed = ((u64::from(incoming) * u64::from(reduction))
            / u64::from(ARMOR_REDUCTION_SCALE)) as u32;
        let durability_lost = incoming.min(self.durability);
        self.durability -= durability_lost;
        ArmorDamage {
            absorbed,
            remaining: incoming - absorbed,
            durability_lost,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RaidEventKind {
    RaidStarted,
    LootCollected {
        item_id: String,
    },
    DamageReceived {
        zone: BodyPart,
        amount: u32,
        damage_type: DamageType,
    },
    EnemyKilled,
    ArmorDamaged {
        durability_lost: u32,
        remaining_durability: u32,
    },
    PlayerDied,
    PlayerExtracted {
        secured_item_count: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RaidEvent {
    pub raid_id: u64,
    pub sequence: u64,
    pub kind: RaidEventKind,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GameDataError {
    #[error("definition ID must not be empty")]
    EmptyDefinitionId,
    #[error("item dimensions must be greater than zero")]
    InvalidItemDimensions,
    #[error("weapon statistics must be greater than zero")]
    InvalidWeaponStatistics,
    #[error("armor statistics or coverage are invalid")]
    InvalidArmorStatistics,
    #[error("armor durability exceeds its maximum")]
    InvalidArmorDurability,
}

pub fn apply_damage(health: &mut u32, damage: u32) -> bool {
    *health = health.saturating_sub(damage);
    *health == 0
}

pub fn apply_hit(
    health: &mut HealthState,
    mut armor: Option<&mut ArmorState>,
    zone: BodyPart,
    incoming_damage: u32,
    damage_type: DamageType,
) -> Option<HitResult> {
    if health.is_dead() || !health.zones.contains_key(&zone) {
        return None;
    }

    let armor_damage = match (armor.as_deref_mut(), damage_type) {
        (Some(armor), DamageType::Ballistic) => armor.apply_damage(zone, incoming_damage),
        _ => ArmorDamage {
            absorbed: 0,
            remaining: incoming_damage,
            durability_lost: 0,
        },
    };
    let health_damage = health.apply_damage(zone, armor_damage.remaining)?;
    Some(HitResult {
        zone,
        incoming_damage,
        armor_absorbed: armor_damage.absorbed,
        armor_durability_lost: armor_damage.durability_lost,
        health_damage: health_damage.applied,
        zone_destroyed: health_damage.zone_destroyed,
        player_dead: health.is_dead(),
    })
}

pub fn heal(health: u32, maximum: u32, amount: u32) -> u32 {
    health.saturating_add(amount).min(maximum)
}

pub fn try_use_medkit(
    layout: &InventoryLayout,
    health: &HealthState,
    heal_amount: u32,
) -> Option<(InventoryLayout, HealthState)> {
    if health.is_fully_healed() || health.is_dead() {
        return None;
    }

    let mut candidate = layout.clone();
    let medkit_index = candidate
        .items
        .iter()
        .position(|item| item.id == "medkit")?;
    candidate.items.remove(medkit_index);
    let mut healed = health.clone();
    if healed.heal_most_injured(heal_amount) == 0 {
        return None;
    }
    Some((candidate, healed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ItemPlacement;

    #[test]
    fn health_zones_take_damage_and_can_be_healed_without_bevy() {
        let mut health = HealthState::standard();
        assert_eq!(
            health.apply_damage(BodyPart::LeftArm, 80),
            Some(HealthDamage {
                applied: 60,
                zone_destroyed: true
            })
        );
        assert!(!health.is_dead());
        assert_eq!(health.heal(BodyPart::LeftArm, 20), Some(20));
        assert_eq!(health.zones[&BodyPart::LeftArm].current, 20);
        assert_eq!(health.heal(BodyPart::Head, 1), Some(0));
    }

    #[test]
    fn armor_only_protects_covered_zones_and_wears_down() {
        let definition = ArmorDefinition {
            id: "test-vest".to_owned(),
            protection_class: 3,
            maximum_durability: 40,
            damage_reduction_basis_points: 5_000,
            covered_zones: vec![BodyPart::Thorax],
        };
        let mut armor = ArmorState::new(definition).unwrap();

        assert_eq!(
            armor.apply_damage(BodyPart::Head, 20),
            ArmorDamage {
                absorbed: 0,
                remaining: 20,
                durability_lost: 0
            }
        );
        assert_eq!(
            armor.apply_damage(BodyPart::Thorax, 30),
            ArmorDamage {
                absorbed: 15,
                remaining: 15,
                durability_lost: 30
            }
        );
        assert_eq!(armor.durability, 10);
        let worn_hit = armor.apply_damage(BodyPart::Thorax, 20);
        assert_eq!(worn_hit.durability_lost, 10);
        assert_eq!(worn_hit.absorbed, 2);
        assert_eq!(armor.durability, 0);
    }

    #[test]
    fn incoming_hits_apply_armor_before_zonal_health_damage() {
        let definition = ArmorDefinition {
            id: "test-vest".to_owned(),
            protection_class: 3,
            maximum_durability: 40,
            damage_reduction_basis_points: 5_000,
            covered_zones: vec![BodyPart::Thorax],
        };
        let mut armor = ArmorState::new(definition).unwrap();
        let mut health = HealthState::standard();

        let torso_hit = apply_hit(
            &mut health,
            Some(&mut armor),
            BodyPart::Thorax,
            20,
            DamageType::Ballistic,
        )
        .unwrap();
        assert_eq!(torso_hit.armor_absorbed, 10);
        assert_eq!(torso_hit.health_damage, 10);
        assert_eq!(torso_hit.armor_durability_lost, 20);
        assert_eq!(health.zones[&BodyPart::Thorax].current, 75);

        let head_hit = apply_hit(
            &mut health,
            Some(&mut armor),
            BodyPart::Head,
            20,
            DamageType::Ballistic,
        )
        .unwrap();
        assert_eq!(head_hit.armor_absorbed, 0);
        assert_eq!(head_hit.health_damage, 20);
        assert_eq!(armor.durability, 20);
    }

    #[test]
    fn definitions_reject_invalid_game_data() {
        let invalid_weapon = WeaponDefinition {
            id: " ".to_owned(),
            damage: 0,
            rate_of_fire_rpm: 0,
            magazine_capacity: 0,
            muzzle_velocity_meters_per_second: 0,
            base_spread_milliradians: 0,
        };
        assert_eq!(
            invalid_weapon.validate(),
            Err(GameDataError::EmptyDefinitionId)
        );

        let invalid_item = ItemDefinition {
            id: "bandage".to_owned(),
            kind: ItemKind::Medical,
            width: 0,
            height: 1,
            weight_grams: 10,
        };
        assert_eq!(
            invalid_item.validate(),
            Err(GameDataError::InvalidItemDimensions)
        );
    }

    #[test]
    fn scalar_health_rules_clamp_damage_and_healing() {
        let mut health = 35;
        assert!(!apply_damage(&mut health, 10));
        assert_eq!(health, 25);
        assert!(apply_damage(&mut health, 30));
        assert_eq!(health, 0);
        assert_eq!(heal(70, 100, MEDKIT_HEAL_AMOUNT), 100);
    }

    #[test]
    fn using_medkit_consumes_inventory_item_and_heals_injured_zone() {
        let layout = InventoryLayout {
            width: 2,
            height: 2,
            items: vec![ItemPlacement {
                id: "medkit".to_owned(),
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                weight_grams: 100,
                durability: None,
            }],
        };
        let mut health = HealthState::standard();
        health.apply_damage(BodyPart::Thorax, 30);
        let (healed, health) = try_use_medkit(&layout, &health, MEDKIT_HEAL_AMOUNT).unwrap();

        assert_eq!(health.zones[&BodyPart::Thorax].current, 85);
        assert!(healed.items.is_empty());
        assert_eq!(layout.items.len(), 1);
        assert!(try_use_medkit(&healed, &health, MEDKIT_HEAL_AMOUNT).is_none());
    }
}
