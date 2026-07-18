# DonutEx

A small, fast, non-console text editor with a VS Code–style layout (file
explorer sidebar, tabbed editor, output console) and hot-reloadable Lua
plugins.

Rewritten in **Rust** on top of **egui/eframe** (GPU-accelerated
immediate-mode UI) with a **rope** buffer (`ropey`) for the text storage and
**Lua via `mlua`** for plugin scripting. The editor's text view is
virtualized — only the lines intersecting the viewport are laid out and
painted per frame, so cost scales with screen size, not file size.

The previous C++/Dear ImGui prototype (piece-table buffer, GLFW/OpenGL3
renderer) is preserved under [`legacy-cpp/`](legacy-cpp/) for reference.

## Layout

- **Left**: file explorer for the current project directory
- **Top of center**: tab bar for open files
- **Center**: the text editor
- **Bottom**: console/output panel (also used for plugin `print()` output)

## Running

```sh
cargo run
# or open a file directly:
cargo run -- path/to/file.rs
```

Requires a stable Rust toolchain (install via [rustup](https://rustup.rs)
if you don't have one).

## Syntax highlighting

Lines are tokenized (keywords, types, strings, numbers, comments, macros,
function calls) and colored per-token as they're painted; language is chosen
from the file extension (Rust, C/C++, Python, Lua, JS/TS, JSON, TOML, Shell
supported so far). Multi-line block comments are tracked via a per-document
cache that's invalidated on edit and lazily rebuilt in one pass over the
file, rather than rescanned every frame.

## Plugins

Plugins live in `plugins/*.lua` and are loaded on startup. Each plugin is
plain interpreted Lua — no build step — so **Plugins > Reload Plugins** in
the menu bar hot-reloads every plugin file by tearing down the Lua VM and
re-running them from disk. Edit a `.lua` file, click reload, see the change
without restarting the editor.

Available from Lua:

- `print(...)` — writes to the console panel (shadows Lua's normal stdout
  print)
- `register_hook(event, fn)` — registers a callback for `"on_render"`
  (called once per frame; keep it cheap) or `"on_text_input"`

See [`plugins/hello.lua`](plugins/hello.lua) for a minimal example.

## Keyboard shortcuts

| Shortcut | Action |
|---|---|
| Ctrl+N | New file |
| Ctrl+O | Open file |
| Ctrl+S | Save |
| Ctrl+Shift+S | Save as |
| Ctrl+W | Close tab |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| Ctrl+A / C / X / V | Select all / copy / cut / paste |

## Status

Early rewrite. Working: multi-tab editing, undo/redo, selection, clipboard,
mouse + keyboard navigation, draggable scrollbars, syntax highlighting, file
explorer, console, manual plugin hot-reload. Not yet ported from the old
prototype: autocomplete, custom fonts/icons, automatic (file-watching)
plugin reload.
