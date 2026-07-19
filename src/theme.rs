use crate::syntax::TokenKind;
use eframe::egui::{self, Color32};

/// A full color theme: UI chrome colors plus a syntax palette. Themes are
/// data (not code) so they can be authored as Lua tables and registered by
/// theme plugins at runtime - see `plugins::register_theme` and
/// `plugins/theme_*.lua`.
#[derive(Clone)]
pub struct Theme {
    pub name: String,

    // UI chrome
    pub background: Color32, // editor content background
    pub panel: Color32,      // sidebar / console / menu bar background
    pub text: Color32,       // default text color
    pub selection_bg: Color32,
    pub selection_border: Color32,
    pub accent: Color32, // active widget bg / active scrollbar thumb / focus color
    pub scrollbar_track: Color32,
    pub scrollbar_thumb: Color32,

    // Syntax colors. `None` means "use the editor's default text color"
    // (used for TokenKind::Plain, i.e. identifiers that aren't a
    // keyword/type/call/macro).
    pub keyword: Option<Color32>,
    pub type_: Option<Color32>,
    pub string: Option<Color32>,
    pub number: Option<Color32>,
    pub comment: Option<Color32>,
    pub function: Option<Color32>,
    pub macro_: Option<Color32>,
    pub variable: Option<Color32>,
    pub parameter: Option<Color32>,
    pub property: Option<Color32>,
    pub namespace: Option<Color32>,
    pub enum_member: Option<Color32>,
    pub decorator: Option<Color32>,
}

impl Theme {
    /// The original hardcoded Dark+-ish palette, kept as a Rust-native
    /// fallback so the editor always has at least one usable theme even if
    /// `plugins/theme_dark_plus.lua` is missing or fails to load.
    pub fn built_in_dark() -> Theme {
        Theme {
            name: "Dark+".to_string(),

            background: Color32::from_rgb(30, 30, 30),
            panel: Color32::from_rgb(37, 37, 38),
            text: Color32::from_rgb(212, 212, 212),
            selection_bg: Color32::from_rgb(38, 79, 120),
            selection_border: Color32::from_rgb(90, 140, 200),
            accent: Color32::from_rgb(14, 99, 156),
            scrollbar_track: Color32::from_rgb(37, 37, 38),
            scrollbar_thumb: Color32::from_rgb(96, 96, 100),

            keyword: Some(Color32::from_rgb(86, 156, 214)),
            type_: Some(Color32::from_rgb(78, 201, 176)),
            string: Some(Color32::from_rgb(206, 145, 120)),
            number: Some(Color32::from_rgb(181, 206, 168)),
            comment: Some(Color32::from_rgb(106, 153, 85)),
            function: Some(Color32::from_rgb(220, 220, 170)),
            macro_: Some(Color32::from_rgb(197, 134, 192)),
            variable: Some(Color32::from_rgb(156, 220, 254)),
            parameter: Some(Color32::from_rgb(156, 220, 254)),
            property: Some(Color32::from_rgb(156, 220, 254)),
            namespace: Some(Color32::from_rgb(78, 201, 176)),
            enum_member: Some(Color32::from_rgb(78, 201, 176)),
            decorator: Some(Color32::from_rgb(220, 220, 170)),
        }
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
pub fn scrollbar_colors(theme: &Theme) -> ScrollbarColors {
    ScrollbarColors {
        track: theme.scrollbar_track,
        thumb: theme.scrollbar_thumb,
        thumb_hover: lighten(theme.scrollbar_thumb, 26),
        thumb_active: theme.accent,
    }
}

pub fn token_color(theme: &Theme, kind: TokenKind) -> Option<Color32> {
    match kind {
        TokenKind::Plain => None,
        TokenKind::Keyword => theme.keyword,
        TokenKind::Type => theme.type_,
        TokenKind::String => theme.string,
        TokenKind::Number => theme.number,
        TokenKind::Comment => theme.comment,
        TokenKind::Function => theme.function,
        TokenKind::Macro => theme.macro_,
        TokenKind::Variable => theme.variable,
        TokenKind::Parameter => theme.parameter,
        TokenKind::Property => theme.property,
        TokenKind::Namespace => theme.namespace,
        TokenKind::EnumMember => theme.enum_member,
        TokenKind::Decorator => theme.decorator,
    }
}

/// Applies a theme's colors to egui's global visuals/style. Called at
/// startup and whenever the user switches themes from the Themes menu.
pub fn apply(ctx: &egui::Context, theme: &Theme) {
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = theme.panel;
    visuals.window_fill = theme.panel;
    visuals.extreme_bg_color = theme.background;
    visuals.faint_bg_color = lighten(theme.panel, 8);
    visuals.override_text_color = Some(theme.text);

    visuals.selection.bg_fill = theme.selection_bg;
    visuals.selection.stroke.color = theme.selection_border;

    visuals.widgets.noninteractive.bg_fill = theme.panel;
    visuals.widgets.inactive.bg_fill = lighten(theme.panel, 8);
    visuals.widgets.hovered.bg_fill = lighten(theme.panel, 25);
    visuals.widgets.active.bg_fill = theme.accent;

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    ctx.set_style(style);
}

fn lighten(c: Color32, amt: u8) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_add(amt),
        c.g().saturating_add(amt),
        c.b().saturating_add(amt),
    )
}

/// Parses a "#rrggbb" or "#rrggbbaa" hex string into a Color32 - the
/// convention theme plugins use to author colors (same as CSS/VS Code theme
/// JSON), since Lua has no native color type.
pub fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(Color32::from_rgb(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            Some(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        _ => None,
    }
}
