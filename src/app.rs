mod profile_dialog;
mod ui;

use profile_dialog::{Outcome as ProfileDialogOutcome, ProfileDialog};

use crate::{
    audio::{AudioController, AudioMode},
    auto_journey::AutoJourney,
    background::BackgroundJourney,
    choreography::Choreographer,
    director::{AsyncDirector, Direction, DirectorInput},
    dsp::{AudioFeatures, FeatureWorker},
    frame_clock::FrameClock,
    look_journey::LookJourney,
    material_colour::MaterialColourJourney,
    math_journey::{MathJourney, MathJourneyState},
    meters::MeterDisplay,
    motion::{MotionPace, RenderMotion, motion_rate, motion_value},
    music_response::MusicResponse,
    presets::{
        BUILT_IN_PROFILE_COUNT, BackgroundPalette, MathForm, Profile, ProfileStore, VisualPalette,
        VisualSettings, WORLD_COUNT, built_in_profiles,
    },
    ripples::RippleField,
    scene::SceneTransition,
    test_track::TestTrack,
    visual::{self, SCENES, ShaderUniforms},
    vocal_response::VocalResponse,
};
use eframe::egui;
use std::time::{Duration, Instant};

/// Readable construction time belongs to the current visible visit, not the
/// app's arbitrary global clock. Partial entrance/exit exposure counts only
/// proportionally, so a long world melt cannot spend the unseen construction.
pub(crate) struct MandelbrotVisit {
    age: f64,
    velocity: f64,
    manual: Option<MandelbrotManualJourney>,
    manual_mix: f64,
    next_region: bool,
}

impl Default for MandelbrotVisit {
    fn default() -> Self {
        Self {
            age: 0.0,
            velocity: 1.0,
            manual: None,
            manual_mix: 0.0,
            next_region: false,
        }
    }
}

struct MandelbrotLeg {
    from: f64,
    to: f64,
    initial_velocity: f64,
    seconds: f64,
    elapsed: f64,
}

impl MandelbrotLeg {
    fn new(from: f64, to: f64, velocity: f64, seconds: f64) -> Self {
        let distance = to - from;
        // Carrying a fast inward tangent into a very short remaining range
        // must shorten the leg; otherwise the cubic overshoots zero and its
        // clamp would turn a quick mode toggle into a stationary plateau.
        let seconds = if velocity * distance > 0.0 {
            seconds.min(2.9 * distance.abs() / velocity.abs())
        } else {
            seconds
        };
        Self {
            from,
            to,
            initial_velocity: velocity,
            seconds: seconds.max(0.00001),
            elapsed: 0.0,
        }
    }

    fn sample(&self) -> (f64, f64) {
        let t = (self.elapsed / self.seconds).clamp(0.0, 1.0);
        // Hermite endpoints carry the incoming velocity and ease through the
        // turn. A turn has one zero-velocity instant, never a held plateau.
        let age = (2.0 * t * t * t - 3.0 * t * t + 1.0) * self.from
            + (t * t * t - 2.0 * t * t + t) * self.seconds * self.initial_velocity
            + (-2.0 * t * t * t + 3.0 * t * t) * self.to;
        let velocity = (6.0 * t * t - 6.0 * t) * (self.from - self.to) / self.seconds
            + (3.0 * t * t - 4.0 * t + 1.0) * self.initial_velocity;
        (age.max(0.0), velocity)
    }
}

struct MandelbrotManualJourney {
    leg: Option<MandelbrotLeg>,
    returning: bool,
    look: usize,
    choice_exposure: f64,
}

impl MandelbrotManualJourney {
    fn new(age: f64, velocity: f64, look: usize) -> Self {
        let mut journey = Self {
            leg: None,
            returning: false,
            look,
            choice_exposure: 0.0,
        };
        if age > 28.0 {
            // Old Auto visits may have accumulated minutes of already-complete
            // detail. Traverse that visually saturated age smoothly, rather
            // than spending a minute returning or snapping to a new clock.
            journey.leg = Some(MandelbrotLeg::new(age, 28.0, velocity, 3.0));
        } else if age >= journey.peak() - 3.0 || velocity < 0.0 {
            journey.leg = Some(MandelbrotLeg::new(age, 0.0, velocity, 18.0));
            journey.returning = true;
        } else if age >= 10.0 {
            journey.start_dive(age, velocity);
        }
        journey
    }

    fn peak(&self) -> f64 {
        22.0 + (self.look * 3 % 5) as f64 * 0.5
    }
    fn dive_seconds(&self) -> f64 {
        22.0 + (self.look * 7 % 5) as f64 * 0.75
    }

    fn start_dive(&mut self, age: f64, velocity: f64) {
        self.returning = false;
        self.leg = Some(MandelbrotLeg::new(
            age,
            self.peak(),
            velocity,
            (self.dive_seconds() - age).max(6.0),
        ));
    }

    fn advance(&mut self, age: &mut f64, velocity: &mut f64, dt: f64, look: usize) -> bool {
        if self.look != look {
            self.look = look;
            self.choice_exposure = 0.0;
        }
        self.choice_exposure += dt;
        let mut remaining = dt;
        let mut request = false;
        while remaining > 1e-10 {
            let Some(leg) = &mut self.leg else {
                // Preserve the liked first reveal before beginning the journey.
                let spent = remaining.min((10.0 - *age).max(0.0));
                *age += spent;
                *velocity = 1.0;
                remaining -= spent;
                if *age >= 10.0 {
                    self.start_dive(*age, *velocity);
                }
                continue;
            };
            let spent = remaining.min(leg.seconds - leg.elapsed);
            leg.elapsed += spent;
            remaining -= spent;
            (*age, *velocity) = leg.sample();
            if leg.elapsed >= leg.seconds - 1e-10 {
                if self.returning {
                    // Region changes happen only at broad framing. The normal
                    // look lane still blends pigment and camera continuously.
                    request |= self.choice_exposure >= 18.0;
                    self.start_dive(*age, *velocity);
                } else {
                    self.returning = true;
                    self.leg = Some(MandelbrotLeg::new(
                        *age,
                        0.0,
                        *velocity,
                        16.0 + (self.look % 3) as f64,
                    ));
                }
            }
        }
        request
    }
}

impl MandelbrotVisit {
    /// Run the same continuous unfold/detail/pullback journey in both modes.
    /// Auto does not replace a region midway through its camera approach.
    /// The optional returned choice only updates the manual selector; Auto's
    /// current/incoming names remain sourced directly from the actual lane.
    pub(crate) fn update_for_mode(
        &mut self,
        scene: &SceneTransition,
        dt: f32,
        window_visible: bool,
        auto: bool,
        selected: usize,
        looks: &mut LookJourney,
    ) -> (f32, Option<usize>) {
        looks.guide_mandelbrot_with_camera();
        let look = if auto || scene.to != 10 {
            looks.current(10)
        } else {
            selected
        };
        let age = self.update(scene, dt, window_visible, Some(look));
        let mut manual_choice = None;
        if self.take_next_region()
            && scene.to == 10
            && let Some(next) = looks.next_manual_mandelbrot_region(look)
        {
            self.selected_region(next);
            if !auto {
                manual_choice = Some(next);
            }
        }
        (age, manual_choice)
    }

    pub(crate) fn update(
        &mut self,
        scene: &SceneTransition,
        dt: f32,
        window_visible: bool,
        manual_look: Option<usize>,
    ) -> f32 {
        self.next_region = false;
        const MANDELBROT: usize = 10;
        let blend = scene.blend();
        let exposure = if scene.from == MANDELBROT && scene.to == MANDELBROT {
            1.0
        } else if scene.to == MANDELBROT {
            blend
        } else if scene.from == MANDELBROT {
            1.0 - blend
        } else {
            0.0
        };
        if exposure <= 0.0 {
            *self = Self::default();
        } else if window_visible && dt.is_finite() && dt > 0.0 {
            let dt = f64::from(dt.min(0.1) * exposure);
            let target = if manual_look.is_some() { 1.0 } else { 0.0 };
            self.manual_mix += (target - self.manual_mix) * (1.0 - (-dt / 1.5).exp());
            if let Some(look) = manual_look {
                let journey = self.manual.get_or_insert_with(|| {
                    MandelbrotManualJourney::new(self.age, self.velocity, look)
                });
                self.next_region = journey.advance(&mut self.age, &mut self.velocity, dt, look);
            } else {
                self.manual = None;
                // Leaving manual during a pullback keeps both position and
                // velocity continuous while returning to Auto's forward clock.
                let decay = (-dt / 0.8).exp();
                self.age =
                    (self.age + dt + (self.velocity - 1.0) * 0.8 * (1.0 - decay)).clamp(0.0, 120.0);
                self.velocity = 1.0 + (self.velocity - 1.0) * decay;
                if (self.age == 0.0 && self.velocity < 0.0)
                    || (self.age == 120.0 && self.velocity > 0.0)
                {
                    self.velocity = 0.0;
                }
            }
        }
        self.age as f32
    }

    pub(crate) fn take_next_region(&mut self) -> bool {
        std::mem::take(&mut self.next_region)
    }

    pub(crate) fn manual_motion(&self) -> f32 {
        self.manual_mix as f32
    }

    pub(crate) fn selected_region(&mut self, look: usize) {
        if let Some(journey) = &mut self.manual {
            journey.look = look;
            journey.choice_exposure = 0.0;
        }
    }
}

pub struct MusicVisualiserApp {
    audio: AudioController,
    analysis: FeatureWorker,
    features: AudioFeatures,
    meters: MeterDisplay,
    render_motion: RenderMotion,
    choreographer: Choreographer,
    music_response: MusicResponse,
    ripples: RippleField,
    vocal_response: VocalResponse,
    math_journey: MathJourney,
    math_name: &'static str,
    auto_journey: AutoJourney,
    look_journey: LookJourney,
    background_journey: BackgroundJourney,
    material_colour_journey: MaterialColourJourney,
    auto_mode: bool,
    auto_mix: f32,
    auto_seed: f32,
    auto_morph_seconds: f32,
    audio_fresh: bool,
    audio_age: Option<Duration>,
    source_epoch: u64,
    director: AsyncDirector,
    direction: Option<Direction>,
    profile_store: ProfileStore,
    profile_dialog: ProfileDialog,
    profiles: Vec<Profile>,
    active_profile: usize,
    profile_name: String,
    profile: Profile,
    rendered_visual: VisualSettings,
    scene: SceneTransition,
    mandelbrot_visit: MandelbrotVisit,
    visual_time: f64,
    motion_pace: MotionPace,
    clock: FrameClock,
    last_director_submit: Instant,
    fullscreen: bool,
    show_controls: bool,
    show_equaliser: bool,
    sharp_rendering: bool,
    test_track: TestTrack,
    palette_mix: f32,
    ui_palette: VisualPalette,
    message: Option<(String, Instant)>,
    prepared_worlds: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    preparation_started: Instant,
}

impl MusicVisualiserApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let profile_store = ProfileStore::new();
        let mut profiles = built_in_profiles();
        profiles.extend(profile_store.load_all());
        let mut profile = cc
            .storage
            .and_then(|storage| eframe::get_value::<Profile>(storage, "last-profile"))
            .filter(|profile| profile.validate().is_ok())
            .unwrap_or_else(Profile::astral_bloom);
        // Listening is an app preference, not a side effect of choosing a look.
        // Upgrade from the old always-Demo behaviour to actual Mac playback.
        // Afterwards an explicit Demo choice remains respected across launches.
        let initial_mode = cc
            .storage
            .and_then(|storage| eframe::get_value::<AudioMode>(storage, "listening-source"))
            .unwrap_or(AudioMode::System);
        let sharp_rendering = cc
            .storage
            .and_then(|storage| eframe::get_value::<bool>(storage, "sharp-rendering"))
            .unwrap_or(true);
        let show_equaliser = cc
            .storage
            .and_then(|storage| eframe::get_value::<bool>(storage, "show-equaliser"))
            .unwrap_or(true);
        let show_controls = restore_sidebar_visibility(cc.storage);
        let auto_mode = cc
            .storage
            .and_then(|storage| eframe::get_value::<bool>(storage, "auto-mode"))
            .unwrap_or(true);
        profile.audio_mode = initial_mode;
        let active_profile = profiles
            .iter()
            .enumerate()
            .skip(BUILT_IN_PROFILE_COUNT)
            .find_map(|(index, item)| (*item == profile).then_some(index))
            .or_else(|| profiles.iter().position(|item| *item == profile))
            .unwrap_or_else(|| {
                profiles.push(profile.clone());
                profiles.len() - 1
            });
        let audio = AudioController::with_mode(initial_mode);
        let analysis = FeatureWorker::new(audio.shared());
        let palette_mix = profile.visual.palette.amount();
        let ui_palette = profile.visual.palette;
        ui::configure_style(&cc.egui_ctx, ui_palette);
        let auto_journey = AutoJourney::new(profile.visual.scene, auto_mode);
        let initial_scene = auto_journey.current_world();
        let prepared_worlds = visual::install(cc, initial_scene);
        Self {
            prepared_worlds,
            preparation_started: Instant::now(),
            audio,
            analysis,
            features: AudioFeatures::default(),
            meters: MeterDisplay::new(),
            render_motion: RenderMotion::new(),
            choreographer: Choreographer::new(),
            music_response: MusicResponse::new(),
            ripples: RippleField::default(),
            vocal_response: VocalResponse::default(),
            math_journey: MathJourney::new(),
            math_name: "Julia",
            auto_journey,
            look_journey: LookJourney::new(),
            background_journey: BackgroundJourney::new(profile.visual.background),
            material_colour_journey: MaterialColourJourney::new(auto_mode),
            auto_mode,
            auto_mix: if auto_mode { 1.0 } else { 0.0 },
            auto_seed: 0.0,
            auto_morph_seconds: 8.0,
            audio_fresh: false,
            audio_age: None,
            source_epoch: 0,
            director: AsyncDirector::new(),
            direction: None,
            profile_store,
            profile_dialog: ProfileDialog::default(),
            profiles,
            active_profile,
            profile_name: profile.name.clone(),
            rendered_visual: profile.visual.clone(),
            motion_pace: MotionPace::new(profile.visual.motion),
            scene: SceneTransition::new(initial_scene),
            mandelbrot_visit: MandelbrotVisit::default(),
            profile,
            visual_time: 0.0,
            clock: FrameClock::new(Instant::now()),
            last_director_submit: Instant::now() - Duration::from_secs(5),
            fullscreen: false,
            show_controls,
            show_equaliser,
            sharp_rendering,
            test_track: TestTrack::new(),
            palette_mix,
            ui_palette,
            message: None,
        }
    }

    fn set_scene(&mut self, scene: usize) {
        self.set_auto_mode(false);
        let scene = scene.min(SCENES.len() - 1);
        self.profile.visual.scene = scene;
        self.scene.request(scene);
        self.profile.visual.look = self.look_journey.current(scene);
    }

    fn set_auto_mode(&mut self, enabled: bool) {
        if enabled != self.auto_mode {
            self.auto_mode = enabled;
            if enabled {
                self.auto_journey.set_current(self.scene.to);
                self.look_journey.resume_auto(self.scene.to);
            } else {
                self.profile.visual.scene = self.scene.to;
                self.profile.visual.look = self.look_journey.current(self.scene.to);
            }
        }
    }

    fn apply_profile(&mut self, index: usize) {
        if let Some(mut profile) = self.profiles.get(index).cloned() {
            self.active_profile = index;
            self.profile_name = profile.name.clone();
            self.set_scene(profile.visual.scene);
            profile.audio_mode = self.audio.mode();
            self.profile = profile;
            self.direction = None;
        }
    }

    fn select_audio(&mut self, mode: AudioMode) {
        self.audio.select(mode);
        self.profile.audio_mode = mode;
        self.features = AudioFeatures::default();
        self.render_motion.reset();
        self.meters.reset();
        self.choreographer.reset();
        self.music_response.reset();
        self.ripples.reset();
        self.vocal_response.reset();
        self.math_journey.reset_evidence();
        self.auto_journey.reset_evidence();
        self.audio_fresh = false;
        self.direction = None;
    }

    fn snapshot_profile(&self) -> Profile {
        current_profile_snapshot(
            &self.profile,
            &self.profile_name,
            self.audio.mode(),
            self.scene.to,
            self.look_journey.current(self.scene.to),
        )
    }

    fn remember_profile(&mut self, profile: Profile) {
        let index = register_profile(&mut self.profiles, profile);
        self.apply_profile(index);
    }

    fn save_profile(&mut self) {
        let profile = self.snapshot_profile();
        match self.profile_store.save(&profile) {
            Ok(_) => {
                let name = profile.name.clone();
                // Saving records what is visible; it is not a request to
                // apply a world, stop Auto or clear an in-flight choice.
                record_saved_profile(
                    &mut self.profiles,
                    &mut self.active_profile,
                    &mut self.profile_name,
                    &mut self.profile,
                    profile,
                );
                self.flash_message(format!("Saved {name}"));
            }
            Err(error) => self.flash_message(format!("Could not save: {error}")),
        }
    }

    fn flash_message(&mut self, message: impl Into<String>) {
        self.message = Some((message.into(), Instant::now()));
    }

    fn update_features_and_direction(&mut self, dt: f32) {
        let snapshot = self.analysis.latest();
        self.audio_fresh = snapshot.is_fresh;
        self.audio_age = snapshot.last_sample_age;
        self.features = snapshot.features;
        if snapshot.source_epoch != self.source_epoch {
            self.source_epoch = snapshot.source_epoch;
            self.render_motion.reset();
            self.meters.reset();
            self.choreographer.reset();
            self.music_response.reset();
            self.ripples.reset();
            self.vocal_response.reset();
            self.math_journey.reset_evidence();
            self.auto_journey.reset_evidence();
            self.direction = None;
            self.last_director_submit = Instant::now() - Duration::from_secs(2);
        }
        let guidance_enabled = self.profile.director_enabled
            && [
                self.profile.mappings.bass_to_pulse,
                self.profile.mappings.mids_to_geometry,
                self.profile.mappings.treble_to_sparkle,
                self.profile.mappings.beat_to_flash,
            ]
            .iter()
            .any(|gain| *gain > 0.0);
        if guidance_enabled
            && self.audio_fresh
            && self.last_director_submit.elapsed() >= Duration::from_secs(1)
        {
            self.director.submit(DirectorInput {
                source_epoch: self.source_epoch,
                dna: self.features.dna.clone(),
                energy: self.features.energy,
                bass: self.features.bass,
                transient: self.features.transient,
            });
            self.last_director_submit = Instant::now();
        }
        if let Some(direction) = self.director.latest()
            && guidance_enabled
            && self.audio_fresh
            && direction.source_epoch == self.source_epoch
        {
            self.direction = Some(direction);
        }

        let mut target = self.profile.visual.clone();
        if guidance_enabled
            && self.audio_fresh
            && let Some(direction) = &self.direction
        {
            let freedom = self.profile.director_freedom;
            // A profile is the user's stable baseline. Guidance is a temporary
            // rendering target and cannot silently rewrite saved choices.
            target.hue = hue_lerp(target.hue, direction.hue, freedom * 0.45);
            // The user's pace is literal. Musical travel already has a slow,
            // bounded energy response; guidance must not move the speed knob.
            target.complexity = lerp(target.complexity, direction.complexity, freedom * 0.55);
            target.glow = lerp(target.glow, direction.glow, freedom * 0.45);
        }
        let smoothing = 1.0 - (-dt * 3.2).exp();
        let visual = &mut self.rendered_visual;
        visual.hue = hue_lerp(visual.hue, target.hue, smoothing);
        visual.intensity = lerp(visual.intensity, target.intensity, smoothing);
        visual.complexity = lerp(visual.complexity, target.complexity, smoothing);
        visual.glow = lerp(visual.glow, target.glow, smoothing);
        visual.warp = lerp(visual.warp, target.warp, smoothing);
        visual.feedback = lerp(visual.feedback, target.feedback, smoothing);
        visual.palette = target.palette;
        self.palette_mix = lerp(self.palette_mix, target.palette.amount(), smoothing);
        self.auto_mix = lerp(
            self.auto_mix,
            if self.auto_mode { 1.0 } else { 0.0 },
            smoothing,
        );
    }

    fn advance_world(&mut self, dt: f32) {
        if self.auto_mode {
            if let Some(next) = self
                .auto_journey
                .update(&self.features, &self.profile.mappings, dt)
            {
                if self.scene.amount == 1.0 {
                    self.auto_seed = self.auto_journey.blend_seed();
                    self.auto_morph_seconds = self.auto_journey.morph_seconds();
                }
                self.scene.request(next);
            }
        } else {
            self.scene.request(self.profile.visual.scene);
        }
        if self.auto_mode {
            self.scene.advance_over(dt, self.auto_morph_seconds);
        } else {
            // Manual changes also remain a smooth eight-second passage,
            // independent of the user's flight speed.
            self.scene.advance(dt * 0.625);
        }
    }

    fn advance_visual_evolution(
        &mut self,
        audio: &AudioFeatures,
        dt: f32,
    ) -> ([[f32; 4]; WORLD_COUNT], MathJourneyState) {
        // Theme/structure clocks use physical elapsed time. Pace controls
        // flight, never how quickly a dot formation gets replaced.
        let steps = (dt / 0.1).ceil().max(1.0) as usize;
        let step = dt / steps as f32;
        let mut result = None;
        self.look_journey
            .set_visible_pair(self.scene.from, self.scene.to, self.scene.blend());
        for _ in 0..steps {
            let looks = self.look_journey.update(
                audio,
                &self.profile.mappings,
                self.auto_mode,
                self.scene.to,
                self.profile.visual.look,
                step,
            );
            let mathematics = self.math_journey.update(
                audio,
                &self.profile.mappings,
                if self.auto_mode {
                    self.auto_journey.math_form()
                } else {
                    self.profile.visual.math_form
                },
                step,
            );
            result = Some((looks, mathematics));
        }
        result.expect("at least one visual step")
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) && self.fullscreen {
            self.fullscreen = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        if ctx.text_edit_focused() {
            return;
        }
        if ctx.input(|input| input.modifiers.any()) {
            return;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::E)) {
            self.show_equaliser = !self.show_equaliser;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::A)) {
            self.set_auto_mode(!self.auto_mode);
        }
        if ctx.input(|input| input.key_pressed(egui::Key::H)) {
            self.show_controls = !self.show_controls;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::F)) {
            self.toggle_fullscreen(ctx);
        }
        let keys: [egui::Key; WORLD_COUNT] = [
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
            egui::Key::Num5,
            egui::Key::Num6,
            egui::Key::Num7,
            egui::Key::Num8,
            egui::Key::Num9,
            egui::Key::Num0,
            egui::Key::M,
            egui::Key::G,
            egui::Key::S,
            egui::Key::N,
            egui::Key::C,
            egui::Key::Q,
            egui::Key::W,
        ];
        for (scene, key) in keys.into_iter().enumerate() {
            if ctx.input(|input| input.key_pressed(key)) {
                self.set_scene(scene);
            }
        }
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }
}

impl eframe::App for MusicVisualiserApp {
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        match self.profile_dialog.advance(frame, &ctx) {
            Some(ProfileDialogOutcome::Imported(profile)) => {
                self.remember_profile(*profile);
                self.flash_message("Profile imported. Save to keep it in your library.");
            }
            Some(ProfileDialogOutcome::Exported) => self.flash_message("Profile exported"),
            Some(ProfileDialogOutcome::Failed(error)) => self.flash_message(error),
            Some(ProfileDialogOutcome::Cancelled) | None => {}
        }
        if self.ui_palette != self.profile.visual.palette {
            self.ui_palette = self.profile.visual.palette;
            ui::configure_style(&ctx, self.ui_palette);
        }
        let dt = self.clock.step(ctx.cumulative_frame_nr(), Instant::now());
        self.fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(self.fullscreen));
        if dt > 0.0 {
            self.update_features_and_direction(dt);
            self.meters.update(&self.features, dt);
            self.keyboard(&ctx);
        }
        let backdrop = visual::reserve_canvas(root);
        let auto_readouts = self.controls(root, &ctx);
        let background = self
            .background_journey
            .update(self.profile.visual.background, dt);
        let material_hue = self.material_colour_journey.update(self.auto_mode, dt);
        let canvas = root.available_rect_before_wrap();
        let render_features = self.render_motion.update(&self.features, dt);
        let vocal_response = self.vocal_response.update(
            &self.features,
            &self.profile.mappings,
            self.audio_fresh,
            self.source_epoch,
            dt,
        );
        let music_response = self.music_response.update(
            &self.features,
            &self.profile.mappings,
            self.audio_fresh,
            self.source_epoch,
            dt,
        );
        let pace = self.motion_pace.advance(self.profile.visual.motion, dt);
        let ripple_events = self.ripples.update(
            &self.features,
            &self.profile.mappings,
            self.audio_fresh,
            self.source_epoch,
            dt,
            pace.average_rate,
        );
        self.rendered_visual.motion = motion_value(pace.average_rate);
        if pace.world_delta > 0.0 {
            self.advance_world(pace.world_delta);
        }
        let (mandelbrot_age, next_mandelbrot_region) = self.mandelbrot_visit.update_for_mode(
            &self.scene,
            dt,
            ctx.input(|input| input.viewport().visible() != Some(false)),
            self.auto_mode,
            self.profile.visual.look,
            &mut self.look_journey,
        );
        if let Some(next) = next_mandelbrot_region {
            // The selector and saved profile name the same real destination
            // as the renderer, rather than an invisible shader-only shuffle.
            self.profile.visual.look = next;
        }
        let (looks, mathematics) =
            self.advance_visual_evolution(&render_features, pace.evolution_delta);
        // Phrase motion respects the same audio disconnects as the renderer.
        // The raw input meters/DNA remain unchanged and honest.
        let mut choreography =
            self.choreographer
                .update_mapped(&render_features, &self.profile.mappings, dt);
        self.visual_time += f64::from(pace.travel_delta) * f64::from(choreography.travel_speed);
        // Camera/orbit phase follows the same integrated user pace as travel.
        // Choreography's audio envelopes still use real elapsed seconds.
        choreography.phase = self.visual_time as f32;
        self.math_name = mathematics.family_name();
        let centre =
            2.0 * (canvas.center().x - backdrop.rect.center().x) / backdrop.rect.height().max(1.0);
        let uniforms = ShaderUniforms::new(
            self.visual_time as f32,
            self.clock.delta(),
            self.scene.to,
            self.scene.from,
            self.scene.blend(),
            &self.rendered_visual,
            &self.profile.mappings,
            &render_features,
        )
        .with_choreography(choreography)
        .with_palette_mix(self.palette_mix)
        .with_math_blend(
            mathematics.from,
            mathematics.to,
            mathematics.blend,
            mathematics.phase,
        )
        .with_canvas_centre(centre);
        let uniforms = uniforms
            .with_auto_mix(self.auto_mix, self.auto_seed)
            .with_vocal_response(vocal_response)
            .with_music_response(music_response)
            .with_ripples(ripple_events)
            .with_background(background)
            .with_material_hue(material_hue)
            .with_mandelbrot_age(mandelbrot_age)
            .with_mandelbrot_motion(self.mandelbrot_visit.manual_motion())
            .with_looks(looks);
        let uniforms = uniforms.with_audio_state(self.audio_fresh, self.source_epoch);
        if let Some(readouts) = auto_readouts {
            readouts.paint(
                &self.scene,
                &looks,
                mathematics,
                self.auto_journey.next_world_progress(),
                self.profile.visual.palette,
            );
        }
        visual::paint_canvas(
            root,
            uniforms,
            self.sharp_rendering,
            backdrop,
            self.scene.pending(),
        );
        if self.preparation_started.elapsed() > Duration::from_secs(2)
            && self
                .prepared_worlds
                .load(std::sync::atomic::Ordering::Relaxed)
                < visual::PREPARATION_COUNT
        {
            root.painter().text(
                canvas.left_bottom() + egui::vec2(14.0, -14.0),
                egui::Align2::LEFT_BOTTOM,
                "Preparing worlds…",
                egui::TextStyle::Body.resolve(root.style()),
                root.visuals().weak_text_color(),
            );
        }
        self.overlays(&ctx, canvas);
        // Keep visible windows paced by vsync, including a second display.
        // Only unpaced startup callbacks need a timer guard. A display-paced
        // frame requests its successor immediately rather than adding idle time.
        if ctx.input(|input| input.viewport().visible() == Some(false)) {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.clock.delta() >= 0.004 {
            ctx.request_repaint();
        } else {
            // egui subtracts predicted_dt from delayed repaint requests. Add it
            // back so the small guard survives even before vsync is pacing us.
            let predicted = ctx
                .input(|input| Duration::try_from_secs_f32(input.predicted_dt).unwrap_or_default());
            ctx.request_repaint_after(predicted.saturating_add(Duration::from_millis(2)));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "last-profile", &self.snapshot_profile());
        eframe::set_value(storage, "listening-source", &self.audio.mode());
        eframe::set_value(storage, "sharp-rendering", &self.sharp_rendering);
        eframe::set_value(storage, "show-equaliser", &self.show_equaliser);
        persist_sidebar_visibility(storage, self.show_controls);
        eframe::set_value(storage, "auto-mode", &self.auto_mode);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.002, 0.001, 0.008, 1.0]
    }
}

fn current_profile_snapshot(
    baseline: &Profile,
    name: &str,
    source: AudioMode,
    destination: usize,
    look: usize,
) -> Profile {
    let mut profile = baseline.clone();
    profile.name = if name.trim().is_empty() {
        "Untitled Universe".into()
    } else {
        name.trim().to_owned()
    };
    profile.audio_mode = source;
    // Auto evolves outside the baseline profile. During a melt, save its
    // displayed destination, not an old manual choice or an unseen queue.
    profile.visual.scene = destination.min(WORLD_COUNT - 1);
    profile.visual.look = look.min(9);
    profile
}

fn register_profile(profiles: &mut Vec<Profile>, profile: Profile) -> usize {
    // Factory presets remain intact even when a personal copy has that name.
    let index = profiles
        .iter()
        .enumerate()
        .skip(BUILT_IN_PROFILE_COUNT)
        .find_map(|(index, item)| (item.name == profile.name).then_some(index))
        .unwrap_or_else(|| {
            profiles.push(profile.clone());
            profiles.len() - 1
        });
    profiles[index] = profile;
    index
}

fn record_saved_profile(
    profiles: &mut Vec<Profile>,
    active_profile: &mut usize,
    profile_name: &mut String,
    baseline: &mut Profile,
    snapshot: Profile,
) {
    *profile_name = snapshot.name.clone();
    baseline.name = snapshot.name.clone();
    *active_profile = register_profile(profiles, snapshot);
    // Deliberately no access to Auto, scene/look timelines, source or motion.
    // In particular preserve a pending manual choice in baseline.visual.
}

fn restore_sidebar_visibility(storage: Option<&dyn eframe::Storage>) -> bool {
    storage
        .and_then(|storage| eframe::get_value::<bool>(storage, "show-controls"))
        .unwrap_or(true)
}

fn persist_sidebar_visibility(storage: &mut dyn eframe::Storage, visible: bool) {
    eframe::set_value(storage, "show-controls", &visible);
}

fn lerp(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount.clamp(0.0, 1.0)
}

fn hue_lerp(from: f32, to: f32, amount: f32) -> f32 {
    let delta = (to - from + 0.5).rem_euclid(1.0) - 0.5;
    (from + delta * amount).rem_euclid(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look_journey::LOOK_COUNT;

    #[test]
    fn mandelbrot_visit_counts_visible_exposure_and_restarts_only_after_leaving() {
        for hz in [30, 60, 120] {
            let dt = 1.0 / hz as f32;
            let mut visit = MandelbrotVisit::default();
            let mut scene = SceneTransition::new(7);
            assert_eq!(visit.update(&scene, dt, true, None), 0.0);
            scene.request(10);
            for _ in 0..8 * hz {
                scene.advance_over(dt, 8.0);
                visit.update(&scene, dt, true, None);
            }
            let arrived = visit.update(&scene, 0.0, true, None);
            assert!((arrived - 4.0).abs() < 0.04, "{hz} Hz: {arrived}");
            // Finish sub-frame floating-point remainder before issuing a new
            // request; this does not advance the visit's exposure clock.
            scene.advance_over(dt, 8.0);
            for _ in 0..4 * hz {
                visit.update(&scene, dt, true, None);
            }
            let held = visit.update(&scene, 0.0, true, None);
            assert!((held - 8.0).abs() < 0.04);
            // Duplicate UI passes, resize/repaint and hidden windows do not
            // spend or restart the current visit's construction.
            assert_eq!(visit.update(&scene, 0.0, true, None), held);
            assert_eq!(visit.update(&scene, dt, false, None), held);
            assert_eq!(visit.update(&scene, f32::NAN, true, None), held);
            scene.request(8);
            assert_eq!(visit.update(&scene, 0.0, true, None), held);
            scene.advance_over(8.0, 8.0);
            assert_eq!(visit.update(&scene, 0.0, true, None), 0.0);
            scene.request(10);
            assert_eq!(visit.update(&scene, 0.0, true, None), 0.0);
            scene.advance_over(dt, 8.0);
            assert!(visit.update(&scene, dt, true, None) < 0.001);
        }
    }

    #[test]
    fn mandelbrot_startup_and_long_holds_have_no_periodic_restart() {
        let mut visit = MandelbrotVisit::default();
        let scene = SceneTransition::new(10);
        assert_eq!(visit.update(&scene, 0.0, true, None), 0.0);
        for _ in 0..1200 {
            visit.update(&scene, 0.1, true, None);
        }
        assert_eq!(visit.update(&scene, 0.0, true, None), 120.0);
        assert_eq!(visit.update(&scene, 0.1, true, None), 120.0);
    }

    #[test]
    fn manual_mandelbrot_journeys_keep_moving_and_cover_regions_at_all_frame_rates() {
        let run = |hz: usize| {
            let scene = SceneTransition::new(10);
            let mut visit = MandelbrotVisit::default();
            let mut looks = LookJourney::seeded(931);
            let mut selected = looks.current(10);
            let mut choices = vec![(selected, 0.0)];
            let mut previous_age = 0.0;
            let dt = 1.0 / hz as f32;
            for frame in 1..=hz * 440 {
                let age = visit.update(&scene, dt, true, Some(selected));
                assert!((age - previous_age).abs() < 3.0 * dt);
                previous_age = age;
                if visit.take_next_region()
                    && let Some(next) = looks.next_manual_mandelbrot_region(selected)
                {
                    assert!(age < 0.01, "region changed before broad framing");
                    selected = next;
                    visit.selected_region(next);
                    choices.push((next, frame as f64 / hz as f64));
                }
                let states = looks.update(
                    &AudioFeatures::default(),
                    &crate::presets::AudioMappings::default(),
                    false,
                    10,
                    selected,
                    dt,
                );
                assert_eq!(states[10][1] as usize, selected);
            }
            assert!(choices.len() >= 11);
            let mut seen = 0_u16;
            for (region, _) in choices.iter().take(10) {
                assert_eq!(seen & (1 << region), 0, "region repeated before coverage");
                seen |= 1 << region;
            }
            assert_eq!(seen, (1 << 10) - 1);
            for pair in choices.windows(2) {
                assert!((35.0..=45.1).contains(&(pair[1].1 - pair[0].1)));
            }
            choices
        };
        let reference = run(120);
        for hz in [30, 60] {
            let choices = run(hz);
            assert_eq!(choices.len(), reference.len());
            for (a, b) in choices.iter().zip(&reference) {
                assert_eq!(a.0, b.0);
                assert!((a.1 - b.1).abs() < 0.05);
            }
        }
    }

    #[test]
    fn manual_mandelbrot_selection_pause_and_mode_changes_preserve_visible_state() {
        let scene = SceneTransition::new(10);
        let mut visit = MandelbrotVisit::default();
        for _ in 0..350 {
            visit.update(&scene, 0.1, true, Some(0));
            visit.take_next_region();
        }
        let selected_at = visit.age;
        let first = visit.update(&scene, 0.01, true, Some(6));
        assert!((f64::from(first) - selected_at).abs() < 0.03);
        for _ in 0..170 {
            visit.update(&scene, 0.1, true, Some(6));
            assert!(
                !visit.take_next_region(),
                "manual selection replaced too early"
            );
        }
        let held = visit.age;
        assert_eq!(visit.update(&scene, 0.1, false, Some(6)), held as f32);
        assert_eq!(visit.update(&scene, 0.0, true, Some(6)), held as f32);
        assert_eq!(visit.update(&scene, f32::NAN, true, Some(6)), held as f32);
        let resumed_auto = visit.update(&scene, 1.0 / 60.0, true, None);
        assert!((f64::from(resumed_auto) - held).abs() < 0.05);
        assert!(!visit.take_next_region());

        let mut late = MandelbrotVisit::default();
        for _ in 0..1200 {
            late.update(&scene, 0.1, true, None);
        }
        assert_eq!(late.update(&scene, 0.0, true, Some(8)), 120.0);
        let first = late.update(&scene, 1.0 / 60.0, true, Some(8));
        assert!((first - 120.0).abs() < 0.02, "late manual entry snapped");
        for _ in 0..60 * 6 {
            late.update(&scene, 1.0 / 60.0, true, Some(8));
        }
        assert!(late.age < 28.0, "late Auto hold spent dead time returning");
        let before = late.age;
        let after = late.update(&scene, 1.0 / 60.0, true, None);
        assert!((f64::from(after) - before).abs() < 0.06);
    }

    #[test]
    fn manual_mandelbrot_reentry_with_inward_velocity_cannot_hold_a_clamped_zero() {
        for starting_age in [0.01, 0.1, 1.0, 6.0, 12.0, 28.001] {
            for starting_velocity in [-0.01, -0.2, -1.8, -3.0] {
                let mut age = starting_age;
                let mut velocity = starting_velocity;
                let mut journey = MandelbrotManualJourney::new(age, velocity, 8);
                let mut zero_run = 0;
                for _ in 0..60 * 45 {
                    journey.advance(&mut age, &mut velocity, 1.0 / 60.0, 8);
                    assert!(age.is_finite() && velocity.is_finite());
                    zero_run = if age < 1e-8 { zero_run + 1 } else { 0 };
                    assert!(
                        zero_run < 3,
                        "clamped plateau from {starting_age}/{starting_velocity}"
                    );
                }
            }
        }
    }

    #[test]
    fn auto_mandelbrot_unfolds_dives_and_unfolds_again_before_its_long_visit_ends() {
        for hz in [30, 60, 120] {
            for seed in [931, 57, 6] {
                let dt = 1.0 / hz as f32;
                let mut scene = SceneTransition::new(7);
                scene.request(10);
                let mut visit = MandelbrotVisit::default();
                let mut looks = LookJourney::seeded(seed);
                let first_region = looks.current(10);
                // Auto's real lane owns the name; a stale saved manual choice
                // must neither redirect it nor replace a broad-endpoint choice.
                let saved_manual = (first_region + 3) % LOOK_COUNT;
                let mut previous_age = 0.0_f32;
                let mut region_change = None;
                let mut first_peak = 0.0_f32;
                let mut renewed_peak = 0.0_f32;
                let mut saw_pullback = false;
                let mut renewed_age_at_64 = 0.0;
                for frame in 1..=hz * 70 {
                    scene.advance_over(dt, 8.0);
                    let (age, manual_choice) =
                        visit.update_for_mode(&scene, dt, true, true, saved_manual, &mut looks);
                    assert_eq!(
                        manual_choice, None,
                        "Auto must not overwrite the saved choice"
                    );
                    assert!((age - previous_age).abs() < 3.0 * dt);
                    if region_change.is_none() {
                        first_peak = first_peak.max(age);
                        saw_pullback |= age < previous_age - 0.0001;
                    }
                    let current = looks.current(10);
                    if current != first_region && region_change.is_none() {
                        assert!(age < 0.01, "new region must start at broad framing");
                        assert!(first_peak >= 21.9 && saw_pullback);
                        let seconds = frame as f64 / hz as f64;
                        assert!((41.0..=47.2).contains(&seconds), "{seconds}");
                        region_change = Some(current);
                    }
                    if region_change.is_some() {
                        renewed_peak = renewed_peak.max(age);
                    }
                    looks.set_visible_pair(scene.from, scene.to, scene.blend());
                    let actual = looks.update(
                        &AudioFeatures::default(),
                        &crate::presets::AudioMappings::default(),
                        true,
                        scene.to,
                        saved_manual,
                        dt,
                    )[10];
                    assert_eq!(
                        actual[1] as usize, current,
                        "label and rendered region must agree"
                    );
                    if frame == hz * 64 {
                        renewed_age_at_64 = age;
                    }
                    previous_age = age;
                }
                assert!(region_change.is_some());
                assert!(
                    renewed_age_at_64 > 12.0,
                    "second unfolding must finish before the earliest departure"
                );
                assert!(
                    renewed_peak > 20.0,
                    "the renewed structure must begin its detail dive"
                );
            }
        }
    }

    #[test]
    fn guided_mandelbrot_mode_changes_and_hidden_windows_keep_the_same_camera_leg() {
        let scene = SceneTransition::new(10);
        let mut visit = MandelbrotVisit::default();
        let mut looks = LookJourney::seeded(931);
        let mut auto = true;
        let mut selected = looks.current(10);
        let dt = 1.0 / 60.0;
        for frame in 1..=60 * 85 {
            if [12 * 60, 26 * 60, 40 * 60, 46 * 60].contains(&frame) {
                auto = !auto;
                selected = looks.current(10); // actual set_auto_mode behaviour
                let before = (visit.age, visit.velocity);
                let (age, choice) =
                    visit.update_for_mode(&scene, 0.0, true, auto, selected, &mut looks);
                assert_eq!(age, before.0 as f32);
                assert_eq!((visit.age, visit.velocity), before);
                assert_eq!(choice, None);
            }
            let before = visit.age;
            if frame == 30 * 60 {
                let current = looks.current(10);
                for _ in 0..120 {
                    let (age, choice) =
                        visit.update_for_mode(&scene, dt, false, auto, selected, &mut looks);
                    assert_eq!(age, before as f32);
                    assert_eq!(choice, None);
                    assert_eq!(looks.current(10), current);
                }
            }
            let (age, choice) = visit.update_for_mode(&scene, dt, true, auto, selected, &mut looks);
            assert!((f64::from(age) - before).abs() < 3.0 * f64::from(dt));
            if let Some(next) = choice {
                assert!(!auto);
                selected = next;
            }
            looks.update(
                &AudioFeatures::default(),
                &crate::presets::AudioMappings::default(),
                auto,
                10,
                selected,
                dt,
            );
        }
    }

    #[derive(Default)]
    struct MemoryStorage(std::collections::HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    #[test]
    fn snapshots_capture_current_destination_without_mutating_baseline() {
        let baseline = Profile::astral_bloom();
        let snapshot = current_profile_snapshot(&baseline, "  My dots  ", AudioMode::System, 4, 8);
        assert_eq!(snapshot.visual.scene, 4);
        assert_eq!(snapshot.visual.look, 8);
        assert_eq!(snapshot.name, "My dots");
        assert_eq!(snapshot.audio_mode, AudioMode::System);
        assert_eq!(snapshot.visual.motion, baseline.visual.motion);
        assert_eq!(snapshot.mappings, baseline.mappings);
        assert_eq!(baseline, Profile::astral_bloom());
        snapshot.validate().unwrap();
    }

    #[test]
    fn recording_a_save_preserves_active_settings_and_queued_choice() {
        let mut baseline = Profile::astral_bloom();
        baseline.visual.scene = 6; // a newer manual request still queued
        baseline.visual.look = 9;
        baseline.visual.motion = 1.5;
        baseline.audio_mode = AudioMode::System;
        let before = baseline.clone();
        let snapshot =
            current_profile_snapshot(&baseline, "Saved visible dots", AudioMode::System, 4, 2);
        let mut profiles = built_in_profiles();
        let mut selected = 0;
        let mut name = baseline.name.clone();
        record_saved_profile(
            &mut profiles,
            &mut selected,
            &mut name,
            &mut baseline,
            snapshot.clone(),
        );
        assert_eq!(profiles[selected], snapshot);
        assert_eq!(name, "Saved visible dots");
        assert_eq!(baseline.visual, before.visual);
        assert_eq!(baseline.mappings, before.mappings);
        assert_eq!(baseline.audio_mode, before.audio_mode);
        assert_eq!(baseline.director_enabled, before.director_enabled);
        assert_eq!(baseline.director_freedom, before.director_freedom);
    }

    #[test]
    fn resaving_a_personal_snapshot_updates_it_without_replacing_factory_profiles() {
        let mut profiles = built_in_profiles();
        let factory = profiles[0].clone();
        let first = current_profile_snapshot(&factory, &factory.name, AudioMode::System, 4, 2);
        let index = register_profile(&mut profiles, first);
        let second = current_profile_snapshot(&factory, &factory.name, AudioMode::System, 1, 7);
        assert_eq!(register_profile(&mut profiles, second.clone()), index);
        assert_eq!(profiles.len(), BUILT_IN_PROFILE_COUNT + 1);
        assert_eq!(profiles[0], factory);
        assert_eq!(profiles[index], second);
    }

    #[test]
    fn sidebar_visibility_round_trips_independently_and_defaults_visible() {
        assert!(restore_sidebar_visibility(None));
        let mut storage = MemoryStorage::default();
        assert!(restore_sidebar_visibility(Some(&storage)));
        eframe::set_value(&mut storage, "show-equaliser", &true);
        for visible in [false, true, false] {
            persist_sidebar_visibility(&mut storage, visible);
            assert_eq!(restore_sidebar_visibility(Some(&storage)), visible);
            assert_eq!(
                eframe::get_value::<bool>(&storage, "show-equaliser"),
                Some(true)
            );
        }
        storage
            .0
            .insert("show-controls".into(), "invalid preference".into());
        assert!(restore_sidebar_visibility(Some(&storage)));
    }
}
