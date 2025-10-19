// FileExplorer.hpp
#pragma once

#include "imgui.h"

class TextEditor;
class IconManager;
class EditorCommands;

class FileExplorer
{
public:
    FileExplorer(TextEditor* editor, IconManager* iconMgr, EditorCommands* commands);
    ~FileExplorer() = default;
    
    void render(ImVec2 workPos, ImVec2 workSize, float explorerWidth);
    
private:
    TextEditor* editor_;
    IconManager* iconMgr_;
    EditorCommands* commands_;
};
