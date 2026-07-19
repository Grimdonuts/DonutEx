-- Theme plugin: Night Owl, ported from the popular VS Code theme by Sarah
-- Drasner (https://github.com/sdras/night-owl-vscode-theme). Pick it from
-- the Themes menu. Colors below are taken directly from that theme's
-- "Night Owl-color-theme.json" (editor.*/sideBar.* for `colors`,
-- tokenColors scopes for `syntax`), except `selection_border`, `accent` and
-- `macro`, which have no direct equivalent in the source theme and were
-- chosen to stay in its palette.
register_theme({
  name = "Night Owl",
  colors = {
    background = "#011627",       -- editor.background
    panel = "#011627",            -- sideBar.background (same navy as the editor, not a lighter shade)
    text = "#d6deeb",             -- editor.foreground
    selection_bg = "#1d3b53",     -- editor.selectionBackground
    selection_border = "#5f7e97",
    accent = "#82aaff",
    scrollbar_track = "#011627",
    scrollbar_thumb = "#084d8180", -- scrollbarSlider.background (semi-transparent steel blue)
  },
  syntax = {
    keyword = "#c792ea",     -- keyword, storage, keyword.control
    type = "#ffcb8b",        -- entity.name.class
    string = "#ecc48d",      -- string
    number = "#f78c6c",      -- constant.numeric
    comment = "#637777",     -- comment
    func = "#82aaff",        -- entity.name.function
    macro = "#c792ea",
    variable = "#d6deeb",    -- variable
    parameter = "#7fdbca",   -- variable.parameter.function
    property = "#baebe2",    -- variable.instance / variable.other.property
    namespace = "#c5e478",   -- support.type / support.class
    enum_member = "#82aaff", -- constant.language / constant.other
    decorator = "#82aaff",   -- meta.decorator punctuation.decorator
  },
})
