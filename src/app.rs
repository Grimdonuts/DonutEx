use crate::console;
use crate::document::Document;
use crate::editor_view::{self, EditorMetrics};
use crate::explorer::{self, FileNode};
use crate::plugins::{PluginEngine, PluginMessage};
use crate::theme;
use eframe::egui;
use std::path::PathBuf;

pub struct App {
    documents: Vec<Document>,
    active: usize,
    untitled_counter: usize,

    show_explorer: bool,
    show_console: bool,

    project_root: PathBuf,
    file_tree: FileNode,

    console_lines: Vec<String>,

    plugins: PluginEngine,
    plugins_dir: PathBuf,

    metrics: Option<EditorMetrics>,
    clipboard: arboard::Clipboard,
    editor_focused: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);

        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let plugins_dir = project_root.join("plugins");
        let file_tree = explorer::build_tree(&project_root);

        let mut plugins = PluginEngine::new();
        let mut console_lines = plugins.reload(&plugins_dir);
        console_lines.insert(0, "DonutEx starting up.".to_string());

        Self {
            documents: vec![Document::new_untitled(0)],
            active: 0,
            untitled_counter: 1,
            show_explorer: true,
            show_console: true,
            project_root,
            file_tree,
            console_lines,
            plugins,
            plugins_dir,
            metrics: None,
            clipboard: arboard::Clipboard::new().expect("open system clipboard"),
            editor_focused: true,
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        if let Some(idx) = self
            .documents
            .iter()
            .position(|d| d.path.as_deref() == Some(path.as_path()))
        {
            self.active = idx;
            return;
        }
        match Document::from_path(path.clone()) {
            Ok(doc) => {
                self.documents.push(doc);
                self.active = self.documents.len() - 1;
                self.console_lines
                    .push(format!("opened {}", path.display()));
            }
            Err(e) => {
                self.console_lines
                    .push(format!("failed to open {}: {}", path.display(), e));
            }
        }
    }

    fn new_file(&mut self) {
        self.documents
            .push(Document::new_untitled(self.untitled_counter));
        self.untitled_counter += 1;
        self.active = self.documents.len() - 1;
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.project_root)
            .pick_file()
        {
            self.open_path(path);
        }
    }

    fn save_active(&mut self) {
        let needs_path = self.documents[self.active].path.is_none();
        if needs_path {
            self.save_active_as();
            return;
        }
        let doc = &mut self.documents[self.active];
        match doc.save() {
            Ok(()) => self.console_lines.push(format!("saved {}", doc.display_name)),
            Err(e) => self.console_lines.push(format!("save failed: {}", e)),
        }
    }

    fn save_active_as(&mut self) {
        let default_name = self.documents[self.active].display_name.clone();
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.project_root)
            .set_file_name(&default_name)
            .save_file()
        {
            let doc = &mut self.documents[self.active];
            match doc.save_as(path) {
                Ok(()) => self
                    .console_lines
                    .push(format!("saved {}", doc.display_name)),
                Err(e) => self.console_lines.push(format!("save failed: {}", e)),
            }
        }
    }

    fn close_tab(&mut self, idx: usize) {
        if self.documents.len() == 1 {
            self.documents[0] = Document::new_untitled(self.untitled_counter);
            self.untitled_counter += 1;
            self.active = 0;
            return;
        }
        self.documents.remove(idx);
        if self.active >= self.documents.len() {
            self.active = self.documents.len() - 1;
        } else if idx < self.active {
            self.active -= 1;
        }
    }

    fn reload_plugins(&mut self) {
        let log = self.plugins.reload(&self.plugins_dir);
        for line in log {
            self.console_lines.push(line);
        }
        self.console_lines.push("plugins reloaded".to_string());
    }

    fn refresh_explorer(&mut self) {
        self.file_tree = explorer::build_tree(&self.project_root);
    }

    fn drain_plugin_messages(&mut self) {
        for msg in self.plugins.drain_messages() {
            match msg {
                PluginMessage::Output(s) => self.console_lines.push(s),
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.metrics.is_none() {
            self.metrics = Some(EditorMetrics::compute(ctx, 15.0));
        }

        // global keyboard shortcuts
        let (ctrl_n, ctrl_o, ctrl_s, ctrl_shift_s, ctrl_w) = ctx.input(|i| {
            let ctrl = i.modifiers.ctrl || i.modifiers.command;
            (
                ctrl && i.key_pressed(egui::Key::N),
                ctrl && i.key_pressed(egui::Key::O),
                ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::S),
                ctrl && i.modifiers.shift && i.key_pressed(egui::Key::S),
                ctrl && i.key_pressed(egui::Key::W),
            )
        });
        if ctrl_n {
            self.new_file();
        }
        if ctrl_o {
            self.open_dialog();
        }
        if ctrl_shift_s {
            self.save_active_as();
        } else if ctrl_s {
            self.save_active();
        }
        if ctrl_w {
            let active = self.active;
            self.close_tab(active);
        }

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New\tCtrl+N").clicked() {
                        self.new_file();
                        ui.close_menu();
                    }
                    if ui.button("Open...\tCtrl+O").clicked() {
                        self.open_dialog();
                        ui.close_menu();
                    }
                    if ui.button("Save\tCtrl+S").clicked() {
                        self.save_active();
                        ui.close_menu();
                    }
                    if ui.button("Save As...\tCtrl+Shift+S").clicked() {
                        self.save_active_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Close Tab\tCtrl+W").clicked() {
                        let active = self.active;
                        self.close_tab(active);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_explorer, "File Explorer");
                    ui.checkbox(&mut self.show_console, "Console");
                    if ui.button("Refresh Explorer").clicked() {
                        self.refresh_explorer();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Plugins", |ui| {
                    if ui.button("Reload Plugins").clicked() {
                        self.reload_plugins();
                        ui.close_menu();
                    }
                    ui.label(format!("{} loaded", self.plugins.loaded_files.len()));
                });
            });
        });

        if self.show_explorer {
            egui::SidePanel::left("explorer")
                .resizable(true)
                .default_width(230.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(
                            self.project_root
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "project".to_string()),
                        );
                    });
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(path) = explorer::show(ui, &self.file_tree) {
                                self.open_path(path);
                            }
                        });
                });
        }

        if self.show_console {
            egui::TopBottomPanel::bottom("console")
                .resizable(true)
                .default_height(180.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Console");
                    });
                    ui.separator();
                    console::show(ui, &self.console_lines);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // tab bar
            egui::TopBottomPanel::top("tabs")
                .show_separator_line(true)
                .show_inside(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let mut close_request = None;
                        for (i, doc) in self.documents.iter().enumerate() {
                            let selected = i == self.active;
                            let label = if doc.dirty {
                                format!("\u{25CF} {}", doc.display_name)
                            } else {
                                doc.display_name.clone()
                            };
                            ui.horizontal(|ui| {
                                if ui.selectable_label(selected, label).clicked() {
                                    self.active = i;
                                    self.editor_focused = true;
                                }
                                if ui.small_button("x").clicked() {
                                    close_request = Some(i);
                                }
                            });
                        }
                        if let Some(idx) = close_request {
                            self.close_tab(idx);
                        }
                    });
                });

            let metrics = self.metrics.as_ref().unwrap();
            let response = editor_view::show(
                ui,
                &mut self.documents[self.active],
                metrics,
                &mut self.clipboard,
                self.editor_focused,
            );
            if response.clicked() || response.dragged() {
                self.editor_focused = true;
            }
        });

        self.plugins.run_hook("on_render");
        self.drain_plugin_messages();

        // Editor content changes constantly while typing; keep redrawing so
        // the caret/scroll stay responsive without waiting on OS events.
        ctx.request_repaint();
    }
}
