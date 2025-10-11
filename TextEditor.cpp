#include "TextEditor.hpp"
#include "LuaBindings.hpp"
#include <cstring>
#include <filesystem>
#include <fstream>
#include <misc/cpp/imgui_stdlib.h>
#include <nfd.h>
#include <sstream>
#define NANOSVG_IMPLEMENTATION
#include "nanosvg.h"
#define NANOSVGRAST_IMPLEMENTATION
#include "imgui_internal.h"
#include "nanosvgrast.h"
#include <GLFW/glfw3.h>

TextEditor::TextEditor()
    : content(), filename(""), modified(false), showFileExplorer(true),
      showOutput(true), focusEditor(false), cursorLine(0), cursorColumn(0),
      selectionStart(-1), selectionEnd(-1),
      isDragging(false) { // <-- ADD THIS LINE
  inputFlags =
      ImGuiInputTextFlags_AllowTabInput | ImGuiInputTextFlags_CallbackAlways;
  lua_ = new LuaBindings(this);
  loadIcons(ImGui::GetIO().FontGlobalScale);
  registerCommands();
  lua_->loadPlugins();
  refreshFileList();
}

TextEditor::~TextEditor() { delete lua_; }

// prevent new windows from escaping viewport (currently only used by Settings)
// Lock it in the main window
static void ClampToViewport(ImGuiSizeCallbackData *data) {
  ImGuiViewport *viewport = ImGui::GetMainViewport();
  ImVec2 workMin = viewport->WorkPos;
  ImVec2 work_max(viewport->WorkPos.x + viewport->WorkSize.x,
                  viewport->WorkPos.y + viewport->WorkSize.y);
  ImVec2 pos = ImGui::GetWindowPos();
  ImVec2 size = data->CurrentSize;
  if (pos.x < workMin.x)
    pos.x = workMin.x;
  if (pos.y < workMin.y)
    pos.y = workMin.y;
  if (pos.x + size.x > work_max.x)
    pos.x = work_max.x - size.x;
  if (pos.y + size.y > work_max.y)
    pos.y = work_max.y - size.y;
  ImGui::SetWindowPos(pos, ImGuiCond_Always);
}

// ======= Menu bar =======
void TextEditor::renderMenuBar() {
  if (ImGui::BeginMainMenuBar()) {
    if (ImGui::BeginMenu("File")) {
      if (ImGui::MenuItem("New", "Ctrl+N"))
        newFile();
      if (ImGui::MenuItem("Open", "Ctrl+O")) {
        showOpenDialog();
      }
      if (ImGui::MenuItem("Save As", "Ctrl+Shift+S")) {
        showSaveDialog(filename.empty() ? "untitled.txt" : filename);
      }
      if (ImGui::MenuItem("Save", "Ctrl+S"))
        saveFile();
      ImGui::Separator();
      if (ImGui::MenuItem("Exit", "Alt+F4")) {
        closeEditor = true;
      }
      ImGui::EndMenu();
    }

    if (ImGui::BeginMenu("View")) {
      ImGui::MenuItem("File Explorer", nullptr, &showFileExplorer);
      ImGui::MenuItem("Output Panel", nullptr, &showOutput);
      ImGui::MenuItem("Show Gridlines", nullptr, &showGrid);
      ImGui::EndMenu();
    }

    if (ImGui::BeginMenu("Plugins")) {
      if (ImGui::MenuItem("Reload Plugins")) {
        lua_->loadPlugins();
        addOutput("Plugins reloaded!");
      }
      if (ImGui::MenuItem("List Commands")) {
        addOutput("Available commands:");
        for (const auto &cmd : commands) {
          addOutput("  • " + cmd.first);
        }
      }
      ImGui::EndMenu();
    }
    ImGui::EndMainMenuBar();
  }
}

// ======= Explorer =======
void TextEditor::renderExplorer(ImVec2 workPos, ImVec2 workSize,
                                float explorerWidth) {
  if (ImGui::Begin("File Explorer", nullptr,
                   ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoResize |
                       ImGuiWindowFlags_NoCollapse |
                       ImGuiWindowFlags_NoBringToFrontOnFocus)) {
    if (IconTextButton("##refresh_btn", icons["refresh"], "Refresh",
                       ImVec2(120, 30))) {
      refreshFileList();
    }
    ImGui::Separator();
    ImGui::BeginChild("FileList", ImVec2(0, -40), true);

    for (size_t i = 0; i < fileList.size(); ++i) {
      const auto &file = fileList[i];
      bool isDirectory = std::filesystem::is_directory(file);
      ImGui::PushID((int)i);
      ImGui::Selectable("##fileitem", false);
      ImVec2 pos = ImGui::GetItemRectMin();
      ImVec2 size = ImGui::GetItemRectSize();
      ImTextureID iconTex = isDirectory ? icons["folder"] : icons["document"];
      float iconSize = 18.0f;
      ImVec2 iconPos(pos.x + 4, pos.y + (size.y - iconSize) * 0.5f);
      ImGui::GetWindowDrawList()->AddImage(
          iconTex, iconPos, ImVec2(iconPos.x + iconSize, iconPos.y + iconSize),
          ImVec2(0, 0), ImVec2(1, 1), ImGui::GetColorU32(ImGuiCol_Text));
      ImVec2 textPos(iconPos.x + iconSize + 6,
                     pos.y + (size.y - ImGui::GetTextLineHeight()) * 0.5f);
      ImGui::GetWindowDrawList()->AddText(
          textPos, ImGui::GetColorU32(ImGuiCol_Text), file.c_str());
      if (ImGui::IsItemClicked() && !isDirectory) {
        openFile(file);
      }

      ImGui::PopID();
    }
    ImGui::EndChild();
    ImGui::Separator();
    if (IconTextButton("##settings_btn", icons["settings"], "Settings",
                       ImVec2(120, 30))) {
      showSettings = !showSettings;
    }
  }
  ImGui::End();
}

// ======= Output =======
void TextEditor::renderOutput(ImVec2 workPos, ImVec2 workSize,
                              float outputHeight, float explorerWidth) {
  ImVec2 outputPos(workPos.x + explorerWidth,
                   workPos.y + workSize.y - outputHeight);
  ImVec2 outputSize(workSize.x - explorerWidth, outputHeight);
  ImGui::SetNextWindowPos(outputPos, ImGuiCond_Always);
  ImGui::SetNextWindowSize(outputSize, ImGuiCond_Always);
  if (ImGui::Begin("Output", nullptr,
                   ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoResize |
                       ImGuiWindowFlags_NoCollapse |
                       ImGuiWindowFlags_NoBringToFrontOnFocus)) {
    ImGui::Text("Command:");
    ImGui::SameLine();
    ImGui::SetNextItemWidth(-100);
    bool commandEntered = ImGui::InputText(
        "##command", &commandInput, ImGuiInputTextFlags_EnterReturnsTrue);
    ImGui::SameLine();
    if (ImGui::Button("Execute") || commandEntered) {
      if (!commandInput.empty()) {
        executeCommand(commandInput);
        commandInput.clear();
      }
    }
    ImGui::Separator();
    ImGui::BeginChild("OutputText");
    float iconSize = 18.0f;
    for (const auto &line : outputLines) {
      ImVec2 pos = ImGui::GetCursorScreenPos();

      if (line.icon != (ImTextureID)0) {
        ImGui::GetWindowDrawList()->AddImage(
            line.icon, pos, ImVec2(pos.x + iconSize, pos.y + iconSize),
            ImVec2(0, 0), ImVec2(1, 1), IM_COL32_WHITE);
        ImGui::SetCursorScreenPos(ImVec2(pos.x + iconSize + 6, pos.y));
      }

      ImGui::TextWrapped("%s", line.text.c_str());
    }

    // Auto-scroll the output window
    if (ImGui::GetScrollY() >= ImGui::GetScrollMaxY()) {
      ImGui::SetScrollHereY(1.0f);
    }
    ImGui::EndChild();
  }
  ImGui::End();
}

// ======= Settings =======
void TextEditor::renderSettings() {
  ImGui::SetNextWindowSize(ImVec2(500, 400), ImGuiCond_Once);
  ImGui::SetNextWindowSizeConstraints(
      ImVec2(200, 100), ImVec2(FLT_MAX, FLT_MAX), ClampToViewport);
  ImGui::Begin("Settings", &showSettings,
               ImGuiWindowFlags_NoCollapse | ImGuiWindowFlags_NoDocking);
  ImGui::Text("Editor Settings");
  lua_->eval("if show_font_menu then show_font_menu() end");
  ImGui::End();
}

// ======= Open/Save dialogs =======
void TextEditor::showOpenDialog() {
  nfdchar_t *outPath = nullptr;

  nfdu8filteritem_t filters[4] = {{"C++ Source", "cpp"},
                                  {"C++ Header", "hpp"},
                                  {"Text File", "txt"},
                                  {"Lua Script", "lua"}};

  // Initialize NFD only if needed
  static bool nfd_initialized = false;
  if (!nfd_initialized) {
    if (NFD_Init() == NFD_OKAY) {
      nfd_initialized = true;
    }
  }

  nfdresult_t result = NFD_OpenDialog(&outPath, filters, 4, nullptr);
  if (result == NFD_OKAY) {
    openFile(outPath);
    free(outPath);
  } else if (result == NFD_ERROR) {
    addOutput(icons["error"], std::string("NFD Error: ") + NFD_GetError());
  }
}

void TextEditor::showSaveDialog(const std::string &defaultFileName) {
  nfdchar_t *savePath = nullptr;

  nfdu8filteritem_t filters[4] = {{"C++ Source", "cpp"},
                                  {"C++ Header", "hpp"},
                                  {"Text File", "txt"},
                                  {"Lua Script", "lua"}};

  // Initialize NFD only if needed
  static bool nfd_initialized = false;
  if (!nfd_initialized) {
    if (NFD_Init() == NFD_OKAY) {
      nfd_initialized = true;
    }
  }

  nfdresult_t result =
      NFD_SaveDialog(&savePath, filters, 4, nullptr, defaultFileName.c_str());
  if (result == NFD_OKAY) {
    filename = savePath;
    saveFile();
    free(savePath);
  } else if (result == NFD_ERROR) {
    addOutput(icons["error"], std::string("NFD Error: ") + NFD_GetError());
  }
}

// ======= Main render (called every frame by your app) =======
bool TextEditor::render() {
  ImGuiStyle &style = ImGui::GetStyle();
  style.Colors[ImGuiCol_TitleBgActive] = style.Colors[ImGuiCol_TitleBg];

  // Global shortcuts
  handleKeyboardShortcuts();

  // Main menu bar
  renderMenuBar();

  // File Explorer
  ImGuiViewport *viewport = ImGui::GetMainViewport();
  ImVec2 workPos = viewport->WorkPos;
  ImVec2 workSize = viewport->WorkSize;
  float explorerWidth = showFileExplorer ? 250.0f : 0.0f;
  float outputHeight = showOutput ? 200.0f : 0.0f;

  if (showFileExplorer) {
    ImGui::SetNextWindowPos(workPos, ImGuiCond_Always);
    ImGui::SetNextWindowSize(ImVec2(explorerWidth, workSize.y),
                             ImGuiCond_Always);
    renderExplorer(workPos, workSize, explorerWidth);
  }

  // Editor
  ImVec2 editorPos(workPos.x + explorerWidth, workPos.y);
  ImVec2 editorSize(workSize.x - explorerWidth, workSize.y - outputHeight);
  ImGui::SetNextWindowPos(editorPos, ImGuiCond_Always);
  ImGui::SetNextWindowSize(editorSize, ImGuiCond_Always);
  renderEditor();

  // Output Panel
  if (showOutput) {
    renderOutput(workPos, workSize, outputHeight, explorerWidth);
  }

  if (showSettings) {
    renderSettings();
  }

  // Run Lua hooks
  lua_->runHook("on_text_input");
  lua_->runHook("on_render");

  return closeEditor;
}

// ======= Global shortcuts =======
// Replace your handleKeyboardShortcuts function with this:

void TextEditor::handleKeyboardShortcuts() {
  ImGuiIO &io = ImGui::GetIO();

  // File operations work globally
  if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_N))
    newFile();
  if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_O))
    showOpenDialog();
  if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_S)) {
    if (io.KeyShift) {
      showSaveDialog(filename.empty() ? "untitled.txt" : filename);
    } else {
      saveFile();
    }
  }
}

// ======= Editor (monospaced grid) =======
void TextEditor::renderEditor() {
  if (ImGui::Begin("Editor", nullptr,
                   ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoResize |
                       ImGuiWindowFlags_NoCollapse |
                       ImGuiWindowFlags_NoTitleBar |
                       ImGuiWindowFlags_NoBringToFrontOnFocus)) {
    ImDrawList *draw_list = ImGui::GetWindowDrawList();
    ImVec2 pos = ImGui::GetCursorScreenPos();

    // Monospaced grid metrics
    float lineHeight = ImGui::GetTextLineHeightWithSpacing();
    const char *sample =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    float cellWidth = ImGui::CalcTextSize(sample).x / (float)strlen(sample);

    // Layout
    ImVec2 winSize = ImGui::GetContentRegionAvail();
    const float padX = 4.0f;
    const float padY = 4.0f;
    const float scrollbarW = 12.0f;
    const float scrollbarH = 12.0f;
    const float viewW = winSize.x - scrollbarW;
    const float viewH = winSize.y - scrollbarH;

    // Background
    draw_list->AddRectFilled(pos, ImVec2(pos.x + winSize.x, pos.y + winSize.y),
                             ImGui::GetColorU32(ImGuiCol_FrameBg));
    if (showGrid) {
      ImU32 gridColor = IM_COL32(100, 100, 100, 50);
      float gridSpacingX = cellWidth;
      float gridSpacingY = lineHeight;

      float startX = pos.x + padX - fmodf(scrollX, gridSpacingX);
      float startY = pos.y + padY - fmodf(scrollY, gridSpacingY);

      // Vertical columns
      for (float x = startX; x < pos.x + viewW; x += gridSpacingX)
        draw_list->AddLine(ImVec2(roundf(x), pos.y),
                           ImVec2(roundf(x), pos.y + viewH), gridColor);

      // Horizontal lines (middle of each line height)
      for (float y = startY; y < pos.y + viewH; y += gridSpacingY) {
        float yMid = roundf(
            y + gridSpacingY *
                    0.75f); // tweak this 0.75f if lines seem too high/low
        draw_list->AddLine(ImVec2(pos.x, yMid), ImVec2(pos.x + viewW, yMid),
                           gridColor);
      }
    }

    // Editor interactive area
    ImGuiIO &io = ImGui::GetIO(); // <-- ADD THIS HERE
    ImGui::InvisibleButton("editor_area", ImVec2(viewW, viewH),
                           ImGuiButtonFlags_MouseButtonLeft);
    bool isFocused = ImGui::IsItemFocused();

    // Replace the mouse click handling section with this safer version:

    if (ImGui::IsItemClicked()) {
      ImGui::SetKeyboardFocusHere();
      isFocused = true;

      // SAFETY CHECK: Ensure lineCache is not empty
      if (lineCache.empty()) {
        cursorIndex = 0;
        selectionStart = selectionEnd = -1;
        isDragging = false;
      } else {
        // Calculate click position to index
        ImVec2 clickPos = io.MousePos;
        float localX = clickPos.x - pos.x - padX + scrollX;
        float localY = clickPos.y - pos.y - padY + scrollY;

        int clickedLine = (int)(localY / lineHeight);
        clickedLine = ImClamp(clickedLine, 0, (int)lineCache.size() - 1);

        int clickedCol = (int)(localX / cellWidth + 0.5f);
        clickedCol =
            ImClamp(clickedCol, 0, (int)lineCache[clickedLine].text.size());

        cursorIndex = lineColToIndex(clickedLine, clickedCol);

        if (!io.KeyShift) {
          selectionStart = selectionEnd = -1;
        }
        isDragging = true;
      }
    }

    if (isDragging && ImGui::IsMouseDown(ImGuiMouseButton_Left)) {
      // SAFETY CHECK: Ensure lineCache is not empty
      if (!lineCache.empty()) {
        ImVec2 dragPos = io.MousePos;
        float localX = dragPos.x - pos.x - padX + scrollX;
        float localY = dragPos.y - pos.y - padY + scrollY;

        int dragLine = (int)(localY / lineHeight);
        dragLine = ImClamp(dragLine, 0, (int)lineCache.size() - 1);

        int dragCol = (int)(localX / cellWidth + 0.5f);
        dragCol = ImClamp(dragCol, 0, (int)lineCache[dragLine].text.size());

        int dragIndex = lineColToIndex(dragLine, dragCol);

        if (selectionStart == -1) {
          selectionStart = cursorIndex;
        }
        cursorIndex = dragIndex;
        selectionEnd = dragIndex;
      }
    }

    if (isDragging && !ImGui::IsMouseDown(ImGuiMouseButton_Left)) {
      isDragging = false;
      if (selectionStart == selectionEnd) {
        selectionStart = selectionEnd = -1;
      }
    }
    // ----- Clipping -----
    ImVec2 clipMin = ImVec2(pos.x, pos.y);
    ImVec2 clipMax = ImVec2(pos.x + viewW, pos.y + viewH);
    draw_list->PushClipRect(clipMin, clipMax, true);

    // Calculate visible line range
    int firstVisibleLine = (int)(scrollY / lineHeight);
    int visibleLineCount = (int)(viewH / lineHeight) + 2;
    int lastVisibleLine =
        std::min(firstVisibleLine + visibleLineCount, (int)lineCache.size());

    // ADD THIS ENTIRE BLOCK:
    // Draw selection highlight
    if (hasSelection()) {
      int selMin = std::min(selectionStart, selectionEnd);
      int selMax = std::max(selectionStart, selectionEnd);

      ImU32 selectionColor = IM_COL32(60, 120, 200, 100);

      int selMinLine, selMinCol, selMaxLine, selMaxCol;
      indexToLineCol(selMin, selMinLine, selMinCol);
      indexToLineCol(selMax, selMaxLine, selMaxCol);

      for (int line = selMinLine;
           line <= selMaxLine && line < (int)lineCache.size(); line++) {
        if (line < firstVisibleLine || line >= lastVisibleLine)
          continue;

        int colStart = (line == selMinLine) ? selMinCol : 0;
        int colEnd =
            (line == selMaxLine) ? selMaxCol : (int)lineCache[line].text.size();

        float lineY = pos.y + padY - scrollY + line * lineHeight;
        float x1 = pos.x + padX - scrollX + colStart * cellWidth;
        float x2 = pos.x + padX - scrollX + colEnd * cellWidth;

        draw_list->AddRectFilled(
            ImVec2(x1, lineY), ImVec2(x2, lineY + lineHeight), selectionColor);
      }
    }

    // Draw visible lines with offsets
    maxContentWidth = 0.0f;
    float y = pos.y + padY - (scrollY - firstVisibleLine * lineHeight);

    for (int i = firstVisibleLine; i < lastVisibleLine; i++) {
      const auto &cl = lineCache[i];
      if (cl.width > maxContentWidth)
        maxContentWidth = cl.width;
      ImVec2 textPos(roundf(pos.x + padX - scrollX), roundf(y)); // pixel snap
      draw_list->AddText(textPos, ImGui::GetColorU32(ImGuiCol_Text),
                         cl.text.c_str());
      y += lineHeight;
    }

    // ----- Input handling -----

    if (isFocused) {
      // Enter key
      if (ImGui::IsKeyPressed(ImGuiKey_Enter) ||
          ImGui::IsKeyPressed(ImGuiKey_KeypadEnter)) {
        deleteSelection();
        content.insert(cursorIndex, "\n");
        cursorIndex++;
        onTextChanged();
        caretFollow = true;
      }

      // Text input: accept printable chars; ignore explicit newlines here
      for (unsigned int c : io.InputQueueCharacters) {
        if (c == '\n' || c == '\r')
          continue;
        if (c >= 32) {
          deleteSelection();
          content.insert(cursorIndex, std::string(1, (char)c));
          cursorIndex++;
          caretFollow = true;
          onTextChanged();
        }
      }

      if (ImGui::IsKeyPressed(ImGuiKey_Backspace)) {
        if (hasSelection()) {
          deleteSelection();
          onTextChanged();
        } else if (cursorIndex > 0) {
          content.erase(cursorIndex - 1, 1);
          cursorIndex--;
          onTextChanged();
        }
        caretFollow = true;
      }

      if (ImGui::IsKeyPressed(ImGuiKey_Delete)) {
        if (hasSelection()) {
          deleteSelection();
          onTextChanged();
        } else if (cursorIndex < (int)content.size()) {
          content.erase(cursorIndex, 1);
          onTextChanged();
        }
        caretFollow = true;
      }

      if (ImGui::IsKeyPressed(ImGuiKey_LeftArrow)) {
        if (io.KeyShift) {
          if (selectionStart == -1)
            selectionStart = cursorIndex;
          if (cursorIndex > 0)
            cursorIndex--;
          selectionEnd = cursorIndex;
        } else {
          if (hasSelection()) {
            cursorIndex = std::min(selectionStart, selectionEnd);
            selectionStart = selectionEnd = -1;
          } else if (cursorIndex > 0) {
            cursorIndex--;
          }
        }
        caretFollow = true;
      }

      if (ImGui::IsKeyPressed(ImGuiKey_RightArrow)) {
        if (io.KeyShift) {
          if (selectionStart == -1)
            selectionStart = cursorIndex;
          if (cursorIndex < (int)content.size())
            cursorIndex++;
          selectionEnd = cursorIndex;
        } else {
          if (hasSelection()) {
            cursorIndex = std::max(selectionStart, selectionEnd);
            selectionStart = selectionEnd = -1;
          } else if (cursorIndex < (int)content.size()) {
            cursorIndex++;
          }
        }
        caretFollow = true;
      }

      if (ImGui::IsKeyPressed(ImGuiKey_UpArrow)) {
        int line, col;
        indexToLineCol(cursorIndex, line, col);
        if (line > 0) {
          if (io.KeyShift && selectionStart == -1)
            selectionStart = cursorIndex;
          cursorIndex = lineColToIndex(line - 1, col);
          if (io.KeyShift)
            selectionEnd = cursorIndex;
          else
            selectionStart = selectionEnd = -1;
          caretFollow = true;
        }
      }

      if (ImGui::IsKeyPressed(ImGuiKey_DownArrow)) {
        int line, col;
        indexToLineCol(cursorIndex, line, col);
        if (line < (int)lineCache.size() - 1) {
          if (io.KeyShift && selectionStart == -1)
            selectionStart = cursorIndex;
          cursorIndex = lineColToIndex(line + 1, col);
          if (io.KeyShift)
            selectionEnd = cursorIndex;
          else
            selectionStart = selectionEnd = -1;
          caretFollow = true;
        }
      }

      if (ImGui::IsKeyPressed(ImGuiKey_Home)) {
        int line, col;
        indexToLineCol(cursorIndex, line, col);
        if (io.KeyShift && selectionStart == -1)
          selectionStart = cursorIndex;
        cursorIndex = lineColToIndex(line, 0);
        if (io.KeyShift)
          selectionEnd = cursorIndex;
        else
          selectionStart = selectionEnd = -1;
        caretFollow = true;
      }

      if (ImGui::IsKeyPressed(ImGuiKey_End)) {
        int line, col;
        indexToLineCol(cursorIndex, line, col);
        if (line < (int)lineCache.size()) {
          if (io.KeyShift && selectionStart == -1)
            selectionStart = cursorIndex;
          cursorIndex = lineColToIndex(line, (int)lineCache[line].text.size());
          if (io.KeyShift)
            selectionEnd = cursorIndex;
          else
            selectionStart = selectionEnd = -1;
        }
        caretFollow = true;
      }

      if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_C)) {
        copySelection();
      }
      if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_V)) {
        pasteFromClipboard();
      }
      if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_X)) {
        cutSelection();
      }
      if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_A)) {
        selectAll();
      }

      // Mouse wheel for vertical scrolling
      if (!io.KeyShift && io.MouseWheel != 0.0f) {
        scrollY -= io.MouseWheel * lineHeight * 3.0f;
      }

      // Shift+wheel for horizontal scrolling
      if (io.KeyShift && io.MouseWheel != 0.0f) {
        scrollX -= io.MouseWheel * 40.0f;
      }
    }

    // ----- Caret position calculation -----
    int caretLine, caretCol;
    indexToLineCol(cursorIndex, caretLine, caretCol);

    float caretXLocal = padX + caretCol * cellWidth; // monospaced grid
    float caretYLocal = caretLine * lineHeight;

    float visibleW = viewW - padX * 2.0f;
    float visibleH = viewH - padY * 2.0f;

    // Auto-scroll when caretFollow is set
    if (caretFollow) {
      // Horizontal
      if (caretXLocal < scrollX)
        scrollX = caretXLocal;
      if (caretXLocal > scrollX + visibleW)
        scrollX = caretXLocal - visibleW;

      // Vertical
      if (caretYLocal < scrollY)
        scrollY = caretYLocal;
      if (caretYLocal + lineHeight > scrollY + visibleH)
        scrollY = caretYLocal + lineHeight - visibleH;

      caretFollow = false;
    }

    // Clamp scrolls
    float extraPad = cellWidth; // monospaced padding
    float maxScrollX = ImMax(0.0f, (maxContentWidth + extraPad) - visibleW);
    scrollX = ImClamp(scrollX, 0.0f, maxScrollX);

    float totalContentHeight = lineCache.size() * lineHeight;
    float maxScrollY = ImMax(0.0f, totalContentHeight - visibleH);
    scrollY = ImClamp(scrollY, 0.0f, maxScrollY);

    // Draw caret (blink)
    if (isFocused && (int)(ImGui::GetTime() * 2) % 2 == 0) {
      float caretScreenX = pos.x + caretXLocal - scrollX;
      float caretScreenY = pos.y + padY + caretYLocal - scrollY;

      float caretTop = caretScreenY + lineHeight * 0.15f; // shift down a bit
      float caretBottom =
          caretTop + lineHeight * 0.75f; // slightly shorter caret
      draw_list->AddLine(ImVec2(caretScreenX, caretTop),
                         ImVec2(caretScreenX, caretBottom),
                         ImGui::GetColorU32(ImGuiCol_Text), 2.0f);
    }

    draw_list->PopClipRect();

    // ----- Horizontal scrollbar -----
    ImVec2 hBarPos = ImVec2(pos.x, pos.y + viewH);
    ImVec2 hBarSize = ImVec2(viewW, scrollbarH);

    ImU32 bgCol = ImGui::GetColorU32(ImGuiCol_FrameBgHovered);
    ImU32 fillCol = ImGui::GetColorU32(ImGuiCol_ScrollbarGrab);
    ImU32 fillHot = ImGui::GetColorU32(ImGuiCol_ScrollbarGrabHovered);
    ImU32 fillAct = ImGui::GetColorU32(ImGuiCol_ScrollbarGrabActive);

    ImDrawList *dl = ImGui::GetWindowDrawList();
    dl->AddRectFilled(
        hBarPos, ImVec2(hBarPos.x + hBarSize.x, hBarPos.y + hBarSize.y), bgCol);

    float contentW = ImMax(visibleW, maxContentWidth + extraPad);
    float trackW = hBarSize.x;
    float minThumb = 24.0f;

    float thumbW = (visibleW / contentW) * trackW;
    thumbW = ImClamp(thumbW, minThumb, trackW);

    float trackRange = trackW - thumbW;
    float denom = ImMax(1.0f, contentW - visibleW);
    float thumbX = (denom > 0.0f) ? (scrollX / denom) * trackRange : 0.0f;

    ImVec2 thumbMin = ImVec2(hBarPos.x + thumbX, hBarPos.y);
    ImVec2 thumbMax =
        ImVec2(hBarPos.x + thumbX + thumbW, hBarPos.y + hBarSize.y);

    ImGuiIO &io2 = ImGui::GetIO();

    ImGui::SetCursorScreenPos(thumbMin);
    ImGui::InvisibleButton("hthumb", ImVec2(thumbW, scrollbarH));
    bool thumbHovered = ImGui::IsItemHovered();

    if (ImGui::IsItemActivated()) {
      hDragging = true;
      hDragMouseStart = io2.MousePos.x;
      hDragScrollStart = scrollX;
    }
    if (hDragging) {
      if (ImGui::IsMouseDown(ImGuiMouseButton_Left)) {
        float dx = io2.MousePos.x - hDragMouseStart;
        float newThumbX = (hDragScrollStart / denom) * trackRange + dx;
        newThumbX = ImClamp(newThumbX, 0.0f, trackRange);
        scrollX = (denom > 0.0f) ? (newThumbX / trackRange) * denom : 0.0f;
      } else {
        hDragging = false;
      }
    }

    ImGui::SetCursorScreenPos(hBarPos);
    ImGui::InvisibleButton("htrack", ImVec2(trackW, scrollbarH));
    if (ImGui::IsItemClicked()) {
      float mouseX = io2.MousePos.x - hBarPos.x;
      float page = visibleW * 0.8f;
      if (mouseX < thumbX)
        scrollX = ImMax(0.0f, scrollX - page);
      else if (mouseX > thumbX + thumbW)
        scrollX = ImMin(maxScrollX, scrollX + page);
    }

    ImU32 thumbCol = hDragging ? fillAct : (thumbHovered ? fillHot : fillCol);
    dl->AddRectFilled(thumbMin, thumbMax, thumbCol, 3.0f);

    // ----- Vertical scrollbar -----
    ImVec2 vBarPos = ImVec2(pos.x + viewW, pos.y);
    ImVec2 vBarSize = ImVec2(scrollbarW, viewH);

    dl->AddRectFilled(
        vBarPos, ImVec2(vBarPos.x + vBarSize.x, vBarPos.y + vBarSize.y), bgCol);

    float trackH = vBarSize.y;
    float thumbH = (visibleH / totalContentHeight) * trackH;
    thumbH = ImClamp(thumbH, minThumb, trackH);

    float vTrackRange = trackH - thumbH;
    float vDenom = ImMax(1.0f, totalContentHeight - visibleH);
    float thumbY = (vDenom > 0.0f) ? (scrollY / vDenom) * vTrackRange : 0.0f;

    ImVec2 vThumbMin = ImVec2(vBarPos.x, vBarPos.y + thumbY);
    ImVec2 vThumbMax =
        ImVec2(vBarPos.x + vBarSize.x, vBarPos.y + thumbY + thumbH);

    ImGui::SetCursorScreenPos(vThumbMin);
    ImGui::InvisibleButton("vthumb", ImVec2(scrollbarW, thumbH));
    bool vThumbHovered = ImGui::IsItemHovered();

    if (ImGui::IsItemActivated()) {
      vDragging = true;
      vDragMouseStart = io2.MousePos.y;
      vDragScrollStart = scrollY;
    }
    if (vDragging) {
      if (ImGui::IsMouseDown(ImGuiMouseButton_Left)) {
        float dy = io2.MousePos.y - vDragMouseStart;
        float newThumbY = (vDragScrollStart / vDenom) * vTrackRange + dy;
        newThumbY = ImClamp(newThumbY, 0.0f, vTrackRange);
        scrollY = (vDenom > 0.0f) ? (newThumbY / vTrackRange) * vDenom : 0.0f;
      } else {
        vDragging = false;
      }
    }

    ImGui::SetCursorScreenPos(vBarPos);
    ImGui::InvisibleButton("vtrack", ImVec2(scrollbarW, trackH));
    if (ImGui::IsItemClicked()) {
      float mouseY = io2.MousePos.y - vBarPos.y;
      float page = visibleH * 0.8f;
      if (mouseY < thumbY)
        scrollY = ImMax(0.0f, scrollY - page);
      else if (mouseY > thumbY + thumbH)
        scrollY = ImMin(maxScrollY, scrollY + page);
    }

    ImU32 vThumbCol = vDragging ? fillAct : (vThumbHovered ? fillHot : fillCol);
    dl->AddRectFilled(vThumbMin, vThumbMax, vThumbCol, 3.0f);

    scrollX = ImClamp(scrollX, 0.0f, maxScrollX);
    scrollY = ImClamp(scrollY, 0.0f, maxScrollY);
  }
  ImGui::End();
}

// ======= File ops =======
void TextEditor::openFile(const std::string &fname) {
  std::ifstream file(fname, std::ios::binary);
  if (!file.is_open()) {
    addOutput(icons["error"], "Could not open file: " + fname + " (" +
                                  std::strerror(errno) + ")");
    return;
  }
  std::ostringstream buffer;
  buffer << file.rdbuf();
  content = PieceTable(buffer.str());
  rebuildCache();
  filename = fname;
  modified = false;
  focusEditor = true;
  cursorLine = 0;
  cursorColumn = 0;
  if (content.size() == 0)
    addOutput(icons["folder"], "Opened empty file: " + fname);
  else
    addOutput(icons["folder"], "Opened: " + fname + " (" +
                                   std::to_string(content.size()) + " bytes)");
}

void TextEditor::newFile() {
  content.clear();
  content.insert(0, ""); // ensures clean buffer
  rebuildCache();
  filename.clear();
  modified = false;
  focusEditor = true;
  cursorIndex = 0; // reset caret
  addOutput(icons["document"], "New file created");
  cursorIndex = 0;
  scrollX = 0.0f;
  scrollY = 0.0f;
  caretFollow = true;
}

void TextEditor::saveFile() {
  if (filename.empty()) {
    showSaveDialog("untitled.txt");
    return;
  }

  std::ofstream file(filename);
  if (file.is_open()) {
    file << content.getText();
    modified = false;
    addOutput(icons["save"], "Saved: " + filename);
    file.close();
  } else {
    addOutput(icons["error"], "Could not save file: " + filename);
  }
}

// ======= Commands =======
void TextEditor::registerCommands() {
  commands["save"] = [this]() { saveFile(); };
  commands["new"] = [this]() { newFile(); };
  commands["clear"] = [this]() { outputLines.clear(); };
  commands["refresh"] = [this]() { refreshFileList(); };
  commands["focus"] = [this]() { focusEditor = true; };
}

void TextEditor::executeCommand(const std::string &cmd) {
  addOutput("> " + cmd);
  if (commands.find(cmd) != commands.end()) {
    commands[cmd]();
  } else {
    std::string luaCmd = "if type(command_" + cmd +
                         ")=='function' then command_" + cmd + "() end";
    if (!lua_->eval(luaCmd)) {
      addOutput(icons["error"], "Unknown command: " + cmd);
    }
  }
}

// ======= File list =======
void TextEditor::refreshFileList() {
  fileList.clear();
  try {
    for (const auto &entry : std::filesystem::directory_iterator(".")) {
      fileList.push_back(entry.path().filename().string());
    }
  } catch (...) {
    addOutput(icons["error"], "Error reading directory");
  }
}

// ======= Icon loading =======
ImTextureID TextEditor::loadSVGTexture(const char *filename,
                                       float targetHeight) {
  NSVGimage *image = nsvgParseFromFile(filename, "px", 96);
  if (!image) {
    addOutput(icons["error"], std::string("Could not open SVG: ") + filename);
    return (ImTextureID)0;
  }

  float scale = targetHeight / image->height;
  int w = (int)(image->width * scale);
  int h = (int)(image->height * scale);

  NSVGrasterizer *rast = nsvgCreateRasterizer();
  std::vector<unsigned char> img(w * h * 4);
  nsvgRasterize(rast, image, 0, 0, scale, img.data(), w, h, w * 4);

  GLuint tex;
  glGenTextures(1, &tex);
  glBindTexture(GL_TEXTURE_2D, tex);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
  glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE,
               img.data());

  nsvgDeleteRasterizer(rast);
  nsvgDelete(image);

  return (ImTextureID)(intptr_t)tex;
}

void TextEditor::loadIcons(float dpiScale) {
  float targetIconSize = 40.0f * dpiScale;
  icons["folder"] = loadSVGTexture("icons/folder.svg", targetIconSize);
  icons["document"] = loadSVGTexture("icons/document.svg", targetIconSize);
  icons["settings"] = loadSVGTexture("icons/settings.svg", targetIconSize);
  icons["save"] = loadSVGTexture("icons/save.svg", targetIconSize);
  icons["error"] = loadSVGTexture("icons/error.svg", targetIconSize);
  icons["refresh"] = loadSVGTexture("icons/refresh.svg", targetIconSize);
  icons["checkmark"] = loadSVGTexture("icons/checkmark.svg", targetIconSize);
}

// ======= Icon+Text button =======
bool TextEditor::IconTextButton(const char *id, ImTextureID icon,
                                const char *label, const ImVec2 &button_size) {
  ImGui::PushStyleColor(ImGuiCol_Button,
                        ImGui::GetStyle().Colors[ImGuiCol_Button]);
  ImGui::PushStyleColor(ImGuiCol_ButtonHovered,
                        ImGui::GetStyle().Colors[ImGuiCol_ButtonHovered]);
  ImGui::PushStyleColor(ImGuiCol_ButtonActive,
                        ImGui::GetStyle().Colors[ImGuiCol_ButtonActive]);
  bool pressed = ImGui::Button(id, button_size);
  ImGui::PopStyleColor(3);
  ImVec2 pos = ImGui::GetItemRectMin();
  ImVec2 size = ImGui::GetItemRectSize();
  float iconSize = 20.0f;
  float padding = 5.0f;
  ImVec2 iconPos(pos.x + padding, pos.y + (size.y - iconSize) * 0.5f);
  ImGui::GetWindowDrawList()->AddImage(
      icon, iconPos, ImVec2(iconPos.x + iconSize, iconPos.y + iconSize),
      ImVec2(0, 0), ImVec2(1, 1), IM_COL32_WHITE);
  ImVec2 text_size = ImGui::CalcTextSize(label);
  ImVec2 textPos(iconPos.x + iconSize + padding,
                 pos.y + (size.y - text_size.y) * 0.5f);
  ImGui::GetWindowDrawList()->AddText(textPos, IM_COL32_WHITE, label);
  return pressed;
}

// ======= Cache & positions =======
void TextEditor::rebuildCache() {
  lineCache.clear();

  const std::string full = content.getText();
  const size_t n = full.size();

  const char *sample =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  float cellWidth = ImGui::CalcTextSize(sample).x / (float)strlen(sample);

  // Split on '\n' but PRESERVE a trailing empty line if the text ends with '\n'
  size_t start = 0;
  for (size_t i = 0; i < n; ++i) {
    if (full[i] == '\n') {
      std::string seg = full.substr(start, i - start); // may be empty
      CachedLine cl;
      cl.text = std::move(seg);
      cl.width = cl.text.size() * cellWidth;
      lineCache.push_back(std::move(cl));
      start = i + 1;
    }
  }

  if (start < n) {
    // Remainder after the last newline
    std::string seg = full.substr(start);
    CachedLine cl;
    cl.text = std::move(seg);
    cl.width = cl.text.size() * cellWidth;
    lineCache.push_back(std::move(cl));
  } else {
    // If the text ENDS with '\n', we need a trailing empty visual line
    if (!full.empty() && full.back() == '\n') {
      lineCache.push_back({"", 0.0f});
    }
  }

  // Ensure there's ALWAYS at least one visual line for an empty buffer
  if (lineCache.empty()) {
    lineCache.push_back({"", 0.0f});
  }
}

void TextEditor::onTextChanged() {
  rebuildCache();
  modified = true;
}

void TextEditor::indexToLineCol(int index, int &line, int &col) {
  int pos = 0;
  line = 0;
  for (const auto &cl : lineCache) {
    int lineLen = (int)cl.text.size();
    if (index <= pos + lineLen) {
      col = index - pos;
      return;
    }
    pos += lineLen + 1; // +1 for newline
    line++;
  }
  // Clamp to last position
  if (!lineCache.empty()) {
    line = (int)lineCache.size() - 1;
    col = (int)lineCache.back().text.size();
  } else {
    line = 0;
    col = 0;
  }
}

int TextEditor::lineColToIndex(int line, int col) {
  int index = 0;
  for (int i = 0; i < line && i < (int)lineCache.size(); i++) {
    index += (int)lineCache[i].text.size() + 1; // +1 for newline
  }
  if (line < (int)lineCache.size()) {
    col = std::min(col, (int)lineCache[line].text.size());
    index += col;
  }
  return index;
}

// ======= Output helpers =======
void TextEditor::addOutput(ImTextureID icon, const std::string &text) {
  outputLines.push_back({icon, text});
  if (outputLines.size() > 1000) {
    outputLines.erase(outputLines.begin());
  }
}

void TextEditor::addOutput(const std::string &text) {
  addOutput((ImTextureID)0, text);
}

bool TextEditor::hasSelection() const {
  return selectionStart != -1 && selectionEnd != -1 &&
         selectionStart != selectionEnd;
}

std::string TextEditor::getSelectedText() const {
  if (!hasSelection())
    return "";

  int selMin = std::min(selectionStart, selectionEnd);
  int selMax = std::max(selectionStart, selectionEnd);

  std::string fullText = content.getText();
  if (selMin < 0 || selMax > (int)fullText.size())
    return "";

  return fullText.substr(selMin, selMax - selMin);
}

// Also update deleteSelection to ensure it rebuilds cache:
void TextEditor::deleteSelection() {
  if (!hasSelection())
    return;

  int selMin = std::min(selectionStart, selectionEnd);
  int selMax = std::max(selectionStart, selectionEnd);

  content.erase(selMin, selMax - selMin);
  cursorIndex = selMin;
  selectionStart = selectionEnd = -1;
  // NOTE: Don't call onTextChanged here - let the caller do it
  // This prevents double cache rebuilds
}

void TextEditor::copySelection() {
  if (!hasSelection())
    return;

  std::string text = getSelectedText();
  ImGui::SetClipboardText(text.c_str());
  addOutput(icons["checkmark"],
            "Copied " + std::to_string(text.size()) + " characters");
}

void TextEditor::cutSelection() {
  if (!hasSelection())
    return;

  copySelection();
  deleteSelection();
  onTextChanged();
  caretFollow = true;
}

// Replace your pasteFromClipboard function with this fixed version:

void TextEditor::pasteFromClipboard() {
  const char *clipText = ImGui::GetClipboardText();
  if (!clipText || !clipText[0])
    return;

  // Delete selection if any
  if (hasSelection()) {
    deleteSelection();
    onTextChanged(); // Rebuild cache after deletion
  }

  // Insert the clipboard text
  content.insert(cursorIndex, clipText);
  cursorIndex += strlen(clipText);
  onTextChanged(); // Rebuild cache after insertion
  caretFollow = true;
  addOutput(icons["checkmark"], "Pasted text");
}
void TextEditor::selectAll() {
  selectionStart = 0;
  selectionEnd = (int)content.size();
  cursorIndex = selectionEnd;
  caretFollow = true;
}

