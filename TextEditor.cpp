// TextEditor.cpp
#include "TextEditor.hpp"
#include "LuaBindings.hpp"
#include "FileOperations.hpp"
#include "EditorRenderer.hpp"
#include "FileExplorer.hpp"
#include "OutputPanel.hpp"
#include "IconManager.hpp"
#include "EditorCommands.hpp"
#include "imgui.h"
#include <cstring>
#include <sstream>

TextEditor::TextEditor()
    : filename(""),
      content(),
      modified(false),
      showFileExplorer(true),
      showOutput(true),
      showSettings(false),
      showGrid(false),
      showLineNumbers(true),
      focusEditor(false),
      closeEditor(false),
      cursorIndex(0),
      cursorLine(0),
      cursorColumn(0),
      selectionStart(-1),
      selectionEnd(-1),
      isDragging(false),
      scrollX(0.0f),
      scrollY(0.0f),
      maxContentWidth(0.0f),
      lineHeight(0.0f),
      caretFollow(true),
      hDragging(false),
      hDragMouseStart(0.0f),
      hDragScrollStart(0.0f),
      vDragging(false),
      vDragMouseStart(0.0f),
      vDragScrollStart(0.0f)
{
    // Initialize subsystems
    lua_ = new LuaBindings(this);
    iconManager_ = new IconManager(this);
    commands_ = new EditorCommands(this);
    fileOps_ = new FileOperations(this);
    renderer_ = new EditorRenderer(this, lua_);
    explorer_ = new FileExplorer(this, iconManager_, commands_);
    outputPanel_ = new OutputPanel(this, commands_);
    
    iconManager_->loadIcons(ImGui::GetIO().FontGlobalScale);
    commands_->registerCommands();
    lua_->loadPlugins();
    commands_->refreshFileList();
}

TextEditor::~TextEditor()
{
    delete outputPanel_;
    delete explorer_;
    delete renderer_;
    delete fileOps_;
    delete commands_;
    delete iconManager_;
    delete lua_;
}

bool TextEditor::render()
{
    ImGuiStyle& style = ImGui::GetStyle();
    style.Colors[ImGuiCol_TitleBgActive] = style.Colors[ImGuiCol_TitleBg];

    // Global shortcuts
    handleKeyboardShortcuts();

    // Menu bar
    renderer_->renderMenuBar();

    // Layout calculations
    ImGuiViewport* viewport = ImGui::GetMainViewport();
    ImVec2 workPos = viewport->WorkPos;
    ImVec2 workSize = viewport->WorkSize;
    float explorerWidth = showFileExplorer ? 250.0f : 0.0f;
    float outputHeight = showOutput ? 200.0f : 0.0f;

    // File Explorer
    if (showFileExplorer)
    {
        ImGui::SetNextWindowPos(workPos, ImGuiCond_Always);
        ImGui::SetNextWindowSize(ImVec2(explorerWidth, workSize.y), ImGuiCond_Always);
        explorer_->render(workPos, workSize, explorerWidth);
    }

    // Editor
    ImVec2 editorPos(workPos.x + explorerWidth, workPos.y);
    ImVec2 editorSize(workSize.x - explorerWidth, workSize.y - outputHeight);
    ImGui::SetNextWindowPos(editorPos, ImGuiCond_Always);
    ImGui::SetNextWindowSize(editorSize, ImGuiCond_Always);
    renderer_->renderEditor();

    // Output Panel
    if (showOutput)
    {
        outputPanel_->render(workPos, workSize, outputHeight, explorerWidth);
    }

    // Settings
    if (showSettings)
    {
        renderer_->renderSettings();
    }

    // Lua hooks
    lua_->runHook("on_text_input");
    lua_->runHook("on_render");

    return closeEditor;
}

void TextEditor::handleKeyboardShortcuts()
{
    ImGuiIO& io = ImGui::GetIO();

    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_N))
        fileOps_->newFile();
    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_O))
        fileOps_->showOpenDialog();
    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_S))
    {
        if (io.KeyShift)
        {
            fileOps_->showSaveDialog(filename.empty() ? "untitled.txt" : filename);
        }
        else
        {
            fileOps_->saveFile();
        }
    }
}

void TextEditor::openFile(const std::string& fname)
{
    fileOps_->openFile(fname);
}

void TextEditor::addOutput(ImTextureID icon, const std::string& text)
{
    outputLines.push_back({icon, text});
    if (outputLines.size() > 1000)
    {
        outputLines.erase(outputLines.begin());
    }
}

void TextEditor::addOutput(const std::string& text)
{
    addOutput((ImTextureID)0, text);
}

// Cache and position helpers
void TextEditor::rebuildCache()
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

void TextEditor::onTextChanged()
{
    rebuildCache();
    modified = true;
}

void TextEditor::indexToLineCol(int index, int& line, int& col)
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

int TextEditor::lineColToIndex(int line, int col)
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

// Selection helpers
bool TextEditor::hasSelection() const
{
    return selectionStart != -1 && selectionEnd != -1 && selectionStart != selectionEnd;
}

std::string TextEditor::getSelectedText() const
{
    if (!hasSelection())
        return "";

    int selMin = std::min(selectionStart, selectionEnd);
    int selMax = std::max(selectionStart, selectionEnd);

    std::string fullText = content.getText();
    if (selMin < 0 || selMax > (int)fullText.size())
        return "";

    return fullText.substr(selMin, selMax - selMin);
}

void TextEditor::deleteSelection()
{
    if (!hasSelection())
        return;

    int selMin = std::min(selectionStart, selectionEnd);
    int selMax = std::max(selectionStart, selectionEnd);

    content.erase(selMin, selMax - selMin);
    cursorIndex = selMin;
    selectionStart = selectionEnd = -1;
}

void TextEditor::copySelection()
{
    if (!hasSelection())
        return;

    std::string text = getSelectedText();
    ImGui::SetClipboardText(text.c_str());
    addOutput(icons["checkmark"], "Copied " + std::to_string(text.size()) + " characters");
}

void TextEditor::cutSelection()
{
    if (!hasSelection())
        return;

    copySelection();
    deleteSelection();
    onTextChanged();
    caretFollow = true;
}

void TextEditor::pasteFromClipboard()
{
    const char* clipText = ImGui::GetClipboardText();
    if (!clipText || !clipText[0])
        return;

    if (hasSelection())
    {
        deleteSelection();
        onTextChanged();
    }

    content.insert(cursorIndex, clipText);
    cursorIndex += strlen(clipText);
    onTextChanged();
    caretFollow = true;
    addOutput(icons["checkmark"], "Pasted text");
}

void TextEditor::selectAll()
{
    selectionStart = 0;
    selectionEnd = (int)content.size();
    cursorIndex = selectionEnd;
    caretFollow = true;
}
