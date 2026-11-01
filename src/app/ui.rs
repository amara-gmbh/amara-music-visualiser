use super::*;
use eframe::egui::{Color32, RichText};

const ACCENT: Color32 = Color32::from_rgb(179, 165, 219);
const TEXT: Color32 = Color32::from_rgb(224, 224, 224);
const MUTED: Color32 = Color32::from_rgba_unmultiplied_const(224, 224, 224, 172);
const H1_SIZE: f32 = 18.0;
const H2_SIZE: f32 = 15.0;
const BODY_SIZE: f32 = 13.0;
const READOUT_ROW_HEIGHT: f32 = 22.0;

impl MusicVisualiserApp {
    pub(super) fn controls(
        &mut self,
        root: &mut egui::Ui,
        ctx: &egui::Context,
    ) -> Option<AutoReadoutSlots> {
        if !self.show_controls {
            return None;
        }
        let mut readouts = None;
        let accent = panel_accent(self.profile.visual.palette);
        egui::Panel::right("controls").resizable(false).exact_size(340.0)
            .frame(glass_frame(self.profile.visual.palette)
                .inner_margin(egui::Margin::symmetric(18, 16)))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Your universe").size(H1_SIZE).color(TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("×").on_hover_text("Hide controls · H").clicked() {
                            self.show_controls = false;
                        }
                    });
                });
                ui.label(RichText::new("Shape the music in your own way.").color(MUTED));
                ui.add_space(9.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    section(ui, "LISTEN");
                    let mut mode = self.audio.mode();
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut mode, AudioMode::System, "Mac audio");
                        ui.selectable_value(&mut mode, AudioMode::Microphone, "Mic");
                        ui.selectable_value(&mut mode, AudioMode::Demo, "Demo");
                    });
                    if mode != self.audio.mode() { self.select_audio(mode); }
                    let source_note = match mode {
                        AudioMode::System => "Music playing on this Mac · any player",
                        AudioMode::Microphone => "The Mac's default input device",
                        AudioMode::Demo => "Silent synthetic music · 120 BPM",
                    };
                    ui.label(RichText::new(source_note).color(MUTED));
                    if mode != AudioMode::Demo {
                        let colour = if self.audio_fresh { accent } else { MUTED };
                        ui.add(egui::Label::new(RichText::new(self.audio.status()).color(colour)).truncate())
                            .on_hover_text(self.audio.status());
                    }
                    if mode != AudioMode::Demo && !self.audio_fresh {
                        ui.horizontal(|ui| {
                            if ui.small_button("Try again").clicked() { self.select_audio(mode); }
                            if ui.small_button("Use demo").clicked() { self.select_audio(AudioMode::Demo); }
                        });
                        ui.collapsing("Permission help", |ui| {
                            ui.label("Mac audio needs Audio Capture permission (under Screen & System Audio Recording in Privacy & Security). Allow amaraMusicVisualiser when macOS asks. Older macOS uses screen-capture permission instead. Mic has its own Microphone permission.");
                            ui.label("After allowing access, choose Try again. If macOS asks to quit and reopen, do that first. Demo is never substituted silently.");
                            ui.label("No audio or video is recorded or uploaded.");
                        });
                    }
                    space(ui);
                    section(ui, "WORLD");
                    let mut auto = self.auto_mode;
                    if ui.checkbox(&mut auto, "Auto journey · A").on_hover_text(format!("An endless evolving mix: {} worlds, {} variations each. Main worlds change every 18–24 seconds; variations evolve within them. World colours gently change on their own rhythm, separately from Background. Turn Auto off to hold the current colour.", SCENES.len(), visual::LOOKS[0].len())).changed() {
                        self.set_auto_mode(auto);
                    }
                    if self.auto_mode {
                        // Reserve stable space now; paint after this frame's
                        // scene/look/math advancement, using the GPU snapshot.
                        readouts = Some(AutoReadoutSlots::reserve(ui));
                        ui.add_space(5.0);
                    }
                    let mut scene = self.profile.visual.scene;
                    let mut picked_world = false;
                    let world_label = if self.auto_mode { format!("Auto · {}", SCENES[self.scene.to]) } else { SCENES[scene].to_owned() };
                    egui::ComboBox::from_id_salt("world").selected_text(world_label)
                        .height(330.0)
                        .width(ui.available_width()).truncate().show_ui(ui, |ui| {
                            for (index, name) in SCENES.iter().enumerate() {
                                if ui.selectable_value(&mut scene, index, *name).clicked() { picked_world = true; }
                            }
                        });
                    if picked_world { self.set_scene(scene); }
                    if !self.auto_mode {
                        selector_row(ui, "Structure", |ui, width| {
                            egui::ComboBox::from_id_salt("structural-look")
                                .selected_text(visual::LOOKS[scene][self.profile.visual.look])
                                .width(width).truncate().show_ui(ui, |ui| {
                                    for (index, name) in visual::LOOKS[scene].iter().enumerate() {
                                        ui.selectable_value(&mut self.profile.visual.look, index, *name);
                                    }
                                });
                        });
                    }
                    if scene == 1 && !self.auto_mode {
                        selector_row(ui, "Mathematics", |ui, width| {
                            egui::ComboBox::from_id_salt("math-form")
                                .selected_text(self.profile.visual.math_form.label())
                                .width(width).truncate().show_ui(ui, |ui| {
                                    for form in [MathForm::Journey, MathForm::Julia, MathForm::InversionLace,
                                        MathForm::HarmonicSculpture, MathForm::VortexWeave] {
                                        ui.selectable_value(&mut self.profile.visual.math_form, form, form.label());
                                    }
                                });
                        });
                        let note = if self.profile.visual.math_form == MathForm::Journey {
                            format!("Journey · {} · music can guide the next form", self.math_name)
                        } else { "A held form, still evolving. Six-second fluid morphs.".into() };
                        ui.label(RichText::new(note).color(MUTED));
                    }
                    selector_row(ui, "Palette", |ui, width| {
                        egui::ComboBox::from_id_salt("palette")
                            .selected_text(self.profile.visual.palette.label())
                            .width(width).truncate().show_ui(ui, |ui| {
                                for palette in [VisualPalette::Cosmic, VisualPalette::MoccaVanilla] {
                                    ui.selectable_value(&mut self.profile.visual.palette, palette, palette.label());
                                }
                            }).response.on_hover_text("Mocca & Vanilla keeps colours warm. Cosmic reveals the full spectrum.");
                    });
                    selector_row(ui, "Background", |ui, width| {
                        egui::ComboBox::from_id_salt("background-colour")
                            .selected_text(self.profile.visual.background.label())
                            .width(width).truncate().height(330.0).show_ui(ui, |ui| {
                                for background in BackgroundPalette::ALL {
                                    ui.selectable_value(&mut self.profile.visual.background, background, background.label());
                                }
                            });
                        if self.profile.visual.background == BackgroundPalette::Auto
                            && ui.small_button("↻").on_hover_text(format!("New colour · {}. Melt into another background without changing the world or its music response.", self.background_journey.target_label())).clicked() {
                            self.background_journey.shuffle();
                        }
                    });
                    space(ui);
                    section(ui, "FEEL");
                    slider(ui, &mut self.profile.visual.intensity, 0.15..=1.6, "Intensity");
                    pace_slider(ui, &mut self.profile.visual.motion);
                    slider(ui, &mut self.profile.visual.complexity, 0.15..=1.5, "Detail");
                    space(ui);
                    egui::CollapsingHeader::new("Visual details").id_salt("visual-details-quiet").default_open(false).show(ui, |ui| {
                        slider(ui, &mut self.profile.visual.hue, 0.0..=1.0, "Colour orbit");
                        slider(ui, &mut self.profile.visual.glow, 0.0..=1.6, "Glow");
                        slider(ui, &mut self.profile.visual.warp, 0.0..=1.6, "Warp");
                        slider(ui, &mut self.profile.visual.feedback, 0.0..=0.92, "Echo trails");
                    });
                    egui::CollapsingHeader::new("Audio mappings").default_open(false).show(ui, |ui| {
                        ui.label(RichText::new("Zero disconnects a band from its effect.").color(MUTED));
                        mapping_slider(ui, &mut self.profile.mappings.bass_to_pulse, "Bass / pulse");
                        mapping_slider(ui, &mut self.profile.mappings.mids_to_geometry, "Mids / shape");
                        mapping_slider(ui, &mut self.profile.mappings.treble_to_sparkle, "Air / particles");
                        mapping_slider(ui, &mut self.profile.mappings.beat_to_flash, "Beat / bloom");
                    });
                    egui::CollapsingHeader::new("Creative director").id_salt("creative-director-quiet").default_open(false).show(ui, |ui| {
                    ui.checkbox(&mut self.profile.director_enabled, "Let the music guide the world")
                        .on_hover_text("Local, rule-based guidance. No model or online service is connected.");
                    ui.add_enabled_ui(self.profile.director_enabled, |ui| {
                        slider(ui, &mut self.profile.director_freedom, 0.0..=1.0, "Freedom");
                    });
                    let note = if !self.profile.director_enabled {
                        "You have full control."
                    } else if self.auto_mode {
                        "Auto weaves the worlds; Freedom shapes the musical guidance."
                    } else if self.profile.visual.scene == 1 {
                        "Evolves within your chosen mathematics."
                    } else if self.profile.director_freedom > 0.58 {
                        "Freely shapes the style; your chosen world remains held."
                    } else {
                        "Gently varies your settings within this world."
                    };
                    ui.label(RichText::new(note).color(MUTED))
                        .on_hover_text(self.direction.as_ref().map(|direction| format!("{} · Suggested atmosphere: {}", direction.note, SCENES[direction.scene.min(SCENES.len() - 1)])).unwrap_or_else(|| "Waiting for the next musical phrase.".into()));
                    });
                    egui::CollapsingHeader::new("Profiles").id_salt("profiles-quiet").default_open(false).show(ui, |ui| {
                    let mut selected = self.active_profile;
                    egui::ComboBox::from_id_salt("profile").selected_text(profile_label(&self.profiles[selected], selected))
                        .width(ui.available_width()).show_ui(ui, |ui| {
                            for (index, item) in self.profiles.iter().enumerate() {
                                ui.selectable_value(&mut selected, index, profile_label(item, index));
                            }
                        });
                    if selected != self.active_profile { self.apply_profile(selected); }
                    ui.add(egui::TextEdit::singleline(&mut self.profile_name).hint_text("Name your universe")
                        .desired_width(f32::INFINITY).char_limit(80));
                    ui.horizontal(|ui| {
                        if ui.button("Save profile").clicked() { self.save_profile(); }
                        let ready = !self.profile_dialog.is_busy();
                        if ui.add_enabled(ready, egui::Button::new("Import…")).clicked() {
                            self.profile_dialog.import();
                            ctx.request_repaint();
                        }
                        if ui.add_enabled(ready, egui::Button::new("Export…")).clicked() {
                            let snapshot = self.snapshot_profile();
                            self.profile_dialog.export(snapshot);
                            ctx.request_repaint();
                        }
                    });
                    if self.profile_dialog.is_busy() {
                        ui.label(RichText::new("Profile file dialog is open or finishing…").color(MUTED));
                    }
                    });
                    egui::CollapsingHeader::new("Listening tools").id_salt("listening-tools-quiet").default_open(false).show(ui, |ui| {
                        let playing_test = self.test_track.is_playing();
                        if ui.small_button(if playing_test { "Stop test track" } else { "Play 15s test track" }).clicked() {
                            if playing_test {
                                self.test_track.stop();
                            } else {
                                if self.audio.mode() != AudioMode::System { self.select_audio(AudioMode::System); }
                                if let Err(error) = self.test_track.play() {
                                    self.flash_message(format!("Could not play test track: {error}"));
                                }
                            }
                        }
                        ui.label(RichText::new("Original test music via Mac output and live capture. Not Demo.").color(MUTED));
                    });
                    space(ui);
                    section(ui, "DISPLAY");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.show_equaliser, "Equaliser")
                            .on_hover_text("Show or hide the equaliser · E. Music listening and analysis continue independently of either panel.");
                        ui.checkbox(&mut self.sharp_rendering, "Fine")
                            .on_hover_text("Fine rendering: more pixels for finer lines and textures. Turn off to favour speed on a busy Mac. Remembered on this Mac, separately from your portable profile.");
                        if ui.button(if self.fullscreen { "Windowed" } else { "Full screen" }).on_hover_text("Toggle full screen · F").clicked() {
                            self.toggle_fullscreen(ctx);
                        }
                    });
                    ui.label(RichText::new("A auto  ·  H controls  ·  E equaliser  ·  F full screen  ·  1–9, 0 worlds  ·  M Mandelbrot  ·  G glass  ·  S silk  ·  N memory  ·  C coast").color(MUTED));
                    ui.add_space(4.0);
                });
            });
        readouts
    }

    pub(super) fn overlays(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        let accent = panel_accent(self.profile.visual.palette);
        if self.show_controls {
            egui::Area::new("brand".into())
                .fixed_pos(canvas.min + egui::vec2(24.0, 22.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    overlay_frame(self.profile.visual.palette).show(ui, |ui| {
                        ui.label(
                            RichText::new("amaraMusicVisualiser")
                                .size(H1_SIZE)
                                .color(TEXT),
                        );
                        ui.horizontal(|ui| {
                            let (text, colour) = if self.audio.mode() == AudioMode::Demo {
                                ("DEMO", accent)
                            } else if self.audio_fresh && self.meters.energy > 0.015 {
                                ("LIVE", accent)
                            } else if self.audio_fresh {
                                ("QUIET", MUTED)
                            } else {
                                ("WAITING", MUTED)
                            };
                            ui.label(RichText::new(text).color(colour));
                            ui.label(RichText::new(self.audio.mode().label()).color(MUTED))
                                .on_hover_text(
                                    self.audio_age
                                        .map(|age| {
                                            format!(
                                                "Latest audio arrived {} ms ago",
                                                age.as_millis()
                                            )
                                        })
                                        .unwrap_or_else(|| self.audio.status()),
                                );
                        });
                    });
                });
        }
        if self.show_equaliser {
            let width = 330.0_f32.min(canvas.width() - 48.0).max(240.0);
            egui::Area::new("music-dna".into())
                .pivot(egui::Align2::LEFT_BOTTOM)
                .fixed_pos(egui::pos2(canvas.left() + 24.0, canvas.bottom() - 22.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    overlay_frame(self.profile.visual.palette).show(ui, |ui| {
                        ui.set_width(width);
                        ui.spacing_mut().item_spacing.y = 3.0;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Music DNA").size(H2_SIZE).color(TEXT));
                            let status = if self.audio.mode() == AudioMode::Demo { "DEMO" }
                                else if !self.audio_fresh { "NO INPUT" }
                                else if self.meters.energy > 0.015 { "LIVE" } else { "QUIET" };
                            ui.label(RichText::new(status).color(accent))
                                .on_hover_text(self.audio.status());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .small_button("×")
                                        .on_hover_text("Hide equaliser panel · E")
                                        .clicked()
                                    {
                                        self.show_equaliser = false;
                                    }
                                },
                            );
                        });
                        ui.add(egui::Label::new(RichText::new(&self.features.dna.genre_hint).color(TEXT)).truncate())
                            .on_hover_text(self.features.dna.inference_method);
                        ui.add(egui::Label::new(RichText::new(&self.features.dna.mood).color(MUTED)).truncate())
                            .on_hover_text(&self.features.dna.mood);
                        if self.show_equaliser {
                            spectrum(ui, &self.meters.spectrum, &self.meters.peaks, self.profile.visual.palette);
                            let bpm = self.features.bpm.filter(|_| self.audio_fresh)
                                .map(|bpm| format!("≈{bpm:.0}")).unwrap_or_else(|| "—".into());
                            equaliser_readings(ui, &bpm);
                            egui::CollapsingHeader::new("Signal hints").id_salt("eq-signal-hints").default_open(false).show(ui, |ui| {
                                ui.label(RichText::new(&self.features.dna.instrument_hints).color(MUTED));
                                let voice_present = self.audio_fresh && self.features.voice.presence > 0.30;
                                let voice_label = if !self.audio_fresh { "Voice-like lead · waiting for audio" }
                                    else if voice_present { "Voice-like lead · sustained pattern" }
                                    else { "Voice-like lead · no sustained evidence" };
                                ui.label(RichText::new(voice_label).color(MUTED))
                                    .on_hover_text(format!("{}\nSustained harmonic patterns illuminate the existing shapes in every world. Mids / shape controls their strength. This is not reliable vocal recognition.", crate::voice::VOICE_METHOD));
                                ui.label(RichText::new(self.features.dna.inference_method).color(MUTED));
                            });
                        }
                    });
                });
        }
        if !self.show_controls {
            egui::Area::new("show-controls".into())
                .anchor(egui::Align2::RIGHT_TOP, [-20.0, 18.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = QUICK_PANEL_GAP;
                        if quick_panel_button(
                            ui,
                            self.profile.visual.palette,
                            "Controls",
                            self.show_controls,
                            "H",
                        )
                        .clicked()
                        {
                            self.show_controls = true;
                        }
                        if quick_panel_button(
                            ui,
                            self.profile.visual.palette,
                            "Equaliser",
                            self.show_equaliser,
                            "E",
                        )
                        .clicked()
                        {
                            self.show_equaliser = !self.show_equaliser;
                        }
                    });
                });
        }
        if let Some((message, at)) = &self.message
            && at.elapsed() < Duration::from_secs(5)
        {
            egui::Area::new("toast".into())
                .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -20.0])
                .order(egui::Order::Tooltip)
                .show(ctx, |ui| {
                    overlay_frame(self.profile.visual.palette).show(ui, |ui| {
                        ui.label(message);
                    });
                });
        }
    }
}

fn glass_fill(palette: VisualPalette) -> Color32 {
    match palette {
        VisualPalette::MoccaVanilla => Color32::from_rgba_unmultiplied(28, 24, 19, 137),
        VisualPalette::Cosmic => Color32::from_rgba_unmultiplied(16, 14, 25, 137),
    }
}

fn panel_accent(palette: VisualPalette) -> Color32 {
    match palette {
        VisualPalette::Cosmic => ACCENT,
        VisualPalette::MoccaVanilla => Color32::from_rgb(209, 192, 160),
    }
}

fn glass_frame(palette: VisualPalette) -> egui::Frame {
    let border = match palette {
        VisualPalette::MoccaVanilla => Color32::from_rgba_unmultiplied(239, 222, 188, 24),
        VisualPalette::Cosmic => Color32::from_rgba_unmultiplied(217, 204, 255, 24),
    };
    // Both surfaces use the same material, border, corners and shadow. Their
    // independent visibility is a display preference, not a different theme.
    egui::Frame::new()
        .fill(glass_fill(palette))
        .stroke(egui::Stroke::new(0.65, border))
        .corner_radius(12)
        .shadow(egui::epaint::Shadow::NONE)
}

fn overlay_frame(palette: VisualPalette) -> egui::Frame {
    glass_frame(palette).inner_margin(egui::Margin::symmetric(14, 10))
}

const QUICK_PANEL_SIZE: egui::Vec2 = egui::vec2(112.0, 32.0);
const QUICK_PANEL_GAP: f32 = 8.0;

/// A shared, quiet glass treatment: visibility changes the small status mark,
/// never the button's size, label weight or background material.
fn quick_panel_button(
    ui: &mut egui::Ui,
    palette: VisualPalette,
    label: &str,
    shown: bool,
    shortcut: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(QUICK_PANEL_SIZE, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), shown, label)
    });
    let text = TEXT;
    let accent = panel_accent(palette);
    if ui.is_rect_visible(rect) {
        let material = glass_frame(palette);
        let stroke = if response.hovered() || response.has_focus() {
            egui::Stroke::new(1.0, accent.gamma_multiply(0.7))
        } else {
            material.stroke
        };
        ui.painter().rect(
            rect,
            material.corner_radius,
            material.fill,
            stroke,
            egui::StrokeKind::Inside,
        );
        let mark = egui::pos2(rect.left() + 13.0, rect.center().y);
        if shown {
            ui.painter().circle_filled(mark, 2.5, accent);
        } else {
            ui.painter()
                .circle_stroke(mark, 2.5, egui::Stroke::new(1.0, text.gamma_multiply(0.5)));
        }
        ui.painter().text(
            rect.center() + egui::vec2(6.0, 0.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(BODY_SIZE),
            text,
        );
    }
    response.on_hover_text(format!(
        "{} {label} · {shortcut}",
        if shown { "Hide" } else { "Show" }
    ))
}

fn profile_label(profile: &Profile, index: usize) -> String {
    if index < crate::presets::BUILT_IN_PROFILE_COUNT {
        format!("{} · Built-in", profile.name)
    } else {
        profile.name.clone()
    }
}

fn live_selection_label(label: &str, from: &str, to: &str, blend: f32) -> String {
    if from != to && blend < 1.0 {
        format!(
            "{label} · {from} → {to} · {:.0}%",
            blend.clamp(0.0, 1.0) * 100.0
        )
    } else {
        format!("{label} · {to}")
    }
}

/// Child UIs retain the panel's clip/layer, but their space is reserved before
/// controls are laid out. Content is filled only after the render state advances.
pub(super) struct AutoReadoutSlots {
    rows: [egui::Ui; 3],
    countdown: egui::Ui,
}

impl AutoReadoutSlots {
    fn reserve(ui: &mut egui::Ui) -> Self {
        let rows = std::array::from_fn(|_| reserve_readout_ui(ui, READOUT_ROW_HEIGHT));
        Self {
            rows,
            countdown: reserve_readout_ui(ui, READOUT_ROW_HEIGHT + 2.0 + 4.0),
        }
    }

    pub(super) fn paint(
        mut self,
        scene: &SceneTransition,
        looks: &[[f32; 4]; WORLD_COUNT],
        mathematics: MathJourneyState,
        countdown: (f32, f32),
        palette: VisualPalette,
    ) {
        let labels = auto_readout_labels(scene.from, scene.to, scene.blend(), looks, mathematics);
        for (index, (ui, (text, tooltip))) in self.rows.iter_mut().zip(labels).enumerate() {
            if !text.is_empty() {
                fixed_readout(
                    ui,
                    &text,
                    &tooltip,
                    if index == 0 {
                        panel_accent(palette)
                    } else {
                        MUTED
                    },
                );
            }
        }
        auto_timing(&mut self.countdown, scene, countdown, palette);
    }
}

fn reserve_readout_ui(ui: &mut egui::Ui, height: f32) -> egui::Ui {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    )
}

/// Names come from this frame's actual endpoints, not a future queued choice.
/// The outer world melt and either world's inner look morph are separate clocks.
fn auto_readout_labels(
    source: usize,
    destination: usize,
    world_blend: f32,
    looks: &[[f32; 4]; WORLD_COUNT],
    mathematics: MathJourneyState,
) -> [(String, String); 3] {
    let source = source.min(WORLD_COUNT - 1);
    let destination = destination.min(WORLD_COUNT - 1);
    let world_blend = world_blend.clamp(0.0, 1.0);
    let family = |index: usize| {
        MathJourneyState {
            from: index.min(3),
            to: index.min(3),
            blend: 1.0,
            phase: 0.0,
        }
        .family_name()
    };
    let mathematics_label = live_selection_label(
        "Mathematics",
        family(mathematics.from),
        family(mathematics.to),
        mathematics.blend,
    );
    let variation = |world: usize, role: &str| {
        let lane = looks[world];
        let last_look = (visual::LOOKS[world].len() - 1) as f32;
        let from = lane[0].round().clamp(0.0, last_look) as usize;
        let to = lane[1].round().clamp(0.0, last_look) as usize;
        let label = live_selection_label(
            role,
            visual::LOOKS[world][from],
            visual::LOOKS[world][to],
            lane[2],
        );
        let mut tooltip = format!(
            "{role} world: {}\n{}\nAny percentage in this variation row is its inner look morph, not the world melt.",
            SCENES[world],
            live_selection_label(
                "Variation",
                visual::LOOKS[world][from],
                visual::LOOKS[world][to],
                lane[2]
            ),
        );
        if world == 1 {
            tooltip.push_str(&format!(
                "\n{mathematics_label} · independent geometry-family morph"
            ));
        }
        (label, tooltip)
    };
    let world = live_selection_label("World", SCENES[source], SCENES[destination], world_blend);
    if source != destination && world_blend < 1.0 {
        [
            (
                world,
                format!(
                    "Leaving world: {}\nArriving world: {}\nWorld melt: {:.0}% (not the inner variation morph). Both endpoint worlds can remain visible.",
                    SCENES[source],
                    SCENES[destination],
                    world_blend * 100.0
                ),
            ),
            variation(source, "Leaving"),
            variation(destination, "Arriving"),
        ]
    } else {
        [
            (world, format!("Current world: {}", SCENES[destination])),
            variation(destination, "Variation"),
            if destination == 1 {
                (
                    mathematics_label.clone(),
                    format!(
                        "Fractal Bloom's actual geometry family: {mathematics_label}. Its family morph is independent of the named structural variation."
                    ),
                )
            } else {
                (String::new(), String::new())
            },
        ]
    }
}

/// Fixed one-line slots keep every control below the live names stationary.
/// The full current/incoming names and morph percentage remain in the tooltip.
fn fixed_readout(ui: &mut egui::Ui, text: &str, tooltip: &str, colour: Color32) -> egui::Response {
    let size = egui::vec2(ui.available_width(), READOUT_ROW_HEIGHT);
    // add_sized centers the label's galley even when its text is left-aligned.
    // Reserve the full slot, but explicitly anchor its single line at the left.
    ui.allocate_ui_with_layout(
        size,
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_size(size);
            ui.add(
                egui::Label::new(RichText::new(text).color(colour))
                    .halign(egui::Align::Min)
                    .truncate()
                    .show_tooltip_when_elided(false),
            );
        },
    )
    .response
    .on_hover_text(tooltip)
}

/// One shared label/selector grid, including an accessory lane for the
/// background shuffle button. Long selections truncate, never move a column.
fn selector_row(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui, f32)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            egui::vec2(88.0, ui.spacing().interact_size.y),
            egui::Label::new(label).halign(egui::Align::Min).truncate(),
        );
        let width = (ui.available_width() - 29.0).max(80.0);
        content(ui, width);
    });
}

/// The scene is the same advanced snapshot sent to the GPU this frame. Auto's
/// request clock resets as a melt begins, but must not hide that active melt
/// behind a fresh countdown. A queued world is not an arriving world yet.
fn auto_timing_readout(scene: &SceneTransition, countdown: (f32, f32)) -> (String, String, f32) {
    if scene.from != scene.to && scene.amount < 1.0 {
        let destination = SCENES[scene.to.min(WORLD_COUNT - 1)];
        let progress = scene.blend().clamp(0.0, 1.0);
        (
            format!("Transforming · {destination}"),
            format!(
                "Transforming into {destination} · {:.0}%. This bar follows the world transformation rendered now. The Leaving and Arriving rows show each world's actual variation.",
                progress * 100.0
            ),
            progress,
        )
    } else {
        let (progress, remaining) = countdown;
        (
            format!("Next world in {:.0}s", remaining.max(0.0).ceil()),
            "Time until Auto starts the next world transformation. At its first frame this indicator switches to the active transformation. Mandelbrot stays 64–70 seconds; other worlds stay 18–24. Pace does not change this clock; music can gently adjust it.".into(),
            progress.clamp(0.0, 1.0),
        )
    }
}

fn auto_timing(
    ui: &mut egui::Ui,
    scene: &SceneTransition,
    countdown: (f32, f32),
    palette: VisualPalette,
) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        let (text, tooltip, progress) = auto_timing_readout(scene, countdown);
        fixed_readout(ui, &text, &tooltip, MUTED);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.0), egui::Sense::hover());
        let colour = panel_accent(palette);
        ui.painter()
            .rect_filled(rect, 2.0, Color32::from_white_alpha(24));
        let filled = egui::Rect::from_min_size(
            rect.min,
            egui::vec2(rect.width() * progress.clamp(0.0, 1.0), rect.height()),
        );
        ui.painter().rect_filled(filled, 2.0, colour);
        response.on_hover_text(tooltip);
    });
}

fn section(ui: &mut egui::Ui, label: &str) {
    ui.label(RichText::new(label).size(H2_SIZE).color(TEXT));
    ui.add_space(2.0);
}

fn space(ui: &mut egui::Ui) {
    ui.add_space(9.0);
}

fn slider(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str) {
    ui.add(
        egui::Slider::new(value, range)
            .text(label)
            .show_value(false),
    );
}

fn mapping_slider(ui: &mut egui::Ui, value: &mut f32, label: &str) {
    ui.add(
        egui::Slider::new(value, 0.0..=1.5)
            .text(label)
            .fixed_decimals(2),
    );
}

fn pace_slider(ui: &mut egui::Ui, value: &mut f32) {
    let mut rate = motion_rate(*value);
    ui.horizontal(|ui| {
        if ui.add(egui::Slider::new(&mut rate, 0.05..=10.8)
            .logarithmic(true).show_value(false).text(format!("Pace · {:.2}×", motion_rate(*value))))
            .on_hover_text("Flight and orbiting span 0.05–10.8×. Auto requests a new main world every 18–24 seconds with a 7–9 second melt. Visible structures explore every 12–18 seconds. Pace changes movement, not theme timing; your music and equaliser always run in real time.")
            .changed()
        {
            *value = motion_value(rate);
        }
        if ui.small_button("1×").on_hover_text("Return to normal pace").clicked() {
            *value = 1.0;
        }
    });
}

fn equaliser_readings(ui: &mut egui::Ui, bpm: &str) {
    let column_width = (ui.available_width() - 21.0) / 4.0;
    egui::Grid::new("equaliser-readings")
        .num_columns(4)
        .min_col_width(column_width)
        .spacing(egui::vec2(7.0, 1.0))
        .show(ui, |ui| {
            for label in ["Bass", "Mid", "Air"] {
                ui.label(RichText::new(label).color(MUTED));
            }
            ui.label(RichText::new(format!("BPM {bpm}")).color(TEXT));
            ui.end_row();
        });
}

fn frequency_colour(position: f32, palette: VisualPalette) -> Color32 {
    let stops = match palette {
        VisualPalette::Cosmic => [
            [244.0, 107.0, 177.0],
            [184.0, 157.0, 255.0],
            [110.0, 222.0, 244.0],
        ],
        VisualPalette::MoccaVanilla => [
            [233.0, 144.0, 109.0],
            [229.0, 193.0, 131.0],
            [128.0, 203.0, 193.0],
        ],
    };
    let x = position.clamp(0.0, 1.0) * 2.0;
    let index = (x.floor() as usize).min(1);
    let amount = x - index as f32;
    let channels: [u8; 3] = std::array::from_fn(|channel| {
        (stops[index][channel] + (stops[index + 1][channel] - stops[index][channel]) * amount)
            .round() as u8
    });
    Color32::from_rgb(channels[0], channels[1], channels[2])
}

/// Eased measured frequency energies, never a scripted decorative animation.
fn spectrum(ui: &mut egui::Ui, bands: &[f32], peaks: &[f32], palette: VisualPalette) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::hover());
    response.on_hover_text("Measured frequency energy, bass to treble. Peak lines hold for 0.7 seconds then fall gently. Bars use smooth display ballistics; FFT and beat timing remain undelayed. Demo uses a synthetic signal.");
    let count = bands.len().max(1);
    let stride = rect.width() / count as f32;
    for (index, energy) in bands.iter().enumerate() {
        let position = index as f32 / count.saturating_sub(1).max(1) as f32;
        let frequency_colour = frequency_colour(position, palette);
        let colour = frequency_colour.gamma_multiply(0.68 + 0.32 * energy.clamp(0.0, 1.0).sqrt());
        let height = (energy.clamp(0.0, 1.0).sqrt() * rect.height()).max(1.5);
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left() + index as f32 * stride, rect.bottom() - height),
            egui::vec2((stride - 2.0).max(1.0), height),
        );
        ui.painter().rect_filled(bar, 1.0, colour);
        let peak = peaks.get(index).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        if peak > 0.002 {
            let y = (rect.bottom() - peak.sqrt() * rect.height() - 1.5).max(rect.top() + 0.5);
            ui.painter().line_segment(
                [egui::pos2(bar.left(), y), egui::pos2(bar.right(), y)],
                egui::Stroke::new(1.0, frequency_colour),
            );
        }
    }
}

fn interface_fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    let name = "JetBrains Mono Nerd Font Regular".to_owned();
    fonts.font_data.insert(
        name.clone(),
        egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/JetBrainsMonoNerdFont-Regular.ttf"
        ))
        .into(),
    );
    // Keep egui's complete default chains behind the chosen face: arrows,
    // mathematical notation, emoji and missing glyphs retain their fallbacks.
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.clone());
    }
    fonts
}

pub(super) fn configure_style(ctx: &egui::Context, palette: VisualPalette) {
    ctx.set_fonts(interface_fonts());
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.spacing.item_spacing = egui::vec2(7.0, 5.0);
    style.spacing.slider_width = 124.0;
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(H1_SIZE),
    );
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(BODY_SIZE));
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(BODY_SIZE),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(BODY_SIZE),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(BODY_SIZE),
    );
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = None;
    let glass = glass_frame(palette);
    visuals.panel_fill = glass.fill;
    // Expanded menus must remain readable over moving art. The sidebar and
    // closed selector buttons keep their explicit translucent glass frames.
    visuals.window_fill = match palette {
        VisualPalette::Cosmic => Color32::from_rgb(20, 18, 29),
        VisualPalette::MoccaVanilla => Color32::from_rgb(35, 29, 23),
    };
    visuals.window_stroke = glass.stroke;
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.popup_shadow = egui::epaint::Shadow::NONE;
    visuals.extreme_bg_color = Color32::from_black_alpha(35);
    let accent = panel_accent(palette);
    visuals.hyperlink_color = accent;
    visuals.selection.bg_fill = accent.gamma_multiply(0.24);
    visuals.selection.stroke = egui::Stroke::new(0.65, accent);
    visuals.widgets.inactive.bg_fill = Color32::from_white_alpha(12);
    visuals.widgets.hovered.bg_fill = Color32::from_white_alpha(22);
    visuals.widgets.active.bg_fill = accent.gamma_multiply(0.22);
    visuals.widgets.open.bg_fill = accent.gamma_multiply(0.22);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.fg_stroke = egui::Stroke::new(0.8, TEXT);
        widget.bg_stroke = egui::Stroke::new(0.5, TEXT.gamma_multiply(0.12));
        // ComboBox button_frame paints weak_bg_fill, not bg_fill; leaving
        // its defaults here made open/hovered selectors opaque.
        widget.weak_bg_fill = widget.bg_fill;
    }
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(0.8, MUTED);
    visuals.widgets.inactive.corner_radius = 7.into();
    visuals.widgets.hovered.corner_radius = 7.into();
    visuals.widgets.active.corner_radius = 7.into();
    style.visuals = visuals;
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_style_of(egui::Theme::Dark, style);
}

#[cfg(test)]
mod quick_panel_tests {
    use super::*;

    #[test]
    fn changing_auto_names_keeps_control_positions_and_selector_glass_stable() {
        for size in [egui::vec2(1440.0, 842.0), egui::vec2(960.0, 640.0)] {
            for palette in [VisualPalette::Cosmic, VisualPalette::MoccaVanilla] {
                let ctx = egui::Context::default();
                configure_style(&ctx, palette);
                let style = ctx.style_of(egui::Theme::Dark);
                assert_eq!(egui::Frame::popup(&style).fill.a(), 255);
                assert_eq!(style.visuals.panel_fill, glass_frame(palette).fill);
                for widget in [
                    &style.visuals.widgets.inactive,
                    &style.visuals.widgets.hovered,
                    &style.visuals.widgets.active,
                    &style.visuals.widgets.open,
                ] {
                    assert!(widget.bg_fill.a() < 255);
                    assert_eq!(widget.bg_fill, widget.weak_bg_fill);
                }
                let mut previous = None;
                for (frame, (from, to, blend)) in [
                    (8, 8, 1.0),
                    (4, 6, 0.35),
                    (1, 3, 0.75),
                    (1, 1, 1.0),
                    (3, 3, 1.0),
                ]
                .into_iter()
                .enumerate()
                {
                    let mut geometry = None;
                    let mut expected_text = Vec::new();
                    let output = ctx.run_ui(
                        egui::RawInput {
                            time: Some(frame as f64),
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                            ..Default::default()
                        },
                        |root| {
                            egui::Panel::right("fixed-auto-layout-test")
                                .resizable(false)
                                .exact_size(340.0)
                                .frame(
                                    glass_frame(palette)
                                        .inner_margin(egui::Margin::symmetric(18, 16)),
                                )
                                .show(root, |ui| {
                                    let slots = AutoReadoutSlots::reserve(ui);
                                    let readouts = std::array::from_fn::<_, 3, _>(|index| {
                                        slots.rows[index].max_rect()
                                    });
                                    let mut selector = egui::Rect::NOTHING;
                                    selector_row(ui, "Palette", |ui, width| {
                                        selector =
                                            egui::ComboBox::from_id_salt("fixed-palette-test")
                                                .selected_text(palette.label())
                                                .width(width)
                                                .truncate()
                                                .show_ui(ui, |_ui| {})
                                                .response
                                                .rect;
                                    });
                                    let marker = ui.checkbox(&mut false, "Equaliser").rect;
                                    geometry = Some((readouts, selector, marker));
                                    // Advance/select only AFTER reserving/layout:
                                    // text must consume these same-frame endpoints.
                                    let mut scene = SceneTransition::new(from);
                                    scene.request(to);
                                    scene.amount = blend;
                                    let looks = [[0.0, 9.0, blend, 0.0]; WORLD_COUNT];
                                    let mathematics = MathJourneyState {
                                        from: 0,
                                        to: 2,
                                        blend: 0.4,
                                        phase: 0.0,
                                    };
                                    expected_text = auto_readout_labels(
                                        from,
                                        to,
                                        scene.blend(),
                                        &looks,
                                        mathematics,
                                    )
                                    .into_iter()
                                    .zip(readouts)
                                    .filter_map(|((text, _), rect)| {
                                        (!text.is_empty()).then_some((text, rect))
                                    })
                                    .collect();
                                    expected_text.push((
                                        if from != to && blend < 1.0 {
                                            format!("Transforming · {}", SCENES[to])
                                        } else {
                                            format!("Next world in {:.0}s", 24.0 - frame as f32)
                                        },
                                        egui::Rect::from_min_size(
                                            slots.countdown.max_rect().min,
                                            egui::vec2(
                                                slots.countdown.max_rect().width(),
                                                READOUT_ROW_HEIGHT,
                                            ),
                                        ),
                                    ));
                                    slots.paint(
                                        &scene,
                                        &looks,
                                        mathematics,
                                        (blend, 24.0 - frame as f32),
                                        palette,
                                    );
                                });
                        },
                    );
                    let geometry = geometry.unwrap();
                    assert!(
                        geometry
                            .0
                            .iter()
                            .all(|rect| rect.height() == READOUT_ROW_HEIGHT)
                    );
                    let readout_text: Vec<_> = output
                        .shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::epaint::Shape::Text(text)
                                if expected_text
                                    .iter()
                                    .any(|(expected, _)| text.galley.job.text == *expected) =>
                            {
                                Some(text)
                            }
                            _ => None,
                        })
                        .collect();
                    assert_eq!(readout_text.len(), expected_text.len());
                    for (text, (expected, rect)) in readout_text.iter().zip(&expected_text) {
                        assert_eq!(&text.galley.job.text, expected);
                        assert_eq!(text.galley.rows.len(), 1);
                        assert!(
                            (text.pos.x + text.galley.rect.left() - geometry.0[0].left()).abs()
                                < 0.01,
                            "readout must paint at the sidebar's left edge: {}",
                            text.galley.job.text
                        );
                        assert!(
                            (text.pos.y + text.galley.rect.center().y - rect.center().y).abs()
                                <= 1.0,
                            "readout must stay vertically centered in its fixed-height slot"
                        );
                    }
                    if let Some(previous) = previous {
                        assert_eq!(
                            geometry, previous,
                            "live names must not move later controls"
                        );
                    }
                    previous = Some(geometry);
                    output.drop_without_applying_deltas();
                }
            }
        }
    }

    #[test]
    fn auto_timing_switches_on_the_first_transform_frame_and_returns_after_completion() {
        let mut scene = SceneTransition::new(4);
        let holding = auto_timing_readout(&scene, (0.98, 0.2));
        assert_eq!(holding.0, "Next world in 1s");
        assert_eq!(holding.2, 0.98);

        scene.request(8);
        // A reset next-request clock cannot conceal the transform that starts
        // now, even at the exact zero-blend boundary before its first advance.
        let started = auto_timing_readout(&scene, (0.0, 21.0));
        assert_eq!(started.0, "Transforming · Water Ripples");
        assert_eq!(started.2, 0.0);
        scene.advance_over(1.0 / 60.0, 8.0);
        let first_frame = auto_timing_readout(&scene, (0.001, 20.9));
        assert_eq!(first_frame.0, started.0);
        assert_eq!(first_frame.2, scene.blend());
        assert!(first_frame.2 > 0.0);

        scene.advance_over(4.0, 8.0);
        let halfway = auto_timing_readout(&scene, (0.2, 16.0));
        assert_eq!(halfway.0, started.0);
        assert_eq!(halfway.2, scene.blend());
        assert_ne!(halfway.2, 0.2);
        scene.advance_over(8.0, 8.0);
        let settled = auto_timing_readout(&scene, (0.4, 12.0));
        assert_eq!(settled.0, "Next world in 12s");
        assert_eq!(settled.2, 0.4);
    }

    #[test]
    fn auto_timing_names_the_rendered_destination_not_a_queued_world() {
        let mut scene = SceneTransition::new(0);
        scene.request(4);
        scene.advance_over(2.0, 8.0);
        scene.request(9);
        let active = auto_timing_readout(&scene, (0.0, 20.0));
        assert_eq!(active.0, "Transforming · Particle Constellation");
        assert_eq!(active.2, scene.blend());
        scene.advance_over(6.0, 8.0);
        let next = auto_timing_readout(&scene, (0.3, 14.0));
        assert_eq!(next.0, "Transforming · Northern Lights");
        assert_eq!(next.2, 0.0);
    }

    #[test]
    fn auto_readouts_show_both_actual_world_look_endpoints_through_the_melt() {
        let mut looks = [[0.0, 0.0, 1.0, 0.0]; WORLD_COUNT];
        looks[4] = [2.0, 5.0, 0.25, 0.0];
        looks[8] = [6.0, 7.0, 0.75, 0.0];
        let math = MathJourneyState {
            from: 0,
            to: 0,
            blend: 1.0,
            phase: 0.0,
        };
        for outer in [0.0, 0.01, 0.4, 0.99] {
            let rows = auto_readout_labels(4, 8, outer, &looks, math);
            assert_eq!(rows[1].0, "Leaving · Trefoil Orbit → Harmonic Ribbon · 25%");
            assert_eq!(rows[2].0, "Arriving · Cross Current → Mercury Pool · 75%");
            assert!(rows[0].1.contains("Leaving world: Particle Constellation"));
            assert!(rows[0].1.contains("Arriving world: Water Ripples"));
            assert!(rows[1].1.contains("Leaving world: Particle Constellation"));
            assert!(rows[2].1.contains("Arriving world: Water Ripples"));
            assert!(rows[2].1.contains("inner look morph, not the world melt"));
        }
        let settled = auto_readout_labels(4, 8, 1.0, &looks, math);
        assert_eq!(settled[0].0, "World · Water Ripples");
        assert_eq!(
            settled[1].0,
            "Variation · Cross Current → Mercury Pool · 75%"
        );
        assert!(settled[2].0.is_empty());
        assert!(!settled.iter().any(|(_, tip)| tip.contains("Trefoil Orbit")));
    }

    #[test]
    fn auto_fractal_readouts_report_the_actual_independent_math_family_blend() {
        let mut looks = [[0.0, 0.0, 1.0, 0.0]; WORLD_COUNT];
        looks[1] = [7.0, 7.0, 1.0, 0.0];
        let math = MathJourneyState {
            from: 0,
            to: 2,
            blend: 0.35,
            phase: 0.0,
        };
        let arriving = auto_readout_labels(8, 1, 0.9, &looks, math);
        assert_eq!(arriving[2].0, "Arriving · Crystal Estuary");
        assert!(
            arriving[2]
                .1
                .contains("Mathematics · Julia → Harmonic Sculpture · 35%")
        );
        let settled = auto_readout_labels(8, 1, 1.0, &looks, math);
        assert_eq!(settled[1].0, "Variation · Crystal Estuary");
        assert_eq!(
            settled[2].0,
            "Mathematics · Julia → Harmonic Sculpture · 35%"
        );
    }

    #[test]
    fn quiet_panels_share_light_glass_and_only_three_text_sizes() {
        for palette in [VisualPalette::Cosmic, VisualPalette::MoccaVanilla] {
            let ctx = egui::Context::default();
            configure_style(&ctx, palette);
            let style = ctx.style_of(egui::Theme::Dark);
            for (kind, font) in &style.text_styles {
                let expected = if *kind == egui::TextStyle::Heading {
                    H1_SIZE
                } else {
                    BODY_SIZE
                };
                assert_eq!(font.size, expected);
            }
            let sidebar = glass_frame(palette);
            let equaliser = overlay_frame(palette);
            assert_eq!(sidebar.fill, equaliser.fill);
            assert_eq!(sidebar.stroke, equaliser.stroke);
            assert_eq!(sidebar.corner_radius, equaliser.corner_radius);
            assert!(sidebar.stroke.width < 1.0);
            assert!(sidebar.fill.a() < 150);
        }
    }

    #[test]
    fn interface_font_is_regular_jetbrains_mono_without_removing_symbol_fallbacks() {
        let fonts = interface_fonts();
        let defaults = egui::FontDefinitions::default();
        let chosen = "JetBrains Mono Nerd Font Regular";
        assert!(fonts.font_data.contains_key(chosen));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let chain = &fonts.families[&family];
            assert_eq!(chain[0], chosen);
            assert_eq!(&chain[1..], defaults.families[&family].as_slice());
        }
    }

    #[test]
    fn live_auto_labels_distinguish_melting_from_settled_selections() {
        assert_eq!(
            live_selection_label("World", "Deep Flight", "Water Ripples", 0.35),
            "World · Deep Flight → Water Ripples · 35%"
        );
        assert_eq!(
            live_selection_label("Variation", "Listening Pool", "Twin Springs", 1.0),
            "Variation · Twin Springs"
        );
        assert_eq!(
            live_selection_label(
                "Variation (incoming)",
                "Listening Pool",
                "Twin Springs",
                0.5
            ),
            "Variation (incoming) · Listening Pool → Twin Springs · 50%"
        );
    }

    #[test]
    fn water_factory_profile_is_labeled_built_in_but_personal_copy_is_not() {
        let profiles = built_in_profiles();
        let index = profiles
            .iter()
            .position(|profile| profile.name == "Prismatic Water")
            .unwrap();
        assert_eq!(
            profile_label(&profiles[index], index),
            "Prismatic Water · Built-in"
        );
        assert_eq!(
            profile_label(&profiles[index], crate::presets::BUILT_IN_PROFILE_COUNT),
            "Prismatic Water"
        );
    }

    #[test]
    fn quick_buttons_keep_matching_geometry_and_glass_in_both_states_and_palettes() {
        for palette in [VisualPalette::Cosmic, VisualPalette::MoccaVanilla] {
            for equaliser_shown in [false, true] {
                let ctx = egui::Context::default();
                configure_style(&ctx, palette);
                // Warm the Area's measured size so its right anchor is settled.
                for frame in 0..3 {
                    let mut rectangles = None;
                    let output = ctx.run_ui(
                        egui::RawInput {
                            time: Some(frame as f64),
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(1920.0, 1080.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            egui::Area::new("quick-panel-layout-test".into())
                                .anchor(egui::Align2::RIGHT_TOP, [-20.0, 18.0])
                                .show(ui.ctx(), |ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = QUICK_PANEL_GAP;
                                        let controls =
                                            quick_panel_button(ui, palette, "Controls", false, "H");
                                        let equaliser = quick_panel_button(
                                            ui,
                                            palette,
                                            "Equaliser",
                                            equaliser_shown,
                                            "E",
                                        );
                                        rectangles = Some((controls.rect, equaliser.rect));
                                    });
                                });
                        },
                    );
                    if frame == 2 {
                        let (controls, equaliser) = rectangles.unwrap();
                        assert_eq!(controls.size(), QUICK_PANEL_SIZE);
                        assert_eq!(equaliser.size(), QUICK_PANEL_SIZE);
                        assert_eq!(controls.top(), equaliser.top());
                        assert_eq!(equaliser.left() - controls.right(), QUICK_PANEL_GAP);
                        assert_eq!(controls.top(), 18.0);
                        assert_eq!(equaliser.right(), 1900.0);
                        let material = glass_frame(palette);
                        let backgrounds: Vec<_> = output
                            .shapes
                            .iter()
                            .filter_map(|shape| match &shape.shape {
                                egui::epaint::Shape::Rect(rect)
                                    if rect.rect == controls || rect.rect == equaliser =>
                                {
                                    Some(rect)
                                }
                                _ => None,
                            })
                            .collect();
                        assert_eq!(backgrounds.len(), 2);
                        for background in backgrounds {
                            assert_eq!(background.fill, material.fill);
                            assert_eq!(background.stroke, material.stroke);
                            assert_eq!(background.corner_radius, material.corner_radius);
                        }
                    }
                    output.drop_without_applying_deltas();
                }
            }
        }
    }
}
