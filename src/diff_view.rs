//! A VS Code-style side-by-side diff view, rendered as a regular editor
//! tab. Built by re-flowing the unified diff text `git diff` produces: runs
//! of consecutive removed/added lines within a hunk are paired up row by
//! row (padding the shorter run with a blank cell) so a changed line lands
//! on the same row on both sides, the same heuristic simple diff viewers
//! use in the absence of a real line-matching algorithm.

use crate::git::{self, FileEntry};
use eframe::egui::{self, Color32};
use std::path::{Path, PathBuf};

enum RowKind {
    Context,
    Added,
    Removed,
    Modified,
    HunkHeader,
}

struct DiffRow {
    kind: RowKind,
    left: Option<(usize, String)>,
    right: Option<(usize, String)>,
    header: Option<String>,
}

pub struct DiffTab {
    pub path: PathBuf,
    pub staged: bool,
    pub title: String,
    rows: Vec<DiffRow>,
}

/// Runs `git diff` (or the untracked-file equivalent) for `entry` and
/// builds the side-by-side row layout.
pub fn build(root: &Path, entry: &FileEntry, staged: bool) -> Result<DiffTab, String> {
    let raw = if entry.status == '?' {
        git::diff_untracked(root, &entry.path)?
    } else {
        git::diff(root, &entry.path, staged)?
    };
    let name = entry
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| entry.path.display().to_string());
    let title = format!("{} ({})", name, if staged { "Staged" } else { "Working Tree" });
    Ok(DiffTab {
        path: entry.path.clone(),
        staged,
        title,
        rows: parse_rows(&raw),
    })
}

fn flush(
    rows: &mut Vec<DiffRow>,
    rem_buf: &mut Vec<String>,
    add_buf: &mut Vec<String>,
    old_no: &mut usize,
    new_no: &mut usize,
) {
    let n = rem_buf.len().max(add_buf.len());
    for i in 0..n {
        let left = rem_buf.get(i).cloned().map(|t| {
            let ln = *old_no;
            *old_no += 1;
            (ln, t)
        });
        let right = add_buf.get(i).cloned().map(|t| {
            let ln = *new_no;
            *new_no += 1;
            (ln, t)
        });
        let kind = match (&left, &right) {
            (Some(_), Some(_)) => RowKind::Modified,
            (Some(_), None) => RowKind::Removed,
            (None, Some(_)) => RowKind::Added,
            (None, None) => continue,
        };
        rows.push(DiffRow {
            kind,
            left,
            right,
            header: None,
        });
    }
    rem_buf.clear();
    add_buf.clear();
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    // "@@ -old_start,old_count +new_start,new_count @@ optional trailing context"
    let inner = line.strip_prefix("@@ ")?;
    let end = inner.find(" @@")?;
    let inner = &inner[..end];
    let mut parts = inner.split_whitespace();
    let old = parts.next()?.trim_start_matches('-');
    let new = parts.next()?.trim_start_matches('+');
    let old_start: usize = old.split(',').next()?.parse().ok()?;
    let new_start: usize = new.split(',').next()?.parse().ok()?;
    Some((old_start, new_start))
}

fn parse_rows(raw: &str) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    let mut old_no = 0usize;
    let mut new_no = 0usize;
    let mut rem_buf = Vec::new();
    let mut add_buf = Vec::new();

    for line in raw.lines() {
        if line.starts_with("@@") {
            flush(&mut rows, &mut rem_buf, &mut add_buf, &mut old_no, &mut new_no);
            if let Some((o, n)) = parse_hunk_header(line) {
                old_no = o;
                new_no = n;
            }
            rows.push(DiffRow {
                kind: RowKind::HunkHeader,
                left: None,
                right: None,
                header: Some(line.to_string()),
            });
            continue;
        }
        if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("+++")
            || line.starts_with("---")
            || line.starts_with("new file")
            || line.starts_with("deleted file")
            || line.starts_with("similarity index")
            || line.starts_with("rename ")
            || line.starts_with('\\')
        {
            continue;
        }
        if let Some(text) = line.strip_prefix('+') {
            add_buf.push(text.to_string());
        } else if let Some(text) = line.strip_prefix('-') {
            rem_buf.push(text.to_string());
        } else {
            flush(&mut rows, &mut rem_buf, &mut add_buf, &mut old_no, &mut new_no);
            let text = line.strip_prefix(' ').unwrap_or(line).to_string();
            rows.push(DiffRow {
                kind: RowKind::Context,
                left: Some((old_no, text.clone())),
                right: Some((new_no, text)),
                header: None,
            });
            old_no += 1;
            new_no += 1;
        }
    }
    flush(&mut rows, &mut rem_buf, &mut add_buf, &mut old_no, &mut new_no);
    rows
}

pub fn show(ui: &mut egui::Ui, tab: &DiffTab) {
    if tab.rows.is_empty() {
        ui.label("No differences.");
        return;
    }
    egui::ScrollArea::both()
        .id_salt(("diff_scroll", &tab.path, tab.staged))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new(("diff_grid", &tab.path, tab.staged))
                .num_columns(2)
                .striped(false)
                .spacing([0.0, 0.0])
                .min_col_width(ui.available_width() / 2.0 - 4.0)
                .show(ui, |ui| {
                    for row in &tab.rows {
                        match row.kind {
                            RowKind::HunkHeader => {
                                ui.colored_label(
                                    Color32::from_rgb(97, 175, 239),
                                    row.header.as_deref().unwrap_or("@@"),
                                );
                                ui.label("");
                            }
                            _ => {
                                let removed_side = matches!(row.kind, RowKind::Removed | RowKind::Modified);
                                let added_side = matches!(row.kind, RowKind::Added | RowKind::Modified);
                                render_side(ui, &row.left, removed_side, true);
                                render_side(ui, &row.right, added_side, false);
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}

fn render_side(ui: &mut egui::Ui, cell: &Option<(usize, String)>, changed: bool, is_left: bool) {
    let bg = if changed {
        if is_left {
            Color32::from_rgba_unmultiplied(224, 108, 117, 35)
        } else {
            Color32::from_rgba_unmultiplied(129, 193, 105, 35)
        }
    } else if cell.is_none() {
        Color32::from_rgba_unmultiplied(128, 128, 128, 20)
    } else {
        Color32::TRANSPARENT
    };

    egui::Frame::none()
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            match cell {
                Some((no, text)) => {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("{:>5}", no)).monospace().weak(),
                            )
                            .selectable(false),
                        );
                        ui.add(
                            egui::Label::new(egui::RichText::new(text).monospace())
                                .selectable(false)
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });
                }
                None => {
                    ui.label("");
                }
            }
        });
}
