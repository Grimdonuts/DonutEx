// CursorUtils.hpp
#pragma once

#include <string>
#include <utility>
#include "imgui.h"

class TextEditor;

class CursorUtils
{
public:
    CursorUtils(TextEditor* editor);
    ~CursorUtils() = default;
    
    std::string getCurrentLine();
    std::string getCurrentWord();
    std::pair<int, int> getCursorPosition();
    ImVec2 getCursorScreenPos();
    void replaceCurrentWordWith(const std::string& full);
    
private:
    TextEditor* editor_;
};
