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

    // Edit / Undo/Redo API
    struct EditAction {
        enum class Type { Insert, Erase } type;
        int pos;
        std::string text; // inserted text (for Insert) or erased text (for Erase)
    };

    void applyInsert(int pos, const std::string& text); // records action
    void applyErase(int pos, int len); // records action (captures erased text)
    void undo();
    void redo();
    void clearUndoRedo();

private:
    LuaBindings* lua_;
    FileOperations* fileOps_;
    EditorRenderer* renderer_;
    FileExplorer* explorer_;
    OutputPanel* outputPanel_;
    IconManager* iconManager_;
    EditorCommands* commands_;

    // Undo/redo stacks
    std::vector<EditAction> undoStack_;
    std::vector<EditAction> redoStack_;
    
    friend class FileOperations;
    friend class EditorRenderer;
    friend class FileExplorer;
    friend class OutputPanel;
    friend class EditorCommands;
};

// Inline implementations to ensure definitions are available across translation units
inline void TextEditor::rebuildCache()
{
    lineCache.clear();

    const std::string full = content.getText();
    const size_t n = full.size();

    const char* sample = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    float cellWidth = ImGui::CalcTextSize(sample).x / (float)strlen(sample);

    size_t start = 0;
    for (size_t i = 0; i < n; ++i)
    {
        if (full[i] == '\n')
        {
            std::string seg = full.substr(start, i - start);
            CachedLine cl;
            cl.text = std::move(seg);
            cl.width = cl.text.size() * cellWidth;
            lineCache.push_back(std::move(cl));
            start = i + 1;
        }
    }

    if (start < n)
    {
        std::string seg = full.substr(start);
        CachedLine cl;
        cl.text = std::move(seg);
        cl.width = cl.text.size() * cellWidth;
        lineCache.push_back(std::move(cl));
    }
    else
    {
        if (!full.empty() && full.back() == '\n')
        {
            lineCache.push_back({"", 0.0f});
        }
    }

    if (lineCache.empty())
    {
        lineCache.push_back({"", 0.0f});
    }
}

inline void TextEditor::onTextChanged()
{
    rebuildCache();
    modified = true;
}

inline void TextEditor::indexToLineCol(int index, int& line, int& col)
{
    int pos = 0;
    line = 0;
    for (const auto& cl : lineCache)
    {
        int lineLen = (int)cl.text.size();
        if (index <= pos + lineLen)
        {
            col = index - pos;
            return;
        }
        pos += lineLen + 1;
        line++;
    }

    if (!lineCache.empty())
    {
        line = (int)lineCache.size() - 1;
        col = (int)lineCache.back().text.size();
    }
    else
    {
        line = 0;
        col = 0;
    }
}

inline int TextEditor::lineColToIndex(int line, int col)
{
    int index = 0;
    for (int i = 0; i < line && i < (int)lineCache.size(); i++)
    {
        index += (int)lineCache[i].text.size() + 1;
    }
    if (line < (int)lineCache.size())
    {
        col = std::min(col, (int)lineCache[line].text.size());
        index += col;
    }
    return index;
}
