use eframe::egui::{self, Color32};

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
