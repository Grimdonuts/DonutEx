#pragma once
#include <string>
#include <vector>

struct Piece {
    enum class BufferKind { Original, Add };

    BufferKind buffer;  // which buffer this piece belongs to
    size_t start;       // starting index in that buffer
    size_t length;      // length of text in that buffer
};

class PieceTable {
public:
    PieceTable();
    PieceTable(const std::string& original);

    void insert(size_t pos, const std::string& text);
    void erase(size_t pos, size_t len);

    std::string getText() const;
    size_t size() const;
    bool empty() const { return size() == 0; }

    void clear();

private:
    std::string originalBuffer;
    std::string addBuffer;
    std::vector<Piece> pieces;
};
