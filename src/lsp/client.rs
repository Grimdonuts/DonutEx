use super::semantic;
use super::transport::LspTransport;
use super::uri::{path_to_uri, uri_to_path};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Events surfaced to the app once per poll. Kept flat and app-agnostic
/// (paths, not `Document`s) so `LspManager`/`LspClient` don't need to know
/// about editor state - matches the `PluginMessage` channel pattern already
/// used for the Lua plugin engine.
pub enum LspEvent {
    SemanticTokens {
        doc: PathBuf,
        version: i32,
        by_line: Vec<Vec<crate::syntax::Token>>,
    },
    Definition {
        locations: Vec<(PathBuf, u32, u32)>, // path, line, UTF-16 character
    },
    Log(String),
}

enum PendingKind {
    Initialize,
    SemanticTokens {
        doc: PathBuf,
        version: i32,
        lines: Vec<String>,
    },
    Definition,
}

/// One language-server process plus enough request/response bookkeeping to
/// drive it from a synchronous, once-per-frame `poll()`. Requests we send
/// get an id we stash in `pending`; when the response arrives on the
/// transport's channel we look the id back up to know how to interpret it.
pub struct LspClient {
    transport: LspTransport,
    name: String,
    next_id: i64,
    pending: HashMap<i64, PendingKind>,
    initialized: bool,
    /// Set when `initialize` itself comes back as an error (e.g.
    /// typescript-language-server refusing to start because no `typescript`
    /// package is resolvable from the workspace). Without this, `initialized`
    /// stays false forever and every later did_open/semanticTokens/definition
    /// call just piles into the `pending_*` queues below with no way to ever
    /// flush them - silently "the LSP does nothing" for the rest of the
    /// session. `LspManager::poll` checks this to retire the client instead.
    dead: bool,
    announced_startup: bool,
    legend: Vec<String>,
    open_docs: HashSet<PathBuf>,
    // (path, language_id, text, version) - buffered until `initialize` completes.
    pending_opens: Vec<(PathBuf, String, String, i32)>,
    // (path, version, lines) - same idea, for semantic-token requests.
    pending_token_requests: Vec<(PathBuf, i32, Vec<String>)>,
    // (path, line, character) - same idea, for goto-definition requests. A
    // ctrl+click that lands while the server is still spawning/handshaking
    // used to be silently dropped instead of queued like the other request
    // kinds above, which is exactly what made goto-definition look broken
    // right after opening a file.
    pending_definition_requests: Vec<(PathBuf, u32, u32)>,
}

impl LspClient {
    pub fn spawn(cmd: &Path, args: &[&str], root: &Path) -> std::io::Result<Self> {
        let transport = LspTransport::spawn(cmd, args)?;
        let name = cmd
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| cmd.display().to_string());
        let mut client = Self {
            transport,
            name,
            next_id: 0,
            pending: HashMap::new(),
            initialized: false,
            dead: false,
            announced_startup: false,
            legend: Vec::new(),
            open_docs: HashSet::new(),
            pending_opens: Vec::new(),
            pending_token_requests: Vec::new(),
            pending_definition_requests: Vec::new(),
        };

        let root_uri = path_to_uri(root).map(|u| u.as_str().to_string());
        let params = json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "synchronization": { "didSave": true },
                    "semanticTokens": {
                        "requests": { "full": true },
                        "tokenTypes": [],
                        "tokenModifiers": [],
                        "formats": ["relative"],
                    },
                    "definition": { "linkSupport": true },
                },
            },
        });
        client.request("initialize", params, PendingKind::Initialize);
        Ok(client)
    }

    fn request(&mut self, method: &str, params: Value, kind: PendingKind) {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, kind);
        let msg = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        let _ = self.transport.send(&msg);
    }

    fn notify(&mut self, method: &str, params: Value) {
        let msg = json!({"jsonrpc":"2.0","method":method,"params":params});
        let _ = self.transport.send(&msg);
    }

    pub fn did_open(&mut self, path: PathBuf, language_id: &str, text: &str, version: i32) {
        if !self.initialized {
            self.pending_opens
                .push((path, language_id.to_string(), text.to_string(), version));
            return;
        }
        self.send_did_open(&path, language_id, text, version);
    }

    fn send_did_open(&mut self, path: &Path, language_id: &str, text: &str, version: i32) {
        let Some(uri) = path_to_uri(path) else { return };
        self.open_docs.insert(path.to_path_buf());
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri.as_str(),
                    "languageId": language_id,
                    "version": version,
                    "text": text,
                }
            }),
        );
    }

    pub fn did_change(&mut self, path: &Path, text: &str, version: i32) {
        if !self.initialized {
            // The doc's didOpen is itself still buffered (see `did_open`) -
            // fold this edit into it rather than dropping it, since the
            // open hasn't gone out yet and can just carry the latest text.
            if let Some(entry) = self.pending_opens.iter_mut().find(|(p, ..)| p == path) {
                entry.2 = text.to_string();
                entry.3 = version;
            }
            return;
        }
        if !self.open_docs.contains(path) {
            return;
        }
        let Some(uri) = path_to_uri(path) else { return };
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri.as_str(), "version": version },
                "contentChanges": [ { "text": text } ],
            }),
        );
    }

    pub fn did_close(&mut self, path: &Path) {
        if !self.open_docs.remove(path) {
            return;
        }
        let Some(uri) = path_to_uri(path) else { return };
        self.notify(
            "textDocument/didClose",
            json!({ "textDocument": { "uri": uri.as_str() } }),
        );
    }

    pub fn request_semantic_tokens(&mut self, path: &Path, version: i32, lines: Vec<String>) {
        if !self.initialized {
            // The `initialize` round trip (spawn + handshake) can easily
            // outlast the app's first sync attempt for a freshly opened
            // doc. Buffer and flush from `handle_initialize_result`,
            // mirroring `pending_opens` - otherwise the request is just
            // dropped, and since the caller (app.rs) marks the document as
            // synced regardless, it would never be retried.
            self.pending_token_requests
                .push((path.to_path_buf(), version, lines));
            return;
        }
        self.send_semantic_tokens_request(path, version, lines);
    }

    fn send_semantic_tokens_request(&mut self, path: &Path, version: i32, lines: Vec<String>) {
        if self.legend.is_empty() {
            return;
        }
        let Some(uri) = path_to_uri(path) else { return };
        self.request(
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": uri.as_str() } }),
            PendingKind::SemanticTokens {
                doc: path.to_path_buf(),
                version,
                lines,
            },
        );
    }

    pub fn request_definition(&mut self, path: &Path, line: u32, character: u32) {
        if !self.initialized {
            self.pending_definition_requests
                .push((path.to_path_buf(), line, character));
            return;
        }
        self.send_definition_request(path, line, character);
    }

    fn send_definition_request(&mut self, path: &Path, line: u32, character: u32) {
        let Some(uri) = path_to_uri(path) else { return };
        self.request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri.as_str() },
                "position": { "line": line, "character": character },
            }),
            PendingKind::Definition,
        );
    }

    /// True once `initialize` has come back as an error. The manager retires
    /// clients in this state instead of leaving them running forever with
    /// every request silently swallowed.
    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Drains everything the reader thread has decoded since the last call
    /// and turns it into editor-facing events.
    pub fn poll(&mut self) -> Vec<LspEvent> {
        let mut events = Vec::new();
        if !self.announced_startup {
            self.announced_startup = true;
            events.push(LspEvent::Log(format!("starting {}...", self.name)));
        }
        let messages: Vec<Value> = self.transport.rx.try_iter().collect();
        for msg in messages {
            self.handle_message(msg, &mut events);
        }
        events
    }

    fn handle_message(&mut self, msg: Value, events: &mut Vec<LspEvent>) {
        let has_method = msg.get("method").and_then(|m| m.as_str());
        if let Some(id) = msg.get("id").cloned() {
            match has_method {
                Some(method) => self.handle_server_request(id, method, msg.get("params")),
                None => {
                    if let Some(id_num) = id.as_i64() {
                        if let Some(kind) = self.pending.remove(&id_num) {
                            self.handle_response(kind, msg, events);
                        }
                    }
                }
            }
        } else if let Some(method) = has_method {
            self.handle_notification(method, msg.get("params"), events);
        }
    }

    fn handle_response(&mut self, kind: PendingKind, msg: Value, events: &mut Vec<LspEvent>) {
        if let Some(err) = msg.get("error") {
            if matches!(kind, PendingKind::Initialize) {
                self.dead = true;
                self.pending_opens.clear();
                self.pending_token_requests.clear();
                self.pending_definition_requests.clear();
                events.push(LspEvent::Log(format!(
                    "{} failed to start: {}",
                    self.name, err
                )));
            } else {
                events.push(LspEvent::Log(format!("{}: {}", self.name, err)));
            }
            return;
        }
        let result = msg.get("result").cloned().unwrap_or(Value::Null);
        match kind {
            PendingKind::Initialize => self.handle_initialize_result(result, events),
            PendingKind::SemanticTokens { doc, version, lines } => {
                if result.is_null() {
                    return;
                }
                if let Ok(res) = serde_json::from_value::<lsp_types::SemanticTokensResult>(result)
                {
                    let data = match res {
                        lsp_types::SemanticTokensResult::Tokens(t) => t.data,
                        lsp_types::SemanticTokensResult::Partial(p) => p.data,
                    };
                    let by_line = semantic::decode(&self.legend, &data, &lines);
                    events.push(LspEvent::SemanticTokens { doc, version, by_line });
                }
            }
            PendingKind::Definition => {
                if result.is_null() {
                    events.push(LspEvent::Log(format!(
                        "{}: no definition found",
                        self.name
                    )));
                    return;
                }
                if let Ok(resp) =
                    serde_json::from_value::<lsp_types::GotoDefinitionResponse>(result)
                {
                    let locations = match resp {
                        lsp_types::GotoDefinitionResponse::Scalar(loc) => {
                            loc_to_tuple(&loc).into_iter().collect()
                        }
                        lsp_types::GotoDefinitionResponse::Array(locs) => {
                            locs.iter().filter_map(loc_to_tuple).collect()
                        }
                        lsp_types::GotoDefinitionResponse::Link(links) => links
                            .iter()
                            .filter_map(|l| {
                                uri_to_path(&l.target_uri).map(|p| {
                                    (
                                        p,
                                        l.target_selection_range.start.line,
                                        l.target_selection_range.start.character,
                                    )
                                })
                            })
                            .collect(),
                    };
                    events.push(LspEvent::Definition { locations });
                }
            }
        }
    }

    fn handle_initialize_result(&mut self, result: Value, events: &mut Vec<LspEvent>) {
        if let Ok(init) = serde_json::from_value::<lsp_types::InitializeResult>(result) {
            if let Some(provider) = init.capabilities.semantic_tokens_provider {
                let legend = match provider {
                    lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(o) => {
                        o.legend
                    }
                    lsp_types::SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(o) => {
                        o.semantic_tokens_options.legend
                    }
                };
                self.legend = legend
                    .token_types
                    .iter()
                    .map(|t| t.as_str().to_string())
                    .collect();
            }
        }
        self.initialized = true;
        self.notify("initialized", json!({}));
        events.push(LspEvent::Log(format!("{} ready", self.name)));

        let opens = std::mem::take(&mut self.pending_opens);
        for (path, language_id, text, version) in opens {
            self.send_did_open(&path, &language_id, &text, version);
        }
        let token_requests = std::mem::take(&mut self.pending_token_requests);
        for (path, version, lines) in token_requests {
            self.send_semantic_tokens_request(&path, version, lines);
        }
        let definition_requests = std::mem::take(&mut self.pending_definition_requests);
        for (path, line, character) in definition_requests {
            self.send_definition_request(&path, line, character);
        }
    }

    fn handle_server_request(&mut self, id: Value, method: &str, params: Option<&Value>) {
        let result = match method {
            "workspace/configuration" => {
                let n = params
                    .and_then(|p| p.get("items"))
                    .and_then(|i| i.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                Value::Array(vec![Value::Null; n])
            }
            "workspace/applyEdit" => json!({ "applied": false }),
            _ => Value::Null,
        };
        let response = json!({"jsonrpc":"2.0","id":id,"result":result});
        let _ = self.transport.send(&response);
    }

    fn handle_notification(&mut self, method: &str, params: Option<&Value>, events: &mut Vec<LspEvent>) {
        if matches!(method, "window/logMessage" | "window/showMessage") {
            if let Some(text) = params.and_then(|p| p.get("message")).and_then(|m| m.as_str()) {
                events.push(LspEvent::Log(text.to_string()));
            }
        }
    }
}

fn loc_to_tuple(loc: &lsp_types::Location) -> Option<(PathBuf, u32, u32)> {
    let path = uri_to_path(&loc.uri)?;
    Some((path, loc.range.start.line, loc.range.start.character))
}
