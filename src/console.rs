use eframe::egui;

pub fn show(ui: &mut egui::Ui, lines: &[String]) {
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for line in lines {
                ui.monospace(line);
            }
        });
}
