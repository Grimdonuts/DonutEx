// FileOperations.hpp
#pragma once

#include <string>

class TextEditor;

class FileOperations
{
public:
    FileOperations(TextEditor* editor);
    ~FileOperations() = default;
    
    void openFile(const std::string& fname);
    void newFile();
    void saveFile();
    void showOpenDialog();
    void showSaveDialog(const std::string& defaultFileName);
    
private:
    TextEditor* editor_;
};
