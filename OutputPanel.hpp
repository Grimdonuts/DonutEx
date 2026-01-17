#pragma once

#include "imgui.h"
#include <string>

class TextEditor;
class EditorCommands;

class OutputPanel {
public:
  OutputPanel(TextEditor *editor, EditorCommands *commands);
  ~OutputPanel() = default;

  void render(ImVec2 workPos, ImVec2 workSize, float outputHeight,
              float explorerWidth);

private:
  TextEditor *editor_;
  EditorCommands *commands_;
  std::string commandInput_;
};
