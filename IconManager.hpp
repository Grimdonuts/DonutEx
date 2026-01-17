#pragma once

#include "imgui.h"

class TextEditor;

class IconManager {
public:
  IconManager(TextEditor *editor);
  ~IconManager() = default;

  void loadIcons(float dpiScale = 1.0f);
  ImTextureID loadSVGTexture(const char *filename, float targetHeight);
  bool iconTextButton(const char *id, ImTextureID icon, const char *label,
                      const ImVec2 &buttonSize);

private:
  TextEditor *editor_;
};
