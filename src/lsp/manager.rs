use super::client::{LspClient, LspEvent};
use super::servers;
use crate::syntax::Language;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

enum ClientSlot {
    Running(Box<LspClient>),
    /// No server binary was found (or it failed to spawn) for this
    /// language; remembered so we don't retry the PATH search every frame.
    Unavailable,
}

pub struct LspManager {
    clients: HashMap<Language, ClientSlot>,
    project_root: PathBuf,
    // Drained into `LspEvent::Log`s on the next `poll`. Needed because
    // `try_start` (called from `ensure_client`, itself called from request
    // methods that don't take a `Vec<LspEvent>`) has nowhere else to surface
    // "no server binary found" - previously that case produced zero
    // feedback at all, which looked identical to the editor just not trying.
    pending_logs: Vec<String>,
}

impl LspManager {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            clients: HashMap::new(),
            project_root,
            pending_logs: Vec::new(),
        }
    }

    fn ensure_client(&mut self, lang: Language) -> Option<&mut LspClient> {
        if !self.clients.contains_key(&lang) {
            let slot = self.try_start(lang);
            self.clients.insert(lang, slot);
        }
        match self.clients.get_mut(&lang)? {
            ClientSlot::Running(c) => Some(c),
            ClientSlot::Unavailable => None,
        }
    }

    fn try_start(&mut self, lang: Language) -> ClientSlot {
        let candidates = servers::candidates(lang);
        // Empty candidate list means "no LSP support attempted for this
        // language" by design (see `servers::candidates`) - not worth a log.
        if candidates.is_empty() {
            return ClientSlot::Unavailable;
        }
        for (cmd, args) in candidates {
            if let Some(path) = servers::find_binary(cmd, &self.project_root) {
                match LspClient::spawn(&path, args, &self.project_root) {
                    Ok(client) => return ClientSlot::Running(Box::new(client)),
                    Err(e) => {
                        self.pending_logs
                            .push(format!("failed to launch {}: {}", cmd, e));
                    }
                }
            }
        }
        let tried: Vec<&str> = candidates.iter().map(|(cmd, _)| *cmd).collect();
        self.pending_logs.push(format!(
            "no language server on PATH for this file type (tried: {})",
            tried.join(", ")
        ));
        ClientSlot::Unavailable
    }

    pub fn did_open(&mut self, path: &Path, lang: Language, text: &str, version: i32) {
        if let Some(c) = self.ensure_client(lang) {
            c.did_open(path.to_path_buf(), servers::language_id(lang, path), text, version);
        }
    }

    pub fn did_change(&mut self, path: &Path, lang: Language, text: &str, version: i32) {
        if let Some(c) = self.ensure_client(lang) {
            c.did_change(path, text, version);
        }
    }

    pub fn did_close(&mut self, path: &Path, lang: Language) {
        if let Some(ClientSlot::Running(c)) = self.clients.get_mut(&lang) {
            c.did_close(path);
        }
    }

    pub fn request_semantic_tokens(
        &mut self,
        path: &Path,
        lang: Language,
        version: i32,
        lines: Vec<String>,
    ) {
        if let Some(c) = self.ensure_client(lang) {
            c.request_semantic_tokens(path, version, lines);
        }
    }

    pub fn request_definition(&mut self, path: &Path, lang: Language, line: u32, character: u32) {
        if let Some(c) = self.ensure_client(lang) {
            c.request_definition(path, line, character);
        }
    }

    pub fn poll(&mut self) -> Vec<LspEvent> {
        let mut events: Vec<LspEvent> = self
            .pending_logs
            .drain(..)
            .map(LspEvent::Log)
            .collect();
        let mut newly_dead = Vec::new();
        for (&lang, slot) in self.clients.iter_mut() {
            if let ClientSlot::Running(c) = slot {
                events.extend(c.poll());
                if c.is_dead() {
                    newly_dead.push(lang);
                }
            }
        }
        // Retire clients whose `initialize` failed rather than leaving them
        // as zombies that silently swallow every future request for this
        // language - see the `dead` field comment in `LspClient`.
        for lang in newly_dead {
            self.clients.insert(lang, ClientSlot::Unavailable);
        }
        events
    }
}
