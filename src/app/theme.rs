//! CopyWheel's drawing colors and egui widget styles.

use crate::store::Theme;
use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub(super) const TRANSPARENT: Color32 = Color32::TRANSPARENT;

const ACCENT_LIGHT: Color32 = hex(0x0FA89A);
const ACCENT_DARK: Color32 = ACCENT_GRADIENT.0;
// Premultiplied for egui's gamma-space blending.
const ACCENT_SOFT_LIGHT: Color32 = Color32::from_rgba_premultiplied(2, 20, 18, 30);
const ACCENT_SOFT_DARK: Color32 = Color32::from_rgba_premultiplied(5, 35, 32, 46);
const ON_ACCENT: Color32 = hex(0x06201D);
const ACCENT_GRADIENT: (Color32, Color32) = (hex(0x19C2B0), hex(0x0E8F84));

pub(super) struct Palette {
    pub dark_mode: bool,
    pub bg: Color32,
    pub bg2: Color32,
    pub line: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub hover: Color32,
    pub keycap_bg: Color32,
    pub sector_fill: Color32,
    pub sector_line: Color32,
    pub sector_track: Color32,
    pub wheel_disc: Color32,
    pub sector_selected_text: Color32,
    pub sector_selected_opacity: f32,
    pub list_bg: Color32,
    pub list_line: Color32,
    pub row_bg: Color32,
    pub row_line: Color32,
    pub row_hover: Color32,
    pub command_bg: Color32,
    pub command_text: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub danger: Color32,
    pub wheel_action_hover: Color32,
    pub segment_shadow: Color32,
    pub popup_shadow: Color32,
    pub card_shadow: Color32,
    pub badge_shadow: Color32,
    widget_hover: Color32,
    widget_line: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

const LIGHT: Palette = Palette {
    dark_mode: false,
    bg: hex(0xF5F6F8),
    bg2: Color32::WHITE,
    line: hex(0xDFE2E9),
    text: hex(0x14151A),
    dim: hex(0x707582),
    accent: ACCENT_LIGHT,
    accent_soft: ACCENT_SOFT_LIGHT,
    on_accent: ON_ACCENT,
    hover: hex(0xECEEF2),
    keycap_bg: hex(0xECEEF2),
    sector_fill: hex(0xFFFFFF),
    sector_line: hex(0xD9DDE3),
    sector_track: hex(0xECEEF2),
    wheel_disc: hex(0xF5F6F8),
    sector_selected_text: hex(0x0B7A70),
    sector_selected_opacity: 0.10,
    list_bg: hex(0xF3EFEA),
    list_line: hex(0xDED7D0),
    row_bg: hex(0xFFFFFF),
    row_line: hex(0xE2DBD3),
    row_hover: hex(0xFBF9F6),
    command_bg: hex(0x2A2E36),
    command_text: hex(0xF2F4F8),
    success: Color32::from_rgb(36, 148, 82),
    warning: hex(0x796300),
    error: Color32::from_rgb(215, 58, 50),
    danger: Color32::from_rgb(220, 62, 54),
    wheel_action_hover: hex(0x076D64),
    segment_shadow: Color32::from_black_alpha(20),
    popup_shadow: Color32::from_black_alpha(32),
    card_shadow: Color32::from_black_alpha(38),
    badge_shadow: Color32::BLACK,
    widget_hover: Color32::from_rgb(225, 228, 235),
    widget_line: hex(0xDFE2E9),
};

const DARK_SHADOW: Color32 = Color32::from_black_alpha(90);
const DARK: Palette = Palette {
    dark_mode: true,
    bg: hex(0x1C1D21),
    bg2: hex(0x26272C),
    line: hex(0x34363D),
    text: hex(0xECEDF0),
    dim: hex(0x9A9EAA),
    accent: ACCENT_DARK,
    accent_soft: ACCENT_SOFT_DARK,
    on_accent: ON_ACCENT,
    hover: hex(0x2C2E34),
    keycap_bg: hex(0x2C2E34),
    sector_fill: hex(0x1C1F26),
    sector_line: hex(0x3A3F4A),
    sector_track: hex(0x2A2E36),
    wheel_disc: hex(0x1A1D23),
    sector_selected_text: hex(0x9FE9E0),
    sector_selected_opacity: 0.14,
    list_bg: hex(0x1C1D21),
    list_line: hex(0x34363D),
    row_bg: hex(0x2B2D33),
    row_line: hex(0x3C3F47),
    row_hover: hex(0x32353C),
    command_bg: hex(0x121316),
    command_text: hex(0xECEDF0),
    success: hex(0x65C78E),
    warning: hex(0xE4CB5D),
    error: hex(0xFF827B),
    danger: hex(0xBD3B35),
    wheel_action_hover: ACCENT_DARK,
    segment_shadow: DARK_SHADOW,
    popup_shadow: DARK_SHADOW,
    card_shadow: DARK_SHADOW,
    badge_shadow: DARK_SHADOW,
    widget_hover: hex(0x2C2E34),
    widget_line: hex(0x474A52),
};

pub(super) fn palette(ctx: &egui::Context) -> &'static Palette {
    match ctx.theme() {
        egui::Theme::Light => &LIGHT,
        egui::Theme::Dark => &DARK,
    }
}

pub(super) fn apply(ctx: &egui::Context, theme: Theme) {
    ctx.set_theme(match theme {
        Theme::System => egui::ThemePreference::System,
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
    });
}

impl Palette {
    pub(super) fn wheel_selection(&self, opacity: f32) -> Color32 {
        self.accent_soft.gamma_multiply(opacity)
    }

    fn visuals(&self) -> egui::Visuals {
        // Preserve egui's existing light widget states.
        let mut visuals = if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = self.bg;
        visuals.window_fill = self.bg2;
        visuals.window_stroke = Stroke::new(1.0_f32, self.line);
        visuals.window_corner_radius = CornerRadius::same(14);
        visuals.extreme_bg_color = self.bg2;
        visuals.widgets.inactive.bg_fill = self.hover;
        visuals.widgets.inactive.weak_bg_fill = self.hover;
        visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
        visuals.widgets.hovered.weak_bg_fill = self.widget_hover;
        visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
        visuals.widgets.active.corner_radius = CornerRadius::same(8);
        visuals.selection.bg_fill = self.accent_soft;
        visuals.selection.stroke = Stroke::new(1.0_f32, self.accent);
        visuals.override_text_color = Some(self.text);

        if self.dark_mode {
            let widgets = &mut visuals.widgets;
            widgets.noninteractive.bg_fill = self.bg;
            widgets.noninteractive.weak_bg_fill = self.bg;
            widgets.noninteractive.bg_stroke.color = self.line;
            widgets.noninteractive.fg_stroke.color = self.dim;
            widgets.inactive.bg_stroke = Stroke::new(1.0_f32, self.widget_line);
            widgets.inactive.fg_stroke.color = self.dim;
            widgets.hovered.bg_fill = self.hover;
            widgets.hovered.bg_stroke.color = self.dim;
            widgets.hovered.fg_stroke.color = self.text;
            widgets.active.bg_fill = self.hover;
            widgets.active.weak_bg_fill = self.hover;
            widgets.active.bg_stroke.color = self.accent;
            widgets.active.fg_stroke.color = self.text;
            widgets.open.bg_fill = self.bg2;
            widgets.open.weak_bg_fill = self.hover;
            widgets.open.bg_stroke.color = self.line;
            widgets.open.fg_stroke.color = self.text;
            visuals.hyperlink_color = self.accent;
            visuals.faint_bg_color = self.hover;
            visuals.code_bg_color = self.command_bg;
            visuals.warn_fg_color = self.warning;
            visuals.error_fg_color = self.error;
            visuals.text_cursor.stroke.color = self.accent;
            visuals.window_shadow.color = DARK_SHADOW;
            visuals.popup_shadow.color = DARK_SHADOW;
        }
        visuals
    }
}

pub(super) fn setup_visuals(ctx: &egui::Context) {
    ctx.style_mut_of(egui::Theme::Light, |style| style.visuals = LIGHT.visuals());
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.visuals = DARK.visuals();
        style.spacing.scroll.foreground_color = true;
    });
    ctx.options_mut(|options| options.fallback_theme = egui::Theme::Light);
}

pub(super) fn with_alpha(c: Color32, a: f32) -> Color32 {
    let [r, g, b, a0] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (a0 as f32 * a.clamp(0.0, 1.0)) as u8)
}

pub(super) fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_selection_uses_the_shared_soft_accent() {
        for (colors, opacity) in [(&LIGHT, 30), (&DARK, 46)] {
            let expected = colors.accent.gamma_multiply_u8(opacity);
            assert_eq!(colors.accent_soft, expected);
            assert_eq!(colors.wheel_selection(1.0), expected);
        }
    }

    #[test]
    fn system_theme_updates_palette_and_visuals_between_frames() {
        let ctx = egui::Context::default();
        setup_visuals(&ctx);
        for (preference, system, dark) in [
            (Theme::System, egui::Theme::Light, false),
            (Theme::System, egui::Theme::Dark, true),
            (Theme::Light, egui::Theme::Dark, false),
            (Theme::Dark, egui::Theme::Light, true),
            (Theme::System, egui::Theme::Light, false),
        ] {
            let input = egui::RawInput {
                system_theme: Some(system),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                apply(ctx, preference);
                assert_eq!(palette(ctx).dark_mode, dark);
                let style = ctx.style();
                assert_eq!(style.visuals.dark_mode, dark);
                assert_eq!(style.visuals.panel_fill, palette(ctx).bg);
            });
        }
    }
}
