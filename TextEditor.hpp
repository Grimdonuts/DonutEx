#pragma once

#include <string>
#include <vector>
#include <map>
#include <unordered_map>
#include <functional>
#include <filesystem>
#include <cctype>
#include "imgui.h"
#include "PieceTable.hpp"

struct ImGuiInputTextCallbackData;
struct lua_State;

class LuaBindings;

struct OutputLine {
    ImTextureID icon;
    std::string text;
};

class TextEditor
{
public:
    TextEditor();
    ~TextEditor();
    std::string filename;
    std::unordered_map<std::string, ImFont *> fontPreviews;
   // std::string getCurrentLine();
  //  std::string getCurrentWord();
    //std::pair<int, int> getCursorPosition();
   // void insertText(const std::string &text);
    void addOutput(ImTextureID icon, const std::string &text);
    void addOutput(const std::string &text); // text only
    // void replaceCurrentWordWith(const std::string &full);
    bool render();
    void renderSettings();
    void handleKeyboardShortcuts();
    void openFile(const std::string &fname);
  //  ImVec2 getCursorScreenPos();
    float lineHeight;
    std::unordered_map<std::string, ImTextureID> icons;
    std::vector<OutputLine> outputLines;
private:
    bool closeEditor = false;
    PieceTable content;
    int cursorIndex = 0;   // NEW: position in content string
    bool focusEditor = false;
    std::string commandInput;
    std::vector<std::string> fileList;
    bool modified;
    bool showFileExplorer = true;
    bool showOutput = true;
    bool showSettings = false;
    int cursorLine;
    int cursorColumn;
    std::map<std::string, std::function<void()>> commands;
    ImGuiInputTextFlags inputFlags{};
    static int textInputCallback(ImGuiInputTextCallbackData *data);
    LuaBindings *lua_;
    void registerCommands();
    void executeCommand(const std::string &cmd);
    void refreshFileList();
    void newFile();
    void saveFile();
    void showOpenDialog();
    void showSaveDialog(const std::string &defaultFileName);
    ImTextureID loadSVGTexture(const char *filename, float targetHeight);
    void loadIcons(float dpiScale = 1.0f);
    bool IconTextButton(const char *id, ImTextureID icon, const char *label, const ImVec2 &button_size);
    ImGuiID editorInputId = 0;
    ImVec2 lastCaretScreenPos = ImVec2(0, 0);
    void renderEditor();
    void renderExplorer(ImVec2 workPos, ImVec2 workSize, float explorerWidth);
    void renderOutput(ImVec2 workPos, ImVec2 workSize, float outputHeight, float explorerWidth);
    void renderMenuBar();
};
