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
struct CachedLine {
    std::string text;
    float width;
};
class TextEditor
{
public:
    TextEditor();
    ~TextEditor();
    std::string filename;
    std::unordered_map<std::string, ImFont *> fontPreviews;
    void addOutput(ImTextureID icon, const std::string &text);
    void addOutput(const std::string &text); // text only
    bool render();
    void renderSettings();
    void handleKeyboardShortcuts();
    void openFile(const std::string &fname);
    float lineHeight;
    std::unordered_map<std::string, ImTextureID> icons;
    std::vector<OutputLine> outputLines;
    std::vector<CachedLine> lineCache;
    void rebuildCache();
    void onTextChanged();
    
private:
    bool showGrid = false;
    float scrollY = 0.0f;
    bool vDragging = false;
    float vDragMouseStart = 0.0f;
    float vDragScrollStart = 0.0f;
    void indexToLineCol(int index, int& line, int& col);
    int lineColToIndex(int line, int col);
    bool closeEditor = false;
    PieceTable content;
    int cursorIndex = 0;
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
    
    // Horizontal scrolling state
    float scrollX = 0.0f;
    float maxContentWidth = 0.0f;
    bool  hDragging = false;
    float hDragMouseStart = 0.0f;
    float hDragScrollStart = 0.0f;
    bool caretFollow = true;
    
    // ========== ADD THESE 3 LINES FOR SELECTION ==========
    int selectionStart = -1;
    int selectionEnd = -1;
    bool isDragging = false;
    
    // ========== ADD THESE 7 METHOD DECLARATIONS ==========
    bool hasSelection() const;
    std::string getSelectedText() const;
    void deleteSelection();
    void copySelection();
    void cutSelection();
    void pasteFromClipboard();
    void selectAll();
};

