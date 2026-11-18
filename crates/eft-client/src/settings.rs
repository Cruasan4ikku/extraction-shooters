use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

pub const DEFAULT_MOUSE_SENSITIVITY: f32 = 0.002;

const SETTINGS_VERSION: u32 = 1;
const MIN_MOUSE_SENSITIVITY: f32 = 0.0005;
const MAX_MOUSE_SENSITIVITY: f32 = 0.004;
const MOUSE_SENSITIVITY_STEP: f32 = 0.00025;
const MASTER_VOLUME_STEP: f32 = 0.1;

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionPreset {
    Hd,
    HdPlus,
    FullHd,
}

impl ResolutionPreset {
    pub fn dimensions(self) -> (f32, f32) {
        match self {
            Self::Hd => (1280.0, 720.0),
            Self::HdPlus => (1600.0, 900.0),
            Self::FullHd => (1920.0, 1080.0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hd => "1280x720",
            Self::HdPlus => "1600x900",
            Self::FullHd => "1920x1080",
        }
    }

    fn cycle(self, direction: i8) -> Self {
        let presets = [Self::Hd, Self::HdPlus, Self::FullHd];
        let index = presets
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or(0) as i8;
        let next = (index + direction).rem_euclid(presets.len() as i8) as usize;
        presets[next]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameSettings {
    pub mouse_sensitivity: f32,
    pub master_volume: f32,
    pub resolution: ResolutionPreset,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: DEFAULT_MOUSE_SENSITIVITY,
            master_volume: 1.0,
            resolution: ResolutionPreset::Hd,
        }
    }
}

impl GameSettings {
    pub fn adjust_mouse_sensitivity(&mut self, direction: i8) {
        let next = self.mouse_sensitivity + MOUSE_SENSITIVITY_STEP * f32::from(direction);
        self.mouse_sensitivity = (next / MOUSE_SENSITIVITY_STEP).round() * MOUSE_SENSITIVITY_STEP;
        self.mouse_sensitivity = self
            .mouse_sensitivity
            .clamp(MIN_MOUSE_SENSITIVITY, MAX_MOUSE_SENSITIVITY);
    }

    pub fn adjust_master_volume(&mut self, direction: i8) {
        let next = self.master_volume + MASTER_VOLUME_STEP * f32::from(direction);
        self.master_volume = (next * 10.0).round() / 10.0;
        self.master_volume = self.master_volume.clamp(0.0, 1.0);
    }

    pub fn cycle_resolution(&mut self, direction: i8) {
        self.resolution = self.resolution.cycle(direction);
    }

    fn validate(&self) -> Result<(), String> {
        if !self.mouse_sensitivity.is_finite()
            || !(MIN_MOUSE_SENSITIVITY..=MAX_MOUSE_SENSITIVITY).contains(&self.mouse_sensitivity)
        {
            return Err(format!(
                "mouse sensitivity must be between {MIN_MOUSE_SENSITIVITY} and {MAX_MOUSE_SENSITIVITY}"
            ));
        }
        if !self.master_volume.is_finite() || !(0.0..=1.0).contains(&self.master_volume) {
            return Err("master volume must be between 0 and 1".to_owned());
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsDocument {
    version: u32,
    settings: GameSettings,
}

#[derive(Resource)]
pub struct GameSettingsState {
    pub values: GameSettings,
    pub open: bool,
    pub notice: String,
    path: Option<PathBuf>,
    persistence_enabled: bool,
}

impl Default for GameSettingsState {
    fn default() -> Self {
        Self {
            values: GameSettings::default(),
            open: false,
            notice: "Settings changes are only for this session.".to_owned(),
            path: None,
            persistence_enabled: false,
        }
    }
}

impl GameSettingsState {
    pub fn load() -> Self {
        let path = match settings_path() {
            Ok(path) => path,
            Err(error) => {
                return Self {
                    notice: format!("Settings could not be saved: {error}"),
                    ..Self::default()
                };
            }
        };
        Self::load_from_path(path)
    }

    fn load_from_path(path: PathBuf) -> Self {
        match load_settings_file(&path) {
            Ok(Some(values)) => Self {
                values,
                open: false,
                notice: "Changes are saved automatically.".to_owned(),
                path: Some(path),
                persistence_enabled: true,
            },
            Ok(None) => Self {
                values: GameSettings::default(),
                open: false,
                notice: "Changes are saved automatically.".to_owned(),
                path: Some(path),
                persistence_enabled: true,
            },
            Err(error) => Self {
                values: GameSettings::default(),
                open: false,
                notice: format!(
                    "Settings could not be loaded; the file was preserved and defaults are active: {error}"
                ),
                path: Some(path),
                persistence_enabled: false,
            },
        }
    }

    pub fn save(&mut self) {
        if !self.persistence_enabled {
            return;
        }
        let Some(path) = self.path.as_deref() else {
            self.notice = "Settings path is unavailable; changes are not saved.".to_owned();
            self.persistence_enabled = false;
            return;
        };
        match save_settings_file(path, &self.values) {
            Ok(()) => self.notice = "Settings saved.".to_owned(),
            Err(error) => {
                self.notice = format!("Settings applied but could not be saved: {error}");
                self.persistence_enabled = false;
            }
        }
    }

    pub fn reset_to_defaults(&mut self) {
        self.values = GameSettings::default();
        let Some(path) = self.path.as_deref() else {
            self.notice =
                "Defaults applied for this session; settings path is unavailable.".to_owned();
            return;
        };
        match save_settings_file(path, &self.values) {
            Ok(()) => {
                self.persistence_enabled = true;
                self.notice = "Default settings restored and saved.".to_owned();
            }
            Err(error) => {
                self.persistence_enabled = false;
                self.notice = format!("Defaults applied but could not be saved: {error}");
            }
        }
    }
}

fn settings_path() -> io::Result<PathBuf> {
    let local_app_data =
        env::var_os("LOCALAPPDATA").ok_or_else(|| io::Error::other("LOCALAPPDATA is not set"))?;
    Ok(PathBuf::from(local_app_data)
        .join("Escape From Tarkov Rust")
        .join("settings.json"))
}

fn load_settings_file(path: &Path) -> Result<Option<GameSettings>, String> {
    let contents = match fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!("could not read {}: {error}", path.display()));
        }
    };
    let document: SettingsDocument = serde_json::from_slice(&contents)
        .map_err(|error| format!("invalid settings JSON in {}: {error}", path.display()))?;
    if document.version != SETTINGS_VERSION {
        return Err(format!(
            "unsupported settings version {} in {}",
            document.version,
            path.display()
        ));
    }
    document
        .settings
        .validate()
        .map_err(|error| format!("invalid settings in {}: {error}", path.display()))?;
    Ok(Some(document.settings))
}

fn save_settings_file(path: &Path, settings: &GameSettings) -> io::Result<()> {
    settings
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let contents = serde_json::to_vec_pretty(&SettingsDocument {
        version: SETTINGS_VERSION,
        settings: settings.clone(),
    })
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    atomic_write(path, &contents)
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)?;
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "settings path has no file name",
            )
        })?
        .to_string_lossy();
    let (temp_path, mut temp_file) = loop {
        let id = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let temp_path =
            path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), id));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => break (temp_path, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };

    if let Err(error) = temp_file.write_all(contents) {
        return Err(cleanup_temp_file(&temp_path, error));
    }
    if let Err(error) = temp_file.sync_all() {
        return Err(cleanup_temp_file(&temp_path, error));
    }
    drop(temp_file);

    if let Err(error) = fs::rename(&temp_path, path) {
        return Err(cleanup_temp_file(&temp_path, error));
    }
    Ok(())
}

fn cleanup_temp_file(path: &Path, cause: io::Error) -> io::Error {
    match fs::remove_file(path) {
        Ok(()) => cause,
        Err(error) if error.kind() == io::ErrorKind::NotFound => cause,
        Err(error) => io::Error::new(
            cause.kind(),
            format!(
                "{cause}; failed to remove temporary settings file {}: {error}",
                path.display()
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_settings_path() -> PathBuf {
        static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);
        env::temp_dir().join(format!(
            "eft-settings-{}-{}.json",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn settings_round_trip_and_reject_invalid_values_or_versions() {
        let path = temporary_settings_path();
        let settings = GameSettings {
            mouse_sensitivity: 0.0025,
            master_volume: 0.7,
            resolution: ResolutionPreset::HdPlus,
        };

        save_settings_file(&path, &settings).unwrap();
        assert_eq!(load_settings_file(&path).unwrap(), Some(settings));

        let invalid = br#"{"version":1,"settings":{"mouse_sensitivity":0.002,"master_volume":2.0,"resolution":"hd"}}"#;
        fs::write(&path, invalid).unwrap();
        assert!(load_settings_file(&path).is_err());

        let unsupported = br#"{"version":2,"settings":{"mouse_sensitivity":0.002,"master_volume":1.0,"resolution":"hd"}}"#;
        fs::write(&path, unsupported).unwrap();
        assert!(load_settings_file(&path).is_err());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_settings_are_preserved_until_explicit_reset() {
        let path = temporary_settings_path();
        let original = b"not valid JSON";
        fs::write(&path, original).unwrap();

        let mut state = GameSettingsState::load_from_path(path.clone());
        assert!(!state.persistence_enabled);
        state.values.master_volume = 0.5;
        state.save();
        assert_eq!(fs::read(&path).unwrap(), original);

        state.reset_to_defaults();
        assert!(state.persistence_enabled);
        assert_eq!(
            load_settings_file(&path).unwrap(),
            Some(GameSettings::default())
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn setting_adjustments_stay_within_supported_ranges() {
        let mut settings = GameSettings::default();
        for _ in 0..20 {
            settings.adjust_mouse_sensitivity(1);
            settings.adjust_master_volume(1);
            settings.cycle_resolution(1);
        }
        assert_eq!(settings.mouse_sensitivity, MAX_MOUSE_SENSITIVITY);
        assert_eq!(settings.master_volume, 1.0);
        assert_eq!(settings.resolution, ResolutionPreset::FullHd);

        for _ in 0..20 {
            settings.adjust_mouse_sensitivity(-1);
            settings.adjust_master_volume(-1);
            settings.cycle_resolution(-1);
        }
        assert_eq!(settings.mouse_sensitivity, MIN_MOUSE_SENSITIVITY);
        assert_eq!(settings.master_volume, 0.0);
        assert_eq!(settings.resolution, ResolutionPreset::Hd);
    }
}
