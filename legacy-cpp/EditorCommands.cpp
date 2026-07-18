#include "EditorCommands.hpp"
#include "LuaBindings.hpp"
#include "TextEditor.hpp"

EditorCommands::EditorCommands(TextEditor *editor) : editor_(editor) {}

void EditorCommands::registerCommands() {
  commands_["save"] = [this]() {
    editor_->addOutput("Executing save command...");
  };

  commands_["new"] = [this]() {
    editor_->addOutput("Executing new command...");
  };

  commands_["clear"] = [this]() { editor_->outputLines.clear(); };

  commands_["refresh"] = [this]() { refreshFileList(); };

  commands_["focus"] = [this]() { editor_->focusEditor = true; };
}

void EditorCommands::executeCommand(const std::string &cmd) {
  editor_->addOutput("> " + cmd);

  if (commands_.find(cmd) != commands_.end()) {
    commands_[cmd]();
  } else {
    std::string luaCmd = "if type(command_" + cmd +
                         ")=='function' then command_" + cmd + "() end";
    // Lua evaluation would happen through LuaBindings
    editor_->addOutput(editor_->icons["error"], "Unknown command: " + cmd);
  }
}

void EditorCommands::refreshFileList() {
  fileList_.clear();
  try {
    for (const auto &entry : std::filesystem::directory_iterator(".")) {
      fileList_.push_back(entry.path().filename().string());
    }
  } catch (...) {
    editor_->addOutput(editor_->icons["error"], "Error reading directory");
  }
}
