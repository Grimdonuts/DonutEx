use crate::document::{DragTarget, Document};
use crate::syntax;
use crate::theme;
use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Rect, Sense, Stroke};

const SCROLLBAR_THICKNESS: f32 = 12.0;

pub struct EditorMetrics {
    pub font_id: FontId,
    pub char_w: f32,
    pub row_h: f32,
}

impl EditorMetrics {
    pub fn compute(ctx: &egui::Context, size: f32) -> Self {
        let font_id = FontId::monospace(size);
        let (char_w, row_h) = ctx.fonts(|f| {
            (f.glyph_width(&font_id, 'M'), f.row_height(&font_id))
        });
        Self {
            font_id,
            char_w,
            row_h,
        }
    }
}

/// What happened this frame that the caller (app.rs) needs to act on beyond
/// just repainting - currently only a ctrl+click asking to jump to a
/// definition, carried as the char offset that was clicked.
pub struct EditorOutcome {
    pub response: egui::Response,
    pub goto_definition: Option<usize>,
}

/// Renders one open document as a virtualized monospace grid: only the rows
/// intersecting the viewport are laid out and painted, so editor cost stays
/// proportional to screen size rather than file size.
pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    metrics: &EditorMetrics,
    clipboard: &mut arboard::Clipboard,
    focused: bool,
    theme: &theme::Theme,
) -> EditorOutcome {
    let avail = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(avail, Sense::click_and_drag());

    let visuals = ui.visuals();
    let bg = visuals.extreme_bg_color;
    let text_color = visuals.text_color();
    let gutter_color = visuals.weak_text_color();
    let selection_color = visuals.selection.bg_fill;
    let caret_color = visuals.strong_text_color();
    let sb_colors = theme::scrollbar_colors(theme);

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, bg);

    let row_h = metrics.row_h;
    let char_w = metrics.char_w;

    let total_rows = doc.line_count().max(1);
    let digits = total_rows.to_string().len().max(3);
    let gutter_w = metrics.char_w * (digits as f32 + 2.0);

    // Reserve fixed strips on the right/bottom for the scrollbars so their
    // hit-test regions never overlap the text's click/drag region.
    let content_w = (rect.width() - SCROLLBAR_THICKNESS).max(0.0);
    let content_h = (rect.height() - SCROLLBAR_THICKNESS).max(0.0);
    let content_rect = Rect::from_min_size(rect.min, egui::vec2(content_w, content_h));

    let text_x0 = content_rect.min.x + gutter_w - doc.h_scroll_offset;
    let text_view_w = (content_w - gutter_w).max(1.0);
    let full_content_w = doc.max_line_width_ch as f32 * char_w;

    let visible_rows = (content_h / row_h).ceil().max(1.0) as usize + 1;
    let first_row = (doc.scroll_offset as usize).min(total_rows.saturating_sub(1));

    // --- scrolling (wheel) ---
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta);
        if scroll.y != 0.0 {
            let max_scroll = total_rows.saturating_sub(1) as f32;
            doc.scroll_offset = (doc.scroll_offset - scroll.y / row_h).clamp(0.0, max_scroll);
        }
        if scroll.x != 0.0 {
            let max_w = (full_content_w - text_view_w).max(0.0);
            doc.h_scroll_offset = (doc.h_scroll_offset - scroll.x).clamp(0.0, max_w);
        }
    }

    let cursor_before_input = doc.cursor;

    // --- mouse: decide what an in-progress drag targets, at press time ---
    let pointer_pressed = response.hovered() && ui.input(|i| i.pointer.primary_pressed());
    let pointer_down = ui.input(|i| i.pointer.primary_down());
    if pointer_pressed {
        if let Some(pos) = response.interact_pointer_pos() {
            doc.drag_target = if pos.x >= content_rect.max.x {
                DragTarget::VScroll
            } else if pos.y >= content_rect.max.y {
                DragTarget::HScroll
            } else {
                DragTarget::Text
            };
        }
    }
    if !pointer_down {
        doc.drag_target = DragTarget::None;
    }

    let mut goto_definition: Option<usize> = None;
    if let Some(pos) = response.interact_pointer_pos() {
        match doc.drag_target {
            DragTarget::Text => {
                let local_y = (pos.y - content_rect.min.y).max(0.0);
                let row =
                    (first_row + (local_y / row_h) as usize).min(total_rows.saturating_sub(1));
                let col_f = (pos.x - text_x0) / char_w;
                let col = col_f.round().max(0.0) as usize;
                let idx = doc.line_col_to_char(row, col);

                if pointer_pressed {
                    let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    if ctrl {
                        // Ctrl+click asks for "go to definition" rather than
                        // placing the cursor - don't disturb selection/drag
                        // state, just surface the click position.
                        goto_definition = Some(idx);
                        doc.drag_target = DragTarget::None;
                    } else {
                        let old_cursor = doc.cursor;
                        doc.cursor = idx;
                        if ui.input(|i| i.modifiers.shift) {
                            if doc.selection_anchor.is_none() {
                                doc.selection_anchor = Some(old_cursor);
                            }
                        } else {
                            doc.selection_anchor = None;
                        }
                        doc.drag_anchor = Some(idx);
                    }
                } else if pointer_down {
                    if doc.selection_anchor.is_none() {
                        doc.selection_anchor = doc.drag_anchor.or(Some(doc.cursor));
                    }
                    doc.cursor = idx;
                }
            }
            DragTarget::VScroll => {
                let track_h = content_rect.height().max(1.0);
                let scrollable_rows = (total_rows as f32 - visible_rows as f32).max(1.0);
                let frac = ((pos.y - content_rect.min.y) / track_h).clamp(0.0, 1.0);
                doc.scroll_offset = frac * scrollable_rows;
            }
            DragTarget::HScroll => {
                let track_x0 = content_rect.min.x + gutter_w;
                let track_w = (content_rect.width() - gutter_w).max(1.0);
                let scrollable_w = (full_content_w - text_view_w).max(0.0);
                let frac = ((pos.x - track_x0) / track_w).clamp(0.0, 1.0);
                doc.h_scroll_offset = frac * scrollable_w;
            }
            DragTarget::None => {}
        }
    }

    // --- keyboard input ---
    if focused {
        handle_keyboard(ui, doc, clipboard);
    }

    // Keep the cursor within view, but only when something actually moved it
    // this frame (typing, arrow keys, a click placing it). Wheel-scrolling
    // and scrollbar dragging never touch doc.cursor, so without this guard
    // this would immediately snap the view straight back to the cursor's
    // line on every single frame, making manual scrolling look like it does
    // nothing.
    let total_rows = doc.line_count().max(1);
    let (cur_line, _) = doc.char_to_line_col(doc.cursor);
    if doc.cursor != cursor_before_input {
        if (cur_line as f32) < doc.scroll_offset {
            doc.scroll_offset = cur_line as f32;
        } else if (cur_line as f32) >= doc.scroll_offset + visible_rows as f32 - 1.0 {
            doc.scroll_offset = (cur_line as f32) - visible_rows as f32 + 2.0;
        }
        doc.scroll_offset = doc.scroll_offset.max(0.0);
    }
    let first_row = (doc.scroll_offset as usize).min(total_rows.saturating_sub(1));
    let last_row = (first_row + visible_rows).min(total_rows);

    // --- paint rows ---
    let selection = doc.selection_range();
    let lang = doc.language;
    for row in first_row..last_row {
        let y = content_rect.min.y + ((row - first_row) as f32) * row_h;
        painter.text(
            Pos2::new(content_rect.min.x + gutter_w - char_w, y),
            Align2::RIGHT_TOP,
            (row + 1).to_string(),
            metrics.font_id.clone(),
            gutter_color,
        );

        let line = doc.line_text(row);
        let line_start_char = doc.line_col_to_char(row, 0);
        let line_len = line.chars().count();

        if let Some((s, e)) = selection {
            let row_start = line_start_char;
            let row_end = line_start_char + line_len; // exclusive, before the newline

            // Portion of the selection covering this line's visible text.
            let sel_s = s.max(row_start);
            let sel_e = e.min(row_end);
            if sel_s < sel_e {
                let from_col = sel_s - row_start;
                let to_col = sel_e - row_start;
                let x0 = text_x0 + from_col as f32 * char_w;
                let x1 = text_x0 + to_col as f32 * char_w;
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + row_h)),
                    0.0,
                    selection_color,
                );
            }
            // Selection continues past this line's text (covers the newline) -
            // paint a half-width marker so the wrap is visible.
            if s <= row_end && e > row_end {
                let x0 = text_x0 + line_len as f32 * char_w;
                let x1 = x0 + char_w * 0.5;
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + row_h)),
                    0.0,
                    selection_color,
                );
            }
        }

        if !line.is_empty() {
            // Paint the line as non-overlapping colored segments (tokens plus
            // the plain-colored gaps between them) rather than drawing the
            // whole line and then a colored overlay on top - two separate
            // draws of the same glyphs at the same position don't rasterize
            // pixel-identically and show up as a faint double-struck "ghost".
            // Prefer the language server's semantic tokens when we have a
            // fresh set for this exact document version; otherwise fall
            // back to the regex tokenizer (also what covers languages with
            // no LSP server running).
            let lsp_fresh = doc
                .lsp_tokens
                .as_ref()
                .map(|(v, _)| *v == doc.version)
                .unwrap_or(false);
            let tokens: Vec<syntax::Token> = if lsp_fresh {
                doc.lsp_tokens
                    .as_ref()
                    .unwrap()
                    .1
                    .get(row)
                    .cloned()
                    .unwrap_or_default()
            } else {
                let starts_in_comment = doc.line_starts_in_block_comment(row);
                syntax::tokenize_line(&line, lang, starts_in_comment).0
            };
            let chars: Vec<char> = line.chars().collect();
            let mut cursor = 0usize;
            let draw_segment = |from: usize, to: usize, color: Color32| {
                if to <= from {
                    return;
                }
                let substr: String = chars[from..to].iter().collect();
                painter.text(
                    Pos2::new(text_x0 + from as f32 * char_w, y),
                    Align2::LEFT_TOP,
                    substr,
                    metrics.font_id.clone(),
                    color,
                );
            };
            for tok in &tokens {
                draw_segment(cursor, tok.start, text_color);
                let color = theme::token_color(theme, tok.kind).unwrap_or(text_color);
                draw_segment(tok.start, tok.end, color);
                cursor = tok.end;
            }
            draw_segment(cursor, chars.len(), text_color);
        }

        if focused && cur_line == row {
            let (_, cc) = doc.char_to_line_col(doc.cursor);
            let cx = text_x0 + cc as f32 * char_w;
            painter.line_segment(
                [Pos2::new(cx, y), Pos2::new(cx, y + row_h)],
                Stroke::new(2.0_f32, caret_color),
            );
        }
    }

    let hover_pos = ui.input(|i| i.pointer.hover_pos());

    // Ctrl-hover affordance: underline the identifier under the pointer and
    // switch to a pointing-hand cursor, like the ctrl+click-to-definition
    // hint other editors show.
    let ctrl_held = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
    if ctrl_held {
        if let Some(pos) = hover_pos {
            if content_rect.contains(pos) {
                let local_y = (pos.y - content_rect.min.y).max(0.0);
                let row =
                    (first_row + (local_y / row_h) as usize).min(total_rows.saturating_sub(1));
                let col_f = (pos.x - text_x0) / char_w;
                if col_f >= 0.0 {
                    let col = col_f.floor() as usize;
                    let line = doc.line_text(row);
                    let chars: Vec<char> = line.chars().collect();
                    let is_word = |c: char| c.is_alphanumeric() || c == '_';
                    if col < chars.len() && is_word(chars[col]) {
                        let mut start = col;
                        while start > 0 && is_word(chars[start - 1]) {
                            start -= 1;
                        }
                        let mut end = col;
                        while end < chars.len() && is_word(chars[end]) {
                            end += 1;
                        }
                        let y = content_rect.min.y + ((row - first_row) as f32) * row_h;
                        let x0 = text_x0 + start as f32 * char_w;
                        let x1 = text_x0 + end as f32 * char_w;
                        painter.line_segment(
                            [Pos2::new(x0, y + row_h - 2.0), Pos2::new(x1, y + row_h - 2.0)],
                            Stroke::new(1.0_f32, text_color),
                        );
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                    }
                }
            }
        }
    }

    draw_scrollbars(
        &painter,
        rect,
        content_rect,
        gutter_w,
        total_rows,
        visible_rows,
        doc.scroll_offset,
        full_content_w,
        text_view_w,
        doc.h_scroll_offset,
        doc.drag_target,
        hover_pos,
        &sb_colors,
    );

    EditorOutcome {
        response,
        goto_definition,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_scrollbars(
    painter: &egui::Painter,
    rect: Rect,
    content_rect: Rect,
    gutter_w: f32,
    total_rows: usize,
    visible_rows: usize,
    scroll_offset: f32,
    full_content_w: f32,
    text_view_w: f32,
    h_scroll_offset: f32,
    drag_target: DragTarget,
    hover_pos: Option<Pos2>,
    colors: &theme::ScrollbarColors,
) {
    const MIN_THUMB: f32 = 24.0;

    // Vertical
    let v_track = Rect::from_min_max(
        Pos2::new(content_rect.max.x, rect.min.y),
        Pos2::new(rect.max.x, content_rect.max.y),
    );
    painter.rect_filled(v_track, 0.0, colors.track);
    if total_rows > visible_rows {
        let scrollable_rows = (total_rows as f32 - visible_rows as f32).max(1.0);
        let thumb_h = (v_track.height() * (visible_rows as f32 / total_rows as f32))
            .max(MIN_THUMB)
            .min(v_track.height());
        let travel = (v_track.height() - thumb_h).max(0.0);
        let thumb_y = v_track.min.y + (scroll_offset / scrollable_rows).clamp(0.0, 1.0) * travel;
        let thumb_rect = Rect::from_min_size(
            Pos2::new(v_track.min.x + 2.0, thumb_y),
            egui::vec2(v_track.width() - 4.0, thumb_h),
        );
        let hovering = hover_pos.map(|p| v_track.contains(p)).unwrap_or(false);
        let color = if drag_target == DragTarget::VScroll {
            colors.thumb_active
        } else if hovering {
            colors.thumb_hover
        } else {
            colors.thumb
        };
        painter.rect_filled(thumb_rect, 3.0, color);
    }

    // Horizontal
    let h_track = Rect::from_min_max(
        Pos2::new(content_rect.min.x + gutter_w, content_rect.max.y),
        Pos2::new(content_rect.max.x, rect.max.y),
    );
    painter.rect_filled(h_track, 0.0, colors.track);
    if full_content_w > text_view_w {
        let thumb_w = (h_track.width() * (text_view_w / full_content_w))
            .max(MIN_THUMB)
            .min(h_track.width());
        let travel = (h_track.width() - thumb_w).max(0.0);
        let scrollable_w = (full_content_w - text_view_w).max(1.0);
        let thumb_x = h_track.min.x + (h_scroll_offset / scrollable_w).clamp(0.0, 1.0) * travel;
        let thumb_rect = Rect::from_min_size(
            Pos2::new(thumb_x, h_track.min.y + 2.0),
            egui::vec2(thumb_w, h_track.height() - 4.0),
        );
        let hovering = hover_pos.map(|p| h_track.contains(p)).unwrap_or(false);
        let color = if drag_target == DragTarget::HScroll {
            colors.thumb_active
        } else if hovering {
            colors.thumb_hover
        } else {
            colors.thumb
        };
        painter.rect_filled(thumb_rect, 3.0, color);
    }
}

fn handle_keyboard(ui: &mut egui::Ui, doc: &mut Document, clipboard: &mut arboard::Clipboard) {
    let events = ui.input(|i| i.events.clone());
    for event in events {
        match event {
            egui::Event::Text(text) => {
                if doc.has_selection() {
                    doc.delete_selection();
                }
                doc.insert(doc.cursor, &text);
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let ctrl = modifiers.ctrl || modifiers.command;
                match key {
                    Key::Backspace => {
                        if doc.has_selection() {
                            doc.delete_selection();
                        } else if doc.cursor > 0 {
                            let prev = prev_char_boundary(doc, doc.cursor);
                            doc.erase(prev, doc.cursor - prev);
                        }
                    }
                    Key::Delete => {
                        if doc.has_selection() {
                            doc.delete_selection();
                        } else if doc.cursor < doc.len_chars() {
                            let next = next_char_boundary(doc, doc.cursor);
                            doc.erase(doc.cursor, next - doc.cursor);
                        }
                    }
                    Key::Enter => {
                        if doc.has_selection() {
                            doc.delete_selection();
                        }
                        doc.insert(doc.cursor, "\n");
                    }
                    Key::Tab => {
                        if doc.has_selection() {
                            doc.delete_selection();
                        }
                        doc.insert(doc.cursor, "    ");
                    }
                    Key::ArrowLeft => move_cursor(doc, modifiers.shift, |d| {
                        d.cursor = d.cursor.saturating_sub(1);
                    }),
                    Key::ArrowRight => move_cursor(doc, modifiers.shift, |d| {
                        d.cursor = (d.cursor + 1).min(d.len_chars());
                    }),
                    Key::ArrowUp => move_cursor(doc, modifiers.shift, |d| {
                        let (line, col) = d.char_to_line_col(d.cursor);
                        if line > 0 {
                            d.cursor = d.line_col_to_char(line - 1, col);
                        }
                    }),
                    Key::ArrowDown => move_cursor(doc, modifiers.shift, |d| {
                        let (line, col) = d.char_to_line_col(d.cursor);
                        d.cursor = d.line_col_to_char(line + 1, col);
                    }),
                    Key::Home => move_cursor(doc, modifiers.shift, |d| {
                        let (line, _) = d.char_to_line_col(d.cursor);
                        d.cursor = d.line_col_to_char(line, 0);
                    }),
                    Key::End => move_cursor(doc, modifiers.shift, |d| {
                        let (line, _) = d.char_to_line_col(d.cursor);
                        let len = d.line_text(line).chars().count();
                        d.cursor = d.line_col_to_char(line, len);
                    }),
                    Key::A if ctrl => doc.select_all(),
                    Key::C if ctrl => {
                        if doc.has_selection() {
                            let _ = clipboard.set_text(doc.selected_text());
                        }
                    }
                    Key::X if ctrl => {
                        if doc.has_selection() {
                            let _ = clipboard.set_text(doc.selected_text());
                            doc.delete_selection();
                        }
                    }
                    Key::V if ctrl => {
                        if let Ok(text) = clipboard.get_text() {
                            if doc.has_selection() {
                                doc.delete_selection();
                            }
                            doc.insert(doc.cursor, &text);
                        }
                    }
                    Key::Z if ctrl => doc.undo(),
                    Key::Y if ctrl => doc.redo(),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn move_cursor(doc: &mut Document, shift: bool, f: impl FnOnce(&mut Document)) {
    if shift && doc.selection_anchor.is_none() {
        doc.selection_anchor = Some(doc.cursor);
    }
    f(doc);
    if !shift {
        doc.selection_anchor = None;
    }
}

fn prev_char_boundary(_doc: &Document, idx: usize) -> usize {
    idx.saturating_sub(1)
}

fn next_char_boundary(doc: &Document, idx: usize) -> usize {
    (idx + 1).min(doc.len_chars())
}
