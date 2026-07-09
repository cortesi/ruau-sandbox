use egui::{
    Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Stroke, Style, TextStyle,
    Visuals,
};

const FONT_INTER: &[u8] = include_bytes!("../assets/fonts/Inter.ttf");
const FONT_JETBRAINS_MONO: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

const UI_FONT: &str = "inter";
const MONO_FONT: &str = "jetbrains-mono";

pub const BACKGROUND: Color32 = Color32::from_rgb(0x10, 0x12, 0x18);
pub const PANEL: Color32 = Color32::from_rgb(0x18, 0x1d, 0x24);
pub const HEADER: Color32 = Color32::from_rgb(0x20, 0x25, 0x2e);
pub const EDITOR_BG: Color32 = Color32::from_rgb(0x0c, 0x0e, 0x14);
pub const GUTTER: Color32 = Color32::from_rgb(0x51, 0x5c, 0x6e);
pub const TEXT: Color32 = Color32::from_rgb(0xea, 0xed, 0xf3);
pub const MUTED: Color32 = Color32::from_rgb(0xa8, 0xb1, 0xc2);
pub const ACCENT: Color32 = Color32::from_rgb(0x67, 0xd8, 0xef);
pub const DIVIDER: Color32 = Color32::from_rgb(0x3a, 0x44, 0x52);
pub const GOOD: Color32 = Color32::from_rgb(0x6d, 0xdb, 0x8b);
pub const WARN: Color32 = Color32::from_rgb(0xf2, 0xc4, 0x66);
pub const BAD: Color32 = Color32::from_rgb(0xff, 0x7a, 0x7a);
pub const KEYWORD: Color32 = Color32::from_rgb(0xc7, 0x92, 0xea);
pub const NUMBER: Color32 = Color32::from_rgb(0xf7, 0xc8, 0x73);
pub const STRING: Color32 = Color32::from_rgb(0x9c, 0xd6, 0x7a);
pub const COMMENT: Color32 = Color32::from_rgb(0x7c, 0x88, 0x99);
pub const MACRO: Color32 = Color32::from_rgb(0x82, 0xaa, 0xff);

pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        UI_FONT.to_string(),
        FontData::from_static(FONT_INTER).into(),
    );
    fonts.font_data.insert(
        MONO_FONT.to_string(),
        FontData::from_static(FONT_JETBRAINS_MONO).into(),
    );

    prepend_family(&mut fonts, FontFamily::Proportional, UI_FONT);
    prepend_family(&mut fonts, FontFamily::Monospace, MONO_FONT);
    ctx.set_fonts(fonts);
}

fn prepend_family(fonts: &mut FontDefinitions, family: FontFamily, font: &str) {
    let stack = fonts.families.entry(family).or_default();
    stack.retain(|name| name != font);
    stack.insert(0, font.to_string());
}

pub fn apply(ctx: &Context) {
    let mut style = Style {
        visuals: Visuals::dark(),
        ..(*ctx.global_style()).clone()
    };
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(19.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
    style.visuals.panel_fill = BACKGROUND;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = EDITOR_BG;
    style.visuals.widgets.noninteractive.bg_fill = PANEL;
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(0x20, 0x25, 0x2e);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x29, 0x31, 0x3d);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(0x30, 0x3a, 0x49);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    ctx.set_global_style(style);
}
