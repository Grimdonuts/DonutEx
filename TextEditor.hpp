// TextEditor.hpp
#pragma once

#include <string>
#include <vector>
#include <map>
#include <unordered_map>
#include <functional>
#include "imgui.h"
#include "PieceTable.hpp"

class LuaBindings;
class FileOperations;
class EditorRenderer;
class FileExplorer;
class OutputPanel;
class IconManager;
class EditorCommands;

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
    
    bool render();
    void handleKeyboardShortcuts();
    void openFile(const std::string& fname);
    void addOutput(ImTextureID icon, const std::string& text);
    void addOutput(const std::string& text);
    
    // Public data members (accessed by subsystems)
    std::string filename;
    PieceTable content;
    bool modified;
    bool showFileExplorer;
    bool showOutput;
    bool showSettings;
    bool showGrid;
    bool showLineNumbers;
    
    bool focusEditor;
    bool closeEditor;
    
    int cursorIndex;
    int cursorLine;
    int cursorColumn;
    int selectionStart;
    int selectionEnd;
    bool isDragging;
    
    float scrollX;
    float scrollY;
    float maxContentWidth;
    float lineHeight;
    bool caretFollow;
    
    // Scrollbar state
    bool hDragging;
    float hDragMouseStart;
    float hDragScrollStart;
    bool vDragging;
    float vDragMouseStart;
    float vDragScrollStart;
    
    std::vector<CachedLine> lineCache;
    std::vector<OutputLine> outputLines;
    std::unordered_map<std::string, ImTextureID> icons;
    std::unordered_map<std::string, ImFont*> fontPreviews;
    
    // Cache and position helpers
    void rebuildCache();
    void onTextChanged();
    void indexToLineCol(int index, int& line, int& col);
    int lineColToIndex(int line, int col);
    
    // Selection helpers
    bool hasSelection() const;
    std::string getSelectedText() const;
    void deleteSelection();
    void copySelection();
    void cutSelection();
    void pasteFromClipboard();
    void selectAll();
    
private:
    LuaBindings* lua_;
    FileOperations* fileOps_;
    EditorRenderer* renderer_;
    FileExplorer* explorer_;
    OutputPanel* outputPanel_;
    IconManager* iconManager_;
    EditorCommands* commands_;
    
    friend class FileOperations;
    friend class EditorRenderer;
    friend class FileExplorer;
    friend class OutputPanel;
    friend class EditorCommands;
};
