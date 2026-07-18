use mlua::{Lua, MultiValue};
use std::path::Path;
use std::sync::mpsc;

/// Messages plugins can send back into the editor. Kept as a plain channel so
/// the Lua VM never needs a reference back into app state (which would fight
/// the borrow checker once callbacks are involved).
pub enum PluginMessage {
    Output(String),
}

pub struct PluginEngine {
    lua: Lua,
    rx: mpsc::Receiver<PluginMessage>,
    // Kept alive so the Sender clones handed to Lua closures stay valid.
    _tx: mpsc::Sender<PluginMessage>,
    pub loaded_files: Vec<String>,
}

impl PluginEngine {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let lua = Lua::new();
        let mut engine = Self {
            lua,
            rx,
            _tx: tx,
            loaded_files: Vec::new(),
        };
        engine.register_bridges();
        engine
    }

    /// Tear down the Lua VM and build a fresh one, then reload every plugin
    /// file from disk. This is the "hot reload" entry point: since plugins
    /// are plain interpreted Lua with no compile step, discarding old global
    /// state and re-running the scripts is enough to pick up edits.
    pub fn reload(&mut self, plugins_dir: &Path) -> Vec<String> {
        let (tx, rx) = mpsc::channel();
        self.lua = Lua::new();
        self.rx = rx;
        self._tx = tx;
        self.loaded_files.clear();
        self.register_bridges();
        self.load_dir(plugins_dir)
    }

    fn register_bridges(&mut self) {
        let tx = self._tx.clone();
        let globals = self.lua.globals();

        // print(...) -> routed to the console/output panel instead of stdout
        let print_tx = tx.clone();
        let print_fn = self
            .lua
            .create_function(move |lua, args: MultiValue| {
                let tostring: mlua::Function = lua.globals().get("tostring")?;
                let mut parts = Vec::new();
                for v in args.iter() {
                    let s: String = tostring.call(v.clone())?;
                    parts.push(s);
                }
                let _ = print_tx.send(PluginMessage::Output(parts.join("\t")));
                Ok(())
            })
            .expect("create print fn");
        globals.set("print", print_fn).ok();

        // hook registry: register_hook("on_render", fn) etc.
        self.lua
            .load(
                r#"
                hooks = { on_text_input = {}, on_render = {} }
                function register_hook(event, fn)
                    if hooks[event] then table.insert(hooks[event], fn) end
                end
                "#,
            )
            .exec()
            .expect("install hook registry");
    }

    fn load_dir(&mut self, dir: &Path) -> Vec<String> {
        let mut log = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            log.push(format!("plugins dir not found: {}", dir.display()));
            return log;
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "lua").unwrap_or(false))
            .collect();
        paths.sort();

        for path in paths {
            match std::fs::read_to_string(&path) {
                Ok(src) => match self.lua.load(&src).set_name(path.to_string_lossy()).exec() {
                    Ok(()) => {
                        self.loaded_files.push(path.display().to_string());
                        log.push(format!("loaded plugin: {}", path.display()));
                    }
                    Err(e) => log.push(format!("error in {}: {}", path.display(), e)),
                },
                Err(e) => log.push(format!("could not read {}: {}", path.display(), e)),
            }
        }
        log
    }

    pub fn run_hook(&self, name: &str) {
        let code = format!(
            "for _, fn in ipairs(hooks.{}) do local ok, err = pcall(fn); if not ok then print('hook error: ' .. tostring(err)) end end",
            name
        );
        let _ = self.lua.load(&code).exec();
    }

    /// Drain any messages plugins queued up since the last poll (e.g. print output).
    pub fn drain_messages(&self) -> Vec<PluginMessage> {
        self.rx.try_iter().collect()
    }
}
