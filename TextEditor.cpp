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

// New edit helpers
void TextEditor::clearUndoRedo()
{
    undoStack_.clear();
    redoStack_.clear();
}

void TextEditor::applyInsert(int pos, const std::string& text)
{
    if (text.empty()) return;
    // clamp pos
    int maxPos = (int)content.size();
    if (pos < 0) pos = 0;
    if (pos > maxPos) pos = maxPos;

    // perform insert
    content.insert((size_t)pos, text);

    // record action
    EditAction act;
    act.type = EditAction::Type::Insert;
    act.pos = pos;
    act.text = text;
    undoStack_.push_back(std::move(act));

    // clear redo
    redoStack_.clear();

    // update cursor and state
    cursorIndex = pos + (int)text.size();
    selectionStart = selectionEnd = -1;
    onTextChanged();
    caretFollow = true;
}

void TextEditor::applyErase(int pos, int len)
{
    if (len <= 0) return;
    int maxPos = (int)content.size();
    if (pos < 0) pos = 0;
    if (pos >= maxPos) return;
    if (pos + len > maxPos) len = maxPos - pos;

    // capture erased text
    std::string full = content.getText();
    std::string erased = full.substr((size_t)pos, (size_t)len);

    // perform erase
    content.erase((size_t)pos, (size_t)len);

    // record action
    EditAction act;
    act.type = EditAction::Type::Erase;
    act.pos = pos;
    act.text = erased;
    undoStack_.push_back(std::move(act));

    // clear redo
    redoStack_.clear();

    // update cursor and state
    cursorIndex = pos;
    selectionStart = selectionEnd = -1;
    onTextChanged();
    caretFollow = true;
}

void TextEditor::undo()
{
    if (undoStack_.empty()) return;
    EditAction act = std::move(undoStack_.back());
    undoStack_.pop_back();

    // inverse operation
    if (act.type == EditAction::Type::Insert)
    {
        // remove the inserted text
        int pos = act.pos;
        int len = (int)act.text.size();
        // perform erase without recording a new undo (so push to redo)
        content.erase((size_t)pos, (size_t)len);
        // push to redo stack the same insert action (so redo will reapply)
        redoStack_.push_back(act);
        cursorIndex = pos;
    }
    else if (act.type == EditAction::Type::Erase)
    {
        // re-insert the erased text
        int pos = act.pos;
        const std::string& t = act.text;
        content.insert((size_t)pos, t);
        // push to redo stack the erase action (so redo will reapply erase)
        redoStack_.push_back(act);
        cursorIndex = pos + (int)t.size();
    }

    selectionStart = selectionEnd = -1;
    onTextChanged();
    caretFollow = true;
}

void TextEditor::redo()
{
    if (redoStack_.empty()) return;
    EditAction act = std::move(redoStack_.back());
    redoStack_.pop_back();

    if (act.type == EditAction::Type::Insert)
    {
        // reapply insert
        int pos = act.pos;
        content.insert((size_t)pos, act.text);
        // push back to undo
        undoStack_.push_back(act);
        cursorIndex = pos + (int)act.text.size();
    }
    else if (act.type == EditAction::Type::Erase)
    {
        // reapply erase
        int pos = act.pos;
        int len = (int)act.text.size();
        content.erase((size_t)pos, (size_t)len);
        undoStack_.push_back(act);
        cursorIndex = pos;
    }

    selectionStart = selectionEnd = -1;
    onTextChanged();
    caretFollow = true;
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
    int len = selMax - selMin;

    // use applyErase to record undo
    applyErase(selMin, len);

    // applyErase already sets cursorIndex, selection and onTextChanged
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
    // applyErase called by deleteSelection already invoked onTextChanged
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
        // deleteSelection already recorded the erase
    }

    applyInsert(cursorIndex, clipText);
    addOutput(icons["checkmark"], "Pasted text");
}

void TextEditor::selectAll()
{
    selectionStart = 0;
    selectionEnd = (int)content.size();
    cursorIndex = selectionEnd;
    caretFollow = true;
}
