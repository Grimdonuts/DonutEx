#include "LuaBindings.hpp"
#include "TextEditor.hpp"
#include <filesystem>
#include <string>
#include "imgui.h"

extern "C"
{
#include <lua.h>
#include <lauxlib.h>
#include <lualib.h>
}

LuaBindings::LuaBindings(TextEditor *editor)
    : editor_(editor)
{
    initLua();
    registerBridges();
}

LuaBindings::~LuaBindings()
{
    if (L_)
    {
        lua_close(L_);
    }
}

void LuaBindings::initLua()
{
    L_ = luaL_newstate();
    luaL_openlibs(L_);
    luaL_dostring(L_, "package.path = 'plugins/?.lua;' .. package.path");
    luaL_dostring(L_, R"(
        hooks = { on_text_input = {}, on_render = {} }
        function register_hook(event, fn)
            if hooks[event] then table.insert(hooks[event], fn) end
        end
    )");
}

void LuaBindings::loadPlugins()
{
    try
    {
        for (const auto &entry : std::filesystem::directory_iterator("plugins"))
        {
            if (entry.is_regular_file() && entry.path().extension() == ".lua")
            {
                loadPluginFile(entry.path().string());
            }
        }
    }
    catch (...)
    {
        editor_->addOutput(editor_->icons["error"], "Error reading plugins directory");
    }
}

bool LuaBindings::eval(const std::string &code)
{
    if (luaL_dostring(L_, code.c_str()))
    {
        if (editor_)
        {
            editor_->addOutput(editor_->icons["error"], std::string("Lua: ") + lua_tostring(L_, -1));
        }
        lua_pop(L_, 1);
        return false;
    }
    return true;
}

void LuaBindings::loadPluginFile(const std::string &path)
{
    std::string cleanPath = path; // windows pathing amirite lol
    std::replace(cleanPath.begin(), cleanPath.end(), '\\', '/');

    if (luaL_loadfile(L_, path.c_str()) || lua_pcall(L_, 0, 0, 0))
    {
        if (editor_)
        {
            editor_->addOutput(editor_->icons["error"], "Error loading plugin " + cleanPath + ": " +
                                                            std::string(lua_tostring(L_, -1)));
        }
        lua_pop(L_, 1);
    }
    else if (editor_)
    {
        editor_->addOutput(editor_->icons["checkmark"], "Loaded plugin: " + cleanPath);
    }
}

bool LuaBindings::runHook(const char *hookName)
{
    std::string code =
        std::string("for _, fn in ipairs(hooks.") + hookName + ") do fn() end";
    return eval(code);
}

// register the c++ <-> lua interactions
void LuaBindings::registerBridges()
{
    auto push_editor = [this]()
    {
        lua_pushlightuserdata(L_, editor_);
    };

    // TextEditor stuff

    // editor_insert_text(text)
    // push_editor();
    // lua_pushcclosure(L_, [](lua_State *L) -> int
    //                  {
    //     auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
    //     const char* text = luaL_checkstring(L, 1);
    //     ed->insertText(text);
    //     return 0; }, 1);
    // lua_setglobal(L_, "editor_insert_text");

    // editor_get_current_line()
    // push_editor();
    // lua_pushcclosure(L_, [](lua_State *L) -> int
    //                  {
    //     auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
    //     std::string line = ed->getCurrentLine();
    //     lua_pushlstring(L, line.c_str(), line.size());
    //     return 1; }, 1);
    // lua_setglobal(L_, "editor_get_current_line");

    // editor_get_current_word()
    // push_editor();
    // lua_pushcclosure(L_, [](lua_State *L) -> int
    //                  {
    //     auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
    //     std::string word = ed->getCurrentWord();
    //     lua_pushlstring(L, word.c_str(), word.size());
    //     return 1; }, 1);
    // lua_setglobal(L_, "editor_get_current_word");

    // editor_get_cursor_position() -> line, col
    // push_editor();
    // lua_pushcclosure(L_, [](lua_State *L) -> int
    //                  {
    //     auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
    //     auto [line, col] = ed->getCursorPosition();
    //     lua_pushinteger(L, line);
    //     lua_pushinteger(L, col);
    //     return 2; }, 1);
    // lua_setglobal(L_, "editor_get_cursor_position");

    // print to output window
    push_editor();
    lua_pushcclosure(L_, [](lua_State *L) -> int
                     {
        auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
        int n = lua_gettop(L);
        std::string msg;
        for (int i = 1; i <= n; ++i) {
            size_t len;
            const char* s = luaL_tolstring(L, i, &len);
            if (i > 1) msg += " ";
            msg.append(s, len);
            lua_pop(L, 1);
        }
        ed->addOutput(msg);
        return 0; }, 1);
    lua_setglobal(L_, "print");

    // print but with an icon
    push_editor();
    lua_pushcclosure(L_, [](lua_State *L) -> int
                     {
    auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
    const char* text = luaL_checkstring(L, 1);
    const char* iconKey = luaL_optstring(L, 2, nullptr);
    ImTextureID icon = (ImTextureID)0;

    if (iconKey) {
        auto iconTex = ed->icons.find(iconKey);
        if (iconTex != ed->icons.end()) {
            icon = iconTex->second;
        }
    }

    ed->addOutput(icon, text);
    return 0; }, 1);
    lua_setglobal(L_, "print_with_icon");

    // editor_replace_current_word(full_text)
    push_editor();
    lua_pushcclosure(L_, [](lua_State *L) -> int
                     {
        auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
        const char* full = luaL_checkstring(L, 1);
       // ed->replaceCurrentWordWith(full);
        return 0; }, 1);
    lua_setglobal(L_, "editor_replace_current_word");

    // detect_language()
    push_editor();
    lua_pushcclosure(L_, [](lua_State *L) -> int
                     {
        auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
        std::string ext = std::filesystem::path(ed->filename).extension().string();
        std::string lang = "text";
        if (ext == ".cpp" || ext == ".cxx" || ext == ".cc" ||
            ext == ".hpp" || ext == ".hh" || ext == ".h")
            lang = "cpp";
        else if (ext == ".lua")
            lang = "lua";
        else if (ext == ".js")
            lang = "javascript";
        else if (ext == ".ts")
            lang = "typescript";
        lua_pushstring(L, lang.c_str());
        return 1; }, 1);
    lua_setglobal(L_, "detect_language");

    // Fonts stuff

    // editor_load_font(path, size) -> bool
    push_editor();
    lua_pushcclosure(L_, [](lua_State *L) -> int
                     {
        auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
        const char* fontPath = luaL_checkstring(L, 1);
        float size = (float)luaL_optnumber(L, 2, 16.0f);

        if (!std::filesystem::exists(fontPath)) {
            ed->addOutput(ed->icons["error"], std::string("Font not found: ") + fontPath);
            lua_pushboolean(L, 0);
            return 1;
        }

        ImGuiIO& io = ImGui::GetIO();
        ImFont* font = io.Fonts->AddFontFromFileTTF(fontPath, size);
        if (font) {
            io.FontDefault = font;
            ed->addOutput(ed->icons["checkmark"], std::string("Loaded font: ") + fontPath);
            lua_pushboolean(L, 1);
        } else {
            ed->addOutput(ed->icons["error"], std::string("Could not load font: ") + fontPath);
            lua_pushboolean(L, 0);
        }
        return 1; }, 1);
    lua_setglobal(L_, "editor_load_font");

    // list_fonts
    lua_pushcfunction(L_, [](lua_State *L) -> int
                      {
        const char* dir = luaL_checkstring(L, 1);
        lua_newtable(L);
        int i = 1;
        try {
            for (const auto& entry : std::filesystem::directory_iterator(dir)) {
                if (entry.path().extension() == ".ttf") {
                    std::string p = entry.path().string();
                    lua_pushstring(L, p.c_str());
                    lua_rawseti(L, -2, i++);
                }
            }
        } catch (...) {
            // ignore errors (dir may not exist)
        }
        return 1; });
    lua_setglobal(L_, "list_fonts");

    // font_button
    push_editor();
    lua_pushcclosure(L_, [](lua_State *L) -> int
                     {
        auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
        const char* fontPath = luaL_checkstring(L, 1);
        float size = (float)luaL_optnumber(L, 2, 16.0f);
        const char* label = luaL_optstring(L, 3, fontPath);

        ImGuiIO& io = ImGui::GetIO();
        std::string key = std::string(fontPath) + std::to_string((int)size);

        ImFont* font = nullptr;
        auto it = ed->fontPreviews.find(key);
        if (it != ed->fontPreviews.end()) {
            font = it->second;
        } else if (std::filesystem::exists(fontPath)) {
            font = io.Fonts->AddFontFromFileTTF(fontPath, size);
            ed->fontPreviews[key] = font;
        }

        bool pressed = false;
        if (font) {
            ImGui::PushFont(font);
            pressed = ImGui::Button(label);
            ImGui::PopFont();
        } else {
            pressed = ImGui::Button(label);
        }

        lua_pushboolean(L, pressed);
        return 1; }, 1);
    lua_setglobal(L_, "font_button");

    // // editor_get_line_height good for the autocomplete
    // push_editor();
    // lua_pushcclosure(L_, [](lua_State *L) -> int
    //                  {
    // auto* ed = static_cast<TextEditor*>(lua_touserdata(L, lua_upvalueindex(1)));
    // ImVec2 pos = ed->getCursorScreenPos();
    // lua_pushnumber(L, pos.x);
    // lua_pushnumber(L, pos.y);
    // return 2; }, 1);
    // lua_setglobal(L_, "editor_get_cursor_screen_pos");

    // editor_get_line_height()
    push_editor();
    lua_pushcclosure(L_, [](lua_State *L) -> int
                     {
                         auto *ed = static_cast<TextEditor *>(lua_touserdata(L, lua_upvalueindex(1)));
                         lua_pushnumber(L, ed->lineHeight);
                         return 1; // returning one number
                     },
                     1);
    lua_setglobal(L_, "editor_get_line_height");

    // ImGui hooks
    lua_newtable(L_);

    // imgui.Text(txt)
    lua_pushcfunction(L_, [](lua_State *L) -> int
                      {
        const char* txt = luaL_checkstring(L, 1);
        ImGui::Text("%s", txt);
        return 0; });
    lua_setfield(L_, -2, "Text");

    // imgui.Button(label) -> bool
    lua_pushcfunction(L_, [](lua_State *L) -> int
                      {
        const char* label = luaL_checkstring(L, 1);
        bool pressed = ImGui::Button(label);
        lua_pushboolean(L, pressed);
        return 1; });
    lua_setfield(L_, -2, "Button");

    // imgui.Separator()
    lua_pushcfunction(L_, [](lua_State *L) -> int
                      {
        ImGui::Separator();
        return 0; });
    lua_setfield(L_, -2, "Separator");

    // imgui.BeginChild(name, w?, h?, border?)
    lua_pushcfunction(L_, [](lua_State *L) -> int
                      {
        const char* name = luaL_checkstring(L, 1);
        ImVec2 size((float)luaL_optnumber(L, 2, 0), (float)luaL_optnumber(L, 3, 0));
        bool border = lua_toboolean(L, 4);
        bool open = ImGui::BeginChild(name, size, border);
        lua_pushboolean(L, open);
        return 1; });
    lua_setfield(L_, -2, "BeginChild");

    // imgui.EndChild()
    lua_pushcfunction(L_, [](lua_State *L) -> int
                      {
        ImGui::EndChild();
        return 0; });
    lua_setfield(L_, -2, "EndChild");

    // imgui.Begin(title [, flags])
    lua_pushcfunction(L_, [](lua_State *L) -> int
                      {
    const char* title = luaL_checkstring(L, 1);
    int flags = 0;
    if (lua_gettop(L) >= 2 && lua_isinteger(L, 2)) {
        flags = (int)lua_tointeger(L, 2);
    }

    // Default style
    flags |= ImGuiWindowFlags_NoDecoration | ImGuiWindowFlags_AlwaysAutoResize;

    bool open = ImGui::Begin(title, nullptr, flags);
    lua_pushboolean(L, open);
    return 1; });
    lua_setfield(L_, -2, "Begin");

    // imgui.End()
    lua_pushcfunction(L_, [](lua_State *L) -> int
                      {
        ImGui::End();
        return 0; });
    lua_setfield(L_, -2, "End");

    // imgui.SetNextWindowPos(x, y)
    lua_pushcfunction(L_, [](lua_State *L) -> int
                      {
        float x = (float)luaL_checknumber(L, 1);
        float y = (float)luaL_checknumber(L, 2);
        ImGui::SetNextWindowPos(ImVec2(x, y), ImGuiCond_Always);
        return 0; });
    lua_setfield(L_, -2, "SetNextWindowPos");

    // Window flags
    lua_pushinteger(L_, ImGuiWindowFlags_NoFocusOnAppearing);
    lua_setfield(L_, -2, "NoFocusOnAppearing");

    lua_pushinteger(L_, ImGuiWindowFlags_NoNavFocus);
    lua_setfield(L_, -2, "NoNavFocus");

    lua_pushinteger(L_, ImGuiWindowFlags_NoInputs);
    lua_setfield(L_, -2, "NoInputs");

    // finalize global
    lua_setglobal(L_, "imgui");
}
