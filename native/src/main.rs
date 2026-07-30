// No console window on Windows for release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod noise;
mod palette;

use eframe::egui;

fn main() -> eframe::Result {
    // Pin the size hard. Opening the help panel moves all three clamps together
    // (see `Brownie::toggle_help`), so the window is exactly one of two sizes and
    // never inherits one from the window manager or a restored session.
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Brownie")
        .with_app_id("brownie")
        .with_inner_size(app::COLLAPSED_SIZE)
        .with_min_inner_size(app::COLLAPSED_SIZE)
        .with_max_inner_size(app::COLLAPSED_SIZE)
        .with_resizable(false)
        .with_maximize_button(false);

    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        // Off deliberately. eframe's restore path overwrites the builder's
        // inner_size, and egui-winit's `clamp_size_to_sane_values` forces a 64pt
        // floor — a 46pt-tall window is not "sane" to it, so a restored session
        // silently reopened at winit's 800x600 default. The app asserts its own
        // size on the first frame instead. Volume and mono/stereo still persist.
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "brownie",
        options,
        Box::new(|cc| Ok(Box::new(app::Brownie::new(cc)))),
    )
}
