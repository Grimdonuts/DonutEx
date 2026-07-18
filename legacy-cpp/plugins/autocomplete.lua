-- local autocomplete = {
--   providers = {},
--   suggestions = {},
--   prefix = "",
--   open = false,
--   index = 1,
--   min_len = 2
-- }

-- function autocomplete.register(lang, provider)
--   autocomplete.providers[lang] = provider
-- end

-- local function get_lang()
--   local ok, lang = pcall(detect_language)
--   return ok and lang or "text"
-- end

-- function autocomplete.on_text_input()
--   local word = editor_get_current_word() or ""
--   --   print("DEBUG current word:", "[" .. word .. "]")
--   if not word or #word < autocomplete.min_len then
--     autocomplete.open = false
--     return
--   end
--   autocomplete.prefix = word
--   local provider = autocomplete.providers[get_lang()]
--   if not provider then
--     autocomplete.open = false
--     return
--   end
--   local list = provider(word) or {}
--   autocomplete.suggestions = list
--   autocomplete.index = (#list > 0) and 1 or 0
--   autocomplete.open = (#list > 0)
--   for _, s in ipairs(autocomplete.suggestions) do
--     print("sugg:", s)
--   end
-- end

-- local function accept(i)
--   local s = autocomplete.suggestions[i]
--   if s then
--     editor_replace_current_word(s)
--   end
--   autocomplete.open = false
-- end

-- function autocomplete.render()
--   if not autocomplete.open or #autocomplete.suggestions == 0 then return end

--   local h = editor_get_line_height()
--   local x, y = editor_get_cursor_screen_pos()
--   imgui.SetNextWindowPos(x, y + h) -- a bit below cursorix placement here using lineheight to do that.

--   if not imgui.Begin("Autocomplete",
--         imgui.NoFocusOnAppearing | imgui.NoNavFocus
--       ) then
--     imgui.End()
--     return
--   end


--   for i, s in ipairs(autocomplete.suggestions) do
--     local label = (i == autocomplete.index) and ("> " .. s) or ("  " .. s)
--     local clicked = imgui.Button(label)
--     if clicked then
--       accept(i)
--       imgui.End()
--       return
--     end
--   end
--   imgui.End()
-- end

-- register_hook("on_text_input", autocomplete.on_text_input)
-- register_hook("on_render", autocomplete.render)

-- return autocomplete
