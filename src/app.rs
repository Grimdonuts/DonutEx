use crate::console;
use crate::document::Document;
use crate::editor_view::{self, EditorMetrics};
use crate::explorer::{self, FileNode};
use crate::lsp::{LspEvent, LspManager};
use crate::plugins::{PluginEngine, PluginMessage};
use crate::settings::{self, AppSettings};
use crate::terminal::Terminal;
use crate::terminal_view;
use crate::theme;
use eframe::egui;
use std::path::PathBuf;
use std::time::Duration;

/// Which view the bottom panel is currently showing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BottomTab {
    Console,
    Terminal,
}

pub struct App {
    documents: Vec<Document>,
    active: usize,

    show_explorer: bool,
    show_bottom_panel: bool,
    show_settings: bool,
    bottom_tab: BottomTab,
    /// Spawned lazily the first time the Terminal tab is opened, so an
    /// unused editor never pays for a shell process.
    terminal: Option<Terminal>,

    project_root: PathBuf,
    file_tree: FileNode,

    console_lines: Vec<String>,

    plugins: PluginEngine,
    plugins_dir: PathBuf,

    themes: Vec<theme::Theme>,
    current_theme: usize,

    settings: AppSettings,

    lsp: LspManager,

    metrics: Option<EditorMetrics>,
    clipboard: arboard::Clipboard,
    editor_focused: bool,
    /// Mirrors `editor_focused` for the terminal pane - mutually exclusive
    /// with it, so global shortcuts (Ctrl+N etc.) can be suppressed while
    /// the shell, not the editor, should receive keystrokes.
    terminal_focused: bool,
}

/// Layers plugin-registered themes onto a base list, matched by name: a
/// plugin theme with the same name as an existing entry (e.g. the built-in
/// "Dark+") replaces it in place rather than appearing as a duplicate.
fn merge_themes(base: Vec<theme::Theme>, plugin_themes: Vec<theme::Theme>) -> Vec<theme::Theme> {
    let mut themes = base;
    for t in plugin_themes {
        if let Some(existing) = themes.iter_mut().find(|existing| existing.name == t.name) {
            *existing = t;
        } else {
            themes.push(t);
        }
    }
    themes
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let plugins_dir = project_root.join("plugins");
        let file_tree = explorer::build_tree(&project_root);

        let mut plugins = PluginEngine::new();
        let mut console_lines = plugins.reload(&plugins_dir);
        console_lines.insert(0, "DonutEx starting up.".to_string());

        let themes = merge_themes(vec![theme::Theme::built_in_dark()], plugins.take_themes());
        let settings: AppSettings = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, settings::STORAGE_KEY))
            .unwrap_or_default();
        let current_theme = themes
            .iter()
            .position(|t| t.name == settings.theme_name)
            .unwrap_or(0);
        theme::apply(&cc.egui_ctx, &themes[current_theme]);

        let mut documents = vec![Document::new_untitled("untitled".to_string())];
        if let Some(path) = initial_file {
            match Document::from_path(path.clone()) {
                Ok(doc) => documents = vec![doc],
                Err(e) => console_lines.push(format!("failed to open {}: {}", path.display(), e)),
            }
        }

        let lsp = LspManager::new(project_root.clone());

        Self {
            documents,
            active: 0,
            show_explorer: true,
            show_bottom_panel: true,
            show_settings: false,
            bottom_tab: BottomTab::Console,
            terminal: None,
            project_root,
            file_tree,
            console_lines,
            plugins,
            plugins_dir,
            themes,
            current_theme,
            settings,
            lsp,
            metrics: None,
            clipboard: arboard::Clipboard::new().expect("open system clipboard"),
            editor_focused: true,
            terminal_focused: false,
        }
    }

    /// Adds `doc` as a tab. If an untouched "untitled" placeholder tab is
    /// currently open, it's replaced in place instead of leaving it stranded
    /// alongside the new tab.
    fn push_document(&mut self, doc: Document) {
        if let Some(idx) = self.documents.iter().position(|d| d.is_blank_placeholder()) {
            self.documents[idx] = doc;
            self.active = idx;
        } else {
            self.documents.push(doc);
            self.active = self.documents.len() - 1;
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
                self.push_document(doc);
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
        let doc = Document::new_untitled(self.next_untitled_name());
        self.push_document(doc);
    }

    /// Picks a name for a fresh untitled tab that won't collide with any
    /// currently open unsaved buffer. An untouched blank placeholder tab is
    /// excluded from the collision check since `push_document` replaces it
    /// rather than adding alongside it - so as long as at most one such
    /// placeholder is ever open, its name stays "untitled" instead of
    /// climbing on every "New File" click.
    fn next_untitled_name(&self) -> String {
        let taken: std::collections::HashSet<&str> = self
            .documents
            .iter()
            .filter(|d| d.path.is_none() && !d.is_blank_placeholder())
            .map(|d| d.display_name.as_str())
            .collect();
        let mut n = 0;
        loop {
            let name = if n == 0 {
                "untitled".to_string()
            } else {
                format!("untitled-{}", n)
            };
            if !taken.contains(name.as_str()) {
                return name;
            }
            n += 1;
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.project_root)
            .pick_file()
        {
            self.open_path(path);
        }
    }

    /// Repoints the file explorer and LSP workspace root at `path`. Without
    /// this, `project_root` stayed fixed at whatever directory the process
    /// happened to launch from forever - so opening a loose file from a
    /// different project (the normal way to use this app, since there was no
    /// folder-open before) sent every language server the wrong `rootUri`,
    /// which for typescript-language-server means it can never find that
    /// project's `node_modules/typescript` no matter what's installed.
    fn set_project_root(&mut self, path: PathBuf) {
        self.project_root = path;
        self.file_tree = explorer::build_tree(&self.project_root);
        self.console_lines
            .push(format!("project root: {}", self.project_root.display()));
        // Dropping the old manager kills any running server processes (see
        // `LspTransport`'s `Drop` impl) instead of leaving them attached to
        // the previous, now-wrong, root.
        self.lsp = LspManager::new(self.project_root.clone());
    }

    fn open_folder_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.project_root)
            .pick_folder()
        {
            self.set_project_root(path);
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
        let doc = &self.documents[idx];
        if doc.lsp_synced_version != -1 {
            if let Some(path) = doc.path.clone() {
                self.lsp.did_close(&path, doc.language);
            }
        }
        if self.documents.len() == 1 {
            self.documents[0] = Document::new_untitled("untitled".to_string());
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

    fn reload_plugins(&mut self, ctx: &egui::Context) {
        let log = self.plugins.reload(&self.plugins_dir);
        for line in log {
            self.console_lines.push(line);
        }

        // Preserve the current theme selection by name across the reload
        // (a theme plugin may have been edited in place), falling back to
        // the first theme if it was removed.
        let current_name = self.themes[self.current_theme].name.clone();
        self.themes = merge_themes(vec![theme::Theme::built_in_dark()], self.plugins.take_themes());
        self.current_theme = self
            .themes
            .iter()
            .position(|t| t.name == current_name)
            .unwrap_or(0);
        self.settings.theme_name = self.themes[self.current_theme].name.clone();
        theme::apply(ctx, &self.themes[self.current_theme]);

        self.console_lines.push("plugins reloaded".to_string());
    }

    fn switch_theme(&mut self, ctx: &egui::Context, idx: usize) {
        if idx < self.themes.len() {
            self.current_theme = idx;
            self.settings.theme_name = self.themes[idx].name.clone();
            theme::apply(ctx, &self.themes[idx]);
        }
    }

    fn refresh_explorer(&mut self) {
        self.file_tree = explorer::build_tree(&self.project_root);
    }

    /// Saves any dirty, on-disk document whose last edit is older than the
    /// configured autosave delay - mirrors VS Code's "afterDelay" autosave.
    /// Untitled buffers (no path yet) are left alone, same as VS Code, since
    /// there's nowhere to silently write them without prompting.
    fn autosave_tick(&mut self) {
        if !self.settings.autosave {
            return;
        }
        let delay = Duration::from_millis(self.settings.autosave_delay_ms);
        let mut log = Vec::new();
        for doc in self.documents.iter_mut() {
            if doc.dirty && doc.path.is_some() && doc.last_edit_at.elapsed() >= delay {
                match doc.save() {
                    Ok(()) => log.push(format!("autosaved {}", doc.display_name)),
                    Err(e) => {
                        log.push(format!("autosave failed for {}: {}", doc.display_name, e))
                    }
                }
            }
        }
        self.console_lines.extend(log);
    }

    fn drain_plugin_messages(&mut self) {
        for msg in self.plugins.drain_messages() {
            match msg {
                PluginMessage::Output(s) => self.console_lines.push(s),
            }
        }
    }

    /// Keeps the active document's language server in sync: sends
    /// didOpen/didChange when the buffer has edits the server hasn't seen
    /// yet, then asks for a fresh set of semantic tokens. Debounced against
    /// `last_edit_at` so a burst of keystrokes doesn't fire a request per
    /// character. Only the active document is synced/tokenized - background
    /// tabs aren't painted, so there's nothing to color for them yet.
    fn sync_active_doc_with_lsp(&mut self) {
        let idx = self.active;
        let doc = &self.documents[idx];
        let Some(path) = doc.path.clone() else { return };
        if doc.version == doc.lsp_synced_version {
            return;
        }
        if doc.last_edit_at.elapsed() < Duration::from_millis(150) {
            return;
        }
        let lang = doc.language;
        let version = doc.version;
        let text = doc.rope.to_string();
        let lines: Vec<String> = (0..doc.line_count()).map(|l| doc.line_text(l)).collect();
        let cursor = doc.cursor;
        let (comp_line, comp_char) = doc.char_to_lsp_line_col(cursor);

        if doc.lsp_synced_version == -1 {
            self.lsp.did_open(&path, lang, &text, version);
        } else {
            self.lsp.did_change(&path, lang, &text, version);
        }
        self.lsp.request_semantic_tokens(&path, lang, version, lines);
        self.lsp
            .request_completion(&path, lang, version, cursor, comp_line, comp_char);
        self.documents[idx].lsp_synced_version = version;
    }

    fn drain_lsp_events(&mut self) {
        for event in self.lsp.poll() {
            match event {
                LspEvent::SemanticTokens { doc, version, by_line } => {
                    if let Some(d) = self
                        .documents
                        .iter_mut()
                        .find(|d| d.path.as_deref() == Some(doc.as_path()))
                    {
                        if d.version == version {
                            d.lsp_tokens = Some((version, by_line));
                        }
                    }
                }
                LspEvent::Definition { locations } => {
                    if let Some((path, line, character)) = locations.into_iter().next() {
                        self.open_path(path);
                        let doc = &mut self.documents[self.active];
                        doc.cursor = doc.lsp_line_col_to_char(line, character);
                        doc.selection_anchor = None;
                        doc.scroll_offset = (line as f32 - 5.0).max(0.0);
                        self.editor_focused = true;
                    }
                }
                LspEvent::Completion { doc, version, cursor, items } => {
                    if let Some(d) = self
                        .documents
                        .iter_mut()
                        .find(|d| d.path.as_deref() == Some(doc.as_path()))
                    {
                        if d.version == version {
                            d.completion_selected = 0;
                            d.lsp_completions = Some((version, cursor, items));
                        }
                    }
                }
                LspEvent::Log(s) => self.console_lines.push(format!("[lsp] {}", s)),
            }
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, settings::STORAGE_KEY, &self.settings);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.metrics.is_none() {
            self.metrics = Some(EditorMetrics::compute(ctx, 15.0));
        }

        // global keyboard shortcuts
        let (ctrl_n, ctrl_o, ctrl_shift_o, ctrl_s, ctrl_shift_s, ctrl_w) = ctx.input(|i| {
            let ctrl = i.modifiers.ctrl || i.modifiers.command;
            (
                ctrl && i.key_pressed(egui::Key::N),
                ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::O),
                ctrl && i.modifiers.shift && i.key_pressed(egui::Key::O),
                ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::S),
                ctrl && i.modifiers.shift && i.key_pressed(egui::Key::S),
                ctrl && i.key_pressed(egui::Key::W),
            )
        });
        // Suppressed while the terminal has focus so these don't fire
        // alongside whatever the shell itself does with the same chord.
        if !self.terminal_focused {
            if ctrl_n {
                self.new_file();
            }
            if ctrl_shift_o {
                self.open_folder_dialog();
            } else if ctrl_o {
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
                    if ui.button("Open Folder...\tCtrl+Shift+O").clicked() {
                        self.open_folder_dialog();
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
                    ui.checkbox(&mut self.show_bottom_panel, "Console / Terminal");
                    if ui.button("Refresh Explorer").clicked() {
                        self.refresh_explorer();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Plugins", |ui| {
                    if ui.button("Reload Plugins").clicked() {
                        self.reload_plugins(ctx);
                        ui.close_menu();
                    }
                    ui.separator();
                    if self.plugins.loaded_files.is_empty() {
                        ui.label("No plugins loaded");
                    } else {
                        for path in &self.plugins.loaded_files {
                            let name = std::path::Path::new(path)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| path.clone());
                            ui.label(name);
                        }
                    }
                });
                if ui.button("Settings").clicked() {
                    self.show_settings = true;
                }
            });
        });

        let mut theme_selected = None;
        if self.show_settings {
            let mut open = self.show_settings;
            egui::Window::new("Settings")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.heading("Appearance");
                    ui.horizontal(|ui| {
                        ui.label("Theme:");
                        egui::ComboBox::from_id_salt("settings_theme_combo")
                            .selected_text(&self.themes[self.current_theme].name)
                            .show_ui(ui, |ui| {
                                for (i, t) in self.themes.iter().enumerate() {
                                    if ui
                                        .selectable_label(i == self.current_theme, &t.name)
                                        .clicked()
                                    {
                                        theme_selected = Some(i);
                                    }
                                }
                            });
                    });

                    ui.separator();
                    ui.heading("Editor");
                    ui.checkbox(&mut self.settings.autosave, "Autosave");
                    ui.add_enabled_ui(self.settings.autosave, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Delay (ms):");
                            ui.add(
                                egui::DragValue::new(&mut self.settings.autosave_delay_ms)
                                    .range(100..=10_000)
                                    .speed(50),
                            );
                        });
                    });
                });
            self.show_settings = open;
        }
        if let Some(i) = theme_selected {
            self.switch_theme(ctx, i);
        }

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

        if self.show_bottom_panel {
            egui::TopBottomPanel::bottom("console")
                .resizable(true)
                .default_height(180.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.bottom_tab == BottomTab::Console, "Console")
                            .clicked()
                        {
                            self.bottom_tab = BottomTab::Console;
                            self.editor_focused = true;
                            self.terminal_focused = false;
                        }
                        if ui
                            .selectable_label(self.bottom_tab == BottomTab::Terminal, "Terminal")
                            .clicked()
                        {
                            self.bottom_tab = BottomTab::Terminal;
                            self.terminal_focused = true;
                            self.editor_focused = false;
                        }
                    });
                    ui.separator();
                    match self.bottom_tab {
                        BottomTab::Console => console::show(ui, &self.console_lines),
                        BottomTab::Terminal => {
                            if self.terminal.is_none() {
                                match Terminal::spawn(24, 80, &self.project_root) {
                                    Ok(term) => {
                                        self.console_lines.push(format!(
                                            "terminal: started (pid {})",
                                            term.pid()
                                                .map(|p| p.to_string())
                                                .unwrap_or_else(|| "?".to_string())
                                        ));
                                        self.terminal = Some(term);
                                    }
                                    Err(e) => {
                                        self.console_lines
                                            .push(format!("failed to start terminal: {}", e));
                                    }
                                }
                            }
                            if let Some(term) = self.terminal.as_mut() {
                                let metrics = self.metrics.as_ref().unwrap();
                                let outcome = terminal_view::show(
                                    ui,
                                    term,
                                    metrics,
                                    self.terminal_focused,
                                );
                                if let Some(msg) = outcome.exited_message {
                                    self.console_lines.push(msg);
                                }
                                if outcome.response.clicked() || outcome.response.dragged() {
                                    self.terminal_focused = true;
                                    self.editor_focused = false;
                                }
                            } else {
                                ui.label("Terminal unavailable - see console for the error.");
                            }
                        }
                    }
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
                                    self.terminal_focused = false;
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
            let outcome = editor_view::show(
                ui,
                &mut self.documents[self.active],
                metrics,
                &mut self.clipboard,
                self.editor_focused,
                &self.themes[self.current_theme],
            );
            if outcome.response.clicked() || outcome.response.dragged() {
                self.editor_focused = true;
                self.terminal_focused = false;
            }
            if let Some(offset) = outcome.goto_definition {
                let doc = &self.documents[self.active];
                if let Some(path) = doc.path.clone() {
                    let (line, character) = doc.char_to_lsp_line_col(offset);
                    self.lsp.request_definition(&path, doc.language, line, character);
                }
            }
        });

        self.plugins.run_hook("on_render");
        self.drain_plugin_messages();
        self.sync_active_doc_with_lsp();
        self.drain_lsp_events();
        self.autosave_tick();

        // Editor content changes constantly while typing; keep redrawing so
        // the caret/scroll stay responsive without waiting on OS events.
        ctx.request_repaint();
    }
}
