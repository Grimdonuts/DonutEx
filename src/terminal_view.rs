use crate::editor_view::EditorMetrics;
use crate::terminal::Terminal;
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke};

/// What happened this frame that the caller (app.rs) needs to act on beyond
/// just repainting - currently only a "the shell just exited" notice, so it
/// can be surfaced in the console instead of the pane just going quiet.
pub struct TerminalOutcome {
    pub response: egui::Response,
    pub exited_message: Option<String>,
}

/// Renders one PTY-backed terminal as a virtualized monospace grid, and - if
/// `focused` - forwards keyboard input to the shell. The counterpart to
/// `editor_view::show`, but for `Terminal` instead of `Document`.
pub fn show(
    ui: &mut egui::Ui,
    term: &mut Terminal,
    metrics: &EditorMetrics,
    focused: bool,
) -> TerminalOutcome {
    let char_w = metrics.char_w;
    let row_h = metrics.row_h;

    let avail = ui.available_size();
    let cols = ((avail.x / char_w).floor() as u16).max(1);
    let rows = ((avail.y / row_h).floor() as u16).max(1);
    term.resize(rows, cols);
    let exited_message = term.pump();

    let (rect, response) = ui.allocate_exact_size(avail, Sense::click_and_drag());

    let visuals = ui.visuals();
    let bg_color = visuals.extreme_bg_color;
    let fg_color = visuals.text_color();
    let caret_color = visuals.strong_text_color();

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, bg_color);

    if response.hovered() {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
    }

    let screen = term.screen();
    let (screen_rows, screen_cols) = screen.size();

    let map_color = |c: vt100::Color, default: Color32| match c {
        vt100::Color::Default => default,
        vt100::Color::Idx(i) => ansi_256_color(i),
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    };

    for row in 0..screen_rows {
        let y = rect.min.y + row as f32 * row_h;

        // Background runs: paint one rect per contiguous span of cells that
        // share a non-default background color (the base fill above already
        // covers the default case).
        let mut bg_run_start: Option<usize> = None;
        let mut bg_run_color = bg_color;
        let flush_bg = |from: usize, to: usize, color: Color32| {
            if to <= from {
                return;
            }
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(rect.min.x + from as f32 * char_w, y),
                    Pos2::new(rect.min.x + to as f32 * char_w, y + row_h),
                ),
                0.0,
                color,
            );
        };
        for col in 0..screen_cols {
            let cell = screen.cell(row, col);
            let (fg, bg, inverse) = cell
                .map(|c| (c.fgcolor(), c.bgcolor(), c.inverse()))
                .unwrap_or((vt100::Color::Default, vt100::Color::Default, false));
            let mut resolved_bg = map_color(bg, bg_color);
            if inverse {
                resolved_bg = map_color(fg, fg_color);
            }
            let is_default = resolved_bg == bg_color;
            match bg_run_start {
                Some(_) if resolved_bg == bg_run_color && !is_default => {}
                Some(start) => {
                    flush_bg(start, col as usize, bg_run_color);
                    bg_run_start = if is_default { None } else { Some(col as usize) };
                    bg_run_color = resolved_bg;
                }
                None if !is_default => {
                    bg_run_start = Some(col as usize);
                    bg_run_color = resolved_bg;
                }
                None => {}
            }
        }
        if let Some(start) = bg_run_start {
            flush_bg(start, screen_cols as usize, bg_run_color);
        }

        // Text runs: group consecutive cells sharing the same effective
        // foreground color into one `painter.text` call each.
        let mut text = String::new();
        let mut run_start_col = 0u16;
        let mut run_color = fg_color;
        let draw_text = |from: u16, text: &str, color: Color32| {
            if text.is_empty() {
                return;
            }
            painter.text(
                Pos2::new(rect.min.x + from as f32 * char_w, y),
                Align2::LEFT_TOP,
                text,
                metrics.font_id.clone(),
                color,
            );
        };
        for col in 0..screen_cols {
            let cell = screen.cell(row, col);
            if cell.map(|c| c.is_wide_continuation()).unwrap_or(false) {
                continue;
            }
            let (fg, bg, inverse, contents) = cell
                .map(|c| (c.fgcolor(), c.bgcolor(), c.inverse(), c.contents().to_string()))
                .unwrap_or((vt100::Color::Default, vt100::Color::Default, false, String::new()));
            let resolved_fg = if inverse {
                map_color(bg, bg_color)
            } else {
                map_color(fg, fg_color)
            };
            let ch = if contents.is_empty() { " " } else { &contents };
            if resolved_fg != run_color && !text.is_empty() {
                draw_text(run_start_col, &text, run_color);
                text.clear();
                run_start_col = col;
            } else if text.is_empty() {
                run_start_col = col;
            }
            run_color = resolved_fg;
            text.push_str(ch);
        }
        draw_text(run_start_col, &text, run_color);
    }

    if !screen.hide_cursor() {
        let (cy, cx) = screen.cursor_position();
        let cursor_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + cx as f32 * char_w, rect.min.y + cy as f32 * row_h),
            egui::vec2(char_w, row_h),
        );
        if focused {
            painter.rect_filled(cursor_rect, 0.0, caret_color);
            if let Some(cell) = screen.cell(cy, cx) {
                if cell.has_contents() {
                    painter.text(
                        cursor_rect.min,
                        Align2::LEFT_TOP,
                        cell.contents(),
                        metrics.font_id.clone(),
                        bg_color,
                    );
                }
            }
        } else {
            painter.rect_stroke(cursor_rect, 0.0, Stroke::new(1.0_f32, caret_color));
        }
    }

    if focused {
        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Text(text) => term.write_input(text.as_bytes()),
                egui::Event::Paste(text) => term.write_input(text.as_bytes()),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if let Some(bytes) = key_event_bytes(key, modifiers) {
                        term.write_input(&bytes);
                    }
                }
                _ => {}
            }
        }
    }

    if term.is_alive() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }

    TerminalOutcome {
        response,
        exited_message,
    }
}

/// Translates a non-text key press (plus modifiers) into the byte sequence a
/// real terminal would send, for the keys that produce no `Event::Text`:
/// control characters (ctrl+letter) and cursor/editing keys (their VT100/ANSI
/// escape sequences). Printable characters instead arrive as `Event::Text`
/// and are forwarded as-is by the caller.
fn key_event_bytes(key: egui::Key, modifiers: egui::Modifiers) -> Option<Vec<u8>> {
    use egui::Key;

    if modifiers.ctrl || modifiers.command {
        let byte = match key {
            Key::A => Some(0x01),
            Key::B => Some(0x02),
            Key::C => Some(0x03),
            Key::D => Some(0x04),
            Key::E => Some(0x05),
            Key::F => Some(0x06),
            Key::G => Some(0x07),
            Key::H => Some(0x08),
            Key::I => Some(0x09),
            Key::J => Some(0x0A),
            Key::K => Some(0x0B),
            Key::L => Some(0x0C),
            Key::M => Some(0x0D),
            Key::N => Some(0x0E),
            Key::O => Some(0x0F),
            Key::P => Some(0x10),
            Key::Q => Some(0x11),
            Key::R => Some(0x12),
            Key::S => Some(0x13),
            Key::T => Some(0x14),
            Key::U => Some(0x15),
            Key::V => Some(0x16),
            Key::W => Some(0x17),
            Key::X => Some(0x18),
            Key::Y => Some(0x19),
            Key::Z => Some(0x1A),
            _ => None,
        };
        if let Some(b) = byte {
            return Some(vec![b]);
        }
    }

    match key {
        Key::Enter => Some(b"\r".to_vec()),
        Key::Backspace => Some(vec![0x7f]),
        Key::Tab => Some(b"\t".to_vec()),
        Key::Escape => Some(vec![0x1b]),
        Key::ArrowUp => Some(b"\x1b[A".to_vec()),
        Key::ArrowDown => Some(b"\x1b[B".to_vec()),
        Key::ArrowRight => Some(b"\x1b[C".to_vec()),
        Key::ArrowLeft => Some(b"\x1b[D".to_vec()),
        Key::Home => Some(b"\x1b[H".to_vec()),
        Key::End => Some(b"\x1b[F".to_vec()),
        Key::PageUp => Some(b"\x1b[5~".to_vec()),
        Key::PageDown => Some(b"\x1b[6~".to_vec()),
        Key::Delete => Some(b"\x1b[3~".to_vec()),
        Key::Insert => Some(b"\x1b[2~".to_vec()),
        _ => None,
    }
}

/// Standard xterm 256-color palette: 16 named colors, a 6x6x6 color cube,
/// then a 24-step grayscale ramp.
fn ansi_256_color(i: u8) -> Color32 {
    const BASE16: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 49, 49),
        (13, 188, 121),
        (229, 229, 16),
        (36, 114, 200),
        (188, 63, 188),
        (17, 168, 205),
        (229, 229, 229),
        (102, 102, 102),
        (241, 76, 76),
        (35, 209, 139),
        (245, 245, 67),
        (59, 142, 234),
        (214, 112, 214),
        (41, 184, 219),
        (255, 255, 255),
    ];
    if i < 16 {
        let (r, g, b) = BASE16[i as usize];
        Color32::from_rgb(r, g, b)
    } else if i < 232 {
        let i = i - 16;
        let r = i / 36;
        let g = (i % 36) / 6;
        let b = i % 6;
        let scale = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
        Color32::from_rgb(scale(r), scale(g), scale(b))
    } else {
        let level = 8 + (i - 232) * 10;
        Color32::from_rgb(level, level, level)
    }
}
