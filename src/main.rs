mod app;
mod console;
mod document;
mod editor_view;
mod explorer;
mod plugins;
mod theme;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("DonutEx"),
        ..Default::default()
    };

    eframe::run_native(
        "DonutEx",
        native_options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
