// CursorUtils.cpp
#include "CursorUtils.hpp"
#include "TextEditor.hpp"
#include "imgui_internal.h"
#include <sstream>
#include <algorithm>
#include <cctype>

CursorUtils::CursorUtils(TextEditor* editor)
    : editor_(editor)
{
}

std::string CursorUtils::getCurrentLine()
{
    std::istringstream ss(editor_->content);
    std::string line;
    int currentLine = 0;
    while (std::getline(ss, line) && currentLine <= editor_->cursorLine)
    {
        if (currentLine == editor_->cursorLine)
            return line;
        currentLine++;
    }
    return "";
}

std::string CursorUtils::getCurrentWord()
{
    std::istringstream ss(editor_->content);
    std::string line;
    int currentLine = 0;
    while (std::getline(ss, line) && currentLine <= editor_->cursorLine)
    {
        if (currentLine == editor_->cursorLine)
        {
            int col = std::min<int>(editor_->cursorColumn, (int)line.size());
            int start = col;
            while (start > 0 && (std::isalnum((unsigned char)line[start - 1]) || line[start - 1] == '_'))
                --start;
            int end = col;
            while (end < (int)line.size() && (std::isalnum((unsigned char)line[end]) || line[end] == '_'))
                ++end;
            return line.substr(start, end - start);
        }
        currentLine++;
    }
    return "";
}

std::pair<int, int> CursorUtils::getCursorPosition()
{
    return {editor_->cursorLine, editor_->cursorColumn};
}

ImVec2 CursorUtils::getCursorScreenPos()
{
    ImGuiInputTextState* state = ImGui::GetInputTextState(editor_->editorInputId);
    if (!state)
        return editor_->lastCaretScreenPos;

    const ImGuiStyle& style = ImGui::GetStyle();

    ImVec2 innerOrigin(
        editor_->editorRectMin.x + style.FramePadding.x,
        editor_->editorRectMin.y + style.FramePadding.y);

    std::istringstream ss(editor_->content);
    std::string line;
    int currentLine = 0;
    while (std::getline(ss, line) && currentLine < editor_->cursorLine)
        currentLine++;

    int col = std::min<int>(editor_->cursorColumn, (int)line.size());
    std::string uptoCursor = line.substr(0, col);

    float caretX = ImGui::CalcTextSize(uptoCursor.c_str()).x;
    float caretY = (float)editor_->cursorLine * editor_->lineHeight - state->Scroll.y;
    ImVec2 caretScreen(innerOrigin.x + caretX,
                       innerOrigin.y + caretY);

    editor_->lastCaretScreenPos = caretScreen;
    return caretScreen;
}

void CursorUtils::replaceCurrentWordWith(const std::string& full)
{
    std::istringstream ss(editor_->content);
    std::string line;
    int currentLineIdx = 0;
    size_t offset = 0;
    while (std::getline(ss, line))
    {
        if (currentLineIdx == editor_->cursorLine)
            break;
        offset += line.size() + 1;
        currentLineIdx++;
    }
    if (currentLineIdx != editor_->cursorLine)
        return;

    int col = std::min<int>(editor_->cursorColumn, (int)line.size());
    int start = col;
    while (start > 0 && (std::isalnum((unsigned char)line[start - 1]) || line[start - 1] == '_'))
        --start;
    int end = col;
    while (end < (int)line.size() && (std::isalnum((unsigned char)line[end]) || line[end] == '_'))
        ++end;

    size_t absStart = offset + start;
    size_t absEnd = offset + end;
    editor_->content.replace(absStart, absEnd - absStart, full);
    editor_->cursorColumn = start + (int)full.size();
    editor_->modified = true;
}
