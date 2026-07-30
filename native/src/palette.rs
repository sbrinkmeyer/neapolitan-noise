//! One color scheme per flavor. Vanilla and strawberry are light schemes with
//! dark text, so the whole palette swaps rather than just an accent.

use eframe::egui::Color32;

use crate::noise::Flavor;

pub struct Palette {
    pub bg: Color32,
    pub text: Color32,
    pub btn_bg: Color32,
    pub btn_border: Color32,
    pub btn_text: Color32,
    pub btn_hover: Color32,
    pub btn_pressed: Color32,
    pub lit_bg: Color32,
    pub lit_border: Color32,
    pub lit_text: Color32,
    pub trough: Color32,
    pub accent: Color32,
    pub badge_bg: Color32,
    pub badge_border: Color32,
    pub badge_text: Color32,
    pub error: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

/// The original scheme, inherited from the Electron app back when this was
/// called Brownie and only made brown noise.
pub const CHOCOLATE: Palette = Palette {
    bg: rgb(0x1a1008),
    text: rgb(0xd4a96a),
    btn_bg: rgb(0x372012),
    btn_border: rgb(0x6b3a1f),
    btn_text: rgb(0xc6985d),
    btn_hover: rgb(0x7d4525),
    btn_pressed: rgb(0x5a2f18),
    lit_bg: rgb(0x8a4d27),
    lit_border: rgb(0xa0622a),
    lit_text: rgb(0xffe1ba),
    trough: rgb(0x26170c),
    accent: rgb(0xa0622a),
    badge_bg: rgb(0x120b05),
    badge_border: rgb(0x7b4d2a),
    badge_text: rgb(0xf0d0a0),
    error: rgb(0xff8a6a),
};

pub const VANILLA: Palette = Palette {
    bg: rgb(0xf5ecd7),
    text: rgb(0x4a3b22),
    btn_bg: rgb(0xe8dcc0),
    btn_border: rgb(0xc9b995),
    btn_text: rgb(0x4a3b22),
    btn_hover: rgb(0xdccfae),
    btn_pressed: rgb(0xcdbf9c),
    lit_bg: rgb(0xc9a961),
    lit_border: rgb(0x8f7433),
    lit_text: rgb(0x2e2410),
    trough: rgb(0xe3d7ba),
    accent: rgb(0xb08f45),
    badge_bg: rgb(0xfffaf0),
    badge_border: rgb(0xc9b995),
    badge_text: rgb(0x3a2e18),
    error: rgb(0x9c2f1e),
};

pub const STRAWBERRY: Palette = Palette {
    bg: rgb(0xf2d6da),
    text: rgb(0x6b2f3a),
    btn_bg: rgb(0xe8bfc6),
    btn_border: rgb(0xcf98a2),
    btn_text: rgb(0x6b2f3a),
    btn_hover: rgb(0xdfb0b8),
    btn_pressed: rgb(0xcfa0a8),
    lit_bg: rgb(0xd4737f),
    lit_border: rgb(0xa04b58),
    // Dark, not white: white on mid-pink only reaches 2.9:1. Matches vanilla,
    // where the lit text is also dark.
    lit_text: rgb(0x3a0f18),
    trough: rgb(0xe6c6cc),
    accent: rgb(0xb85d69),
    badge_bg: rgb(0xfdf0f2),
    badge_border: rgb(0xcf98a2),
    badge_text: rgb(0x5a2530),
    error: rgb(0x8f2233),
};

pub fn for_flavor(flavor: Flavor) -> &'static Palette {
    match flavor {
        Flavor::Vanilla => &VANILLA,
        Flavor::Chocolate => &CHOCOLATE,
        Flavor::Strawberry => &STRAWBERRY,
    }
}

/// The color of a flavor's band in the picker.
///
/// Three fixed colors, never tinted or dimmed by the active scheme. Whichever
/// band matches the current background melts into it, which is the point; dimming
/// the other two to show selection turned all three into the same mud on the
/// chocolate scheme, so selection is shown by shape instead (see `flavor_picker`).
pub fn band_color(flavor: Flavor) -> Color32 {
    match flavor {
        Flavor::Chocolate => rgb(0x7a4a26),
        Flavor::Vanilla => rgb(0xf7ecc9),
        Flavor::Strawberry => rgb(0xf2a5b2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance.
    fn luminance(c: Color32) -> f32 {
        let channel = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.039_28 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        let (hi, lo) = if x > y { (x, y) } else { (y, x) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn body_text_is_legible_on_every_flavor() {
        for flavor in Flavor::ALL {
            let p = for_flavor(flavor);
            let ratio = contrast(p.text, p.bg);
            assert!(
                ratio >= 4.5,
                "{} body text contrast is {ratio:.2}, want >= 4.5",
                flavor.label()
            );
        }
    }

    #[test]
    fn button_and_badge_text_are_legible_on_every_flavor() {
        for flavor in Flavor::ALL {
            let p = for_flavor(flavor);
            for (name, fg, bg) in [
                ("button", p.btn_text, p.btn_bg),
                ("lit button", p.lit_text, p.lit_bg),
                ("badge", p.badge_text, p.badge_bg),
            ] {
                let ratio = contrast(fg, bg);
                assert!(
                    ratio >= 4.5,
                    "{} {name} text contrast is {ratio:.2}, want >= 4.5",
                    flavor.label()
                );
            }
        }
    }
}
