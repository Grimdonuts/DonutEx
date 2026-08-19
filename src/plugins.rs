use crate::theme::{self, Theme};
use mlua::{Lua, MultiValue};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;

/// Locates the `plugins/` directory to load from. Historically this was
/// always `<project_root>/plugins`, i.e. relative to the current working
/// directory - which works for `cargo run` from the repo, but breaks the
/// moment the binary is invoked from anywhere else (e.g. a PATH alias in
/// `.bashrc`): the app would silently find zero plugins, losing even the
/// built-in themes, which ship as `plugins/theme_*.lua`.
///
/// Instead, walk up from the *running executable's own location* looking
/// for a `plugins/` directory - this finds the same directory whether
/// launched via `cargo run` (binary under `target/debug/`, a few levels
/// below the repo root) or via a PATH-installed copy, regardless of the
/// caller's current directory. Only falls back to the old
/// project-root-relative path if that walk turns up nothing, so a project
/// that ships its own local `plugins/` folder still works.
pub fn resolve_dir(project_root: &Path) -> PathBuf {
    find_near_exe().unwrap_or_else(|| project_root.join("plugins"))
}

fn find_near_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?.to_path_buf();
    for _ in 0..6 {
        let candidate = dir.join("plugins");
        if candidate.is_dir() {
            return Some(candidate);
        }
        dir = dir.parent()?.to_path_buf();
    }
    None
}

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
    fn resolve_dir_finds_plugins_regardless_of_project_root() {
        // Simulates the reported bug: the app launched (e.g. via a PATH
        // alias) with an unrelated directory as `project_root`. The old
        // `project_root.join("plugins")` behavior would return a path that
        // doesn't exist; resolve_dir should still find the real plugins
        // directory via the executable's own location.
        let bogus_root = Path::new("/tmp/definitely-not-the-donutex-checkout");
        let dir = resolve_dir(bogus_root);
        assert!(
            dir.join("theme_dark_plus.lua").is_file(),
            "resolved plugins dir did not contain the built-in theme plugin: {}",
            dir.display()
        );
    }

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
