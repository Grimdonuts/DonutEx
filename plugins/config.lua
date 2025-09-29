function show_font_menu()
    imgui.Text("Font Picker")
    imgui.Separator()

    local fonts = list_fonts("plugins/fonts")

    for i, path in ipairs(fonts) do
        local filename = path:match("([^/\\]+)$")
        if font_button(path, 18, filename) then
            if editor_load_font(path, 18) then
                print("✅ Switched to " .. filename)
            else
                print("❌ Failed to load " .. filename)
            end
        end
    end
end

editor_load_font("plugins/fonts/Roboto.ttf", 26)
