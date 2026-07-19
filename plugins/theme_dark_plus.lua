-- Theme plugin: registers the editor's default Dark+ palette (VS Code's
-- dark theme) as a switchable theme. Pick it from the Themes menu, or edit
-- the hex values below and use Plugins > Reload Plugins to see changes live.

register_theme({
  name = "Dark+",
  colors = {
    background = "#1e1e1e",
    panel = "#252526",
    text = "#d4d4d4",
    selection_bg = "#264f78",
    selection_border = "#5a8cc8",
    accent = "#0e639c",
    scrollbar_track = "#252526",
    scrollbar_thumb = "#606064",
  },
  syntax = {
    keyword = "#569cd6",
    type = "#4ec9b0",
    string = "#ce9178",
    number = "#b5cea8",
    comment = "#6a9955",
    func = "#dcdcaa",
    macro = "#c586c0",
    variable = "#9cdcfe",
    parameter = "#9cdcfe",
    property = "#9cdcfe",
    namespace = "#4ec9b0",
    enum_member = "#4ec9b0",
    decorator = "#dcdcaa",
  },
})
