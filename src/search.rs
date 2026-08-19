//! The Search sidebar: a VS Code-style "search all files" panel. Walks the
//! project directory on demand (Enter / the Search button, not on every
//! keystroke) and lists matching lines grouped by file.

use crate::explorer;
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
                if meta.len() > MAX_FILE_SIZE {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&path) else { continue };
                if bytes.iter().take(4096).any(|&b| b == 0) {
                    continue; // looks binary
                }
                let Ok(text) = String::from_utf8(bytes) else { continue };

                for (i, line) in text.lines().enumerate() {
                    let haystack = if self.case_sensitive {
                        line.to_string()
                    } else {
                        line.to_lowercase()
                    };
                    if haystack.contains(&needle) {
                        self.results.push(SearchMatch {
                            path: path.clone(),
                            line: i,
                            line_text: line.to_string(),
                        });
                        if self.results.len() >= MAX_RESULTS {
                            self.truncated = true;
                            return;
                        }
                    }
                }
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, root: &Path) -> Option<SearchAction> {
        let mut action = None;

        ui.heading("Search");
        ui.separator();

        let mut submit = false;
        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Search project files")
                    .desired_width(ui.available_width() - 36.0),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit = true;
            }
            if ui
                .selectable_label(self.case_sensitive, "Aa")
                .on_hover_text("Match Case")
                .clicked()
            {
                self.case_sensitive = !self.case_sensitive;
                submit = true;
            }
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
