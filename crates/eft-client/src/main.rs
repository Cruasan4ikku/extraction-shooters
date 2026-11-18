mod backend;
mod combat;
mod inventory_ui;
mod movement;
mod raid;
mod scene;
mod settings;
#[cfg(test)]
mod tests;
mod ui;

use backend::*;
use combat::*;
use inventory_ui::*;
use movement::*;
use raid::*;
use scene::setup;
use ui::*;

use std::{
    env,
    path::PathBuf,
    sync::{mpsc, Mutex},
    thread,
    time::Duration,
};

use bevy::asset::AssetPlugin;
use bevy::audio::{AudioPlayer, GlobalVolume, PlaybackSettings, Volume};
use bevy::{input::mouse::MouseMotion, prelude::*, window::CursorGrabMode};
use eft_core::{
    apply_damage, apply_hit, merge_raid_inventory, next_raid_number, prepare_raid_loadout,
    try_add_inventory_item, try_move_inventory_item, try_rotate_inventory_item, try_use_medkit,
    validate_inventory, ArmorDefinition, ArmorState, BodyPart, DamageType, HealthState, HitResult,
    InventoryLayout, ItemDefinition, ItemKind, ItemPlacement, RaidEvent, RaidEventKind,
    WeaponDefinition, MEDKIT_HEAL_AMOUNT,
};
use settings::{GameSettingsState, DEFAULT_MOUSE_SENSITIVITY};

const ARENA_HALF_SIZE: f32 = 12.0;
const SHELTER_CENTER: Vec3 = Vec3::new(0.0, 0.0, 45.0);
const SHELTER_BED_SPAWN: Vec3 = Vec3::new(-4.2, 1.65, 42.8);
const SHELTER_BED_YAW: f32 = -std::f32::consts::FRAC_PI_2;
const EXTRACTION_POSITIONS: [Vec3; 2] = [Vec3::new(-9.0, 0.08, -9.0), Vec3::new(9.0, 0.08, -9.0)];
const MAP_COVER_SPECS: [(Vec3, Vec3); 6] = [
    (Vec3::new(0.0, 0.9, -1.0), Vec3::new(4.0, 1.8, 4.0)),
    (Vec3::new(-6.0, 0.6, -4.0), Vec3::new(2.5, 1.2, 2.0)),
    (Vec3::new(6.0, 0.6, -4.0), Vec3::new(2.5, 1.2, 2.0)),
    (Vec3::new(-4.5, 0.6, -8.0), Vec3::new(2.2, 1.2, 2.0)),
    (Vec3::new(4.5, 0.6, -8.0), Vec3::new(2.2, 1.2, 2.0)),
    (Vec3::new(0.0, 0.6, 4.0), Vec3::new(2.0, 1.2, 1.2)),
];
const PLAYER_RADIUS: f32 = 0.35;
const MOVE_SPEED: f32 = 5.0;
const SPRINT_SPEED: f32 = 8.0;
const CROUCH_SPEED: f32 = 2.5;
const MAX_STAMINA: f32 = 100.0;
const STAMINA_DRAIN_PER_SECOND: f32 = 25.0;
const STAMINA_REGEN_PER_SECOND: f32 = 18.0;
const ARMOR_MAX_DURABILITY: u32 = 100;
const ENEMY_HEALTH: u32 = 100;
const WEAPON_MOVING_SPREAD: f32 = 0.014;
const WEAPON_SPRINTING_SPREAD: f32 = 0.035;
const WEAPON_CROUCHING_SPREAD: f32 = 0.0015;
const WEAPON_RECOIL_PER_SHOT: f32 = 0.018;
const WEAPON_RECOIL_RECOVERY: f32 = 0.035;
const WEAPON_MAX_RECOIL: f32 = 0.14;
const ENEMY_BASE_SPREAD: f32 = 0.04;
const ENEMY_DISTANCE_SPREAD: f32 = 0.0055;
const ENEMY_MOVEMENT_SPREAD: f32 = 0.018;
const ENEMY_DAMAGE: u32 = 10;
const ENEMY_ATTACK_INTERVAL: f32 = 1.4;
const ENEMY_ATTACK_RANGE: f32 = 9.0;
const ENEMY_STOP_DISTANCE: f32 = 6.5;
const ENEMY_MOVE_SPEED: f32 = 2.2;
const COMBAT_FEEDBACK_DURATION: f32 = 2.5;
const ENEMY_MUZZLE_HEIGHT: f32 = 0.45;
const PLAYER_AIM_HEIGHT: f32 = 0.75;
const ENEMY_VISION_RANGE: f32 = 18.0;
const ENEMY_VISION_FOV_COS: f32 = 0.5;
const ENEMY_GUNSHOT_HEARING_RANGE: f32 = 30.0;
const ENEMY_GUNSHOT_MEMORY: f32 = 4.0;
const ENEMY_TARGET_MEMORY: f32 = 4.0;
const ENEMY_DETECTION_DECAY: f32 = 0.35;
const INVENTORY_GRID_WIDTH: u32 = 10;
const INVENTORY_GRID_HEIGHT: u32 = 6;
const INVENTORY_SLOT_SIZE: f32 = 36.0;
const INVENTORY_HEADER_HEIGHT: f32 = 48.0;
const INVENTORY_FOOTER_HEIGHT: f32 = 56.0;
const INVENTORY_BODY_PANEL_WIDTH: f32 = 128.0;
const INVENTORY_PANEL_GAP: f32 = 12.0;
const INVENTORY_CONTENT_HEIGHT: f32 = 244.0;
const INVENTORY_GRID_PIXEL_WIDTH: f32 = INVENTORY_GRID_WIDTH as f32 * INVENTORY_SLOT_SIZE;
const INVENTORY_GRID_PIXEL_HEIGHT: f32 = INVENTORY_GRID_HEIGHT as f32 * INVENTORY_SLOT_SIZE;
const INVENTORY_PANEL_WIDTH: f32 =
    INVENTORY_BODY_PANEL_WIDTH + INVENTORY_PANEL_GAP + INVENTORY_GRID_PIXEL_WIDTH;
const INVENTORY_PANEL_HEIGHT: f32 =
    INVENTORY_HEADER_HEIGHT + INVENTORY_CONTENT_HEIGHT + INVENTORY_FOOTER_HEIGHT;
const UI_PANEL: Color = Color::srgb(0.06, 0.075, 0.08);
const UI_PANEL_HEADER: Color = Color::srgb(0.11, 0.135, 0.14);
const UI_PANEL_RAISED: Color = Color::srgb(0.085, 0.105, 0.11);
const UI_BORDER: Color = Color::srgb(0.28, 0.32, 0.29);
const UI_ACCENT: Color = Color::srgb(0.72, 0.64, 0.38);
const UI_TEXT: Color = Color::srgb(0.91, 0.92, 0.88);
const UI_TEXT_MUTED: Color = Color::srgb(0.68, 0.73, 0.72);
const UI_BUTTON: Color = Color::srgb(0.16, 0.19, 0.17);
const UI_BUTTON_HOVERED: Color = Color::srgb(0.24, 0.3, 0.24);
const UI_BUTTON_PRESSED: Color = Color::srgb(0.34, 0.4, 0.29);
const UI_SLOT: Color = Color::srgb(0.12, 0.14, 0.15);
const UI_SLOT_BORDER: Color = Color::srgb(0.3, 0.33, 0.31);
const UI_BAR_TRACK: Color = Color::srgb(0.19, 0.21, 0.21);

#[derive(Component)]
struct PlayerCamera {
    yaw: f32,
    pitch: f32,
}

#[derive(Component)]
struct RaidWeaponVisual;

#[derive(Component)]
struct RaidAmbience;

#[derive(Component, Clone, Copy)]
struct ArenaCollider {
    center: Vec3,
    half_extents: Vec3,
}

#[derive(Component)]
struct Enemy {
    health: u32,
    attack_timer: Timer,
    detection: f32,
    last_known_player: Option<Vec3>,
    memory_remaining: f32,
    shot_random_state: u64,
    home_position: Vec3,
    patrol_points: Vec<Vec3>,
    patrol_index: usize,
}

#[derive(Component)]
struct RaidLoot {
    item: ItemPlacement,
    key: &'static str,
    available: bool,
}

#[derive(Component)]
struct ExtractionPoint {
    radius: f32,
    label: &'static str,
}

#[derive(Component)]
struct HudStats;

#[derive(Component)]
struct GameMenuOverlay;

#[derive(Component)]
struct GameMenuTitle;

#[derive(Component)]
struct GameMenuStatus;

#[derive(Component, Clone, Copy)]
enum GameMenuAction {
    StartRaid,
    OpenStash,
    OpenSettings,
    RetryProfile,
    Resume,
    ReturnToStash,
    Exit,
}

#[derive(Component)]
struct GameMenuButton(GameMenuAction);

#[derive(Component)]
struct SettingsOverlay;

#[derive(Component, Clone, Copy)]
struct SettingsValueText(SettingsField);

#[derive(Component)]
struct SettingsNoticeText;

#[derive(Component, Clone, Copy)]
struct SettingsControl(SettingsAction);

#[derive(Clone, Copy)]
enum SettingsField {
    Resolution,
    MasterVolume,
    MouseSensitivity,
}

#[derive(Clone, Copy)]
enum SettingsAction {
    ResolutionDown,
    ResolutionUp,
    VolumeDown,
    VolumeUp,
    SensitivityDown,
    SensitivityUp,
    Reset,
    Back,
}

#[derive(Resource)]
struct WeaponState {
    definition: WeaponDefinition,
    magazine: u32,
    reserve: u32,
    random_state: u64,
}

#[derive(Resource, Default)]
struct WeaponRecoil {
    angle: f32,
}

#[derive(Resource, Default)]
struct PlayerSignature {
    moving: bool,
    sprinting: bool,
    crouching: bool,
    gunshot_remaining: f32,
}

#[derive(Resource)]
struct CombatStats {
    enemies_killed: u32,
    total_enemies: u32,
}

#[derive(Resource, Default)]
struct CombatFeedback {
    message: String,
    remaining: f32,
}

impl CombatFeedback {
    fn show(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.remaining = COMBAT_FEEDBACK_DURATION;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReloadOutcome {
    Loaded(u32),
    MagazineFull,
    NoReserveAmmo,
}

#[derive(Resource)]
struct PlayerVitals {
    health: HealthState,
    armor: Option<ArmorState>,
    stamina: f32,
}

impl Default for PlayerVitals {
    fn default() -> Self {
        Self {
            health: HealthState::standard(),
            armor: None,
            stamina: MAX_STAMINA,
        }
    }
}

#[derive(Resource)]
struct InventoryState {
    open: bool,
    layout: InventoryLayout,
    dragging: Option<String>,
    notice: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RaidPhase {
    Stash,
    Active,
    Paused,
    Debrief,
}

#[derive(Resource)]
struct RaidState {
    phase: RaidPhase,
    menu_open: bool,
    stash_after_loadout: Option<InventoryLayout>,
    raid_number: u64,
    loot_collected: u32,
    events: Vec<RaidEvent>,
    status: String,
}

impl Default for RaidState {
    fn default() -> Self {
        Self {
            phase: RaidPhase::Stash,
            menu_open: false,
            stash_after_loadout: None,
            raid_number: 0,
            loot_collected: 0,
            events: Vec::new(),
            status: "Shelter ready. Your stored gear is safe.".to_owned(),
        }
    }
}

#[derive(Resource, Default)]
struct GameMenuCommands {
    start_raid: bool,
    resume: bool,
    return_to_stash: bool,
}

impl RaidState {
    fn record_event(&mut self, kind: RaidEventKind) {
        self.events.push(RaidEvent {
            raid_id: self.raid_number,
            sequence: self.events.len() as u64,
            kind,
        });
    }
}

fn fatal_wound_label(health: &HealthState) -> &'static str {
    if health
        .zones
        .get(&BodyPart::Head)
        .map_or(true, |zone| zone.current == 0)
    {
        "HEAD DESTROYED"
    } else {
        "THORAX DESTROYED"
    }
}

fn collected_loot_summary(events: &[RaidEvent]) -> String {
    let items = events
        .iter()
        .filter_map(|event| match &event.kind {
            RaidEventKind::LootCollected { item_id } => Some(item_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        "None".to_owned()
    } else {
        items.join(", ")
    }
}

enum BackendRequest {
    Load,
    Save(InventoryLayout),
}

enum BackendMessage {
    Loaded(Result<Option<InventoryLayout>, String>),
    Saved(Result<(), String>),
}

#[derive(Resource)]
struct BackendSync {
    requests: mpsc::Sender<BackendRequest>,
    messages: Mutex<mpsc::Receiver<BackendMessage>>,
    status: String,
    ready: bool,
    loading: bool,
    save_enabled: bool,
    retry_available: bool,
    last_queued: Option<InventoryLayout>,
}

impl Default for InventoryState {
    fn default() -> Self {
        Self {
            open: false,
            layout: default_inventory_layout(),
            dragging: None,
            notice: "Drag items; U uses a medkit; R rotates a dragged item.".to_owned(),
        }
    }
}

fn default_inventory_layout() -> InventoryLayout {
    let mut armor = inventory_item("armor", 7, 0, 2, 2, 4_000);
    armor.durability = Some(ARMOR_MAX_DURABILITY);
    InventoryLayout {
        width: INVENTORY_GRID_WIDTH,
        height: INVENTORY_GRID_HEIGHT,
        items: vec![
            inventory_item("medkit", 0, 0, 2, 2, 350),
            inventory_item("ammo", 2, 0, 1, 2, 120),
            inventory_item("bandage", 3, 0, 2, 1, 80),
            inventory_item("ration", 5, 0, 1, 1, 250),
            inventory_item("water", 6, 0, 1, 2, 500),
            armor,
        ],
    }
}

#[cfg(test)]
fn empty_inventory_layout() -> InventoryLayout {
    InventoryLayout {
        width: INVENTORY_GRID_WIDTH,
        height: INVENTORY_GRID_HEIGHT,
        items: Vec::new(),
    }
}

#[derive(Component)]
struct InventoryOverlay;

#[derive(Component)]
struct InventoryCell {
    x: u32,
    y: u32,
}

#[derive(Component)]
struct InventoryStatusText;

#[derive(Component)]
struct BodyPartSegment(BodyPart);

#[derive(Component)]
struct BodyHealthHoverText;

#[derive(Component)]
struct InventoryHealthText;

#[derive(Component)]
struct InventoryHealthBarFill;

#[derive(Component)]
struct InventoryArmorText;

#[derive(Component)]
struct InventoryArmorBarFill;

#[derive(Component)]
struct StaminaHudRoot;

#[derive(Component)]
struct StaminaHudText;

#[derive(Component)]
struct StaminaHudBarFill;

fn inventory_item(
    id: &str,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    weight_grams: u32,
) -> ItemPlacement {
    let kind = match id {
        "medkit" | "bandage" => ItemKind::Medical,
        "ammo" => ItemKind::Ammunition,
        "armor" => ItemKind::Armor,
        "ration" | "water" => ItemKind::Consumable,
        _ => ItemKind::Valuable,
    };
    ItemDefinition {
        id: id.to_owned(),
        kind,
        width,
        height,
        weight_grams,
    }
    .placement(id, x, y)
}

fn starter_armor_definition() -> ArmorDefinition {
    ArmorDefinition {
        id: "starter-vest".to_owned(),
        protection_class: 2,
        maximum_durability: ARMOR_MAX_DURABILITY,
        damage_reduction_basis_points: 3_500,
        covered_zones: vec![BodyPart::Thorax, BodyPart::Abdomen],
    }
}

fn armor_from_inventory(
    layout: &InventoryLayout,
) -> Result<Option<ArmorState>, eft_core::GameDataError> {
    let Some(item) = layout.items.iter().find(|item| item.id == "armor") else {
        return Ok(None);
    };
    ArmorState::with_durability(
        starter_armor_definition(),
        item.durability.unwrap_or(ARMOR_MAX_DURABILITY),
    )
    .map(Some)
}

fn starter_weapon_definition() -> WeaponDefinition {
    WeaponDefinition {
        id: "starter-rifle".to_owned(),
        damage: 40,
        rate_of_fire_rpm: 600,
        magazine_capacity: 12,
        muzzle_velocity_meters_per_second: 750,
        base_spread_milliradians: 3,
    }
}

fn patrol_route(x: f32, northbound: bool) -> Vec<Vec3> {
    let z_positions = if northbound {
        [3.0, -1.0, -5.0, -1.0]
    } else {
        [-5.0, -1.0, 3.0, -1.0]
    };
    z_positions
        .into_iter()
        .map(|z| Vec3::new(x, 0.9, z))
        .collect()
}

fn reload_magazine(current: u32, reserve: u32, capacity: u32) -> (u32, u32, ReloadOutcome) {
    let needed = capacity.saturating_sub(current);
    if needed == 0 {
        return (current, reserve, ReloadOutcome::MagazineFull);
    }
    if reserve == 0 {
        return (current, reserve, ReloadOutcome::NoReserveAmmo);
    }
    let loaded = needed.min(reserve);
    (
        current + loaded,
        reserve - loaded,
        ReloadOutcome::Loaded(loaded),
    )
}

fn damage_feedback(hit: HitResult) -> String {
    let wound = if hit.player_dead {
        "FATAL WOUND"
    } else if hit.zone_destroyed {
        "ZONE DESTROYED"
    } else if hit.health_damage > 0 {
        "WOUNDED"
    } else {
        "ARMOR HIT"
    };
    let armor_details = if hit.armor_absorbed > 0 || hit.armor_durability_lost > 0 {
        format!(
            " | armor absorbed {} (-{} durability)",
            hit.armor_absorbed, hit.armor_durability_lost
        )
    } else {
        String::new()
    };
    format!(
        "{wound}: {} | -{} HP{armor_details}",
        body_part_name(hit.zone),
        hit.health_damage
    )
}

fn main() {
    let settings = GameSettingsState::load();
    let (window_width, window_height) = settings.values.resolution.dimensions();
    let master_volume = settings.values.master_volume;

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: resolve_asset_root()
                        .expect("could not locate the game's assets directory")
                        .to_string_lossy()
                        .into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "eft-client | WASD: move | Mouse: look | Esc: pause raid".into(),
                        resolution: (window_width, window_height).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(GlobalVolume::new(master_volume))
        .insert_resource(ClearColor(Color::srgb(0.08, 0.1, 0.14)))
        .insert_resource({
            let definition = starter_weapon_definition();
            WeaponState {
                magazine: definition.magazine_capacity,
                reserve: definition.magazine_capacity * 3,
                definition,
                random_state: 0x9E37_79B9_7F4A_7C15,
            }
        })
        .insert_resource(WeaponRecoil::default())
        .insert_resource(CombatStats {
            enemies_killed: 0,
            total_enemies: 2,
        })
        .insert_resource(CombatFeedback::default())
        .insert_resource(PlayerVitals::default())
        .insert_resource(PlayerSignature::default())
        .insert_resource(InventoryState::default())
        .insert_resource(RaidState::default())
        .insert_resource(settings)
        .insert_resource(GameMenuCommands::default())
        .insert_resource(start_backend_sync())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                toggle_inventory,
                handle_game_menu,
                handle_settings_controls,
                capture_cursor,
                look,
                move_player,
                play_player_footsteps,
                update_raid_ambience,
                fire_weapon,
                update_enemies,
                update_combat_feedback,
                update_raid_flow,
                handle_inventory_input,
                poll_backend_sync,
                sync_inventory_changes,
                update_hud,
                update_inventory_ui,
                update_vitals_ui,
                update_game_menu,
                update_settings_ui,
            )
                .chain(),
        )
        .run();
}

fn resolve_asset_root() -> Result<PathBuf, String> {
    let executable = env::current_exe()
        .map_err(|error| format!("could not locate the game executable: {error}"))?;
    let executable_dir = executable
        .parent()
        .ok_or_else(|| "game executable has no parent directory".to_owned())?;
    let mut candidates = vec![
        executable_dir.join("assets"),
        executable_dir.join("..").join("..").join("assets"),
    ];
    if let Ok(working_dir) = env::current_dir() {
        candidates.push(working_dir.join("assets"));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("assets"),
    );

    for candidate in &candidates {
        if candidate.join(AUDIO_PLAYER_GUNSHOT).is_file() {
            return candidate.canonicalize().map_err(|error| {
                format!(
                    "could not resolve asset directory {}: {error}",
                    candidate.display()
                )
            });
        }
    }

    Err(format!(
        "could not find a complete assets directory containing {}; searched: {}",
        AUDIO_PLAYER_GUNSHOT,
        candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

const AUDIO_AMBIENCE: &str = "audio/ambience.ogg";
const AUDIO_FOOTSTEPS: [&str; 6] = [
    "audio/footstep-1.ogg",
    "audio/footstep-2.ogg",
    "audio/footstep-3.ogg",
    "audio/footstep-4.ogg",
    "audio/footstep-5.ogg",
    "audio/footstep-6.ogg",
];
const AUDIO_PLAYER_GUNSHOT: &str = "audio/gunshot-player.wav";
const AUDIO_ENEMY_GUNSHOT: &str = "audio/gunshot-enemy.wav";
const AUDIO_IMPACT_FLESH: &str = "audio/impact-flesh.ogg";
const AUDIO_IMPACT_ARMOR: &str = "audio/impact-metal.ogg";

fn footstep_audio_path(step_index: usize) -> &'static str {
    AUDIO_FOOTSTEPS[step_index % AUDIO_FOOTSTEPS.len()]
}

fn play_sound(
    commands: &mut Commands,
    asset_server: Option<&AssetServer>,
    path: &'static str,
    settings: PlaybackSettings,
) {
    if let Some(asset_server) = asset_server {
        commands.spawn((AudioPlayer::new(asset_server.load(path)), settings));
    }
}

fn update_raid_ambience(
    raid: Res<RaidState>,
    asset_server: Option<Res<AssetServer>>,
    mut commands: Commands,
    ambience: Query<Entity, With<RaidAmbience>>,
) {
    if raid.phase == RaidPhase::Active {
        if ambience.is_empty() {
            if let Some(asset_server) = asset_server.as_deref() {
                commands.spawn((
                    AudioPlayer::new(asset_server.load(AUDIO_AMBIENCE)),
                    PlaybackSettings::LOOP.with_volume(Volume::new(0.2)),
                    RaidAmbience,
                ));
            }
        }
    } else {
        for entity in &ambience {
            commands.entity(entity).despawn();
        }
    }
}
