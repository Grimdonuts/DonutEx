// IconManager.cpp
#include "IconManager.hpp"
#include "TextEditor.hpp"
#define NANOSVG_IMPLEMENTATION
#include "nanosvg.h"
#define NANOSVGRAST_IMPLEMENTATION
#include "nanosvgrast.h"
#include <GLFW/glfw3.h>
#include <vector>

IconManager::IconManager(TextEditor* editor)
    : editor_(editor)
{
}

ImTextureID IconManager::loadSVGTexture(const char* filename, float targetHeight)
{
    NSVGimage* image = nsvgParseFromFile(filename, "px", 96);
    if (!image)
    {
        editor_->addOutput(editor_->icons["error"], std::string("Could not open SVG: ") + filename);
        return (ImTextureID)0;
    }

    float scale = targetHeight / image->height;
    int w = (int)(image->width * scale);
    int h = (int)(image->height * scale);

    NSVGrasterizer* rast = nsvgCreateRasterizer();
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

void IconManager::loadIcons(float dpiScale)
{
    float targetIconSize = 40.0f * dpiScale;
    editor_->icons["folder"] = loadSVGTexture("icons/folder.svg", targetIconSize);
    editor_->icons["document"] = loadSVGTexture("icons/document.svg", targetIconSize);
    editor_->icons["settings"] = loadSVGTexture("icons/settings.svg", targetIconSize);
    editor_->icons["save"] = loadSVGTexture("icons/save.svg", targetIconSize);
    editor_->icons["error"] = loadSVGTexture("icons/error.svg", targetIconSize);
    editor_->icons["refresh"] = loadSVGTexture("icons/refresh.svg", targetIconSize);
    editor_->icons["checkmark"] = loadSVGTexture("icons/checkmark.svg", targetIconSize);
}

bool IconManager::iconTextButton(const char* id, ImTextureID icon, const char* label, const ImVec2& buttonSize)
{
    ImGui::PushStyleColor(ImGuiCol_Button, ImGui::GetStyle().Colors[ImGuiCol_Button]);
    ImGui::PushStyleColor(ImGuiCol_ButtonHovered, ImGui::GetStyle().Colors[ImGuiCol_ButtonHovered]);
    ImGui::PushStyleColor(ImGuiCol_ButtonActive, ImGui::GetStyle().Colors[ImGuiCol_ButtonActive]);
    bool pressed = ImGui::Button(id, buttonSize);
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
    
    ImVec2 textSize = ImGui::CalcTextSize(label);
    ImVec2 textPos(iconPos.x + iconSize + padding,
                   pos.y + (size.y - textSize.y) * 0.5f);
    ImGui::GetWindowDrawList()->AddText(textPos, IM_COL32_WHITE, label);
    
    return pressed;
}
