use ropey::Rope;
use std::path::PathBuf;

#[derive(Clone)]
enum EditAction {
    Insert { pos: usize, text: String },
    Erase { pos: usize, text: String },
}

/// A single open file: its text buffer, cursor/selection state, and undo history.
pub struct Document {
    pub path: Option<PathBuf>,
    pub display_name: String,
    pub rope: Rope,
    pub dirty: bool,

    pub cursor: usize,           // char offset into rope
    pub selection_anchor: Option<usize>, // other end of selection, if any
    pub drag_anchor: Option<usize>, // transient: press position for an in-progress mouse drag

    pub scroll_offset: f32,      // vertical scroll, in rows
    pub h_scroll_offset: f32,    // horizontal scroll, in pixels
    pub max_line_width_ch: usize,

    undo_stack: Vec<EditAction>,
    redo_stack: Vec<EditAction>,
}

impl Document {
    pub fn new_untitled(counter: usize) -> Self {
        Self {
            path: None,
            display_name: if counter == 0 {
                "untitled".to_string()
            } else {
                format!("untitled-{}", counter)
            },
            rope: Rope::new(),
            dirty: false,
            cursor: 0,
            selection_anchor: None,
            drag_anchor: None,
            scroll_offset: 0.0,
            h_scroll_offset: 0.0,
            max_line_width_ch: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn from_path(path: PathBuf) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(&path)?;
        let rope = Rope::from_str(&text);
        let max_w = rope.lines().map(|l| l.len_chars()).max().unwrap_or(0);
        let display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        Ok(Self {
            path: Some(path),
            display_name,
            rope,
            dirty: false,
            cursor: 0,
            selection_anchor: None,
            drag_anchor: None,
            scroll_offset: 0.0,
            h_scroll_offset: 0.0,
            max_line_width_ch: max_w,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(path) = &self.path {
            std::fs::write(path, self.rope.to_string())?;
            self.dirty = false;
        }
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> std::io::Result<()> {
        std::fs::write(&path, self.rope.to_string())?;
        self.display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        self.path = Some(path);
        self.dirty = false;
        Ok(())
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn line_count(&self) -> usize {
        // ropey counts a trailing empty line after a final '\n'; that matches
        // how we want an empty final line to behave in the editor.
        self.rope.len_lines()
    }

    pub fn line_text(&self, line: usize) -> String {
        if line >= self.rope.len_lines() {
            return String::new();
        }
        let slice = self.rope.line(line);
        let mut s = slice.to_string();
        while s.ends_with('\n') || s.ends_with('\r') {
            s.pop();
        }
        s
    }

    pub fn char_to_line_col(&self, idx: usize) -> (usize, usize) {
        let idx = idx.min(self.rope.len_chars());
        let line = self.rope.char_to_line(idx);
        let line_start = self.rope.line_to_char(line);
        (line, idx - line_start)
    }

    pub fn line_col_to_char(&self, line: usize, col: usize) -> usize {
        let line = line.min(self.rope.len_lines().saturating_sub(1));
        let line_start = self.rope.line_to_char(line);
        let line_len = self.line_text(line).chars().count();
        line_start + col.min(line_len)
    }

    fn track_line_width(&mut self, line: usize) {
        let w = self.line_text(line).chars().count();
        if w > self.max_line_width_ch {
            self.max_line_width_ch = w;
        }
    }

    fn record(&mut self, action: EditAction) {
        self.undo_stack.push(action);
        self.redo_stack.clear();
        self.dirty = true;
    }

    pub fn insert(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let pos = pos.min(self.rope.len_chars());
        self.rope.insert(pos, text);
        let (line, _) = self.char_to_line_col(pos + text.chars().count());
        self.track_line_width(line.saturating_sub(1).max(0));
        self.track_line_width(line);
        self.record(EditAction::Insert {
            pos,
            text: text.to_string(),
        });
        self.cursor = pos + text.chars().count();
        self.selection_anchor = None;
    }

    pub fn erase(&mut self, pos: usize, len: usize) {
        let max = self.rope.len_chars();
        if pos >= max || len == 0 {
            return;
        }
        let end = (pos + len).min(max);
        let erased: String = self.rope.slice(pos..end).to_string();
        self.rope.remove(pos..end);
        self.record(EditAction::Erase { pos, text: erased });
        self.cursor = pos;
        self.selection_anchor = None;
    }

    pub fn has_selection(&self) -> bool {
        matches!(self.selection_anchor, Some(a) if a != self.cursor)
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|a| {
            if a < self.cursor {
                (a, self.cursor)
            } else {
                (self.cursor, a)
            }
        })
    }

    pub fn selected_text(&self) -> String {
        match self.selection_range() {
            Some((s, e)) => self.rope.slice(s..e).to_string(),
            None => String::new(),
        }
    }

    pub fn delete_selection(&mut self) {
        if let Some((s, e)) = self.selection_range() {
            self.erase(s, e - s);
        }
    }

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.rope.len_chars();
    }

    pub fn undo(&mut self) {
        let Some(action) = self.undo_stack.pop() else {
            return;
        };
        match &action {
            EditAction::Insert { pos, text } => {
                let len = text.chars().count();
                self.rope.remove(*pos..*pos + len);
                self.cursor = *pos;
            }
            EditAction::Erase { pos, text } => {
                self.rope.insert(*pos, text);
                self.cursor = pos + text.chars().count();
            }
        }
        self.selection_anchor = None;
        self.redo_stack.push(action);
        self.dirty = true;
    }

    pub fn redo(&mut self) {
        let Some(action) = self.redo_stack.pop() else {
            return;
        };
        match &action {
            EditAction::Insert { pos, text } => {
                self.rope.insert(*pos, text);
                self.cursor = pos + text.chars().count();
            }
            EditAction::Erase { pos, text } => {
                let len = text.chars().count();
                self.rope.remove(*pos..*pos + len);
                self.cursor = *pos;
            }
        }
        self.selection_anchor = None;
        self.undo_stack.push(action);
        self.dirty = true;
    }
}
