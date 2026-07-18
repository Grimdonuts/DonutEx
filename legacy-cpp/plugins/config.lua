function show_font_menu()
    imgui.Text("Font Picker")
    imgui.Separator()

    local fonts = list_fonts("plugins/fonts")

    for i, path in ipairs(fonts) do
        local filename = path:match("([^/\\]+)$")
        if font_button(path, 18, filename) then
            if editor_load_font(path, 18) then
                print_with_icon("Switched to " .. filename, "checkmark")
            else
                print_with_icon("Failed to load " .. filename, "error")
            end
        end
    end
end

editor_load_font("plugins/fonts/Roboto.ttf", 26)
