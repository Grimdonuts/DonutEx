#pragma once
#include <string>

struct lua_State;
class TextEditor;

class LuaBindings
{
public:
    LuaBindings(TextEditor *editor);
    ~LuaBindings();
    lua_State *L() const { return L_; }
    bool eval(const std::string &code);
    void loadPluginFile(const std::string &path);
    bool runHook(const char *hookName);
    void loadPlugins();
private:
    void initLua();
    void registerBridges();
    TextEditor *editor_;
    lua_State *L_{nullptr};
};
