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

const HANDLE_WIDTH: f32 = 8.0;
const CODE_FONT_SIZE: f32 = 13.0;
const MIN_SPLIT: f32 = 0.15;
const MAX_SPLIT: f32 = 0.85;
const CELL_PADDING: f32 = 8.0;

/// Renders the diff as two panes (old | new) separated by a draggable
/// splitter, wrapping text within each pane so it scales down with the
/// window instead of overflowing. Row heights are measured up front so a
/// wrapped multi-line cell on one side still keeps both sides - and the
/// splitter segment between them - aligned on the same row.
pub fn show(ui: &mut egui::Ui, tab: &DiffTab) {
    if tab.rows.is_empty() {
        ui.label("No differences.");
        return;
    }

    let split_id = egui::Id::new("diff_view_split_fraction");
    let mut split = ui.memory_mut(|m| *m.data.get_temp_mut_or(split_id, 0.5f32));

    let font_id = egui::FontId::monospace(CODE_FONT_SIZE);
    let line_h = ui.fonts(|f| f.row_height(&font_id));

    egui::ScrollArea::vertical()
        .id_salt(("diff_scroll", &tab.path, tab.staged))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

            let divisible = (ui.available_width() - HANDLE_WIDTH).max(80.0);
            let left_width = (divisible * split).max(40.0);
            let right_width = (divisible - left_width).max(40.0);

            for row in &tab.rows {
                let (left_text, right_text) = match row.kind {
                    RowKind::HunkHeader => (row.header.clone().unwrap_or_default(), String::new()),
                    _ => (cell_text(&row.left), cell_text(&row.right)),
                };
                let lh = measure_height(ui, &left_text, left_width - CELL_PADDING, &font_id).max(line_h);
                let rh = measure_height(ui, &right_text, right_width - CELL_PADDING, &font_id).max(line_h);
                let row_height = lh.max(rh);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    match row.kind {
                        RowKind::HunkHeader => {
                            render_header_cell(ui, &left_text, left_width, row_height, &font_id);
                            render_splitter(ui, &mut split, divisible, row_height);
                            ui.allocate_exact_size(egui::vec2(right_width, row_height), egui::Sense::hover());
                        }
                        _ => {
                            let removed = matches!(row.kind, RowKind::Removed | RowKind::Modified);
                            let added = matches!(row.kind, RowKind::Added | RowKind::Modified);
                            render_cell(ui, &row.left, removed, true, left_width, row_height, &font_id);
                            render_splitter(ui, &mut split, divisible, row_height);
                            render_cell(ui, &row.right, added, false, right_width, row_height, &font_id);
                        }
                    }
                });
            }
        });

    ui.memory_mut(|m| m.data.insert_temp(split_id, split));
}

fn cell_text(cell: &Option<(usize, String)>) -> String {
    match cell {
        Some((no, text)) => format!("{:>5} {}", no, text),
        None => String::new(),
    }
}

/// Measures the wrapped height of `text` at `wrap_width` without drawing it.
/// egui caches galleys by (text, font, wrap_width), so once a row's width
/// stabilizes across frames this is a cache hit, not a fresh text layout.
fn measure_height(ui: &egui::Ui, text: &str, wrap_width: f32, font_id: &egui::FontId) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    ui.fonts(|f| {
        f.layout(text.to_string(), font_id.clone(), Color32::WHITE, wrap_width.max(1.0))
            .size()
            .y
    })
}

fn render_cell(
    ui: &mut egui::Ui,
    cell: &Option<(usize, String)>,
    changed: bool,
    is_left: bool,
    width: f32,
    height: f32,
    font_id: &egui::FontId,
) {
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

    ui.allocate_ui(egui::vec2(width, height), |ui| {
        egui::Frame::none()
            .fill(bg)
            .inner_margin(egui::Margin::symmetric(4.0, 1.0))
            .show(ui, |ui| {
                ui.set_min_size(egui::vec2((width - 8.0).max(0.0), (height - 2.0).max(0.0)));
                if let Some((no, text)) = cell {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("{:>5} {}", no, text)).font(font_id.clone()),
                        )
                        .selectable(false)
                        .wrap_mode(egui::TextWrapMode::Wrap),
                    );
                }
            });
    });
}

fn render_header_cell(ui: &mut egui::Ui, text: &str, width: f32, height: f32, font_id: &egui::FontId) {
    ui.allocate_ui(egui::vec2(width, height), |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(4.0, 1.0))
            .show(ui, |ui| {
                ui.set_min_size(egui::vec2((width - 8.0).max(0.0), (height - 2.0).max(0.0)));
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(text)
                            .font(font_id.clone())
                            .color(Color32::from_rgb(97, 175, 239)),
                    )
                    .selectable(false)
                    .wrap_mode(egui::TextWrapMode::Wrap),
                );
            });
    });
}

/// A draggable divider between the two panes, redrawn once per row rather
/// than as a single tall widget - with zero inter-row spacing the segments
/// butt together and read as one continuous bar, while still being
/// grabbable from anywhere along its length.
fn render_splitter(ui: &mut egui::Ui, split: &mut f32, divisible_width: f32, height: f32) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(HANDLE_WIDTH, height), egui::Sense::drag());

    if response.hovered() || response.dragged() {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
    }
    if response.dragged() && divisible_width > 1.0 {
        *split = (*split + response.drag_delta().x / divisible_width).clamp(MIN_SPLIT, MAX_SPLIT);
    }

    let (color, stroke_width): (Color32, f32) = if response.dragged() {
        (ui.visuals().selection.bg_fill, 2.0)
    } else if response.hovered() {
        (ui.visuals().widgets.hovered.bg_fill, 2.0)
    } else {
        (ui.visuals().widgets.noninteractive.bg_stroke.color, 1.0)
    };
    let cx = rect.center().x;
    ui.painter().line_segment(
        [egui::pos2(cx, rect.top()), egui::pos2(cx, rect.bottom())],
        egui::Stroke::new(stroke_width, color),
    );
}
