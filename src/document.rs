use crate::syntax::{Language, Token};
use ropey::Rope;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone)]
enum EditAction {
    Insert { pos: usize, text: String },
    Erase { pos: usize, text: String },
}

/// What an in-progress mouse drag is manipulating, decided at press time and
/// held for the duration of the drag so the pointer wandering over another
/// region (e.g. off the scrollbar) mid-drag doesn't change its meaning.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    None,
    Text,
    VScroll,
    HScroll,
}

/// A single open file: its text buffer, cursor/selection state, and undo history.
pub struct Document {
    pub path: Option<PathBuf>,
    pub display_name: String,
    pub rope: Rope,
    pub dirty: bool,
    pub language: Language,

    pub cursor: usize,           // char offset into rope
    pub selection_anchor: Option<usize>, // other end of selection, if any
    pub drag_anchor: Option<usize>, // transient: press position for an in-progress mouse drag
    pub drag_target: DragTarget,

    pub scroll_offset: f32,      // vertical scroll, in rows
    pub h_scroll_offset: f32,    // horizontal scroll, in pixels
    pub max_line_width_ch: usize,

    // Cache of "does line N start inside an unterminated block comment",
    // used so syntax highlighting only has to look at the lines actually
    // being painted instead of rescanning from the top of the file every
    // frame. Invalidated (set to None) on any edit; lazily rebuilt in
    // `ensure_comment_state`, which costs O(file length) but only runs once
    // per edit rather than once per frame.
    comment_state: Option<Vec<bool>>,

    undo_stack: Vec<EditAction>,
    redo_stack: Vec<EditAction>,

    /// Bumped on every edit/undo/redo; the LSP sync + semantic-tokens
    /// pipeline in `app.rs` uses this to know when a re-sync is needed and
    /// to discard a semantic-tokens response that arrived for a version
    /// that's since been edited past.
    pub version: i32,
    /// Version last sent to the language server via didOpen/didChange.
    /// -1 means "never opened with the LSP".
    pub lsp_synced_version: i32,
    /// Per-line semantic tokens from the language server, paired with the
    /// document version they describe (checked against `version` before
    /// painting so a stale response never gets drawn over newer text).
    pub lsp_tokens: Option<(i32, Vec<Vec<Token>>)>,
    /// Debounce clock for LSP didChange/semanticTokens requests, so we
    /// don't fire one on every keystroke.
    pub last_edit_at: Instant,

    /// Completion items from the language server, tagged with the document
    /// `version` and `cursor` offset they were computed for. Kept alongside
    /// (rather than replacing) `version`/`cursor` comparisons happen in
    /// `active_completions` - a stale pair (from a keystroke since the
    /// request went out) hides the popup instead of showing a wrong list.
    pub lsp_completions: Option<(i32, usize, Vec<crate::lsp::CompletionItem>)>,
    /// Index into the currently active completion list the popup highlights.
    pub completion_selected: usize,
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
            language: Language::PlainText,
            cursor: 0,
            selection_anchor: None,
            drag_anchor: None,
            drag_target: DragTarget::None,
            scroll_offset: 0.0,
            h_scroll_offset: 0.0,
            max_line_width_ch: 0,
            comment_state: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            version: 0,
            lsp_synced_version: -1,
            lsp_tokens: None,
            last_edit_at: Instant::now(),
            lsp_completions: None,
            completion_selected: 0,
        }
    }

    pub fn from_path(path: PathBuf) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(&path)?;
        // Canonicalize so LSP file:// URIs (and tab-dedup path comparisons)
        // are always absolute, even when opened via a relative CLI arg.
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        let rope = Rope::from_str(&text);
        let max_w = rope.lines().map(|l| l.len_chars()).max().unwrap_or(0);
        let display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        let language = Language::from_path(Some(path.as_path()));
        Ok(Self {
            path: Some(path),
            display_name,
            rope,
            dirty: false,
            language,
            cursor: 0,
            selection_anchor: None,
            drag_anchor: None,
            drag_target: DragTarget::None,
            scroll_offset: 0.0,
            h_scroll_offset: 0.0,
            max_line_width_ch: max_w,
            comment_state: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            version: 0,
            lsp_synced_version: -1,
            lsp_tokens: None,
            last_edit_at: Instant::now(),
            lsp_completions: None,
            completion_selected: 0,
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
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        self.display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        self.language = Language::from_path(Some(path.as_path()));
        self.path = Some(path);
        self.dirty = false;
        self.lsp_synced_version = -1;
        self.lsp_tokens = None;
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
        self.comment_state = None;
        self.version += 1;
        self.last_edit_at = Instant::now();
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
        self.comment_state = None;
        self.version += 1;
        self.last_edit_at = Instant::now();
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
        self.comment_state = None;
        self.version += 1;
        self.last_edit_at = Instant::now();
    }

    /// Returns whether `line` starts inside an unterminated block comment,
    /// rebuilding the whole-file cache first if it was invalidated by an
    /// edit since the last call.
    pub fn line_starts_in_block_comment(&mut self, line: usize) -> bool {
        if self.comment_state.is_none() {
            self.rebuild_comment_state();
        }
        self.comment_state
            .as_ref()
            .and_then(|v| v.get(line).copied())
            .unwrap_or(false)
    }

    /// Char offset -> LSP `(line, character)`, where `character` is a
    /// UTF-16 code-unit offset into the line as the protocol requires.
    pub fn char_to_lsp_line_col(&self, idx: usize) -> (u32, u32) {
        let (line, col) = self.char_to_line_col(idx);
        let line_text = self.line_text(line);
        let utf16_col = crate::lsp::char_offset_to_utf16_offset(&line_text, col);
        (line as u32, utf16_col as u32)
    }

    /// The inverse of `char_to_lsp_line_col`.
    pub fn lsp_line_col_to_char(&self, line: u32, utf16_character: u32) -> usize {
        let line_text = self.line_text(line as usize);
        let col = crate::lsp::utf16_offset_to_char_offset(&line_text, utf16_character as usize);
        self.line_col_to_char(line as usize, col)
    }

    /// The completion list to show right now, if any. Requires an exact
    /// match on both `version` and `cursor` against what the request was
    /// fired for - a keystroke or cursor move since then means the list no
    /// longer describes the current prefix/position, so it's hidden rather
    /// than shown stale.
    pub fn active_completions(&self) -> Option<&[crate::lsp::CompletionItem]> {
        self.lsp_completions.as_ref().and_then(|(v, c, items)| {
            (*v == self.version && *c == self.cursor && !items.is_empty())
                .then_some(items.as_slice())
        })
    }

    /// Start of the identifier-ish word ending at the cursor, so accepting a
    /// completion can replace the whole typed prefix instead of inserting at
    /// the cursor and leaving the prefix duplicated in front of it.
    pub fn current_word_start(&self) -> usize {
        let (line, col) = self.char_to_line_col(self.cursor);
        let text = self.line_text(line);
        let chars: Vec<char> = text.chars().collect();
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut start = col.min(chars.len());
        while start > 0 && is_word(chars[start - 1]) {
            start -= 1;
        }
        self.line_col_to_char(line, start)
    }

    /// Replaces the current word prefix with `insert_text` and closes the
    /// completion popup.
    pub fn accept_completion(&mut self, insert_text: &str) {
        let start = self.current_word_start();
        let end = self.cursor;
        if end > start {
            self.erase(start, end - start);
        }
        self.insert(start, insert_text);
        self.lsp_completions = None;
    }

    fn rebuild_comment_state(&mut self) {
        let total = self.line_count();
        let mut states = Vec::with_capacity(total);
        let mut in_block = false;
        for line in 0..total {
            states.push(in_block);
            let text = self.line_text(line);
            let (_tokens, ends_in_block) =
                crate::syntax::tokenize_line(&text, self.language, in_block);
            in_block = ends_in_block;
        }
        self.comment_state = Some(states);
    }
}
