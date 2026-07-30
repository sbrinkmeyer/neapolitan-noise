//! UI: one row — flavor picker, mode toggle, volume slider, volume entry,
//! playback toggle, help. Mirrors the layout of the Electron build's
//! `index.html`, with the palette now driven by the selected flavor.
//!
//! Keyboard: Tab walks the controls left to right. Space/Enter activate the
//! focused button, Up/Down move through the flavor stack or step the number
//! entry, and Left/Right move a focused slider.

use std::sync::Arc;

use eframe::egui;

use crate::audio::{self, Params};
use crate::noise::Flavor;
use crate::palette::{self, Palette};

/// Scroll distance that steps the volume by one, so a wheel notch moves 1 the
/// way it did in the browser build without trackpad inertia running away.
const SCROLL_PER_STEP: f32 = 20.0;

const VOLUME_MIN: u32 = 0;
const VOLUME_MAX: u32 = 99;

/// Top-to-bottom order of the picker, like the bands in Neapolitan ice cream.
const FLAVOR_STACK: [Flavor; 3] = [Flavor::Chocolate, Flavor::Vanilla, Flavor::Strawberry];

const PLAY_BUTTON_SIZE: egui::Vec2 = egui::vec2(52.0, 26.0);
const MODE_BUTTON_SIZE: egui::Vec2 = egui::vec2(62.0, 26.0);
const HELP_BUTTON_SIZE: egui::Vec2 = egui::vec2(26.0, 26.0);
const BADGE_SIZE: egui::Vec2 = egui::vec2(42.0, 26.0);
const PICKER_SIZE: egui::Vec2 = egui::vec2(22.0, 26.0);
/// How far the unselected scoops pull back from the edges. This is the selection
/// cue, chosen over dimming so the three band colors stay static.
const UNSELECTED_INSET: f32 = 4.0;

/// Window size with the help panel closed and open. The help panel lives in the
/// same window rather than a second one, so the app stays a single tiny thing.
pub const COLLAPSED_SIZE: egui::Vec2 = egui::vec2(436.0, 46.0);
pub const EXPANDED_SIZE: egui::Vec2 = egui::vec2(436.0, 320.0);

/// Scroll step per key press in the help panel, in points.
const HELP_KEY_SCROLL: f32 = 24.0;
const HELP_PAGE_SCROLL: f32 = 120.0;

// Help copy lives in consts so `help_copy_uses_available_glyphs` can guard it.
// egui bundles its own fonts and never touches OS fonts, so the only glyphs that
// exist are the ones in those four files — see SAFE_NON_ASCII.
const HELP_TITLE: &str = "Neapolitan noise.";
const HELP_BLURB: &str = "Three scoops. Pairs well with noise-cancelling headphones. Push \
                          buttons, makes noise, either up or down. Minimal system usage — \
                          your fans will never find out.";
const HELP_FOOTER: &str = "Not a medical device. Will not cancel your coworkers.";

const FLAVOR_BLURBS: [(Flavor, &str); 3] = [
    (Flavor::Chocolate, "the original. deep, rumbly, distant surf"),
    (
        Flavor::Vanilla,
        "bright and hissy. all frequencies, no favorites",
    ),
    (Flavor::Strawberry, "halfway there. rain on a tent"),
];

/// Which arrows to draw before a key's label. Drawn as triangles rather than set
/// as text: none of the bundled fonts actually render U+2190..93, and egui never
/// falls back to an OS font, so an arrow character is a guaranteed tofu box.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arrows {
    None,
    UpDown,
    LeftRight,
}

const KEY_HELP: [(Arrows, &str, &str); 9] = [
    (Arrows::None, "Tab", "walk the controls, left to right"),
    (Arrows::UpDown, "on the scoops", "change flavor"),
    (
        Arrows::None,
        "Space / Enter",
        "flip whichever button is focused",
    ),
    (Arrows::LeftRight, "on the slider", "nudge the volume by one"),
    (Arrows::None, "Scroll wheel", "the same, but lazier"),
    (Arrows::UpDown, "on the number", "nudge the volume by one"),
    (Arrows::None, "Click the number", "type an exact 0–99"),
    (Arrows::None, "mono", "both ears get the same rumble"),
    (
        Arrows::None,
        "stereo",
        "each ear gets its own private rumble",
    ),
];

/// Non-ASCII codepoints verified to render in the bundled fonts. Only the dashes
/// from Ubuntu-Light qualify; arrows are painted, not typed.
#[cfg(test)]
const SAFE_NON_ASCII: &[char] = &['–', '—'];

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub volume: u32,
    pub stereo: bool,
    /// `serde(default)` so state written before flavors existed still loads
    /// instead of silently resetting volume and mono/stereo.
    #[serde(default)]
    pub flavor: Flavor,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 30,
            stereo: false,
            flavor: Flavor::default(),
        }
    }
}

pub struct NeapoNoise {
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
    /// Flavor the current egui style was built for, so the theme is only rebuilt
    /// when it actually changes.
    themed: Option<Flavor>,
    scroll_accum: f32,
    error: Option<String>,
}

impl NeapoNoise {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let stored: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        let settings = Settings {
            volume: stored.volume.clamp(VOLUME_MIN, VOLUME_MAX),
            stereo: stored.stereo,
            flavor: stored.flavor,
        };

        

        Self {
            params: Arc::new(Params::new(
                gain_for(settings.volume),
                settings.stereo,
                settings.flavor,
            )),
            stream: None,
            settings,
            playing: false,
            stopping: false,
            help_open: false,
            sized: false,
            themed: None,
            scroll_accum: 0.0,
            error: None,
        }
    }

    fn palette(&self) -> &'static Palette {
        palette::for_flavor(self.settings.flavor)
    }

    fn start(&mut self) {
        self.params.set_gain(gain_for(self.settings.volume));
        self.params.set_stereo(self.settings.stereo);
        self.params.set_flavor(self.settings.flavor);
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

    fn set_flavor(&mut self, flavor: Flavor) {
        if self.settings.flavor == flavor {
            return;
        }
        self.settings.flavor = flavor;
        self.params.set_flavor(flavor);
    }

    /// Move `steps` through the stack, top to bottom. Clamps rather than wraps,
    /// so holding Down does not cycle back to chocolate.
    fn step_flavor(&mut self, steps: i32) {
        if steps == 0 {
            return;
        }
        let current = FLAVOR_STACK
            .iter()
            .position(|f| *f == self.settings.flavor)
            .unwrap_or(0) as i32;
        let next = (current + steps).clamp(0, FLAVOR_STACK.len() as i32 - 1) as usize;
        self.set_flavor(FLAVOR_STACK[next]);
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

/// Draw a small solid triangle pointing `direction`.
///
/// The help panel needs arrow symbols and no bundled font provides them, so they
/// are painted. Crisp at any DPI, identical on every platform, no font involved.
fn arrow_glyph(ui: &mut egui::Ui, direction: egui::Direction, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(9.0, 11.0), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let c = rect.center();
    // Half-extents: across the point, and from base to tip.
    let (across, depth) = (4.0, 3.5);
    let points = match direction {
        egui::Direction::TopDown => vec![
            egui::pos2(c.x, c.y + depth),
            egui::pos2(c.x - across, c.y - depth),
            egui::pos2(c.x + across, c.y - depth),
        ],
        egui::Direction::BottomUp => vec![
            egui::pos2(c.x, c.y - depth),
            egui::pos2(c.x - across, c.y + depth),
            egui::pos2(c.x + across, c.y + depth),
        ],
        egui::Direction::RightToLeft => vec![
            egui::pos2(c.x - depth, c.y),
            egui::pos2(c.x + depth, c.y - across),
            egui::pos2(c.x + depth, c.y + across),
        ],
        egui::Direction::LeftToRight => vec![
            egui::pos2(c.x + depth, c.y),
            egui::pos2(c.x - depth, c.y - across),
            egui::pos2(c.x - depth, c.y + across),
        ],
    };

    ui.painter()
        .add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
}

fn apply_theme(ctx: &egui::Context, p: &Palette) {
    // Pin the theme: each flavor is a single deliberate scheme, so egui must not
    // also apply its own light/dark choice on top.
    ctx.set_theme(egui::ThemePreference::Dark);

    ctx.all_styles_mut(|style| {
        let visuals = &mut style.visuals;

        visuals.dark_mode = true;
        visuals.panel_fill = p.bg;
        visuals.window_fill = p.bg;
        visuals.extreme_bg_color = p.trough;
        visuals.override_text_color = Some(p.text);
        visuals.selection.bg_fill = p.accent;
        visuals.selection.stroke = egui::Stroke::new(1.0, p.lit_text);

        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(6);
            widget.bg_fill = p.trough;
            widget.weak_bg_fill = p.btn_bg;
            widget.bg_stroke = egui::Stroke::new(1.0, p.btn_border);
            widget.fg_stroke = egui::Stroke::new(1.0, p.btn_text);
        }
        visuals.widgets.hovered.weak_bg_fill = p.btn_hover;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, p.lit_border);
        visuals.widgets.active.weak_bg_fill = p.btn_pressed;
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, p.lit_border);
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, p.text);

        visuals.widgets.hovered.expansion = 0.0;
        visuals.widgets.active.expansion = 0.0;

        style.spacing.item_spacing = egui::vec2(10.0, 6.0);
        style.spacing.slider_width = 128.0;
        style.spacing.interact_size = egui::vec2(28.0, 26.0);
    });
}

fn toggle_button(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    lit: bool,
    size: egui::Vec2,
) -> egui::Response {
    let mut button = egui::Button::new(egui::RichText::new(label).size(12.0).strong())
        .min_size(size)
        .corner_radius(6);
    if lit {
        button = button
            .fill(p.lit_bg)
            .stroke(egui::Stroke::new(1.0, p.lit_border));
    }
    ui.add(button)
}

/// The Neapolitan stack: three bands, chocolate on top, vanilla in the middle,
/// strawberry on the bottom.
///
/// One widget and therefore one Tab stop. Click a band to pick it, or focus the
/// stack and use Up/Down.
fn flavor_picker(ui: &mut egui::Ui, p: &Palette, selected: Flavor) -> (egui::Response, i32) {
    let (rect, mut response) = ui.allocate_exact_size(PICKER_SIZE, egui::Sense::click());

    let mut steps = 0;
    if response.has_focus() {
        steps = ui.input_mut(|i| {
            i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as i32
                - i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as i32
        });
    }

    let band_height = rect.height() / FLAVOR_STACK.len() as f32;
    let mut clicked_flavor = None;

    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let index = ((pos.y - rect.top()) / band_height).floor() as usize;
            clicked_flavor = FLAVOR_STACK.get(index.min(FLAVOR_STACK.len() - 1)).copied();
        }
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Selection is shown by width, not brightness: the chosen scoop spans the
        // full stack while the others are inset. Colors stay exactly as mixed, so
        // the active flavor's band melts into its background instead of muddying.
        for (index, flavor) in FLAVOR_STACK.iter().enumerate() {
            let inset = if *flavor == selected { 0.0 } else { UNSELECTED_INSET };
            let band = egui::Rect::from_min_size(
                egui::pos2(rect.left() + inset, rect.top() + band_height * index as f32),
                egui::vec2(rect.width() - inset * 2.0, band_height),
            );
            painter.rect_filled(band, 0.0, palette::band_color(*flavor));
        }

        // Frame in the scheme's own border color, so the stack sits in the window
        // rather than on top of it. Brighter when focused, to show the Tab stop.
        let stroke = if response.has_focus() {
            egui::Stroke::new(2.0, p.lit_border)
        } else {
            egui::Stroke::new(1.0, p.btn_border)
        };
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(3),
            stroke,
            egui::StrokeKind::Inside,
        );
    }

    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::RadioButton,
            true,
            true,
            format!("flavor: {} ({})", selected.label(), selected.noise_name()),
        )
    });

    if let Some(flavor) = clicked_flavor {
        let current = FLAVOR_STACK.iter().position(|f| *f == selected).unwrap_or(0) as i32;
        let target = FLAVOR_STACK.iter().position(|f| *f == flavor).unwrap_or(0) as i32;
        steps = target - current;
        response.mark_changed();
    }

    (response, steps)
}

impl eframe::App for NeapoNoise {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.themed != Some(self.settings.flavor) {
            self.themed = Some(self.settings.flavor);
            apply_theme(ctx, self.palette());
        }

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
        let p = self.palette();
        // The flavor picker and number entry both own Up/Down while focused, so
        // the help panel must not also grab those keys for scrolling.
        let mut arrows_claimed = false;

        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // --- 1. flavor ----------------------------------------------
                    let (picker, flavor_steps) = flavor_picker(ui, p, self.settings.flavor);
                    arrows_claimed |= picker.has_focus();
                    picker.on_hover_text(format!(
                        "{} — {}\nclick a scoop, or Up/Down when focused",
                        self.settings.flavor.label(),
                        self.settings.flavor.noise_name()
                    ));
                    self.step_flavor(flavor_steps);

                    // --- 2. mono / stereo ---------------------------------------
                    let mode_label = if self.settings.stereo {
                        "stereo"
                    } else {
                        "mono"
                    };
                    let mode =
                        toggle_button(ui, p, mode_label, self.settings.stereo, MODE_BUTTON_SIZE)
                            .on_hover_text(if self.settings.stereo {
                                "stereo: each ear gets its own private rumble"
                            } else {
                                "mono: both ears, same rumble"
                            });
                    if mode.clicked() {
                        self.settings.stereo = !self.settings.stereo;
                        self.params.set_stereo(self.settings.stereo);
                    }

                    // --- 3. volume slider ---------------------------------------
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

                    // --- 4. volume entry ----------------------------------------
                    // Own tab stop: click or focus it and type a number.
                    let badge = ui
                        .scope(|ui| {
                            let widgets = &mut ui.visuals_mut().widgets;
                            for widget in [
                                &mut widgets.inactive,
                                &mut widgets.hovered,
                                &mut widgets.active,
                            ] {
                                widget.weak_bg_fill = p.badge_bg;
                                widget.bg_stroke = egui::Stroke::new(1.0, p.badge_border);
                                widget.fg_stroke = egui::Stroke::new(1.0, p.badge_text);
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
                    arrows_claimed |= badge.has_focus();

                    if self.playing {
                        self.params.set_gain(gain_for(self.settings.volume));
                    }

                    // --- 5. on / off --------------------------------------------
                    let play_label = if self.playing { "ON" } else { "OFF" };
                    let play = toggle_button(ui, p, play_label, self.playing, PLAY_BUTTON_SIZE)
                        .on_hover_text("push button, makes noise");
                    if play.clicked() {
                        if self.playing {
                            self.stop();
                        } else {
                            self.start();
                        }
                    }

                    // --- 6. help ------------------------------------------------
                    let help = toggle_button(ui, p, "?", self.help_open, HELP_BUTTON_SIZE)
                        .on_hover_text("what is all this then");
                    if help.clicked() {
                        self.toggle_help(&ctx);
                    }
                });

                if let Some(error) = &self.error {
                    ui.label(egui::RichText::new(error).size(10.0).color(p.error));
                }

                if self.help_open {
                    ui.add_space(2.0);
                    help_panel(ui, p, arrows_claimed);
                }
            });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette().bg.to_normalized_gamma_f32()
    }
}

fn help_panel(ui: &mut egui::Ui, p: &Palette, arrows_claimed: bool) {
    ui.separator();

    // Arrow keys scroll the help unless a focused control is already using them.
    // Mouse wheel and dragging the scrollbar always work.
    let key_scroll = if arrows_claimed {
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
            help_contents(ui, p);
        });
}

fn help_contents(ui: &mut egui::Ui, p: &Palette) {
    ui.label(
        egui::RichText::new(HELP_TITLE)
            .size(13.0)
            .strong()
            .color(p.accent),
    );
    ui.label(egui::RichText::new(HELP_BLURB).size(11.0));

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("THE SCOOPS")
            .size(10.0)
            .strong()
            .color(p.accent),
    );
    ui.add_space(2.0);

    egui::Grid::new("help-flavors")
        .num_columns(2)
        .spacing(egui::vec2(12.0, 3.0))
        .show(ui, |ui| {
            for (flavor, blurb) in FLAVOR_BLURBS {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::same(2),
                        palette::band_color(flavor),
                    );
                    ui.label(
                        egui::RichText::new(flavor.label())
                            .size(11.0)
                            .strong()
                            .color(p.badge_text),
                    );
                });
                ui.label(egui::RichText::new(blurb).size(11.0));
                ui.end_row();
            }
        });

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("KNOBS AND LEVERS")
            .size(10.0)
            .strong()
            .color(p.accent),
    );
    ui.add_space(2.0);

    egui::Grid::new("help-keys")
        .num_columns(2)
        .spacing(egui::vec2(12.0, 3.0))
        .show(ui, |ui| {
            for (arrows, keys, what) in KEY_HELP {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    match arrows {
                        Arrows::UpDown => {
                            arrow_glyph(ui, egui::Direction::BottomUp, p.badge_text);
                            arrow_glyph(ui, egui::Direction::TopDown, p.badge_text);
                        }
                        Arrows::LeftRight => {
                            arrow_glyph(ui, egui::Direction::RightToLeft, p.badge_text);
                            arrow_glyph(ui, egui::Direction::LeftToRight, p.badge_text);
                        }
                        Arrows::None => {}
                    }
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(keys)
                            .size(11.0)
                            .strong()
                            .color(p.badge_text),
                    );
                });
                ui.label(egui::RichText::new(what).size(11.0));
                ui.end_row();
            }
        });

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(HELP_FOOTER)
            .size(10.0)
            .italics()
            .color(p.btn_text),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// egui bundles its fonts and never falls back to the OS, so any codepoint
    /// outside what those four files cover renders as an empty box. Keep help copy
    /// to ASCII plus the handful of glyphs verified present.
    #[test]
    fn help_copy_uses_available_glyphs() {
        let mut copy: Vec<&str> = vec![HELP_TITLE, HELP_BLURB, HELP_FOOTER];
        copy.extend(FLAVOR_BLURBS.iter().map(|(_, blurb)| *blurb));
        for (_, keys, what) in KEY_HELP {
            copy.push(keys);
            copy.push(what);
        }

        for text in copy {
            for ch in text.chars() {
                assert!(
                    ch.is_ascii() || SAFE_NON_ASCII.contains(&ch),
                    "{ch:?} in {text:?} is not in a bundled font and will render \
                     as a box; add it to SAFE_NON_ASCII only after verifying coverage"
                );
            }
        }
    }

    #[test]
    fn flavor_stack_is_the_ice_cream_order() {
        assert_eq!(
            FLAVOR_STACK,
            [Flavor::Chocolate, Flavor::Vanilla, Flavor::Strawberry],
            "chocolate on top, vanilla in the middle, strawberry on the bottom"
        );
    }

    #[test]
    fn every_flavor_appears_once_in_the_stack_and_the_help() {
        for flavor in Flavor::ALL {
            assert_eq!(
                FLAVOR_STACK.iter().filter(|f| **f == flavor).count(),
                1,
                "{} missing from or duplicated in FLAVOR_STACK",
                flavor.label()
            );
            assert_eq!(
                FLAVOR_BLURBS.iter().filter(|(f, _)| *f == flavor).count(),
                1,
                "{} missing from or duplicated in FLAVOR_BLURBS",
                flavor.label()
            );
        }
    }
}
