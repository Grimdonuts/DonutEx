// FileExplorer.cpp
#include "FileExplorer.hpp"
#include "TextEditor.hpp"
#include "IconManager.hpp"
#include "EditorCommands.hpp"
#include <filesystem>

FileExplorer::FileExplorer(TextEditor* editor, IconManager* iconMgr, EditorCommands* commands)
    : editor_(editor), iconMgr_(iconMgr), commands_(commands)
{
}

void FileExplorer::render(ImVec2 workPos, ImVec2 workSize, float explorerWidth)
{
    if (ImGui::Begin("File Explorer", nullptr,
                     ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoResize |
                     ImGuiWindowFlags_NoCollapse | ImGuiWindowFlags_NoBringToFrontOnFocus))
    {
        if (iconMgr_->iconTextButton("##refresh_btn", editor_->icons["refresh"], "Refresh", ImVec2(120, 30)))
        {
            commands_->refreshFileList();
        }
        
        ImGui::Separator();
        ImGui::BeginChild("FileList", ImVec2(0, -40), true);

        for (size_t i = 0; i < commands_->getFileList().size(); ++i)
        {
            const auto& file = commands_->getFileList()[i];
            bool isDirectory = std::filesystem::is_directory(file);
            ImGui::PushID((int)i);
            ImGui::Selectable("##fileitem", false);
            
            ImVec2 pos = ImGui::GetItemRectMin();
            ImVec2 size = ImGui::GetItemRectSize();
            ImTextureID iconTex = isDirectory ? editor_->icons["folder"] : editor_->icons["document"];
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
                editor_->openFile(file);
            }

            ImGui::PopID();
        }
        ImGui::EndChild();
        
        ImGui::Separator();
        if (iconMgr_->iconTextButton("##settings_btn", editor_->icons["settings"], "Settings", ImVec2(120, 30)))
        {
            editor_->showSettings = !editor_->showSettings;
        }
    }
    ImGui::End();
}
