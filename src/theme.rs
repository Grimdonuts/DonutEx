use crate::syntax::TokenKind;
use eframe::egui::{self, Color32};

/// Dark+-ish syntax palette. `None` means "use the editor's default text
/// color" (used for TokenKind::Plain, i.e. identifiers that aren't a
/// keyword/type/call/macro).
pub fn token_color(kind: TokenKind) -> Option<Color32> {
    match kind {
        TokenKind::Plain => None,
        TokenKind::Keyword => Some(Color32::from_rgb(86, 156, 214)),
        TokenKind::Type => Some(Color32::from_rgb(78, 201, 176)),
        TokenKind::String => Some(Color32::from_rgb(206, 145, 120)),
        TokenKind::Number => Some(Color32::from_rgb(181, 206, 168)),
        TokenKind::Comment => Some(Color32::from_rgb(106, 153, 85)),
        TokenKind::Function => Some(Color32::from_rgb(220, 220, 170)),
        TokenKind::Macro => Some(Color32::from_rgb(197, 134, 192)),
        TokenKind::Variable => Some(Color32::from_rgb(156, 220, 254)),
        TokenKind::Parameter => Some(Color32::from_rgb(156, 220, 254)),
        TokenKind::Property => Some(Color32::from_rgb(156, 220, 254)),
        TokenKind::Namespace => Some(Color32::from_rgb(78, 201, 176)),
        TokenKind::EnumMember => Some(Color32::from_rgb(78, 201, 176)),
        TokenKind::Decorator => Some(Color32::from_rgb(220, 220, 170)),
    }
}

pub struct ScrollbarColors {
    pub track: Color32,
    pub thumb: Color32,
    pub thumb_hover: Color32,
    pub thumb_active: Color32,
}

/// Deliberately its own palette rather than reusing `visuals.widgets.*`:
/// those are tuned for buttons and, at the values this theme uses for the
/// rest of the UI, the inactive-widget color and the panel's faint-bg color
/// are identical - which made the scrollbar thumb invisible against its
/// track until it was actively being dragged.
pub fn scrollbar_colors() -> ScrollbarColors {
    ScrollbarColors {
        track: Color32::from_rgb(37, 37, 38),
        thumb: Color32::from_rgb(96, 96, 100),
        thumb_hover: Color32::from_rgb(122, 122, 128),
        thumb_active: Color32::from_rgb(28, 122, 191),
    }
}

/// A dark palette in the same neighborhood as VS Code's default theme.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = Color32::from_rgb(37, 37, 38); // sidebar / panels
    visuals.window_fill = Color32::from_rgb(37, 37, 38);
    visuals.extreme_bg_color = Color32::from_rgb(30, 30, 30); // editor background
    visuals.faint_bg_color = Color32::from_rgb(45, 45, 46);
    visuals.override_text_color = Some(Color32::from_rgb(212, 212, 212));

    visuals.selection.bg_fill = Color32::from_rgb(38, 79, 120);
    visuals.selection.stroke.color = Color32::from_rgb(90, 140, 200);

    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(37, 37, 38);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(45, 45, 46);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(62, 62, 66);
    visuals.widgets.active.bg_fill = Color32::from_rgb(14, 99, 156);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    ctx.set_style(style);
}
