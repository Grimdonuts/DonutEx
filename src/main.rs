mod app;
mod console;
mod diff_view;
mod document;
mod editor_view;
mod explorer;
mod git;
mod lsp;
mod plugins;
mod search;
mod settings;
mod source_control;
mod syntax;
mod terminal;
mod terminal_view;
mod theme;

fn main() -> eframe::Result<()> {
    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("DonutEx"),
        ..Default::default()
    };

    eframe::run_native(
        "DonutEx",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, initial_file)))),
    )
}
