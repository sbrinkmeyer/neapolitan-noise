//! UI: one row — mode toggle, volume slider, volume entry, playback toggle,
//! help. Mirrors the layout and palette of the Electron build's `index.html`.
//!
//! Keyboard: Tab walks the controls in that order. Space/Enter activate the
//! focused button, Left/Right move a focused slider, and Up/Down step a focused
//! number entry — all of it egui's built-in behavior.

use std::sync::Arc;

use eframe::egui;

use crate::audio::{self, Params};

/// Scroll distance that steps the volume by one, so a wheel notch moves 1 the
/// way it did in the browser build without trackpad inertia running away.
const SCROLL_PER_STEP: f32 = 20.0;

const VOLUME_MIN: u32 = 0;
const VOLUME_MAX: u32 = 99;

const BG: egui::Color32 = egui::Color32::from_rgb(0x1a, 0x10, 0x08);
const TEXT: egui::Color32 = egui::Color32::from_rgb(0xd4, 0xa9, 0x6a);
const BTN_BG: egui::Color32 = egui::Color32::from_rgb(0x37, 0x20, 0x12);
const BTN_BORDER: egui::Color32 = egui::Color32::from_rgb(0x6b, 0x3a, 0x1f);
const BTN_TEXT: egui::Color32 = egui::Color32::from_rgb(0xc6, 0x98, 0x5d);
const BTN_HOVER: egui::Color32 = egui::Color32::from_rgb(0x7d, 0x45, 0x25);
const BTN_PRESSED: egui::Color32 = egui::Color32::from_rgb(0x5a, 0x2f, 0x18);
const LIT_BG: egui::Color32 = egui::Color32::from_rgb(0x8a, 0x4d, 0x27);
const LIT_BORDER: egui::Color32 = egui::Color32::from_rgb(0xa0, 0x62, 0x2a);
const LIT_TEXT: egui::Color32 = egui::Color32::from_rgb(0xff, 0xe1, 0xba);
const TROUGH: egui::Color32 = egui::Color32::from_rgb(0x26, 0x17, 0x0c);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(0xa0, 0x62, 0x2a);
const BADGE_BG: egui::Color32 = egui::Color32::from_rgb(0x12, 0x0b, 0x05);
const BADGE_BORDER: egui::Color32 = egui::Color32::from_rgb(0x7b, 0x4d, 0x2a);
const BADGE_TEXT: egui::Color32 = egui::Color32::from_rgb(0xf0, 0xd0, 0xa0);
const ERROR_TEXT: egui::Color32 = egui::Color32::from_rgb(0xff, 0x8a, 0x6a);

const PLAY_BUTTON_SIZE: egui::Vec2 = egui::vec2(52.0, 26.0);
const MODE_BUTTON_SIZE: egui::Vec2 = egui::vec2(62.0, 26.0);
const HELP_BUTTON_SIZE: egui::Vec2 = egui::vec2(26.0, 26.0);
const BADGE_SIZE: egui::Vec2 = egui::vec2(42.0, 26.0);

/// Window size with the help panel closed and open. The help panel lives in the
/// same window rather than a second one, so the app stays a single tiny thing.
pub const COLLAPSED_SIZE: egui::Vec2 = egui::vec2(400.0, 46.0);
pub const EXPANDED_SIZE: egui::Vec2 = egui::vec2(400.0, 292.0);

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub volume: u32,
    pub stereo: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 30,
            stereo: false,
        }
    }
}

pub struct Brownie {
    params: Arc<Params>,
    /// `None` when stopped. cpal streams are not `Send`, and eframe runs the app
    /// on one thread, so holding it here is fine.
    stream: Option<cpal::Stream>,
    settings: Settings,
    playing: bool,
    /// Fading out; the stream is dropped once the callback reports silence.
    stopping: bool,
    help_open: bool,
    /// Whether the window size has been asserted yet (first frame).
    sized: bool,
    scroll_accum: f32,
    error: Option<String>,
}

impl Brownie {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let stored: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        let settings = Settings {
            volume: stored.volume.clamp(VOLUME_MIN, VOLUME_MAX),
            stereo: stored.stereo,
        };

        apply_theme(&cc.egui_ctx);

        Self {
            params: Arc::new(Params::new(gain_for(settings.volume), settings.stereo)),
            stream: None,
            settings,
            playing: false,
            stopping: false,
            help_open: false,
            sized: false,
            scroll_accum: 0.0,
            error: None,
        }
    }

    fn start(&mut self) {
        self.params.set_gain(gain_for(self.settings.volume));
        self.params.set_stereo(self.settings.stereo);
        self.stopping = false;

        match audio::start(Arc::clone(&self.params)) {
            Ok(stream) => {
                self.stream = Some(stream);
                self.playing = true;
                self.error = None;
            }
            Err(err) => {
                self.stream = None;
                self.playing = false;
                self.error = Some(err);
            }
        }
    }

    /// Ramp to zero and let [`Self::logic`] drop the stream once it is silent.
    fn stop(&mut self) {
        self.params.set_gain(0.0);
        self.playing = false;
        self.stopping = self.stream.is_some();
    }

    fn nudge_volume(&mut self, steps: i32) {
        if steps == 0 {
            return;
        }
        self.settings.volume = (self.settings.volume as i32 + steps)
            .clamp(VOLUME_MIN as i32, VOLUME_MAX as i32) as u32;
    }

    fn toggle_help(&mut self, ctx: &egui::Context) {
        self.help_open = !self.help_open;
        self.apply_window_size(ctx);
    }

    /// Drive the window to exactly one of the two sizes.
    ///
    /// Moves the min/max clamps along with the size rather than only setting the
    /// size: leaving them unpinned lets the platform keep whatever size it opened
    /// with. Called on the first frame too, so the window is correct even if
    /// something upstream ignored the builder's hints.
    fn apply_window_size(&self, ctx: &egui::Context) {
        let size = if self.help_open {
            EXPANDED_SIZE
        } else {
            COLLAPSED_SIZE
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
    }
}

fn gain_for(volume: u32) -> f32 {
    volume as f32 / 100.0
}

fn apply_theme(ctx: &egui::Context) {
    // The palette is a single brown scheme, so pin the theme rather than letting
    // the OS pick light mode and half-apply it.
    ctx.set_theme(egui::ThemePreference::Dark);

    ctx.all_styles_mut(|style| {
        let visuals = &mut style.visuals;

        visuals.dark_mode = true;
        visuals.panel_fill = BG;
        visuals.window_fill = BG;
        visuals.extreme_bg_color = TROUGH;
        visuals.override_text_color = Some(TEXT);
        visuals.selection.bg_fill = ACCENT;
        visuals.selection.stroke = egui::Stroke::new(1.0, LIT_TEXT);

        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(6);
            widget.bg_fill = TROUGH;
            widget.weak_bg_fill = BTN_BG;
            widget.bg_stroke = egui::Stroke::new(1.0, BTN_BORDER);
            widget.fg_stroke = egui::Stroke::new(1.0, BTN_TEXT);
        }
        visuals.widgets.hovered.weak_bg_fill = BTN_HOVER;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, LIT_BORDER);
        visuals.widgets.active.weak_bg_fill = BTN_PRESSED;
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, LIT_BORDER);
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT);

        // Keyboard focus has to be obvious, since Tab order is a headline feature.
        visuals.widgets.hovered.expansion = 0.0;
        visuals.widgets.active.expansion = 0.0;

        style.spacing.item_spacing = egui::vec2(10.0, 6.0);
        style.spacing.slider_width = 128.0;
        style.spacing.interact_size = egui::vec2(28.0, 26.0);
    });
}

fn toggle_button(ui: &mut egui::Ui, label: &str, lit: bool, size: egui::Vec2) -> egui::Response {
    let mut button = egui::Button::new(egui::RichText::new(label).size(12.0).strong())
        .min_size(size)
        .corner_radius(6);
    if lit {
        button = button.fill(LIT_BG).stroke(egui::Stroke::new(1.0, LIT_BORDER));
    }
    ui.add(button)
}

impl eframe::App for Brownie {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.sized {
            self.sized = true;
            self.apply_window_size(ctx);
        }

        if self.stopping {
            if self.params.is_silent() {
                self.stream = None;
                self.stopping = false;
            } else {
                // Keep frames coming so we notice the fade finishing.
                ctx.request_repaint_after(std::time::Duration::from_millis(10));
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // The number entry owns Up/Down while focused, so the help panel must not
        // also grab those keys for scrolling.
        let mut badge_focused = false;

        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // --- 1. mono / stereo ---------------------------------------
                    let mode_label = if self.settings.stereo {
                        "stereo"
                    } else {
                        "mono"
                    };
                    let mode = toggle_button(
                        ui,
                        mode_label,
                        self.settings.stereo,
                        MODE_BUTTON_SIZE,
                    )
                    .on_hover_text(if self.settings.stereo {
                        "stereo: each ear gets its own private rumble"
                    } else {
                        "mono: both ears, same rumble"
                    });
                    if mode.clicked() {
                        self.settings.stereo = !self.settings.stereo;
                        self.params.set_stereo(self.settings.stereo);
                    }

                    // --- 2. volume slider ---------------------------------------
                    let slider = ui
                        .add(
                            egui::Slider::new(&mut self.settings.volume, VOLUME_MIN..=VOLUME_MAX)
                                .show_value(false)
                                .integer(),
                        )
                        .on_hover_text("drag, scroll, or left/right-arrow the rumble");

                    if slider.hovered() {
                        self.scroll_accum += ui.input(|i| i.smooth_scroll_delta.y);
                        while self.scroll_accum.abs() >= SCROLL_PER_STEP {
                            let step = self.scroll_accum.signum();
                            self.scroll_accum -= step * SCROLL_PER_STEP;
                            self.nudge_volume(step as i32);
                        }
                    } else {
                        self.scroll_accum = 0.0;
                    }

                    // --- 3. volume entry ----------------------------------------
                    // Own tab stop: click or focus it and type a number.
                    let badge = ui
                        .scope(|ui| {
                            let widgets = &mut ui.visuals_mut().widgets;
                            for widget in [
                                &mut widgets.inactive,
                                &mut widgets.hovered,
                                &mut widgets.active,
                            ] {
                                widget.weak_bg_fill = BADGE_BG;
                                widget.bg_stroke = egui::Stroke::new(1.0, BADGE_BORDER);
                                widget.fg_stroke = egui::Stroke::new(1.0, BADGE_TEXT);
                                widget.corner_radius = egui::CornerRadius::same(4);
                            }
                            ui.add_sized(
                                BADGE_SIZE,
                                egui::DragValue::new(&mut self.settings.volume)
                                    .range(VOLUME_MIN..=VOLUME_MAX)
                                    .speed(0.25),
                            )
                            .on_hover_text("type a number, 0 to 99")
                        })
                        .inner;
                    badge_focused = badge.has_focus();

                    if self.playing {
                        self.params.set_gain(gain_for(self.settings.volume));
                    }

                    // --- 4. on / off --------------------------------------------
                    let play_label = if self.playing { "ON" } else { "OFF" };
                    let play = toggle_button(ui, play_label, self.playing, PLAY_BUTTON_SIZE)
                        .on_hover_text("push button, makes noise");
                    if play.clicked() {
                        if self.playing {
                            self.stop();
                        } else {
                            self.start();
                        }
                    }

                    // --- 5. help ------------------------------------------------
                    let help = toggle_button(ui, "?", self.help_open, HELP_BUTTON_SIZE)
                        .on_hover_text("what is all this then");
                    if help.clicked() {
                        self.toggle_help(&ctx);
                    }
                });

                if let Some(error) = &self.error {
                    ui.label(egui::RichText::new(error).size(10.0).color(ERROR_TEXT));
                }

                if self.help_open {
                    ui.add_space(2.0);
                    help_panel(ui, badge_focused);
                }
            });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }
}

/// Scroll step per Up/Down press, in points.
const HELP_KEY_SCROLL: f32 = 24.0;
const HELP_PAGE_SCROLL: f32 = 120.0;

fn help_panel(ui: &mut egui::Ui, badge_focused: bool) {
    ui.separator();

    // Arrow keys scroll the help unless the number entry has focus and is using
    // them. Mouse wheel and dragging the scrollbar always work.
    let key_scroll = if badge_focused {
        0.0
    } else {
        ui.input(|i| {
            let steps = |down: egui::Key, up: egui::Key| {
                i.num_presses(down) as f32 - i.num_presses(up) as f32
            };
            steps(egui::Key::ArrowDown, egui::Key::ArrowUp) * HELP_KEY_SCROLL
                + steps(egui::Key::PageDown, egui::Key::PageUp) * HELP_PAGE_SCROLL
        })
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if key_scroll != 0.0 {
                // Negative delta scrolls the content up, i.e. moves the view down.
                ui.scroll_with_delta(egui::vec2(0.0, -key_scroll));
            }
            help_contents(ui);
        });
}

fn help_contents(ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new("Tasty brown noise.")
            .size(13.0)
            .strong()
            .color(LIT_TEXT),
    );
    ui.label(
        egui::RichText::new(
            "Pairs well with noise-cancelling headphones. Push buttons, makes \
             noise, either up or down. Minimal system usage — your fans will \
             never find out.",
        )
        .size(11.0),
    );

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("KNOBS AND LEVERS")
            .size(10.0)
            .strong()
            .color(ACCENT),
    );
    ui.add_space(2.0);

    egui::Grid::new("help-keys")
        .num_columns(2)
        .spacing(egui::vec2(12.0, 3.0))
        .show(ui, |ui| {
            for (keys, what) in [
                ("Tab", "walk the controls, left to right"),
                ("Space / Enter", "flip whichever button is focused"),
                ("← →", "nudge the slider by one"),
                ("Scroll wheel", "the same, but lazier"),
                ("↑ ↓", "nudge the number box by one"),
                ("Click the number", "type an exact 0–99"),
                ("mono", "both ears get the same rumble"),
                ("stereo", "each ear gets its own private rumble"),
            ] {
                ui.label(
                    egui::RichText::new(keys)
                        .size(11.0)
                        .strong()
                        .color(BADGE_TEXT),
                );
                ui.label(egui::RichText::new(what).size(11.0));
                ui.end_row();
            }
        });

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Not a medical device. Will not cancel your coworkers.")
            .size(10.0)
            .italics()
            .color(BTN_TEXT),
    );
}
