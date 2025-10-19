// OutputPanel.cpp
#include "OutputPanel.hpp"
#include "TextEditor.hpp"
#include "EditorCommands.hpp"
#include <misc/cpp/imgui_stdlib.h>

OutputPanel::OutputPanel(TextEditor* editor, EditorCommands* commands)
    : editor_(editor), commands_(commands)
{
}

void OutputPanel::render(ImVec2 workPos, ImVec2 workSize, float outputHeight, float explorerWidth)
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
        bool commandEntered = ImGui::InputText("##command", &commandInput_,
                                               ImGuiInputTextFlags_EnterReturnsTrue);
        ImGui::SameLine();
        if (ImGui::Button("Execute") || commandEntered)
        {
            if (!commandInput_.empty())
            {
                commands_->executeCommand(commandInput_);
                commandInput_.clear();
            }
        }
        
        ImGui::Separator();
        ImGui::BeginChild("OutputText");
        
        for (const auto& line : editor_->outputLines)
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

        // Auto-scroll to bottom
        if (ImGui::GetScrollY() >= ImGui::GetScrollMaxY())
        {
            ImGui::SetScrollHereY(1.0f);
        }
        
        ImGui::EndChild();
    }
    ImGui::End();
}
