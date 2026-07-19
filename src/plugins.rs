use crate::theme::{self, Theme};
use mlua::{Lua, MultiValue};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
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
    // Themes registered by plugin files via `register_theme(...)` during the
    // most recent reload. A theme plugin is just a .lua file that calls this
    // instead of (or alongside) `register_hook`.
    loaded_themes: Rc<RefCell<Vec<Theme>>>,
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
            loaded_themes: Rc::new(RefCell::new(Vec::new())),
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
        self.loaded_themes = Rc::new(RefCell::new(Vec::new()));
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

        // register_theme({ name = ..., colors = {...}, syntax = {...} }) ->
        // a theme plugin's entry point. See plugins/theme_*.lua for the
        // table schema.
        let themes = self.loaded_themes.clone();
        let register_theme_fn = self
            .lua
            .create_function(move |_, t: mlua::Table| {
                let theme = theme_from_lua_table(&t)?;
                themes.borrow_mut().push(theme);
                Ok(())
            })
            .expect("create register_theme fn");
        globals.set("register_theme", register_theme_fn).ok();

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

    /// Themes registered by `register_theme(...)` calls during the most
    /// recent `reload()`.
    pub fn take_themes(&self) -> Vec<Theme> {
        self.loaded_themes.borrow().clone()
    }
}

fn theme_from_lua_table(t: &mlua::Table) -> mlua::Result<Theme> {
    let name: String = t.get("name")?;
    let colors: mlua::Table = t.get("colors")?;
    let syntax: mlua::Table = t.get("syntax")?;
    let fallback = Theme::built_in_dark();

    let color = |table: &mlua::Table, key: &str, default: eframe::egui::Color32| {
        table
            .get::<_, String>(key)
            .ok()
            .and_then(|s| theme::parse_hex(&s))
            .unwrap_or(default)
    };
    let optional_color = |table: &mlua::Table, key: &str| {
        table
            .get::<_, String>(key)
            .ok()
            .and_then(|s| theme::parse_hex(&s))
    };

    Ok(Theme {
        name,

        background: color(&colors, "background", fallback.background),
        panel: color(&colors, "panel", fallback.panel),
        text: color(&colors, "text", fallback.text),
        selection_bg: color(&colors, "selection_bg", fallback.selection_bg),
        selection_border: color(&colors, "selection_border", fallback.selection_border),
        accent: color(&colors, "accent", fallback.accent),
        scrollbar_track: color(&colors, "scrollbar_track", fallback.scrollbar_track),
        scrollbar_thumb: color(&colors, "scrollbar_thumb", fallback.scrollbar_thumb),

        keyword: optional_color(&syntax, "keyword"),
        type_: optional_color(&syntax, "type"),
        string: optional_color(&syntax, "string"),
        number: optional_color(&syntax, "number"),
        comment: optional_color(&syntax, "comment"),
        function: optional_color(&syntax, "func"),
        macro_: optional_color(&syntax, "macro"),
        variable: optional_color(&syntax, "variable"),
        parameter: optional_color(&syntax, "parameter"),
        property: optional_color(&syntax, "property"),
        namespace: optional_color(&syntax, "namespace"),
        enum_member: optional_color(&syntax, "enum_member"),
        decorator: optional_color(&syntax, "decorator"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_theme_plugins_from_disk() {
        let mut engine = PluginEngine::new();
        let log = engine.reload(Path::new("plugins"));
        assert!(
            log.iter().all(|l| !l.starts_with("error in")),
            "plugin load errors: {log:?}"
        );

        let themes = engine.take_themes();
        let names: Vec<_> = themes.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Dark+"), "themes: {names:?}");
        assert!(names.contains(&"Night Owl"), "themes: {names:?}");

        let night_owl = themes.iter().find(|t| t.name == "Night Owl").unwrap();
        assert_eq!(night_owl.background, eframe::egui::Color32::from_rgb(0x01, 0x16, 0x27));
        assert_eq!(night_owl.keyword, Some(eframe::egui::Color32::from_rgb(0xc7, 0x92, 0xea)));
    }
}
