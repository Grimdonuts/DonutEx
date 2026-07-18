-- Example plugin. Edit this file, then use Plugins > Reload Plugins in the
-- menu bar (or Ctrl+Shift+R once bound) to hot-reload it without restarting
-- DonutEx: the Lua VM is torn down and every plugins/*.lua file is re-run.

print("hello.lua loaded")

register_hook("on_render", function()
  -- runs once per frame; keep this cheap
end)
