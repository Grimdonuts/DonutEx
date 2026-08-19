//! The Search sidebar: a VS Code-style "search all files" panel. Runs on
//! demand (Enter / the Search button, not on every keystroke) and lists
//! matching lines grouped by file. In a git repo, the candidate file list
//! comes from `git ls-files` (respecting `.gitignore` automatically);
//! otherwise it falls back to walking the directory tree directly.

use crate::explorer;
use crate::git;
use eframe::egui::{self, Color32};
use std::path::{Path, PathBuf};

/// Files larger than this are almost certainly generated/binary/data, not
/// something you'd text-search - skip them rather than read multi-MB files
/// into memory on every search.
const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024;
/// Hard cap on results so a broad query (e.g. a single common letter)
/// against a large project can't grow the results list unboundedly.
const MAX_RESULTS: usize = 500;

pub struct SearchMatch {
    pub path: PathBuf,
    /// 0-indexed, to match `Document::lsp_line_col_to_char`.
    pub line: usize,
    pub line_text: String,
}

pub enum SearchAction {
    OpenResult(PathBuf, usize),
}

pub struct SearchPanel {
    query: String,
    case_sensitive: bool,
    results: Vec<SearchMatch>,
    truncated: bool,
    searched: bool,
}

impl SearchPanel {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            results: Vec::new(),
            truncated: false,
            searched: false,
        }
    }

    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    fn run(&mut self, root: &Path) {
        self.results.clear();
        self.truncated = false;
        self.searched = true;
        if self.query.is_empty() {
            return;
        }
        let needle = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };

        if git::is_repo(root) {
            if let Ok(files) = git::list_files(root) {
                for path in files {
                    if self.search_file(&path, &needle) {
                        return; // hit MAX_RESULTS
                    }
                }
                return;
            }
            // `git` is present (is_repo succeeded) but ls-files failed for
            // some other reason - fall through to the manual walk rather
            // than silently returning zero results.
        }

        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if explorer::IGNORE.contains(&name.as_str()) {
                    continue;
                }
                let path = entry.path();
                let Ok(meta) = entry.metadata() else { continue };
                if meta.is_dir() {
                    stack.push(path);
                    continue;
                }
                if self.search_file(&path, &needle) {
                    return;
                }
            }
        }
    }

    /// Searches one file for `needle`, appending any matching lines to
    /// `self.results`. Returns `true` once `MAX_RESULTS` is hit, telling
    /// the caller to stop walking.
    fn search_file(&mut self, path: &Path, needle: &str) -> bool {
        let Ok(meta) = std::fs::metadata(path) else { return false };
        if !meta.is_file() || meta.len() > MAX_FILE_SIZE {
            return false;
        }
        let Ok(bytes) = std::fs::read(path) else { return false };
        if bytes.iter().take(4096).any(|&b| b == 0) {
            return false; // looks binary
        }
        let Ok(text) = String::from_utf8(bytes) else { return false };

        for (i, line) in text.lines().enumerate() {
            let haystack = if self.case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };
            if haystack.contains(needle) {
                self.results.push(SearchMatch {
                    path: path.to_path_buf(),
                    line: i,
                    line_text: line.to_string(),
                });
                if self.results.len() >= MAX_RESULTS {
                    self.truncated = true;
                    return true;
                }
            }
        }
        false
    }

    pub fn show(&mut self, ui: &mut egui::Ui, root: &Path) -> Option<SearchAction> {
        let mut action = None;

        ui.heading("Search");
        ui.separator();

        let mut submit = false;
        ui.horizontal(|ui| {
            // Right-to-left so the "Aa" toggle is placed (and its real
            // width measured) first; the text field then fills whatever
            // space is *actually* left, rather than guessing a fixed
            // pixel amount to reserve. A guessed reservation that's even
            // slightly too small lets the row overflow its allocated
            // width - and since SidePanel persists its width from the
            // content's measured bounding rect each frame, that overflow
            // compounds every single frame, growing the sidebar
            // indefinitely with no user input at all.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .selectable_label(self.case_sensitive, "Aa")
                    .on_hover_text("Match Case")
                    .clicked()
                {
                    self.case_sensitive = !self.case_sensitive;
                    submit = true;
                }
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Search project files")
                        .desired_width(ui.available_width()),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
            });
        });
        if ui.button("\u{1F50D} Search").clicked() {
            submit = true;
        }
        if submit {
            self.run(root);
        }

        if self.truncated {
            ui.colored_label(
                Color32::from_rgb(229, 192, 123),
                format!("Showing first {} results - refine your search", MAX_RESULTS),
            );
        } else if self.searched && !self.query.is_empty() {
            ui.weak(format!("{} result(s)", self.results.len()));
        }
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.searched && self.results.is_empty() && !self.query.is_empty() {
                    ui.label("No results found.");
                }
                let mut last_path: Option<&Path> = None;
                for m in &self.results {
                    if last_path != Some(m.path.as_path()) {
                        ui.add_space(6.0);
                        ui.strong(
                            m.path
                                .strip_prefix(root)
                                .unwrap_or(&m.path)
                                .display()
                                .to_string(),
                        );
                        last_path = Some(m.path.as_path());
                    }
                    let label = format!("  {}: {}", m.line + 1, truncate(m.line_text.trim()));
                    if ui.selectable_label(false, label).clicked() {
                        action = Some(SearchAction::OpenResult(m.path.clone(), m.line));
                    }
                }
            });

        action
    }
}

/// Caps a result line's displayed length so one very long source line (a
/// minified file, a huge string literal) can't force the results list -
/// and with it the sidebar - wider than the window.
const MAX_LINE_DISPLAY: usize = 200;

fn truncate(line: &str) -> std::borrow::Cow<'_, str> {
    if line.chars().count() <= MAX_LINE_DISPLAY {
        std::borrow::Cow::Borrowed(line)
    } else {
        let mut s: String = line.chars().take(MAX_LINE_DISPLAY).collect();
        s.push('\u{2026}');
        std::borrow::Cow::Owned(s)
    }
}
