//! The Source Control sidebar: a VS Code-style git panel with staged /
//! unstaged file groups, stage/unstage/discard actions, a commit box, and a
//! diff viewer.

use crate::git::{self, FileEntry};
use eframe::egui::{self, Color32};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_secs(2);

enum DiffLineKind {
    Add,
    Remove,
    Hunk,
    Meta,
    Context,
}

struct DiffLine {
    kind: DiffLineKind,
    text: String,
}

struct DiffView {
    title: String,
    lines: Vec<DiffLine>,
}

pub struct SourceControl {
    root: PathBuf,
    pub is_repo: bool,
    branch: Option<String>,
    staged: Vec<FileEntry>,
    unstaged: Vec<FileEntry>,
    commit_message: String,
    error: Option<String>,
    diff_view: Option<DiffView>,
    pending_discard: Option<FileEntry>,
    last_refresh: Instant,
}

impl SourceControl {
    pub fn new(root: PathBuf) -> Self {
        let is_repo = git::is_repo(&root);
        let mut sc = Self {
            root,
            is_repo,
            branch: None,
            staged: Vec::new(),
            unstaged: Vec::new(),
            commit_message: String::new(),
            error: None,
            diff_view: None,
            pending_discard: None,
            last_refresh: Instant::now(),
        };
        if is_repo {
            sc.refresh();
        }
        sc
    }

    pub fn change_count(&self) -> usize {
        self.staged.len() + self.unstaged.len()
    }

    pub fn refresh(&mut self) {
        self.last_refresh = Instant::now();
        if !self.is_repo {
            return;
        }
        match git::status(&self.root) {
            Ok(status) => {
                self.branch = status.branch;
                self.staged = status.staged;
                self.unstaged = status.unstaged;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Cheap periodic refresh so file changes made outside a git action
    /// (editing a tracked file, switching branches in a terminal) show up
    /// without requiring a manual click - throttled since it shells out.
    pub fn maybe_poll(&mut self) {
        if self.is_repo && self.last_refresh.elapsed() >= POLL_INTERVAL {
            self.refresh();
        }
    }

    fn view_diff(&mut self, entry: &FileEntry, staged: bool) {
        let result = if entry.status == '?' {
            git::diff_untracked(&self.root, &entry.path)
        } else {
            git::diff(&self.root, &entry.path, staged)
        };
        match result {
            Ok(text) => {
                self.diff_view = Some(DiffView {
                    title: format!("Diff: {}", entry.path.display()),
                    lines: parse_diff(&text),
                });
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn do_stage(&mut self, path: &std::path::Path) {
        if let Err(e) = git::stage(&self.root, path) {
            self.error = Some(e);
        }
        self.refresh();
    }

    fn do_unstage(&mut self, path: &std::path::Path) {
        if let Err(e) = git::unstage(&self.root, path) {
            self.error = Some(e);
        }
        self.refresh();
    }

    fn do_discard(&mut self, entry: &FileEntry) {
        if let Err(e) = git::discard(&self.root, entry) {
            self.error = Some(e);
        }
        self.refresh();
    }

    fn do_stage_all(&mut self) {
        if let Err(e) = git::stage_all(&self.root) {
            self.error = Some(e);
        }
        self.refresh();
    }

    fn do_unstage_all(&mut self) {
        if let Err(e) = git::unstage_all(&self.root) {
            self.error = Some(e);
        }
        self.refresh();
    }

    fn do_commit(&mut self) {
        match git::commit(&self.root, self.commit_message.trim()) {
            Ok(_) => self.commit_message.clear(),
            Err(e) => self.error = Some(e),
        }
        self.refresh();
    }

    /// Draws the sidebar contents into `ui` and any floating diff/confirm
    /// windows via `ctx`. Returns a path the caller should open as a normal
    /// editor tab, if the user clicked a file's "open" button.
    pub fn show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut open_file = None;

        ui.heading("Source Control");
        ui.separator();

        if !self.is_repo {
            ui.label("Not a git repository.");
            return None;
        }

        ui.horizontal(|ui| {
            ui.label(format!(
                "\u{2387} {}",
                self.branch.as_deref().unwrap_or("(no branch)")
            ));
            if ui.small_button("\u{27F3}").on_hover_text("Refresh").clicked() {
                self.refresh();
            }
        });

        if let Some(err) = self.error.clone() {
            ui.colored_label(Color32::from_rgb(224, 108, 117), err);
        }

        ui.separator();
        ui.add(
            egui::TextEdit::multiline(&mut self.commit_message)
                .desired_rows(3)
                .hint_text("Commit message"),
        );
        let can_commit = !self.staged.is_empty() && !self.commit_message.trim().is_empty();
        if ui
            .add_enabled(can_commit, egui::Button::new("\u{2713} Commit"))
            .clicked()
        {
            self.do_commit();
        }
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if !self.staged.is_empty() {
                    ui.horizontal(|ui| {
                        ui.strong(format!("Staged Changes ({})", self.staged.len()));
                        if ui.small_button("\u{2212}").on_hover_text("Unstage All").clicked() {
                            self.do_unstage_all();
                        }
                    });
                    let mut action = None;
                    for entry in &self.staged {
                        if let Some(a) = show_entry(ui, entry, true) {
                            action = Some(a);
                        }
                    }
                    self.apply_action(action, &mut open_file);
                    ui.add_space(8.0);
                }

                if !self.unstaged.is_empty() {
                    ui.horizontal(|ui| {
                        ui.strong(format!("Changes ({})", self.unstaged.len()));
                        if ui.small_button("+").on_hover_text("Stage All").clicked() {
                            self.do_stage_all();
                        }
                    });
                    let mut action = None;
                    for entry in &self.unstaged {
                        if let Some(a) = show_entry(ui, entry, false) {
                            action = Some(a);
                        }
                    }
                    self.apply_action(action, &mut open_file);
                }

                if self.staged.is_empty() && self.unstaged.is_empty() {
                    ui.label("No changes.");
                }
            });

        self.show_diff_window(ctx);
        self.show_discard_confirm(ctx);

        open_file
    }

    fn apply_action(&mut self, action: Option<Action>, open_file: &mut Option<PathBuf>) {
        match action {
            Some(Action::Open(path)) => *open_file = Some(path),
            Some(Action::ViewDiff(entry, staged)) => self.view_diff(&entry, staged),
            Some(Action::Stage(path)) => self.do_stage(&path),
            Some(Action::Unstage(path)) => self.do_unstage(&path),
            Some(Action::Discard(entry)) => self.pending_discard = Some(entry),
            None => {}
        }
    }

    fn show_diff_window(&mut self, ctx: &egui::Context) {
        let Some(diff) = &self.diff_view else { return };
        let mut open = true;
        egui::Window::new(&diff.title)
            .id(egui::Id::new("source_control_diff_window"))
            .default_size([700.0, 500.0])
            .resizable(true)
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for line in &diff.lines {
                            let color = match line.kind {
                                DiffLineKind::Add => Color32::from_rgb(129, 193, 105),
                                DiffLineKind::Remove => Color32::from_rgb(224, 108, 117),
                                DiffLineKind::Hunk => Color32::from_rgb(97, 175, 239),
                                DiffLineKind::Meta => Color32::GRAY,
                                DiffLineKind::Context => ui.visuals().text_color(),
                            };
                            ui.label(egui::RichText::new(&line.text).monospace().color(color));
                        }
                        if diff.lines.is_empty() {
                            ui.label("No differences.");
                        }
                    });
            });
        if !open {
            self.diff_view = None;
        }
    }

    fn show_discard_confirm(&mut self, ctx: &egui::Context) {
        let Some(entry) = self.pending_discard.clone() else { return };
        let mut open = true;
        let mut do_it = false;
        let mut cancel = false;
        egui::Window::new("Discard Changes")
            .id(egui::Id::new("source_control_discard_confirm"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Discard changes to \"{}\"? This cannot be undone.",
                    entry.path.display()
                ));
                ui.horizontal(|ui| {
                    if ui.button("Discard Changes").clicked() {
                        do_it = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if do_it {
            self.do_discard(&entry);
            self.pending_discard = None;
        } else if !open || cancel {
            self.pending_discard = None;
        }
    }
}

enum Action {
    Open(PathBuf),
    ViewDiff(FileEntry, bool),
    Stage(PathBuf),
    Unstage(PathBuf),
    Discard(FileEntry),
}

fn status_color(status: char) -> Color32 {
    match status {
        'M' => Color32::from_rgb(229, 192, 123),
        'A' => Color32::from_rgb(129, 193, 105),
        'D' => Color32::from_rgb(224, 108, 117),
        'R' | 'C' => Color32::from_rgb(97, 175, 239),
        'U' => Color32::from_rgb(224, 108, 117),
        '?' => Color32::from_rgb(129, 193, 105),
        _ => Color32::GRAY,
    }
}

/// Renders one file row (`status_letter  name    [open] [diff-implicit] [+/-] [discard]`)
/// and returns the action the user requested, if any.
fn show_entry(ui: &mut egui::Ui, entry: &FileEntry, staged: bool) -> Option<Action> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.colored_label(
            status_color(entry.status),
            egui::RichText::new(entry.status.to_string()).monospace().strong(),
        );

        let name = entry
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| entry.path.display().to_string());
        let dir = entry.path.parent().filter(|p| !p.as_os_str().is_empty());

        let hover = match &entry.orig_path {
            Some(orig) => format!("renamed from {}\n{}", orig.display(), entry.path.display()),
            None => entry.path.display().to_string(),
        };
        let label = ui
            .add(egui::Label::new(name).sense(egui::Sense::click()))
            .on_hover_text(hover);
        if label.clicked() {
            action = Some(Action::ViewDiff(entry.clone(), staged));
        }
        if let Some(orig) = &entry.orig_path {
            ui.weak(format!(
                "\u{2190} {}",
                orig.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| orig.display().to_string())
            ));
        } else if let Some(dir) = dir {
            ui.weak(dir.display().to_string());
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("\u{1F4C4}").on_hover_text("Open File").clicked() {
                action = Some(Action::Open(entry.path.clone()));
            }
            if staged {
                if ui.small_button("\u{2212}").on_hover_text("Unstage").clicked() {
                    action = Some(Action::Unstage(entry.path.clone()));
                }
            } else {
                if ui.small_button("+").on_hover_text("Stage").clicked() {
                    action = Some(Action::Stage(entry.path.clone()));
                }
                if ui
                    .small_button("\u{21B6}")
                    .on_hover_text("Discard Changes")
                    .clicked()
                {
                    action = Some(Action::Discard(entry.clone()));
                }
            }
        });
    });
    action
}

fn parse_diff(raw: &str) -> Vec<DiffLine> {
    raw.lines()
        .map(|line| {
            let kind = if line.starts_with("+++")
                || line.starts_with("---")
                || line.starts_with("diff --git")
                || line.starts_with("index ")
                || line.starts_with("new file")
                || line.starts_with("deleted file")
                || line.starts_with("similarity index")
                || line.starts_with("rename ")
            {
                DiffLineKind::Meta
            } else if line.starts_with("@@") {
                DiffLineKind::Hunk
            } else if line.starts_with('+') {
                DiffLineKind::Add
            } else if line.starts_with('-') {
                DiffLineKind::Remove
            } else {
                DiffLineKind::Context
            };
            DiffLine {
                kind,
                text: line.to_string(),
            }
        })
        .collect()
}
