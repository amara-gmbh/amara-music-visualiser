use crate::audio::AudioMode;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::Context;

const FORMAT_VERSION: u32 = 1;
pub const WORLD_COUNT: usize = 17;
pub const BUILT_IN_PROFILE_COUNT: usize = 11;
const MAX_PROFILE_BYTES: u64 = 64 * 1024;
const MAX_NAME_CHARACTERS: usize = 80;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VisualPalette {
    #[default]
    Cosmic,
    MoccaVanilla,
}

impl VisualPalette {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cosmic => "Cosmic",
            Self::MoccaVanilla => "Mocca & Vanilla",
        }
    }

    pub fn amount(self) -> f32 {
        match self {
            Self::Cosmic => 0.0,
            Self::MoccaVanilla => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BackgroundPalette {
    #[default]
    Auto,
    WorldColours,
    VioletNight,
    OceanJade,
    EmberGold,
    RoseGlass,
    DeepBlue,
    PearlAmber,
    Lagoon,
    SlateLilac,
}

impl BackgroundPalette {
    pub const COLOURS: [Self; 8] = [
        Self::VioletNight,
        Self::OceanJade,
        Self::EmberGold,
        Self::RoseGlass,
        Self::DeepBlue,
        Self::PearlAmber,
        Self::Lagoon,
        Self::SlateLilac,
    ];

    pub const ALL: [Self; 10] = [
        Self::Auto,
        Self::WorldColours,
        Self::VioletNight,
        Self::OceanJade,
        Self::EmberGold,
        Self::RoseGlass,
        Self::DeepBlue,
        Self::PearlAmber,
        Self::Lagoon,
        Self::SlateLilac,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto · flowing colours",
            Self::WorldColours => "World colours · original",
            Self::VioletNight => "Violet Night",
            Self::OceanJade => "Ocean Jade",
            Self::EmberGold => "Ember Gold",
            Self::RoseGlass => "Rose Glass",
            Self::DeepBlue => "Deep Blue",
            Self::PearlAmber => "Pearl Amber",
            Self::Lagoon => "Lagoon",
            Self::SlateLilac => "Slate Lilac",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MathForm {
    #[default]
    Journey,
    Julia,
    InversionLace,
    HarmonicSculpture,
    VortexWeave,
}

impl MathForm {
    pub fn label(self) -> &'static str {
        match self {
            Self::Journey => "Journey",
            Self::Julia => "Julia",
            Self::InversionLace => "Inversion Lace",
            Self::HarmonicSculpture => "Harmonic Sculpture",
            Self::VortexWeave => "Vortex Weave",
        }
    }

    pub fn family(self) -> Option<usize> {
        match self {
            Self::Journey => None,
            Self::Julia => Some(0),
            Self::InversionLace => Some(1),
            Self::HarmonicSculpture => Some(2),
            Self::VortexWeave => Some(3),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualSettings {
    pub scene: usize,
    pub intensity: f32,
    pub motion: f32,
    pub complexity: f32,
    pub hue: f32,
    pub glow: f32,
    pub warp: f32,
    #[serde(default = "default_feedback")]
    pub feedback: f32,
    #[serde(default)]
    pub palette: VisualPalette,
    #[serde(default)]
    pub background: BackgroundPalette,
    #[serde(default)]
    pub math_form: MathForm,
    #[serde(default)]
    pub look: usize,
}

fn default_feedback() -> f32 {
    0.22
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMappings {
    pub bass_to_pulse: f32,
    pub mids_to_geometry: f32,
    pub treble_to_sparkle: f32,
    pub beat_to_flash: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub format_version: u32,
    pub name: String,
    pub visual: VisualSettings,
    pub mappings: AudioMappings,
    pub audio_mode: AudioMode,
    pub director_enabled: bool,
    pub director_freedom: f32,
}

impl Default for VisualSettings {
    fn default() -> Self {
        Self {
            scene: 0,
            intensity: 0.92,
            motion: 0.75,
            complexity: 0.8,
            hue: 0.72,
            glow: 0.95,
            warp: 0.7,
            feedback: default_feedback(),
            palette: VisualPalette::default(),
            background: BackgroundPalette::default(),
            math_form: MathForm::default(),
            look: 0,
        }
    }
}

impl Default for AudioMappings {
    fn default() -> Self {
        Self {
            bass_to_pulse: 1.0,
            mids_to_geometry: 0.72,
            treble_to_sparkle: 0.9,
            beat_to_flash: 0.8,
        }
    }
}

impl Profile {
    pub fn astral_bloom() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name: "Astral Bloom".into(),
            visual: VisualSettings::default(),
            mappings: AudioMappings::default(),
            audio_mode: AudioMode::Demo,
            director_enabled: true,
            director_freedom: 0.46,
        }
    }

    /// The import, load, save and export boundaries all enforce these limits.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.format_version == FORMAT_VERSION,
            "unsupported profile format {}; expected {}",
            self.format_version,
            FORMAT_VERSION
        );
        anyhow::ensure!(
            !self.name.is_empty() && self.name == self.name.trim(),
            "profile name must be non-empty with no surrounding whitespace"
        );
        anyhow::ensure!(
            self.name.chars().count() <= MAX_NAME_CHARACTERS,
            "profile name must contain at most {MAX_NAME_CHARACTERS} characters"
        );
        anyhow::ensure!(
            !matches!(self.name.as_str(), "." | "..")
                && !self
                    .name
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '/' | '\\')),
            "profile name cannot contain control characters, / or \\, or be . or .."
        );
        anyhow::ensure!(
            self.visual.scene < WORLD_COUNT,
            "visual.scene must be 0 through {}",
            WORLD_COUNT - 1
        );
        anyhow::ensure!(self.visual.look < 10, "visual.look must be 0 through 9");
        for (field, value, min, max) in [
            ("visual.intensity", self.visual.intensity, 0.15, 1.6),
            ("visual.motion", self.visual.motion, 0.05, 1.8),
            ("visual.complexity", self.visual.complexity, 0.15, 1.5),
            ("visual.hue", self.visual.hue, 0.0, 1.0),
            ("visual.glow", self.visual.glow, 0.0, 1.6),
            ("visual.warp", self.visual.warp, 0.0, 1.6),
            ("visual.feedback", self.visual.feedback, 0.0, 0.92),
            (
                "mappings.bass_to_pulse",
                self.mappings.bass_to_pulse,
                0.0,
                1.5,
            ),
            (
                "mappings.mids_to_geometry",
                self.mappings.mids_to_geometry,
                0.0,
                1.5,
            ),
            (
                "mappings.treble_to_sparkle",
                self.mappings.treble_to_sparkle,
                0.0,
                1.5,
            ),
            (
                "mappings.beat_to_flash",
                self.mappings.beat_to_flash,
                0.0,
                1.5,
            ),
            ("director_freedom", self.director_freedom, 0.0, 1.0),
        ] {
            anyhow::ensure!(
                value.is_finite() && (min..=max).contains(&value),
                "{field} must be a finite number between {min} and {max}"
            );
        }
        Ok(())
    }
}

pub fn built_in_profiles() -> Vec<Profile> {
    let astral = Profile::astral_bloom();
    let mut neon = astral.clone();
    neon.name = "Liquid Reverie".into();
    neon.visual.scene = 3;
    neon.visual.hue = 0.88;
    neon.visual.motion = 1.05;
    neon.visual.complexity = 1.15;
    neon.visual.feedback = 0.64;
    neon.visual.warp = 1.0;

    let mut velvet = astral.clone();
    velvet.name = "Velvet Orbit".into();
    velvet.visual.scene = 2;
    velvet.visual.hue = 0.57;
    velvet.visual.motion = 0.36;
    velvet.visual.glow = 0.72;
    velvet.director_enabled = false;

    let mut acid = astral.clone();
    acid.name = "Acid Pulse".into();
    acid.visual.scene = 1;
    acid.visual.hue = 0.19;
    acid.visual.motion = 1.32;
    acid.visual.complexity = 1.3;
    acid.visual.warp = 1.35;
    acid.mappings.beat_to_flash = 1.25;

    let mut water = astral.clone();
    water.name = "Prismatic Water".into();
    water.visual.scene = 8;
    water.visual.motion = 0.70;
    water.visual.complexity = 1.0;
    water.visual.hue = 0.72;
    water.visual.glow = 0.82;
    water.visual.warp = 0.62;
    water.visual.feedback = 0.12;
    water.director_enabled = false;
    water.audio_mode = AudioMode::System;

    let mut aurora = astral.clone();
    aurora.name = "Northern Lights".into();
    aurora.visual.scene = 9;
    aurora.visual.motion = 0.58;
    aurora.visual.complexity = 1.1;
    aurora.visual.hue = 0.72;
    aurora.visual.glow = 0.90;
    aurora.visual.feedback = 0.15;
    aurora.director_enabled = false;
    aurora.audio_mode = AudioMode::System;

    let mut mandelbrot = astral.clone();
    mandelbrot.name = "Mandelbrot Voyage".into();
    mandelbrot.visual.scene = 10;
    mandelbrot.visual.motion = 0.65;
    mandelbrot.visual.complexity = 1.1;
    mandelbrot.visual.hue = 0.72;
    mandelbrot.visual.glow = 0.75;
    mandelbrot.visual.feedback = 0.0;
    mandelbrot.director_enabled = false;
    mandelbrot.audio_mode = AudioMode::System;

    let mut glass = astral.clone();
    glass.name = "Liquid Stained Glass".into();
    glass.visual.scene = 11;
    glass.visual.feedback = 0.0;
    glass.audio_mode = AudioMode::System;
    glass.director_enabled = false;
    let mut silk = astral.clone();
    silk.name = "Disco Silk".into();
    silk.visual.scene = 12;
    silk.visual.feedback = 0.08;
    silk.audio_mode = AudioMode::System;
    silk.director_enabled = false;
    let mut memory = astral.clone();
    memory.name = "Neon Memory".into();
    memory.visual.scene = 13;
    memory.visual.feedback = 0.38;
    memory.audio_mode = AudioMode::System;
    memory.director_enabled = false;
    let mut coast = astral.clone();
    coast.name = "Neon Coast".into();
    coast.visual.scene = 14;
    coast.visual.motion = 0.80;
    coast.visual.feedback = 0.0;
    coast.audio_mode = AudioMode::System;
    coast.director_enabled = false;
    vec![
        astral, neon, velvet, acid, water, aurora, mandelbrot, glass, silk, memory, coast,
    ]
}

pub struct ProfileStore {
    directory: PathBuf,
}

impl ProfileStore {
    pub fn new() -> Self {
        let directory = ProjectDirs::from("com", "amara", "amaraMusicVisualiser")
            .map(|dirs| dirs.data_local_dir().join("profiles"))
            .unwrap_or_else(|| PathBuf::from("profiles"));
        Self { directory }
    }

    pub fn load_all(&self) -> Vec<Profile> {
        let Ok(entries) = fs::read_dir(&self.directory) else {
            return Vec::new();
        };
        let mut profiles: Vec<_> = entries
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|entry| Self::import(&entry.path()).ok())
            .collect();
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        profiles
    }

    pub fn save(&self, profile: &Profile) -> anyhow::Result<PathBuf> {
        profile.validate()?;
        fs::create_dir_all(&self.directory).context("cannot create the profiles folder")?;
        let path = self
            .directory
            .join(format!("{}.json", safe_filename(&profile.name)));
        // Serialise saves by cooperating app instances, including ownership checks.
        let _lock = SaveLock::acquire(&path)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                anyhow::ensure!(
                    metadata.is_file(),
                    "profile destination is not a regular file"
                );
                let existing = Self::import(&path)
                    .context("existing profile is invalid; move it aside before saving")?;
                anyhow::ensure!(
                    existing.name == profile.name,
                    "filename collision with profile '{}'; choose a different profile name",
                    existing.name
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("cannot inspect the profile destination"),
        }
        atomic_write(&path, &serde_json::to_vec_pretty(profile)?)?;
        Ok(path)
    }

    pub fn import(path: &Path) -> anyhow::Result<Profile> {
        let metadata = fs::symlink_metadata(path).context("cannot inspect the profile")?;
        anyhow::ensure!(
            metadata.is_file(),
            "profile must be a regular file, not a symlink"
        );
        let file = File::open(path).context("cannot open the profile")?;
        let metadata = file.metadata().context("cannot inspect the profile")?;
        anyhow::ensure!(metadata.is_file(), "profile must be a regular file");
        anyhow::ensure!(
            metadata.len() <= MAX_PROFILE_BYTES,
            "profile exceeds the 64 KiB size limit"
        );
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take(MAX_PROFILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .context("cannot read the profile")?;
        anyhow::ensure!(
            bytes.len() as u64 <= MAX_PROFILE_BYTES,
            "profile exceeds the 64 KiB size limit"
        );
        let profile: Profile = serde_json::from_slice(&bytes).context("invalid profile JSON")?;
        profile.validate()?;
        Ok(profile)
    }

    /// Replaces the explicitly chosen export file, after the Save dialog's confirmation.
    pub fn export(profile: &Profile, path: &Path) -> anyhow::Result<()> {
        profile.validate()?;
        atomic_write(path, &serde_json::to_vec_pretty(profile)?)
    }
}

struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

struct SaveLock {
    _cleanup: RemoveOnDrop,
    _file: File,
}

impl SaveLock {
    fn acquire(path: &Path) -> anyhow::Result<Self> {
        let lock_path = path.with_extension("json.lock");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .with_context(|| {
                format!(
                    "cannot lock profile for saving; another save may be active (lock: {})",
                    lock_path.display()
                )
            })?;
        Ok(Self {
            _cleanup: RemoveOnDrop(lock_path),
            _file: file,
        })
    }
}

/// Write beside the destination so rename commits the complete file atomically.
fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    anyhow::ensure!(path.file_name().is_some(), "choose a profile filename");
    if let Ok(metadata) = fs::symlink_metadata(path) {
        anyhow::ensure!(
            metadata.is_file(),
            "destination must be a regular file, not a directory or symlink"
        );
    }
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_path = directory.join(format!(
        ".amara-profile-{}-{sequence}.tmp",
        std::process::id()
    ));
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .context("cannot create a temporary profile beside the destination")?;
    let _cleanup = RemoveOnDrop(temp_path.clone());
    temp.write_all(bytes)
        .context("cannot write the complete profile")?;
    temp.sync_all()
        .context("cannot sync the profile contents")?;
    drop(temp);
    fs::rename(&temp_path, path)
        .context("cannot commit the profile; original file was preserved")?;
    File::open(directory)
        .and_then(|directory| directory.sync_all())
        .context("profile was committed, but syncing its folder failed")?;
    Ok(())
}

fn safe_filename(name: &str) -> String {
    let mut value: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect();
    value.truncate(64);
    let value = value.trim_matches('-').to_owned();
    if value.is_empty() {
        "profile".into()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "amara-profile-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn store(&self) -> ProfileStore {
            ProfileStore {
                directory: self.0.join("profiles"),
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn profile_round_trip() {
        let source = Profile::astral_bloom();
        let json = serde_json::to_string(&source).unwrap();
        let decoded: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, source);
        decoded.validate().unwrap();
    }

    #[test]
    fn legacy_profiles_default_to_auto_background_without_losing_choices() {
        let mut profile = Profile::astral_bloom();
        profile.visual.palette = VisualPalette::MoccaVanilla;
        profile.visual.scene = 8;
        profile.visual.look = 7;
        let mut json = serde_json::to_value(&profile).unwrap();
        json["visual"].as_object_mut().unwrap().remove("background");
        let restored: Profile = serde_json::from_value(json).unwrap();
        assert_eq!(restored, profile);
        assert_eq!(restored.visual.background, BackgroundPalette::Auto);
        restored.validate().unwrap();
    }

    #[test]
    fn every_background_choice_round_trips_with_the_profile() {
        let directory = TestDirectory::new();
        let store = directory.store();
        let mut profile = Profile::astral_bloom();
        for background in BackgroundPalette::ALL {
            profile.visual.background = background;
            let path = store.save(&profile).unwrap();
            assert_eq!(ProfileStore::import(&path).unwrap(), profile);
        }
    }

    #[test]
    fn palette_schema_uses_explicit_strings_and_stable_renderer_amounts() {
        assert_eq!(VisualPalette::default(), VisualPalette::Cosmic);
        for (palette, json, label, amount) in [
            (VisualPalette::Cosmic, "\"Cosmic\"", "Cosmic", 0.0),
            (
                VisualPalette::MoccaVanilla,
                "\"MoccaVanilla\"",
                "Mocca & Vanilla",
                1.0,
            ),
        ] {
            assert_eq!(serde_json::to_string(&palette).unwrap(), json);
            assert_eq!(
                serde_json::from_str::<VisualPalette>(json).unwrap(),
                palette
            );
            assert_eq!(palette.label(), label);
            assert_eq!(palette.amount(), amount);
        }
    }

    #[test]
    fn palette_round_trip_through_save_load_export_and_import() {
        let directory = TestDirectory::new();
        let store = directory.store();
        let mut profile = Profile::astral_bloom();
        profile.visual.palette = VisualPalette::MoccaVanilla;
        let path = store.save(&profile).unwrap();
        assert_eq!(ProfileStore::import(&path).unwrap(), profile);
        assert_eq!(store.load_all(), vec![profile.clone()]);
        let export = directory.0.join("mocca.json");
        ProfileStore::export(&profile, &export).unwrap();
        assert_eq!(ProfileStore::import(&export).unwrap(), profile);
    }

    #[test]
    fn math_form_schema_and_family_ids_are_explicit_and_stable() {
        assert_eq!(MathForm::default(), MathForm::Journey);
        for (form, encoded, label, family) in [
            (MathForm::Journey, "Journey", "Journey", None),
            (MathForm::Julia, "Julia", "Julia", Some(0)),
            (
                MathForm::InversionLace,
                "InversionLace",
                "Inversion Lace",
                Some(1),
            ),
            (
                MathForm::HarmonicSculpture,
                "HarmonicSculpture",
                "Harmonic Sculpture",
                Some(2),
            ),
            (
                MathForm::VortexWeave,
                "VortexWeave",
                "Vortex Weave",
                Some(3),
            ),
        ] {
            assert_eq!(
                serde_json::to_value(form).unwrap(),
                serde_json::json!(encoded)
            );
            assert_eq!(
                serde_json::from_value::<MathForm>(serde_json::json!(encoded)).unwrap(),
                form
            );
            assert_eq!(form.label(), label);
            assert_eq!(form.family(), family);
        }
    }

    #[test]
    fn every_math_form_round_trips_without_changing_the_saved_palette() {
        let directory = TestDirectory::new();
        let store = directory.store();
        let export = directory.0.join("math.json");
        let mut profile = Profile::astral_bloom();
        profile.visual.palette = VisualPalette::MoccaVanilla;
        for form in [
            MathForm::Journey,
            MathForm::Julia,
            MathForm::InversionLace,
            MathForm::HarmonicSculpture,
            MathForm::VortexWeave,
        ] {
            profile.visual.math_form = form;
            let saved = store.save(&profile).unwrap();
            assert_eq!(ProfileStore::import(&saved).unwrap(), profile);
            assert_eq!(store.load_all(), vec![profile.clone()]);
            ProfileStore::export(&profile, &export).unwrap();
            assert_eq!(ProfileStore::import(&export).unwrap(), profile);
        }
    }

    #[test]
    fn version_one_without_look_keeps_original_geometry_and_other_choices() {
        let directory = TestDirectory::new();
        let path = directory.0.join("legacy-look.json");
        let mut profile = Profile::astral_bloom();
        profile.visual.palette = VisualPalette::MoccaVanilla;
        profile.visual.math_form = MathForm::InversionLace;
        let mut json = serde_json::to_value(&profile).unwrap();
        json["visual"].as_object_mut().unwrap().remove("look");
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(ProfileStore::import(&path).unwrap(), profile);
        assert_eq!(VisualSettings::default().look, 0);
        assert!(
            built_in_profiles()
                .iter()
                .all(|profile| profile.visual.look == 0)
        );
        for bundled in [
            include_str!("../presets/Velvet-Flight.json"),
            include_str!("../presets/Liquid-Light.json"),
            include_str!("../presets/Fractal-Mocca.json"),
        ] {
            assert_eq!(
                serde_json::from_str::<Profile>(bundled)
                    .unwrap()
                    .visual
                    .look,
                0
            );
        }
        // A v1 document predating all four optional visual additions remains valid.
        for optional in ["feedback", "palette", "math_form"] {
            json["visual"].as_object_mut().unwrap().remove(optional);
        }
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(
            ProfileStore::import(&path).unwrap(),
            Profile::astral_bloom()
        );
    }

    #[test]
    fn all_ten_looks_round_trip_for_every_world_retaining_palette_and_math() {
        let directory = TestDirectory::new();
        let store = directory.store();
        let export = directory.0.join("look.json");
        let forms = [
            MathForm::Journey,
            MathForm::Julia,
            MathForm::InversionLace,
            MathForm::HarmonicSculpture,
            MathForm::VortexWeave,
        ];
        let mut profile = Profile::astral_bloom();
        for scene in 0..WORLD_COUNT {
            for look in 0..10 {
                for palette in [VisualPalette::Cosmic, VisualPalette::MoccaVanilla] {
                    profile.visual.scene = scene;
                    profile.visual.look = look;
                    profile.visual.palette = palette;
                    profile.visual.math_form = forms[look % forms.len()];
                    let saved = store.save(&profile).unwrap();
                    assert_eq!(ProfileStore::import(&saved).unwrap(), profile);
                    assert_eq!(store.load_all(), vec![profile.clone()]);
                    ProfileStore::export(&profile, &export).unwrap();
                    assert_eq!(ProfileStore::import(&export).unwrap(), profile);
                }
            }
        }
    }

    #[test]
    fn world_schema_preserves_old_indices_and_rejects_the_next_index() {
        assert_eq!(WORLD_COUNT, 17);
        let directory = TestDirectory::new();
        let path = directory.0.join("world.json");
        for scene in 0..WORLD_COUNT {
            let mut profile = Profile::astral_bloom();
            profile.visual.scene = scene;
            let mut json = serde_json::to_value(&profile).unwrap();
            // Old v1 profiles retain their world indices even without the
            // subsequently introduced palette, math and structural-look fields.
            for optional in ["feedback", "palette", "math_form", "look"] {
                json["visual"].as_object_mut().unwrap().remove(optional);
            }
            fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
            assert_eq!(ProfileStore::import(&path).unwrap(), profile);
        }
        let mut invalid = Profile::astral_bloom();
        invalid.visual.scene = WORLD_COUNT;
        assert!(invalid.validate().is_err());
        fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(ProfileStore::import(&path).is_err());
    }

    #[test]
    fn invalid_or_unknown_looks_are_rejected_by_import_load_save_and_export() {
        let directory = TestDirectory::new();
        let store = directory.store();
        let profile = Profile::astral_bloom();
        let saved = store.save(&profile).unwrap();
        let original = fs::read(&saved).unwrap();
        let path = store.directory.join("invalid-look.json");
        for invalid in [
            serde_json::json!(10),
            serde_json::json!(usize::MAX),
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!("UnknownLook"),
            serde_json::json!(true),
            serde_json::Value::Null,
        ] {
            let mut json = serde_json::to_value(&profile).unwrap();
            json["visual"]["look"] = invalid;
            fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
            assert!(ProfileStore::import(&path).is_err());
            assert_eq!(store.load_all(), vec![profile.clone()]);
        }
        for look in [10, usize::MAX] {
            let mut invalid = profile.clone();
            invalid.visual.look = look;
            assert!(store.save(&invalid).is_err());
            assert!(ProfileStore::export(&invalid, &saved).is_err());
            assert_eq!(fs::read(&saved).unwrap(), original);
        }
        let mut unknown = serde_json::to_value(&profile).unwrap();
        unknown["visual"]["unknown_look"] = serde_json::json!(0);
        fs::write(&path, serde_json::to_vec(&unknown).unwrap()).unwrap();
        assert!(ProfileStore::import(&path).is_err());
    }

    #[test]
    fn version_one_without_math_form_defaults_to_journey() {
        let directory = TestDirectory::new();
        let path = directory.0.join("legacy-math.json");
        let profile = Profile::astral_bloom();
        let mut json = serde_json::to_value(&profile).unwrap();
        json["visual"].as_object_mut().unwrap().remove("math_form");
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(ProfileStore::import(&path).unwrap(), profile);
        assert!(
            built_in_profiles()
                .iter()
                .all(|profile| profile.visual.math_form == MathForm::Journey)
        );
        for bundled in [
            include_str!("../presets/Velvet-Flight.json"),
            include_str!("../presets/Liquid-Light.json"),
            include_str!("../presets/Fractal-Mocca.json"),
        ] {
            let decoded: Profile = serde_json::from_str(bundled).unwrap();
            assert_eq!(decoded.visual.math_form, MathForm::Journey);
        }
    }

    #[test]
    fn unsupported_math_forms_are_rejected_instead_of_silently_becoming_journey() {
        let directory = TestDirectory::new();
        let path = directory.0.join("invalid-math.json");
        let mut json = serde_json::to_value(Profile::astral_bloom()).unwrap();
        for invalid in [
            serde_json::json!("UnknownForm"),
            serde_json::json!(""),
            serde_json::json!(1),
            serde_json::Value::Null,
        ] {
            json["visual"]["math_form"] = invalid;
            fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
            assert!(ProfileStore::import(&path).is_err());
        }
    }

    #[test]
    fn unsupported_palettes_are_rejected_without_weakening_visual_schema() {
        let directory = TestDirectory::new();
        let path = directory.0.join("unsupported.json");
        let mut json = serde_json::to_value(Profile::astral_bloom()).unwrap();
        for palette in [
            serde_json::json!("UnknownPalette"),
            serde_json::json!(1),
            serde_json::Value::Null,
        ] {
            json["visual"]["palette"] = palette;
            fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
            assert!(ProfileStore::import(&path).is_err());
        }
        json["visual"]["palette"] = serde_json::json!("MoccaVanilla");
        json["visual"]["unknown_setting"] = serde_json::json!(true);
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(ProfileStore::import(&path).is_err());
    }

    #[test]
    fn profile_name_is_safe() {
        assert_eq!(safe_filename("My cosmic / profile"), "My-cosmic---profile");
        assert_eq!(safe_filename("../../outside"), "outside");
        assert_eq!(safe_filename("星空"), "profile");
    }

    #[test]
    fn built_in_profiles_are_valid() {
        assert_eq!(built_in_profiles().len(), BUILT_IN_PROFILE_COUNT);
        for profile in built_in_profiles() {
            profile.validate().unwrap();
        }
    }

    #[test]
    fn prismatic_water_preset_is_a_held_music_driven_ninth_world() {
        let profile = built_in_profiles()
            .into_iter()
            .find(|profile| profile.name == "Prismatic Water")
            .expect("the water world has a selectable built-in preset");
        assert_eq!(profile.visual.scene, 8);
        assert_eq!(profile.visual.look, 0);
        assert_eq!(profile.audio_mode, AudioMode::System);
        assert!(!profile.director_enabled);
        assert_eq!(profile.visual.palette, VisualPalette::Cosmic);
        profile.validate().unwrap();
        let encoded = serde_json::to_string(&profile).unwrap();
        assert_eq!(serde_json::from_str::<Profile>(&encoded).unwrap(), profile);
    }

    #[test]
    fn bundled_style_profiles_are_valid_and_portable() {
        for source in [
            include_str!("../presets/Velvet-Flight.json"),
            include_str!("../presets/Liquid-Light.json"),
        ] {
            let profile: Profile = serde_json::from_str(source).unwrap();
            profile.validate().unwrap();
            let encoded = serde_json::to_string(&profile).unwrap();
            assert_eq!(profile, serde_json::from_str::<Profile>(&encoded).unwrap());
            assert_eq!(profile.audio_mode, AudioMode::Demo);
            assert!(!profile.director_enabled);
        }
    }

    #[test]
    fn bundled_fractal_mocca_is_warm_explicit_and_portable() {
        let profile: Profile =
            serde_json::from_str(include_str!("../presets/Fractal-Mocca.json")).unwrap();
        profile.validate().unwrap();
        assert_eq!(profile.name, "Fractal Mocca");
        assert_eq!(profile.visual.scene, 1);
        assert_eq!(profile.visual.palette, VisualPalette::MoccaVanilla);
        assert_eq!(profile.visual.complexity, 1.35);
        assert_eq!(profile.visual.motion, 1.10);
        assert_eq!(profile.visual.hue, 0.72);
        assert_eq!(profile.visual.intensity, 0.85);
        assert_eq!(profile.visual.glow, 0.8);
        assert_eq!(profile.visual.warp, 1.0);
        assert_eq!(profile.visual.feedback, 0.55);
        assert_eq!(profile.audio_mode, AudioMode::System);
        assert!(!profile.director_enabled);
        let json = serde_json::to_string(&profile).unwrap();
        assert_eq!(serde_json::from_str::<Profile>(&json).unwrap(), profile);
    }

    #[test]
    fn save_load_replace_export_and_import_use_real_files() {
        let directory = TestDirectory::new();
        let store = directory.store();
        assert!(store.load_all().is_empty());
        let mut profile = Profile::astral_bloom();
        profile.name = "My Universe".into();
        let original_path = store.save(&profile).unwrap();
        assert_eq!(store.load_all(), vec![profile.clone()]);
        profile.visual.hue = 0.13;
        profile.visual.feedback = 0.7;
        let replacement_path = store.save(&profile).unwrap();
        assert_eq!(original_path, replacement_path);
        assert_eq!(store.load_all(), vec![profile.clone()]);
        let export_path = directory.0.join("export.json");
        ProfileStore::export(&profile, &export_path).unwrap();
        assert_eq!(ProfileStore::import(&export_path).unwrap(), profile);
        profile.visual.hue = 0.94;
        ProfileStore::export(&profile, &export_path).unwrap();
        assert_eq!(ProfileStore::import(&export_path).unwrap(), profile);
        let saved_files = fs::read_dir(&store.directory).unwrap().count();
        assert_eq!(saved_files, 1, "no temporary files or locks should remain");
    }

    #[test]
    fn version_one_without_feedback_uses_backward_compatible_default() {
        let directory = TestDirectory::new();
        let path = directory.0.join("legacy.json");
        let profile = Profile::astral_bloom();
        let mut json = serde_json::to_value(&profile).unwrap();
        json["visual"].as_object_mut().unwrap().remove("feedback");
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(ProfileStore::import(&path).unwrap(), profile);
    }

    #[test]
    fn version_one_without_palette_retains_cosmic_style() {
        let directory = TestDirectory::new();
        let path = directory.0.join("legacy-palette.json");
        let profile = Profile::astral_bloom();
        let mut json = serde_json::to_value(&profile).unwrap();
        json["visual"].as_object_mut().unwrap().remove("palette");
        json["visual"].as_object_mut().unwrap().remove("feedback");
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(ProfileStore::import(&path).unwrap(), profile);
        assert!(
            built_in_profiles()
                .iter()
                .all(|profile| profile.visual.palette == VisualPalette::Cosmic)
        );
    }

    #[test]
    fn names_cannot_traverse_paths_or_be_ambiguous() {
        let directory = TestDirectory::new();
        let store = directory.store();
        for name in [
            "",
            " ",
            " leading",
            "trailing ",
            ".",
            "..",
            "../outside",
            "nested/name",
            "nested\\name",
            "bad\nname",
        ] {
            let mut profile = Profile::astral_bloom();
            profile.name = name.into();
            assert!(
                store.save(&profile).is_err(),
                "unexpectedly accepted {name:?}"
            );
        }
        let mut profile = Profile::astral_bloom();
        profile.name = "a".repeat(MAX_NAME_CHARACTERS + 1);
        assert!(store.save(&profile).is_err());
        assert!(
            !store.directory.exists(),
            "invalid saves must not create the folder"
        );
    }

    #[test]
    fn filename_collisions_preserve_the_original() {
        let directory = TestDirectory::new();
        let store = directory.store();
        let mut original = Profile::astral_bloom();
        original.name = "Cosmic?".into();
        let path = store.save(&original).unwrap();
        let bytes = fs::read(&path).unwrap();
        let mut collision = original.clone();
        collision.name = "Cosmic!".into();
        let error = store.save(&collision).unwrap_err().to_string();
        assert!(error.contains("filename collision"));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(store.load_all(), vec![original]);
    }

    #[test]
    fn invalid_values_and_versions_are_rejected_before_save() {
        let directory = TestDirectory::new();
        let store = directory.store();
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.7] {
            let mut profile = Profile::astral_bloom();
            profile.visual.glow = invalid;
            assert!(store.save(&profile).is_err());
        }
        let mut profile = Profile::astral_bloom();
        profile.visual.scene = usize::MAX;
        assert!(store.save(&profile).is_err());
        profile.visual.scene = 0;
        profile.visual.feedback = 0.93;
        assert!(store.save(&profile).is_err());
        profile.visual.feedback = 0.22;
        profile.mappings.beat_to_flash = 1.6;
        assert!(store.save(&profile).is_err());
        profile.mappings.beat_to_flash = 0.8;
        profile.director_freedom = 1.1;
        assert!(store.save(&profile).is_err());
        profile.director_freedom = 0.46;
        profile.format_version = 2;
        assert!(store.save(&profile).is_err());
    }

    #[test]
    fn import_and_loading_enforce_the_same_schema_and_size_limit() {
        let directory = TestDirectory::new();
        let store = directory.store();
        let profile = Profile::astral_bloom();
        store.save(&profile).unwrap();
        let bad_path = store.directory.join("invalid.json");
        for (key, value) in [
            ("format_version", serde_json::json!(99)),
            ("director_freedom", serde_json::json!(2.0)),
            ("unrecognized_setting", serde_json::json!(true)),
        ] {
            let mut json = serde_json::to_value(&profile).unwrap();
            json[key] = value;
            fs::write(&bad_path, serde_json::to_vec(&json).unwrap()).unwrap();
            assert!(ProfileStore::import(&bad_path).is_err());
            assert_eq!(store.load_all(), vec![profile.clone()]);
        }
        fs::write(&bad_path, b"{ not json }").unwrap();
        assert!(ProfileStore::import(&bad_path).is_err());
        fs::write(&bad_path, vec![b' '; MAX_PROFILE_BYTES as usize + 1]).unwrap();
        assert!(
            ProfileStore::import(&bad_path)
                .unwrap_err()
                .to_string()
                .contains("64 KiB")
        );
        assert_eq!(store.load_all(), vec![profile]);
    }

    #[test]
    fn corrupted_destinations_and_active_save_locks_are_preserved() {
        let directory = TestDirectory::new();
        let store = directory.store();
        let profile = Profile::astral_bloom();
        let path = store.save(&profile).unwrap();
        let lock_path = path.with_extension("json.lock");
        fs::write(&lock_path, b"another instance").unwrap();
        assert!(store.save(&profile).is_err());
        assert_eq!(ProfileStore::import(&path).unwrap(), profile);
        fs::remove_file(lock_path).unwrap();
        fs::write(&path, b"damaged original").unwrap();
        assert!(store.save(&profile).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"damaged original");
    }

    #[test]
    fn invalid_export_keeps_existing_file_and_directory_destinations_are_rejected() {
        let directory = TestDirectory::new();
        let path = directory.0.join("export.json");
        fs::write(&path, b"keep me").unwrap();
        let mut profile = Profile::astral_bloom();
        profile.visual.motion = 0.0;
        assert!(ProfileStore::export(&profile, &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"keep me");
        profile.visual.motion = 0.75;
        assert!(ProfileStore::export(&profile, &directory.0).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_destinations_never_overwrite_the_link_or_target() {
        let directory = TestDirectory::new();
        let store = directory.store();
        let profile = Profile::astral_bloom();
        fs::create_dir_all(&store.directory).unwrap();
        let target = directory.0.join("keep.json");
        fs::write(&target, b"private original").unwrap();
        let link = store
            .directory
            .join(format!("{}.json", safe_filename(&profile.name)));
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(store.save(&profile).is_err());
        assert!(ProfileStore::export(&profile, &link).is_err());
        assert!(ProfileStore::import(&link).is_err());
        assert!(store.load_all().is_empty());
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(target).unwrap(), b"private original");
    }
}
