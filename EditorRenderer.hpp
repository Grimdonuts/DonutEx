// EditorRenderer.hpp
#pragma once

#include "imgui.h"

class TextEditor;
class LuaBindings;

class EditorRenderer
{
public:
    EditorRenderer(TextEditor* editor, LuaBindings* lua);
    ~EditorRenderer() = default;
    
    void renderMenuBar();
    void renderEditor();
    void renderSettings();
    
private:
    TextEditor* editor_;
    LuaBindings* lua_;
    
    void renderGrid(ImDrawList* drawList, ImVec2 pos, float viewW, float viewH, 
                   float cellWidth, float lineHeight, float padX, float padY);
    void renderSelection(ImDrawList* drawList, ImVec2 pos, float cellWidth, 
                        float lineHeight, float padX, float padY, 
                        int firstVisibleLine, int lastVisibleLine);
    void renderVisibleLines(ImDrawList* drawList, ImVec2 pos, float padX, float padY,
                           int firstVisibleLine, int lastVisibleLine);
    void handleEditorInput(ImVec2 pos, float viewW, float viewH, float padX, float padY,
                          float cellWidth, float lineHeight, bool isFocused);
    void handleMouseInput(ImVec2 pos, float padX, float padY, float cellWidth, float lineHeight);
    void handleKeyboardInput();
    void renderCaret(ImDrawList* drawList, ImVec2 pos, float padX, float padY,
                    float cellWidth, float lineHeight, bool isFocused);
    void renderScrollbars(ImVec2 pos, float viewW, float viewH, float scrollbarW, 
                         float scrollbarH, float cellWidth, float lineHeight,
                         float visibleW, float visibleH);
    void renderHorizontalScrollbar(ImVec2 pos, float viewW, float viewH, float scrollbarH,
                                   float visibleW, float trackW, float extraPad);
    void renderVerticalScrollbar(ImVec2 pos, float viewW, float viewH, float scrollbarW,
                                 float visibleH, float trackH, float totalContentHeight);
    
    static void clampToViewport(ImGuiSizeCallbackData* data);
};
