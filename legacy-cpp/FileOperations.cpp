#include "FileOperations.hpp"
#include "TextEditor.hpp"
#include <cstring>
#include <fstream>
#include <nfd.h>
#include <sstream>

FileOperations::FileOperations(TextEditor *editor) : editor_(editor) {}

void FileOperations::openFile(const std::string &fname) {
  std::ifstream file(fname, std::ios::binary);
  if (!file.is_open()) {
    editor_->addOutput(editor_->icons["error"],
                       "Could not open file: " + fname + " (" +
                           std::strerror(errno) + ")");
    return;
  }

  std::ostringstream buffer;
  buffer << file.rdbuf();
  editor_->content = PieceTable(buffer.str());
  editor_->rebuildCache();
  editor_->filename = fname;
  editor_->modified = false;
  editor_->focusEditor = true;
  editor_->cursorLine = 0;
  editor_->cursorColumn = 0;

  if (editor_->content.size() == 0)
    editor_->addOutput(editor_->icons["folder"], "Opened empty file: " + fname);
  else
    editor_->addOutput(editor_->icons["folder"],
                       "Opened: " + fname + " (" +
                           std::to_string(editor_->content.size()) + " bytes)");
}

void FileOperations::newFile() {
  editor_->content.clear();
  editor_->content.insert(0, "");
  editor_->rebuildCache();
  editor_->filename.clear();
  editor_->modified = false;
  editor_->focusEditor = true;
  editor_->cursorIndex = 0;
  editor_->scrollX = 0.0f;
  editor_->scrollY = 0.0f;
  editor_->caretFollow = true;
  editor_->addOutput(editor_->icons["document"], "New file created");
}

void FileOperations::saveFile() {
  if (editor_->filename.empty()) {
    showSaveDialog("untitled.txt");
    return;
  }

  std::ofstream file(editor_->filename);
  if (file.is_open()) {
    file << editor_->content.getText();
    editor_->modified = false;
    editor_->addOutput(editor_->icons["save"], "Saved: " + editor_->filename);
    file.close();
  } else {
    editor_->addOutput(editor_->icons["error"],
                       "Could not save file: " + editor_->filename);
  }
}

void FileOperations::showOpenDialog() {
  nfdchar_t *outPath = nullptr;

  nfdu8filteritem_t filters[4] = {{"C++ Source", "cpp"},
                                  {"C++ Header", "hpp"},
                                  {"Text File", "txt"},
                                  {"Lua Script", "lua"}};

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
    editor_->addOutput(editor_->icons["error"],
                       std::string("NFD Error: ") + NFD_GetError());
  }
}

void FileOperations::showSaveDialog(const std::string &defaultFileName) {
  nfdchar_t *savePath = nullptr;

  nfdu8filteritem_t filters[4] = {{"C++ Source", "cpp"},
                                  {"C++ Header", "hpp"},
                                  {"Text File", "txt"},
                                  {"Lua Script", "lua"}};

  static bool nfd_initialized = false;
  if (!nfd_initialized) {
    if (NFD_Init() == NFD_OKAY) {
      nfd_initialized = true;
    }
  }

  nfdresult_t result =
      NFD_SaveDialog(&savePath, filters, 4, nullptr, defaultFileName.c_str());
  if (result == NFD_OKAY) {
    editor_->filename = savePath;
    saveFile();
    free(savePath);
  } else if (result == NFD_ERROR) {
    editor_->addOutput(editor_->icons["error"],
                       std::string("NFD Error: ") + NFD_GetError());
  }
}
