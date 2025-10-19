// EditorCommands.hpp
#pragma once

#include <string>
#include <map>
#include <functional>

class TextEditor;

class EditorCommands
{
public:
    EditorCommands(TextEditor* editor);
    ~EditorCommands() = default;
    
    void registerCommands();
    void executeCommand(const std::string& cmd);
    void refreshFileList();
    
    const std::map<std::string, std::function<void()>>& getCommands() const { return commands_; }
    const std::vector<std::string>& getFileList() const { return fileList_; }
    
private:
    TextEditor* editor_;
    std::map<std::string, std::function<void()>> commands_;
    std::vector<std::string> fileList_;
};
