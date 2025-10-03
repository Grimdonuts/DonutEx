#include "TextEditor.hpp"
#include "LuaBindings.hpp"
#include <misc/cpp/imgui_stdlib.h>
#include <nfd.h>
#include <fstream>
#include <sstream>
#include <cstring>
#include <filesystem>
#define NANOSVG_IMPLEMENTATION
#include "nanosvg.h"
#define NANOSVGRAST_IMPLEMENTATION
#include "nanosvgrast.h"
#include "imgui_internal.h"
#include <GLFW/glfw3.h>

TextEditor::TextEditor()
    : content(),
      filename(""),
      modified(false),
      showFileExplorer(true),
      showOutput(true),
      focusEditor(false),
      cursorLine(0),
      cursorColumn(0)
{
    inputFlags = ImGuiInputTextFlags_AllowTabInput | ImGuiInputTextFlags_CallbackAlways;
    lua_ = new LuaBindings(this);
    loadIcons(ImGui::GetIO().FontGlobalScale);
    registerCommands();
    lua_->loadPlugins();
    refreshFileList();
}

TextEditor::~TextEditor()
{
    delete lua_;
}

// prevent new windows from escaping viewport (currently only used by Settings) Lock it in the main window
static void ClampToViewport(ImGuiSizeCallbackData *data)
{
    ImGuiViewport *viewport = ImGui::GetMainViewport();
    ImVec2 workMin = viewport->WorkPos;
    ImVec2 work_max(viewport->WorkPos.x + viewport->WorkSize.x, viewport->WorkPos.y + viewport->WorkSize.y);
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

void TextEditor::renderEditor()
{
    if (ImGui::Begin("Editor", nullptr,
                     ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoResize |
                         ImGuiWindowFlags_NoCollapse | ImGuiWindowFlags_NoTitleBar |
                         ImGuiWindowFlags_NoBringToFrontOnFocus))
    {
        ImDrawList *draw_list = ImGui::GetWindowDrawList();
        ImVec2 pos = ImGui::GetCursorScreenPos();
        float lineHeight = ImGui::GetTextLineHeightWithSpacing();

        // Layout
        ImVec2 winSize = ImGui::GetContentRegionAvail();
        const float padX = 4.0f;
        const float padY = 4.0f;
        const float scrollbarH = 12.0f;
        const float viewH = winSize.y - scrollbarH;
        const float viewW = winSize.x;

        // Background
        draw_list->AddRectFilled(pos, ImVec2(pos.x + winSize.x, pos.y + winSize.y),
                                 ImGui::GetColorU32(ImGuiCol_FrameBg));

        // Editor interactive area
        ImGui::InvisibleButton("editor_area", ImVec2(winSize.x, viewH),
                               ImGuiButtonFlags_MouseButtonLeft);
        bool isFocused = ImGui::IsItemFocused();

        if (ImGui::IsItemClicked())
        {
            ImGui::SetKeyboardFocusHere();
            isFocused = true;
        }

        // ----- Clipping -----
        ImVec2 clipMin = ImVec2(pos.x, pos.y);
        ImVec2 clipMax = ImVec2(pos.x + viewW, pos.y + viewH);
        draw_list->PushClipRect(clipMin, clipMax, true);

        // Draw text with horizontal offset
        std::istringstream ss(content.getText());
        std::string line;
        float y = pos.y + padY;

        maxContentWidth = 0.0f;
        while (std::getline(ss, line))
        {
            float lineWidth = ImGui::CalcTextSize(line.c_str()).x;
            if (lineWidth > maxContentWidth) maxContentWidth = lineWidth;

            ImVec2 textPos = ImVec2(pos.x + padX - scrollX, y);
            draw_list->AddText(textPos, ImGui::GetColorU32(ImGuiCol_Text), line.c_str());

            y += lineHeight;
        }

        // ----- Input handling -----
        ImGuiIO &io = ImGui::GetIO();
        if (isFocused)
        {
            for (unsigned int c : io.InputQueueCharacters)
            {
                if (c == '\n' || c == '\r')
                {
                    content.insert(cursorIndex, "\n");
                    cursorIndex++;
                    caretFollow = true;
                }
                else if (c >= 32)
                {
                    content.insert(cursorIndex, std::string(1, (char)c));
                    cursorIndex++;
                    caretFollow = true;
                }
            }

            if (ImGui::IsKeyPressed(ImGuiKey_Backspace) && cursorIndex > 0)
            {
                content.erase(cursorIndex - 1, 1);
                cursorIndex--;
                caretFollow = true;
            }

            if (ImGui::IsKeyPressed(ImGuiKey_Delete) && cursorIndex < (int)content.size())
            {
                content.erase(cursorIndex, 1);
                caretFollow = true;
            }

            if (ImGui::IsKeyPressed(ImGuiKey_LeftArrow) && cursorIndex > 0)
            {
                cursorIndex--;
                caretFollow = true;
            }

            if (ImGui::IsKeyPressed(ImGuiKey_RightArrow) && cursorIndex < (int)content.size())
            {
                cursorIndex++;
                caretFollow = true;
            }

            // Shift+wheel horizontal scrolling
            if (io.KeyShift && io.MouseWheel != 0.0f)
            {
                scrollX -= io.MouseWheel * 40.0f;
            }
        }

        // ----- Caret position -----
        int col = 0;
        std::string caretLine;
        {
            int idx = 0;
            std::istringstream ss2(content.getText());
            std::string l;
            int lineStartIdx = 0;
            while (std::getline(ss2, l))
            {
                int lineLen = (int)l.size();
                if (cursorIndex <= lineStartIdx + lineLen)
                {
                    caretLine = l;
                    col = cursorIndex - lineStartIdx;
                    break;
                }
                lineStartIdx += lineLen + 1;
            }
        }

        float caretXLocal = padX + ImGui::CalcTextSize(caretLine.substr(0, col).c_str()).x;
        float visibleW = viewW - padX * 2.0f;

        // Auto-scroll only when caretFollow is set
        if (caretFollow)
        {
            if (caretXLocal < scrollX)
                scrollX = caretXLocal;
            if (caretXLocal > scrollX + visibleW)
                scrollX = caretXLocal - visibleW;

            caretFollow = false;
        }

        // Clamp scroll (FIX: add glyph padding so last char/caret is fully visible)
        float extraPad = ImGui::CalcTextSize("M").x; // safe glyph width
        float maxScroll = ImMax(0.0f, (maxContentWidth + extraPad) - visibleW);
        scrollX = ImClamp(scrollX, 0.0f, maxScroll);

        // Draw caret (blink)
        if (isFocused && (int)(ImGui::GetTime() * 2) % 2 == 0)
        {
            float cy = pos.y + padY;
            {
                int idx = 0;
                std::istringstream ss2(content.getText());
                std::string l;
                int consumed = 0;
                while (std::getline(ss2, l))
                {
                    if (cursorIndex <= consumed + (int)l.size())
                        break;
                    consumed += (int)l.size() + 1;
                    cy += lineHeight;
                }
            }

            ImVec2 cursorPos(pos.x + caretXLocal - scrollX, cy);
            draw_list->AddLine(cursorPos,
                               ImVec2(cursorPos.x, cursorPos.y + lineHeight - 2),
                               ImGui::GetColorU32(ImGuiCol_Text));
        }

        draw_list->PopClipRect();

        // ----- Custom horizontal scrollbar -----
        ImVec2 barPos = ImVec2(pos.x, pos.y + viewH);
        ImVec2 barSize = ImVec2(viewW, scrollbarH);

        ImU32 bgCol   = ImGui::GetColorU32(ImGuiCol_FrameBgHovered);
        ImU32 fillCol = ImGui::GetColorU32(ImGuiCol_ScrollbarGrab);
        ImU32 fillHot = ImGui::GetColorU32(ImGuiCol_ScrollbarGrabHovered);
        ImU32 fillAct = ImGui::GetColorU32(ImGuiCol_ScrollbarGrabActive);

        draw_list->AddRectFilled(barPos, ImVec2(barPos.x + barSize.x, barPos.y + barSize.y), bgCol);

        float contentW = ImMax(visibleW, maxContentWidth + extraPad); // also add pad here
        float trackW   = barSize.x;
        float minThumb = 24.0f;

        float thumbW = (visibleW / contentW) * trackW;
        thumbW = ImClamp(thumbW, minThumb, trackW);

        float trackRange = trackW - thumbW;
        float denom = ImMax(1.0f, contentW - visibleW);
        float thumbX = (denom > 0.0f) ? (scrollX / denom) * trackRange : 0.0f;

        ImVec2 thumbMin = ImVec2(barPos.x + thumbX, barPos.y);
        ImVec2 thumbMax = ImVec2(barPos.x + thumbX + thumbW, barPos.y + barSize.y);

        // Thumb interaction
        ImGui::SetCursorScreenPos(thumbMin);
        ImGui::InvisibleButton("hthumb", ImVec2(thumbW, scrollbarH));
        bool thumbHovered = ImGui::IsItemHovered();

        if (ImGui::IsItemActivated())
        {
            hDragging = true;
            hDragMouseStart = io.MousePos.x;
            hDragScrollStart = scrollX;
        }
        if (hDragging)
        {
            if (ImGui::IsMouseDown(ImGuiMouseButton_Left))
            {
                float dx = io.MousePos.x - hDragMouseStart;
                float newThumbX = (hDragScrollStart / denom) * trackRange + dx;
                newThumbX = ImClamp(newThumbX, 0.0f, trackRange);
                scrollX = (denom > 0.0f) ? (newThumbX / trackRange) * denom : 0.0f;
            }
            else
            {
                hDragging = false;
            }
        }

        // Track clicks
        ImGui::SetCursorScreenPos(barPos);
        ImGui::InvisibleButton("htrack", ImVec2(trackW, scrollbarH));
        if (ImGui::IsItemClicked())
        {
            float mouseX = io.MousePos.x - barPos.x;
            float page = visibleW * 0.8f;
            if (mouseX < thumbX)
                scrollX = ImMax(0.0f, scrollX - page);
            else if (mouseX > thumbX + thumbW)
                scrollX = ImMin(maxScroll, scrollX + page);
        }

        ImU32 thumbCol = hDragging ? fillAct : (thumbHovered ? fillHot : fillCol);
        draw_list->AddRectFilled(thumbMin, thumbMax, thumbCol, 3.0f);

        scrollX = ImClamp(scrollX, 0.0f, maxScroll);
    }
    ImGui::End();
}


void TextEditor::renderExplorer(ImVec2 workPos, ImVec2 workSize, float explorerWidth)
{
    if (ImGui::Begin("File Explorer", nullptr,
                     ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoResize |
                         ImGuiWindowFlags_NoCollapse | ImGuiWindowFlags_NoBringToFrontOnFocus))
    {
        if (IconTextButton("##refresh_btn", icons["refresh"], "Refresh", ImVec2(120, 30)))
        {
            refreshFileList();
        }
        ImGui::Separator();
        ImGui::BeginChild("FileList", ImVec2(0, -40), true);

        for (size_t i = 0; i < fileList.size(); ++i)
        {
            const auto &file = fileList[i];
            bool isDirectory = std::filesystem::is_directory(file);
            ImGui::PushID((int)i);
            ImGui::Selectable("##fileitem", false);
            ImVec2 pos = ImGui::GetItemRectMin();
            ImVec2 size = ImGui::GetItemRectSize();
            ImTextureID iconTex = isDirectory ? icons["folder"] : icons["document"]; // do something cool here instead of just file and folder icons
            float iconSize = 18.0f;
            ImVec2 iconPos(pos.x + 4, pos.y + (size.y - iconSize) * 0.5f);
            ImGui::GetWindowDrawList()->AddImage(
                iconTex,
                iconPos,
                ImVec2(iconPos.x + iconSize, iconPos.y + iconSize),
                ImVec2(0, 0), ImVec2(1, 1),
                ImGui::GetColorU32(ImGuiCol_Text));
            ImVec2 textPos(iconPos.x + iconSize + 6,
                           pos.y + (size.y - ImGui::GetTextLineHeight()) * 0.5f);
            ImGui::GetWindowDrawList()->AddText(textPos, ImGui::GetColorU32(ImGuiCol_Text), file.c_str());
            if (ImGui::IsItemClicked() && !isDirectory)
            {
                openFile(file);
            }

            ImGui::PopID();
        }
        ImGui::EndChild();
        ImGui::Separator();
        if (IconTextButton("##settings_btn", icons["settings"], "Settings", ImVec2(120, 30)))
        {
            showSettings = !showSettings;
        }
    }
    ImGui::End();
}

void TextEditor::renderOutput(ImVec2 workPos, ImVec2 workSize, float outputHeight, float explorerWidth)
{
    ImVec2 outputPos(workPos.x + explorerWidth, workPos.y + workSize.y - outputHeight);
    ImVec2 outputSize(workSize.x - explorerWidth, outputHeight);
    ImGui::SetNextWindowPos(outputPos, ImGuiCond_Always);
    ImGui::SetNextWindowSize(outputSize, ImGuiCond_Always);
    if (ImGui::Begin("Output", nullptr,
                     ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoResize |
                         ImGuiWindowFlags_NoCollapse | ImGuiWindowFlags_NoBringToFrontOnFocus))
    {
        ImGui::Text("Command:");
        ImGui::SameLine();
        ImGui::SetNextItemWidth(-100);
        bool commandEntered = ImGui::InputText("##command", &commandInput,
                                               ImGuiInputTextFlags_EnterReturnsTrue);
        ImGui::SameLine();
        if (ImGui::Button("Execute") || commandEntered)
        {
            if (!commandInput.empty())
            {
                executeCommand(commandInput);
                commandInput.clear();
            }
        }
        ImGui::Separator();
        ImGui::BeginChild("OutputText");
        float iconSize = 18.0f;
        for (const auto &line : outputLines)
        {
            ImVec2 pos = ImGui::GetCursorScreenPos();
            float iconSize = 18.0f;

            if (line.icon != (ImTextureID)0)
            {
                ImGui::GetWindowDrawList()->AddImage(
                    line.icon,
                    pos,
                    ImVec2(pos.x + iconSize, pos.y + iconSize),
                    ImVec2(0, 0), ImVec2(1, 1),
                    IM_COL32_WHITE);
                ImGui::SetCursorScreenPos(ImVec2(pos.x + iconSize + 6, pos.y));
            }

            ImGui::TextWrapped("%s", line.text.c_str());
        }

        // This scrolls the output
        if (ImGui::GetScrollY() >= ImGui::GetScrollMaxY())
        {
            ImGui::SetScrollHereY(1.0f);
        }
        ImGui::EndChild();
    }
    ImGui::End();
}

// console logging in the output window
void TextEditor::addOutput(ImTextureID icon, const std::string &text)
{
    outputLines.push_back({icon, text});
    if (outputLines.size() > 1000)
    {
        outputLines.erase(outputLines.begin());
    }
}

// overload for when you dont need an icon
void TextEditor::addOutput(const std::string &text)
{
    addOutput((ImTextureID)0, text);
}

void TextEditor::renderSettings()
{
    ImGui::SetNextWindowSize(ImVec2(500, 400), ImGuiCond_Once);
    ImGui::SetNextWindowSizeConstraints(ImVec2(200, 100), ImVec2(FLT_MAX, FLT_MAX), ClampToViewport);
    ImGui::Begin("Settings", &showSettings,
                 ImGuiWindowFlags_NoCollapse | ImGuiWindowFlags_NoDocking);
    ImGui::Text("Editor Settings");
    lua_->eval("if show_font_menu then show_font_menu() end");
    ImGui::End();
}

void TextEditor::showOpenDialog()
{
    nfdchar_t *outPath = nullptr;

    nfdu8filteritem_t filters[4] = {
        {"C++ Source", "cpp"},
        {"C++ Header", "hpp"},
        {"Text File", "txt"},
        {"Lua Script", "lua"}};
    nfdresult_t result = NFD_OpenDialog(&outPath, filters, 4, nullptr); // instead of nullptr here, lets pass the current dir our file explorer is looking at
    if (result == NFD_OKAY)
    {
        openFile(outPath);
        free(outPath);
    }
    else if (result == NFD_ERROR)
    {
        addOutput(icons["error"], std::string("NFD Error: ") + NFD_GetError());
    }
}

void TextEditor::showSaveDialog(const std::string &defaultFileName)
{
    nfdchar_t *savePath = nullptr;

    nfdu8filteritem_t filters[4] = {
        {"C++ Source", "cpp"},
        {"C++ Header", "hpp"},
        {"Text File", "txt"},
        {"Lua Script", "lua"}};

    nfdresult_t result = NFD_SaveDialog(&savePath, filters, 4, nullptr, defaultFileName.c_str()); // instead of nullptr here, lets pass the current dir our file explorer is looking at
    if (result == NFD_OKAY)
    {
        filename = savePath;
        saveFile();
        free(savePath);
    }
    else if (result == NFD_ERROR)
    {
        addOutput(icons["error"], std::string("NFD Error: ") + NFD_GetError());
    }
}

void TextEditor::renderMenuBar()
{
    if (ImGui::BeginMainMenuBar())
    {
        if (ImGui::BeginMenu("File"))
        {
            if (ImGui::MenuItem("New", "Ctrl+N"))
                newFile();
            if (ImGui::MenuItem("Open", "Ctrl+O"))
            {
                showOpenDialog();
            }
            if (ImGui::MenuItem("Save As", "Ctrl+Shift+S"))
            {
                showSaveDialog(filename.empty() ? "untitled.txt" : filename);
            }
            if (ImGui::MenuItem("Save", "Ctrl+S"))
                saveFile();
            ImGui::Separator();
            if (ImGui::MenuItem("Exit", "Alt+F4"))
            {
                closeEditor = true;
            }
            ImGui::EndMenu();
        }

        if (ImGui::BeginMenu("View"))
        {
            ImGui::MenuItem("File Explorer", nullptr, &showFileExplorer);
            ImGui::MenuItem("Output Panel", nullptr, &showOutput);
            ImGui::EndMenu();
        }

        if (ImGui::BeginMenu("Plugins"))
        {
            if (ImGui::MenuItem("Reload Plugins"))
            {
                lua_->loadPlugins();
                addOutput("Plugins reloaded!");
            }
            if (ImGui::MenuItem("List Commands"))
            {
                addOutput("Available commands:");
                for (const auto &cmd : commands) // these do things, sure, but maybe come up with actual commands that might be useful TODO:
                {
                    addOutput("  • " + cmd.first);
                }
            }
            ImGui::EndMenu();
        }
        ImGui::EndMainMenuBar();
    }
}

// main render loop
bool TextEditor::render()
{
    ImGuiStyle &style = ImGui::GetStyle();
    style.Colors[ImGuiCol_TitleBgActive] = style.Colors[ImGuiCol_TitleBg];
    // Main menu bar
    renderMenuBar();

    // File Explorer
    ImGuiViewport *viewport = ImGui::GetMainViewport();
    ImVec2 workPos = viewport->WorkPos;
    ImVec2 workSize = viewport->WorkSize;
    float explorerWidth = showFileExplorer ? 250.0f : 0.0f;
    float outputHeight = showOutput ? 200.0f : 0.0f;

    if (showFileExplorer)
    {
        ImGui::SetNextWindowPos(workPos, ImGuiCond_Always);
        ImGui::SetNextWindowSize(ImVec2(explorerWidth, workSize.y), ImGuiCond_Always);
        renderExplorer(workPos, workSize, explorerWidth);
    }

    // Editor
    ImVec2 editorPos(workPos.x + explorerWidth, workPos.y);
    ImVec2 editorSize(workSize.x - explorerWidth, workSize.y - outputHeight);
    ImGui::SetNextWindowPos(editorPos, ImGuiCond_Always);
    ImGui::SetNextWindowSize(editorSize, ImGuiCond_Always);
    renderEditor();

    // Output Panel
    if (showOutput)
    {
        renderOutput(workPos, workSize, outputHeight, explorerWidth);
    }

    if (showSettings)
    {
        renderSettings();
    }

    // Run Lua hooks
    lua_->runHook("on_text_input");
    lua_->runHook("on_render");

    return closeEditor;
}

void TextEditor::openFile(const std::string &fname)
{
    std::ifstream file(fname, std::ios::binary);
    if (!file.is_open())
    {
        addOutput(icons["error"], "Could not open file: " + fname + " (" + std::strerror(errno) + ")");
        return;
    }
    std::ostringstream buffer;
    buffer << file.rdbuf();
    content = PieceTable(buffer.str());
    filename = fname;
    modified = false;
    focusEditor = true;
    cursorLine = 0;
    cursorColumn = 0;
    if (content.size() == 0)
        addOutput(icons["folder"], "Opened empty file: " + fname);
    else
        addOutput(icons["folder"], "Opened: " + fname + " (" + std::to_string(content.size()) + " bytes)");
}

void TextEditor::newFile()
{
    content.clear();
    filename.clear();
    modified = false;
    focusEditor = true;
    addOutput(icons["document"], "New file created");
}

void TextEditor::saveFile()
{
    if (filename.empty())
    {
        showSaveDialog("untitled.txt");
        return;
    }

    std::ofstream file(filename);
    if (file.is_open())
    {
        file << content.getText();
        modified = false;
        addOutput(icons["save"], "Saved: " + filename);
        file.close();
    }
    else
    {
        addOutput(icons["error"], "Could not save file: " + filename);
    }
}

// basic commands, maybe more later...
void TextEditor::registerCommands()
{
    commands["save"] = [this]()
    { saveFile(); };
    commands["new"] = [this]()
    { newFile(); };
    commands["clear"] = [this]()
    { outputLines.clear(); };
    commands["refresh"] = [this]()
    { refreshFileList(); };
    commands["focus"] = [this]()
    { focusEditor = true; };
}

void TextEditor::executeCommand(const std::string &cmd)
{
    addOutput("> " + cmd);
    if (commands.find(cmd) != commands.end())
    {
        commands[cmd]();
    }
    else
    {
        std::string luaCmd = "if type(command_" + cmd + ")=='function' then command_" + cmd + "() end";
        if (!lua_->eval(luaCmd))
        {
            addOutput(icons["error"], "Unknown command: " + cmd);
        }
    }
}

// Keyboard shortcuts ... basic for now. add custom stuff later...
void TextEditor::handleKeyboardShortcuts()
{
    ImGuiIO &io = ImGui::GetIO();
    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_N))
        newFile();
    if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_S))
        saveFile();
    if (io.KeyCtrl && io.KeyShift && ImGui::IsKeyPressed(ImGuiKey_S))
    {
        showSaveDialog(filename.empty() ? "untitled.txt" : filename);
    }
}

void TextEditor::refreshFileList()
{
    fileList.clear();
    try
    {
        for (const auto &entry : std::filesystem::directory_iterator("."))
        {
            fileList.push_back(entry.path().filename().string());
        }
    }
    catch (...)
    {
        addOutput(icons["error"], "Error reading directory");
    }
}

// void TextEditor::replaceCurrentWordWith(const std::string &full)
// {
//     std::istringstream ss(content);
//     std::string line;
//     int currentLineIdx = 0;
//     size_t offset = 0;
//     while (std::getline(ss, line))
//     {
//         if (currentLineIdx == cursorLine)
//             break;
//         offset += line.size() + 1;
//         currentLineIdx++;
//     }
//     if (currentLineIdx != cursorLine)
//         return;

//     int col = std::min<int>(cursorColumn, (int)line.size());
//     int start = col;
//     while (start > 0 && (std::isalnum((unsigned char)line[start - 1]) || line[start - 1] == '_'))
//         --start;
//     int end = col;
//     while (end < (int)line.size() && (std::isalnum((unsigned char)line[end]) || line[end] == '_'))
//         ++end;

//     size_t absStart = offset + start;
//     size_t absEnd = offset + end;
//     content.replace(absStart, absEnd - absStart, full);
//     cursorColumn = start + (int)full.size();
//     modified = true;
// }

// By default all SVGs are loaded by the color palette they come in... when theming is made we should do something about the icons here. TODO:
ImTextureID TextEditor::loadSVGTexture(const char *filename, float targetHeight)
{
    NSVGimage *image = nsvgParseFromFile(filename, "px", 96);
    if (!image)
    {
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
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, w, h, 0, GL_RGBA,
                 GL_UNSIGNED_BYTE, img.data());

    nsvgDeleteRasterizer(rast);
    nsvgDelete(image);

    return (ImTextureID)(intptr_t)tex;
}

void TextEditor::loadIcons(float dpiScale)
{
    float targetIconSize = 40.0f * dpiScale;
    icons["folder"] = loadSVGTexture("icons/folder.svg", targetIconSize);
    icons["document"] = loadSVGTexture("icons/document.svg", targetIconSize);
    icons["settings"] = loadSVGTexture("icons/settings.svg", targetIconSize);
    icons["save"] = loadSVGTexture("icons/save.svg", targetIconSize);
    icons["error"] = loadSVGTexture("icons/error.svg", targetIconSize);
    icons["refresh"] = loadSVGTexture("icons/refresh.svg", targetIconSize);
    icons["checkmark"] = loadSVGTexture("icons/checkmark.svg", targetIconSize);
}

// This just makes a button with an icon to the left and text to the right.
bool TextEditor::IconTextButton(const char *id, ImTextureID icon, const char *label, const ImVec2 &button_size)
{
    ImGui::PushStyleColor(ImGuiCol_Button, ImGui::GetStyle().Colors[ImGuiCol_Button]);
    ImGui::PushStyleColor(ImGuiCol_ButtonHovered, ImGui::GetStyle().Colors[ImGuiCol_ButtonHovered]);
    ImGui::PushStyleColor(ImGuiCol_ButtonActive, ImGui::GetStyle().Colors[ImGuiCol_ButtonActive]);
    bool pressed = ImGui::Button(id, button_size);
    ImGui::PopStyleColor(3);
    ImVec2 pos = ImGui::GetItemRectMin();
    ImVec2 size = ImGui::GetItemRectSize();
    float iconSize = 20.0f;
    float padding = 5.0f;
    ImVec2 iconPos(pos.x + padding, pos.y + (size.y - iconSize) * 0.5f);
    ImGui::GetWindowDrawList()->AddImage(
        icon,
        iconPos,
        ImVec2(iconPos.x + iconSize, iconPos.y + iconSize),
        ImVec2(0, 0), ImVec2(1, 1),
        IM_COL32_WHITE);
    ImVec2 text_size = ImGui::CalcTextSize(label);
    ImVec2 textPos(iconPos.x + iconSize + padding,
                   pos.y + (size.y - text_size.y) * 0.5f);
    ImGui::GetWindowDrawList()->AddText(textPos, IM_COL32_WHITE, label);
    return pressed;
}

