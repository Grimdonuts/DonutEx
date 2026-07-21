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
}

impl LspManager {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            clients: HashMap::new(),
            project_root,
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

    fn try_start(&self, lang: Language) -> ClientSlot {
        for (cmd, args) in servers::candidates(lang) {
            if let Some(path) = servers::find_on_path(cmd) {
                match LspClient::spawn(&path, args, &self.project_root) {
                    Ok(client) => return ClientSlot::Running(Box::new(client)),
                    Err(_) => continue,
                }
            }
        }
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
        let mut events = Vec::new();
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
