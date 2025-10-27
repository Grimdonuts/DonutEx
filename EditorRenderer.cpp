// EditorRenderer.cpp
#include "EditorRenderer.hpp"
#include "TextEditor.hpp"
#include "LuaBindings.hpp"
#include "imgui_internal.h"
#include <cstring>
#include <cmath>
#include <algorithm>

EditorRenderer::EditorRenderer(TextEditor* editor, LuaBindings* lua)
    : editor_(editor), lua_(lua)
{
}

void EditorRenderer::clampToViewport(ImGuiSizeCallbackData* data)
{
    ImGuiViewport* viewport = ImGui::GetMainViewport();
    ImVec2 workMin = viewport->WorkPos;
    ImVec2 workMax(viewport->WorkPos.x + viewport->WorkSize.x, viewport->WorkPos.y + viewport->WorkSize.y);
    ImVec2 pos = ImGui::GetWindowPos();
    ImVec2 size = data->CurrentSize;
    if (pos.x < workMin.x)
        pos.x = workMin.x;
    if (pos.y < workMin.y)
        pos.y = workMin.y;
    if (pos.x + size.x > workMax.x)
        pos.x = workMax.x - size.x;
    if (pos.y + size.y > workMax.y)
        pos.y = workMax.y - size.y;
    ImGui::SetWindowPos(pos, ImGuiCond_Always);
}

void EditorRenderer::renderMenuBar()
{
    if (ImGui::BeginMainMenuBar())
    {
        if (ImGui::BeginMenu("File"))
        {
            if (ImGui::MenuItem("New", "Ctrl+N"))
                ; // Handled by keyboard shortcuts
            if (ImGui::MenuItem("Open", "Ctrl+O"))
                ; // Handled by keyboard shortcuts
            if (ImGui::MenuItem("Save As", "Ctrl+Shift+S"))
                ; // Handled by keyboard shortcuts
            if (ImGui::MenuItem("Save", "Ctrl+S"))
                ; // Handled by keyboard shortcuts
            ImGui::Separator();
            if (ImGui::MenuItem("Exit", "Alt+F4"))
            {
                editor_->closeEditor = true;
            }
            ImGui::EndMenu();
        }

        if (ImGui::BeginMenu("View"))
        {
            ImGui::MenuItem("File Explorer", nullptr, &editor_->showFileExplorer);
            ImGui::MenuItem("Output Panel", nullptr, &editor_->showOutput);
            // toggle grid
            if (ImGui::MenuItem("Show Grid", nullptr, editor_->showGrid))
                editor_->showGrid = !editor_->showGrid;
            // toggle line numbers
            ImGui::MenuItem("Show Line Numbers", nullptr, &editor_->showLineNumbers);
            ImGui::EndMenu();
        }

        if (ImGui::BeginMenu("Plugins"))
        {
            if (ImGui::MenuItem("Reload Plugins"))
            {
                lua_->loadPlugins();
                editor_->addOutput("Plugins reloaded!");
            }
            if (ImGui::MenuItem("List Commands"))
            {
                editor_->addOutput("Available commands:");
                // Commands would be listed here
            }
            ImGui::EndMenu();
        }
        ImGui::EndMainMenuBar();
    }
}

void EditorRenderer::renderSettings()
{
    ImGui::SetNextWindowSize(ImVec2(500, 400), ImGuiCond_Once);
    ImGui::SetNextWindowSizeConstraints(ImVec2(200, 100), ImVec2(FLT_MAX, FLT_MAX), clampToViewport);
    ImGui::Begin("Settings", &editor_->showSettings,
                 ImGuiWindowFlags_NoCollapse | ImGuiWindowFlags_NoDocking);
    ImGui::Text("Editor Settings");
    lua_->eval("if show_font_menu then show_font_menu() end");
    ImGui::End();
}

void EditorRenderer::renderGrid(ImDrawList* drawList, ImVec2 pos, float viewW, float viewH,
                                float cellWidth, float lineHeight, float padX, float padY)
{
    if (!editor_->showGrid)
        return;

    ImU32 gridColor = IM_COL32(100, 100, 100, 50);
    float gridSpacingX = cellWidth;
    float gridSpacingY = lineHeight;

    float startX = pos.x + padX - fmodf(editor_->scrollX, gridSpacingX);
    float startY = pos.y + padY - fmodf(editor_->scrollY, gridSpacingY);

    // Vertical columns
    for (float x = startX; x < pos.x + viewW; x += gridSpacingX)
        drawList->AddLine(ImVec2(roundf(x), pos.y),
                         ImVec2(roundf(x), pos.y + viewH), gridColor);

    // Horizontal lines
    for (float y = startY; y < pos.y + viewH; y += gridSpacingY)
    {
        float yMid = roundf(y + gridSpacingY * 0.75f);
        drawList->AddLine(ImVec2(pos.x, yMid), ImVec2(pos.x + viewW, yMid), gridColor);
    }
}

// helper: compute gutter width
static float computeGutterWidth(int totalLines, float cellWidth, float /*padX*/)
{
    int lines = std::max(1, totalLines);
    int digits = 1;
    while (lines >= 10) { lines /= 10; digits++; }
    // Reserve at least 6 digits (e.g. 100,000) so gutter doesn't jump suddenly
    digits = std::max(digits, 6);

    // give one "character" of padding on left and right inside the gutter
    float sidePadding = cellWidth;
    // total gutter = left sidePadding + digits*cellWidth + right sidePadding
    return sidePadding + (digits * cellWidth) + sidePadding;
}

void EditorRenderer::renderSelection(ImDrawList* drawList, ImVec2 pos, float cellWidth,
                                     float lineHeight, float padX, float padY,
                                     int firstVisibleLine, int lastVisibleLine)
{
    if (!editor_->hasSelection())
        return;

    int selMin = std::min(editor_->selectionStart, editor_->selectionEnd);
    int selMax = std::max(editor_->selectionStart, editor_->selectionEnd);

    ImU32 selectionColor = IM_COL32(60, 120, 200, 100);

    int selMinLine, selMinCol, selMaxLine, selMaxCol;
    editor_->indexToLineCol(selMin, selMinLine, selMinCol);
    editor_->indexToLineCol(selMax, selMaxLine, selMaxCol);

    for (int line = selMinLine; line <= selMaxLine && line < (int)editor_->lineCache.size(); line++)
    {
        if (line < firstVisibleLine || line >= lastVisibleLine)
            continue;

        int colStart = (line == selMinLine) ? selMinCol : 0;
        int colEnd = (line == selMaxLine) ? selMaxCol : (int)editor_->lineCache[line].text.size();

        float lineY = pos.y + padY - editor_->scrollY + (line - firstVisibleLine) * lineHeight + firstVisibleLine * lineHeight;
        float x1 = pos.x + padX - editor_->scrollX + colStart * cellWidth;
        float x2 = pos.x + padX - editor_->scrollX + colEnd * cellWidth;

        drawList->AddRectFilled(ImVec2(x1, lineY), ImVec2(x2, lineY + lineHeight), selectionColor);
    }
}

void EditorRenderer::renderVisibleLines(ImDrawList* drawList, ImVec2 pos, float padX, float padY,
                                        int firstVisibleLine, int lastVisibleLine)
{
    editor_->maxContentWidth = 0.0f;
    float y = pos.y + padY - (editor_->scrollY - firstVisibleLine * editor_->lineHeight);

    for (int i = firstVisibleLine; i < lastVisibleLine && i < (int)editor_->lineCache.size(); i++)
    {
        const auto& cl = editor_->lineCache[i];
        if (cl.width > editor_->maxContentWidth)
            editor_->maxContentWidth = cl.width;
        ImVec2 textPos(roundf(pos.x + padX - editor_->scrollX), roundf(y));
        drawList->AddText(textPos, ImGui::GetColorU32(ImGuiCol_Text), cl.text.c_str());
         y += editor_->lineHeight;
    }
}

void EditorRenderer::handleMouseInput(ImVec2 pos, float padX, float padY, float cellWidth, float lineHeight)
{
    ImGuiIO& io = ImGui::GetIO();
    
    if (ImGui::IsItemClicked())
    {
        ImGui::SetKeyboardFocusHere();

        if (editor_->lineCache.empty())
        {
            editor_->cursorIndex = 0;
            editor_->selectionStart = editor_->selectionEnd = -1;
            editor_->isDragging = false;
        }
        else
        {
            ImVec2 clickPos = io.MousePos;
            float localX = clickPos.x - pos.x - padX + editor_->scrollX;
            float localY = clickPos.y - pos.y - padY + editor_->scrollY;

            int clickedLine = (int)(localY / lineHeight);
            clickedLine = std::clamp(clickedLine, 0, (int)editor_->lineCache.size() - 1);

            int clickedCol = (int)(localX / cellWidth + 0.5f);
            clickedCol = std::clamp(clickedCol, 0, (int)editor_->lineCache[clickedLine].text.size());

            editor_->cursorIndex = editor_->lineColToIndex(clickedLine, clickedCol);

            if (!io.KeyShift)
            {
                editor_->selectionStart = editor_->selectionEnd = -1;
            }
            editor_->isDragging = true;
        }
    }

    if (editor_->isDragging && ImGui::IsMouseDown(ImGuiMouseButton_Left))
    {
        if (!editor_->lineCache.empty())
        {
            ImVec2 dragPos = io.MousePos;
            float localX = dragPos.x - pos.x - padX + editor_->scrollX;
            float localY = dragPos.y - pos.y - padY + editor_->scrollY;

            int dragLine = (int)(localY / lineHeight);
            dragLine = std::clamp(dragLine, 0, (int)editor_->lineCache.size() - 1);

            int dragCol = (int)(localX / cellWidth + 0.5f);
            dragCol = std::clamp(dragCol, 0, (int)editor_->lineCache[dragLine].text.size());

            int dragIndex = editor_->lineColToIndex(dragLine, dragCol);

            if (editor_->selectionStart == -1)
            {
                editor_->selectionStart = editor_->cursorIndex;
            }
            editor_->cursorIndex = dragIndex;
            editor_->selectionEnd = dragIndex;
        }
    }

    if (editor_->isDragging && !ImGui::IsMouseDown(ImGuiMouseButton_Left))
    {
        editor_->isDragging = false;
        if (editor_->selectionStart == editor_->selectionEnd)
        {
            editor_->selectionStart = editor_->selectionEnd = -1;
        }
    }
}

void EditorRenderer::handleKeyboardInput()
{
    ImGuiIO& io = ImGui::GetIO();
    
    // Enter key
    if (ImGui::IsKeyPressed(ImGuiKey_Enter) || ImGui::IsKeyPressed(ImGuiKey_KeypadEnter))
    {
        editor_->deleteSelection();
        editor_->applyInsert(editor_->cursorIndex, "\n");
        // editor_->cursorIndex is updated by applyInsert
        editor_->caretFollow = true;
    }

    // Text input
    for (unsigned int c : io.InputQueueCharacters)
    {
        if (c == '\n' || c == '\r')
            continue;
        if (c >= 32)
        {
            editor_->deleteSelection();
            char ch = (char)c;
            editor_->applyInsert(editor_->cursorIndex, std::string(1, ch));
            editor_->caretFollow = true;
            // onTextChanged called inside applyInsert
        }
    }

    if (ImGui::IsKeyPressed(ImGuiKey_Backspace))
    {
        if (editor_->hasSelection())
        {
            editor_->deleteSelection();
            // onTextChanged already triggered
        }
        else if (editor_->cursorIndex > 0)
        {
            editor_->applyErase(editor_->cursorIndex - 1, 1);
        }
        editor_->caretFollow = true;
    }

    if (ImGui::IsKeyPressed(ImGuiKey_Delete))
    {
        if (editor_->hasSelection())
        {
            editor_->deleteSelection();
            // onTextChanged already triggered
        }
        else if (editor_->cursorIndex < (int)editor_->content.size())
        {
            editor_->applyErase(editor_->cursorIndex, 1);
        }
        editor_->caretFollow = true;
    }

    // Arrow keys with shift selection support
    if (ImGui::IsKeyPressed(ImGuiKey_LeftArrow))
    {
        if (io.KeyShift)
        {
            if (editor_->selectionStart == -1)
                editor_->selectionStart = editor_->cursorIndex;
            if (editor_->cursorIndex > 0)
                editor_->cursorIndex--;
            editor_->selectionEnd = editor_->cursorIndex;
        }
        else
        {
            if (editor_->hasSelection())
            {
                editor_->cursorIndex = std::min(editor_->selectionStart, editor_->selectionEnd);
                editor_->selectionStart = editor_->selectionEnd = -1;
            }
            else if (editor_->cursorIndex > 0)
            {
                editor_->cursorIndex--;
            }
        }
        editor_->caretFollow = true;
    }

    if (ImGui::IsKeyPressed(ImGuiKey_RightArrow))
    {
        if (io.KeyShift)
        {
            if (editor_->selectionStart == -1)
                editor_->selectionStart = editor_->cursorIndex;
            if (editor_->cursorIndex < (int)editor_->content.size())
                editor_->cursorIndex++;
            editor_->selectionEnd = editor_->cursorIndex;
        }
        else
        {
            if (editor_->hasSelection())
            {
                editor_->cursorIndex = std::max(editor_->selectionStart, editor_->selectionEnd);
                editor_->selectionStart = editor_->selectionEnd = -1;
            }
            else if (editor_->cursorIndex < (int)editor_->content.size())
            {
                editor_->cursorIndex++;
            }
        }
        editor_->caretFollow = true;
    }

    if (ImGui::IsKeyPressed(ImGuiKey_UpArrow))
    {
        int line, col;
        editor_->indexToLineCol(editor_->cursorIndex, line, col);
        if (line > 0)
        {
            if (io.KeyShift && editor_->selectionStart == -1)
                editor_->selectionStart = editor_->cursorIndex;
            editor_->cursorIndex = editor_->lineColToIndex(line - 1, col);
            if (io.KeyShift)
                editor_->selectionEnd = editor_->cursorIndex;
            else
                editor_->selectionStart = editor_->selectionEnd = -1;
            editor_->caretFollow = true;
        }
    }

    if (ImGui::IsKeyPressed(ImGuiKey_DownArrow))
    {
        int line, col;
        editor_->indexToLineCol(editor_->cursorIndex, line, col);
        if (line < (int)editor_->lineCache.size() - 1)
        {
            if (io.KeyShift && editor_->selectionStart == -1)
                editor_->selectionStart = editor_->cursorIndex;
            editor_->cursorIndex = editor_->lineColToIndex(line + 1, col);
            if (io.KeyShift)
                editor_->selectionEnd = editor_->cursorIndex;
            else
                editor_->selectionStart = editor_->selectionEnd = -1;
            editor_->caretFollow = true;
        }
    }

    if (ImGui::IsKeyPressed(ImGuiKey_Home))
    {
        int line, col;
        editor_->indexToLineCol(editor_->cursorIndex, line, col);
        if (io.KeyShift && editor_->selectionStart == -1)
            editor_->selectionStart = editor_->cursorIndex;
        editor_->cursorIndex = editor_->lineColToIndex(line, 0);
        if (io.KeyShift)
            editor_->selectionEnd = editor_->cursorIndex;
        else
            editor_->selectionStart = editor_->selectionEnd = -1;
        editor_->caretFollow = true;
    }

    if (ImGui::IsKeyPressed(ImGuiKey_End))
    {
        int line, col;
        editor_->indexToLineCol(editor_->cursorIndex, line, col);
        if (line < (int)editor_->lineCache.size())
        {
            if (io.KeyShift && editor_->selectionStart == -1)
                editor_->selectionStart = editor_->cursorIndex;
            editor_->cursorIndex = editor_->lineColToIndex(line, (int)editor_->lineCache[line].text.size());
            if (io.KeyShift)
                editor_->selectionEnd = editor_->cursorIndex;
            else
                editor_->selectionStart = editor_->selectionEnd = -1;
        }
        editor_->caretFollow = true;
    }

    // Clipboard operations
    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_C))
    {
        editor_->copySelection();
    }
    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_V))
    {
        editor_->pasteFromClipboard();
    }
    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_X))
    {
        editor_->cutSelection();
    }
    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_A))
    {
        editor_->selectAll();
    }

    // Undo / Redo
    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_Z))
    {
        editor_->undo();
    }
    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_Y))
    {
        editor_->redo();
    }

    // Mouse wheel scrolling
    if (!io.KeyShift && io.MouseWheel != 0.0f)
    {
        editor_->scrollY -= io.MouseWheel * editor_->lineHeight * 3.0f;
    }

    if (io.KeyShift && io.MouseWheel != 0.0f)
    {
        editor_->scrollX -= io.MouseWheel * 40.0f;
    }
}

void EditorRenderer::renderCaret(ImDrawList* drawList, ImVec2 pos, float padX, float padY,
                                 float cellWidth, float lineHeight, bool isFocused)
{
    if (!isFocused || (int)(ImGui::GetTime() * 2) % 2 != 0)
        return;

    int caretLine, caretCol;
    editor_->indexToLineCol(editor_->cursorIndex, caretLine, caretCol);

    // padX here is expected to include gutter offset
    float caretXLocal = padX + caretCol * cellWidth;
    float caretYLocal = (caretLine) * lineHeight;

    float caretScreenX = pos.x + caretXLocal - editor_->scrollX;
    float caretScreenY = pos.y + padY + caretYLocal - editor_->scrollY;

    float caretTop = caretScreenY + lineHeight * 0.15f;
    float caretBottom = caretTop + lineHeight * 0.75f;
    drawList->AddLine(ImVec2(caretScreenX, caretTop),
                     ImVec2(caretScreenX, caretBottom),
                     ImGui::GetColorU32(ImGuiCol_Text), 2.0f);
}

void EditorRenderer::handleEditorInput(ImVec2 pos, float viewW, float viewH, float padX, float padY,
                                       float cellWidth, float lineHeight, bool isFocused)
{
    // padX here should already include gutter when called
    handleMouseInput(pos, padX, padY, cellWidth, lineHeight);
    
    if (isFocused)
    {
        handleKeyboardInput();
        
        // Auto-scroll caret into view
        if (editor_->caretFollow)
        {
            int caretLine, caretCol;
            editor_->indexToLineCol(editor_->cursorIndex, caretLine, caretCol);

            float caretXLocal = padX + caretCol * cellWidth;
            float caretYLocal = caretLine * lineHeight;

            float visibleW = viewW - padX * 2.0f;
            float visibleH = viewH - padY * 2.0f;

            // Horizontal
            if (caretXLocal < editor_->scrollX)
                editor_->scrollX = caretXLocal;
            if (caretXLocal > editor_->scrollX + visibleW)
                editor_->scrollX = caretXLocal - visibleW;

            // Vertical
            if (caretYLocal < editor_->scrollY)
                editor_->scrollY = caretYLocal;
            if (caretYLocal + lineHeight > editor_->scrollY + visibleH)
                editor_->scrollY = caretYLocal + lineHeight - visibleH;

            editor_->caretFollow = false;
        }
    }
}

void EditorRenderer::renderHorizontalScrollbar(ImVec2 pos, float viewW, float viewH, float scrollbarH,
                                               float visibleW, float trackW, float extraPad)
{
    ImVec2 hBarPos = ImVec2(pos.x, pos.y + viewH);
    
    ImU32 bgCol = ImGui::GetColorU32(ImGuiCol_FrameBgHovered);
    ImU32 fillCol = ImGui::GetColorU32(ImGuiCol_ScrollbarGrab);
    ImU32 fillHot = ImGui::GetColorU32(ImGuiCol_ScrollbarGrabHovered);
    ImU32 fillAct = ImGui::GetColorU32(ImGuiCol_ScrollbarGrabActive);

    ImDrawList* dl = ImGui::GetWindowDrawList();
    dl->AddRectFilled(hBarPos, ImVec2(hBarPos.x + trackW, hBarPos.y + scrollbarH), bgCol);

    float contentW = std::max(visibleW, editor_->maxContentWidth + extraPad);
    float minThumb = 24.0f;

    float thumbW = (visibleW / contentW) * trackW;
    thumbW = std::clamp(thumbW, minThumb, trackW);

    float trackRange = trackW - thumbW;
    float denom = std::max(1.0f, contentW - visibleW);
    float thumbX = (denom > 0.0f) ? (editor_->scrollX / denom) * trackRange : 0.0f;

    ImVec2 thumbMin = ImVec2(hBarPos.x + thumbX, hBarPos.y);
    ImVec2 thumbMax = ImVec2(hBarPos.x + thumbX + thumbW, hBarPos.y + scrollbarH);

    ImGuiIO& io = ImGui::GetIO();

    ImGui::SetCursorScreenPos(thumbMin);
    ImGui::InvisibleButton("hthumb", ImVec2(thumbW, scrollbarH));
    bool thumbHovered = ImGui::IsItemHovered();

    if (ImGui::IsItemActivated())
    {
        editor_->hDragging = true;
        editor_->hDragMouseStart = io.MousePos.x;
        editor_->hDragScrollStart = editor_->scrollX;
    }
    if (editor_->hDragging)
    {
        if (ImGui::IsMouseDown(ImGuiMouseButton_Left))
        {
            float dx = io.MousePos.x - editor_->hDragMouseStart;
            float newThumbX = (editor_->hDragScrollStart / denom) * trackRange + dx;
            newThumbX = std::clamp(newThumbX, 0.0f, trackRange);
            editor_->scrollX = (denom > 0.0f) ? (newThumbX / trackRange) * denom : 0.0f;
        }
        else
        {
            editor_->hDragging = false;
        }
    }

    ImGui::SetCursorScreenPos(hBarPos);
    ImGui::InvisibleButton("htrack", ImVec2(trackW, scrollbarH));
    if (ImGui::IsItemClicked())
    {
        float mouseX = io.MousePos.x - hBarPos.x;
        float page = visibleW * 0.8f;
        if (mouseX < thumbX)
            editor_->scrollX = std::max(0.0f, editor_->scrollX - page);
        else if (mouseX > thumbX + thumbW)
        {
            float maxScrollX = std::max(0.0f, (editor_->maxContentWidth + extraPad) - visibleW);
            editor_->scrollX = std::min(maxScrollX, editor_->scrollX + page);
        }
    }

    ImU32 thumbCol = editor_->hDragging ? fillAct : (thumbHovered ? fillHot : fillCol);
    dl->AddRectFilled(thumbMin, thumbMax, thumbCol, 3.0f);
}

void EditorRenderer::renderVerticalScrollbar(ImVec2 pos, float viewW, float viewH, float scrollbarW,
                                             float visibleH, float trackH, float totalContentHeight)
{
    ImVec2 vBarPos = ImVec2(pos.x + viewW, pos.y);
    
    ImU32 bgCol = ImGui::GetColorU32(ImGuiCol_FrameBgHovered);
    ImU32 fillCol = ImGui::GetColorU32(ImGuiCol_ScrollbarGrab);
    ImU32 fillHot = ImGui::GetColorU32(ImGuiCol_ScrollbarGrabHovered);
    ImU32 fillAct = ImGui::GetColorU32(ImGuiCol_ScrollbarGrabActive);

    ImDrawList* dl = ImGui::GetWindowDrawList();
    dl->AddRectFilled(vBarPos, ImVec2(vBarPos.x + scrollbarW, vBarPos.y + trackH), bgCol);

    float minThumb = 24.0f;
    float thumbH = (visibleH / totalContentHeight) * trackH;
    thumbH = std::clamp(thumbH, minThumb, trackH);

    float vTrackRange = trackH - thumbH;
    float vDenom = std::max(1.0f, totalContentHeight - visibleH);
    float thumbY = (vDenom > 0.0f) ? (editor_->scrollY / vDenom) * vTrackRange : 0.0f;

    ImVec2 vThumbMin = ImVec2(vBarPos.x, vBarPos.y + thumbY);
    ImVec2 vThumbMax = ImVec2(vBarPos.x + scrollbarW, vBarPos.y + thumbY + thumbH);

    ImGuiIO& io = ImGui::GetIO();

    ImGui::SetCursorScreenPos(vThumbMin);
    ImGui::InvisibleButton("vthumb", ImVec2(scrollbarW, thumbH));
    bool vThumbHovered = ImGui::IsItemHovered();

    if (ImGui::IsItemActivated())
    {
        editor_->vDragging = true;
        editor_->vDragMouseStart = io.MousePos.y;
        editor_->vDragScrollStart = editor_->scrollY;
    }
    if (editor_->vDragging)
    {
        if (ImGui::IsMouseDown(ImGuiMouseButton_Left))
        {
            float dy = io.MousePos.y - editor_->vDragMouseStart;
            float newThumbY = (editor_->vDragScrollStart / vDenom) * vTrackRange + dy;
            newThumbY = std::clamp(newThumbY, 0.0f, vTrackRange);
            editor_->scrollY = (vDenom > 0.0f) ? (newThumbY / vTrackRange) * vDenom : 0.0f;
        }
        else
        {
            editor_->vDragging = false;
        }
    }

    ImGui::SetCursorScreenPos(vBarPos);
    ImGui::InvisibleButton("vtrack", ImVec2(scrollbarW, trackH));
    if (ImGui::IsItemClicked())
    {
        float mouseY = io.MousePos.y - vBarPos.y;
        float page = visibleH * 0.8f;
        if (mouseY < thumbY)
            editor_->scrollY = std::max(0.0f, editor_->scrollY - page);
        else if (mouseY > thumbY + thumbH)
        {
            float maxScrollY = std::max(0.0f, totalContentHeight - visibleH);
            editor_->scrollY = std::min(maxScrollY, editor_->scrollY + page);
        }
    }

    ImU32 vThumbCol = editor_->vDragging ? fillAct : (vThumbHovered ? fillHot : fillCol);
    dl->AddRectFilled(vThumbMin, vThumbMax, vThumbCol, 3.0f);
}

void EditorRenderer::renderScrollbars(ImVec2 pos, float viewW, float viewH, float scrollbarW,
                                      float scrollbarH, float cellWidth, float lineHeight,
                                      float visibleW, float visibleH)
{
    float extraPad = cellWidth;
    float totalContentHeight = editor_->lineCache.size() * lineHeight;
    
    renderHorizontalScrollbar(pos, viewW, viewH, scrollbarH, visibleW, viewW, extraPad);
    renderVerticalScrollbar(pos, viewW, viewH, scrollbarW, visibleH, viewH, totalContentHeight);
    
    // Clamp scrolls
    float maxScrollX = std::max(0.0f, (editor_->maxContentWidth + extraPad) - visibleW);
    editor_->scrollX = std::clamp(editor_->scrollX, 0.0f, maxScrollX);

    float maxScrollY = std::max(0.0f, totalContentHeight - visibleH);
    editor_->scrollY = std::clamp(editor_->scrollY, 0.0f, maxScrollY);
}

void EditorRenderer::renderEditor()
{
    if (ImGui::Begin("Editor", nullptr,
                     ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoResize |
                     ImGuiWindowFlags_NoCollapse | ImGuiWindowFlags_NoTitleBar |
                     ImGuiWindowFlags_NoBringToFrontOnFocus))
    {
        ImDrawList* drawList = ImGui::GetWindowDrawList();
        ImVec2 pos = ImGui::GetCursorScreenPos();

        // Monospaced grid metrics
        editor_->lineHeight = ImGui::GetTextLineHeightWithSpacing();
        const char* sample = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        float cellWidth = ImGui::CalcTextSize(sample).x / (float)strlen(sample);

        // Layout
        ImVec2 winSize = ImGui::GetContentRegionAvail();
        const float padX = 4.0f;
        const float padY = 4.0f;
        const float scrollbarW = 12.0f;
        const float scrollbarH = 12.0f;
        const float viewW = winSize.x - scrollbarW;
        const float viewH = winSize.y - scrollbarH;

        // compute visible range
        int firstVisibleLine = std::max(0, (int)std::floor(editor_->scrollY / editor_->lineHeight));
        int lastVisibleLine = std::min((int)editor_->lineCache.size(), firstVisibleLine + (int)std::ceil(viewH / editor_->lineHeight) + 1);

        // gutter for line numbers
        // compute gutterWidth (does NOT include the external padX)
        float gutterWidth = editor_->showLineNumbers ? computeGutterWidth((int)editor_->lineCache.size(), cellWidth, padX) : 0.0f;
        // textPadX shifts the text area: gutter occupies gutterWidth, then we apply padX between gutter and text
        float textPadX = gutterWidth + padX;

        // Background
        drawList->AddRectFilled(pos, ImVec2(pos.x + winSize.x, pos.y + winSize.y),
                               ImGui::GetColorU32(ImGuiCol_FrameBg));

        // draw gutter background + line numbers
        if (editor_->showLineNumbers)
        {
            // gutter should start flush at the left content area
            ImVec2 gutterStart = ImVec2(pos.x, pos.y);
            ImVec2 gutterEnd   = ImVec2(pos.x + gutterWidth, pos.y + viewH);

            ImU32 gutterBg = ImGui::GetColorU32(ImGuiCol_ScrollbarBg);
            // round coordinates for crisp fill
            drawList->AddRectFilled(ImVec2(roundf(gutterStart.x), roundf(gutterStart.y)),
                                    ImVec2(roundf(gutterEnd.x), roundf(gutterEnd.y)),
                                    gutterBg);

            // compute digits reserved (must match computeGutterWidth logic: reserve at least 6)
            int totalLines = std::max(1, (int)editor_->lineCache.size());
            int digits = 1;
            int tmp = totalLines;
            while (tmp >= 10) { tmp /= 10; digits++; }
            digits = std::max(digits, 6);

            // internal padding used for numbers (one character on each side)
            float sidePad = cellWidth;
            // inner area where numbers live: start + sidePad, width = digits*cellWidth
            float innerStartX = gutterStart.x + sidePad;
            float innerWidth  = (float)digits * cellWidth;

            for (int line = firstVisibleLine; line < lastVisibleLine; ++line)
            {
                char buf[32];
                snprintf(buf, sizeof(buf), "%d", line + 1);
                float numWidth = ImGui::CalcTextSize(buf).x;

                // horizontally center the number inside the reserved digits area and round
                float numX = innerStartX + (innerWidth - numWidth) * 0.5f;
                numX = roundf(numX);

                // vertical center the number within the line and round
                float lineTop = pos.y + padY + (line - firstVisibleLine) * editor_->lineHeight - (editor_->scrollY - firstVisibleLine * editor_->lineHeight);
                float textLineHeight = ImGui::GetTextLineHeight();
                float numY = lineTop + (editor_->lineHeight - textLineHeight) * 0.5f;
                numY = roundf(numY);

                drawList->AddText(ImVec2(numX, numY), ImGui::GetColorU32(ImGuiCol_TextDisabled), buf);
            }
        }

        // Grid
        renderGrid(drawList, pos, viewW, viewH, cellWidth, editor_->lineHeight, textPadX, padY);

        // Editor interactive area
        ImGui::InvisibleButton("editor_area", ImVec2(viewW, viewH), ImGuiButtonFlags_MouseButtonLeft);
        bool isFocused = ImGui::IsItemFocused();

        // render selection, lines, caret, and handle input — pass textPadX so rendering is shifted
        renderSelection(drawList, pos, cellWidth, editor_->lineHeight, textPadX, padY, firstVisibleLine, lastVisibleLine);
        renderVisibleLines(drawList, pos, textPadX, padY, firstVisibleLine, lastVisibleLine);
        renderCaret(drawList, pos, textPadX, padY, cellWidth, editor_->lineHeight, isFocused);

        handleEditorInput(pos, viewW, viewH, textPadX, padY, cellWidth, editor_->lineHeight, isFocused);

        // scrollbars (leave cellWidth so they can compute extraPad)
        renderScrollbars(pos, viewW, viewH, scrollbarW, scrollbarH, cellWidth, editor_->lineHeight, viewW, viewH);
    }
    ImGui::End();
}
