#include "PieceTable.hpp"
#include <algorithm> // for std::min

PieceTable::PieceTable() = default;

PieceTable::PieceTable(const std::string& original)
    : originalBuffer(original) {
    if (!original.empty()) {
        pieces.push_back({Piece::BufferKind::Original, 0, original.size()});
    }
}

void PieceTable::insert(size_t pos, const std::string& text) {
    if (text.empty()) return;
    size_t addStart = addBuffer.size();
    addBuffer += text;

    size_t cur = 0;
    for (size_t i = 0; i < pieces.size(); ++i) {
        auto& p = pieces[i];
        if (pos <= cur + p.length) {
            size_t offset = pos - cur;
            Piece before   = {p.buffer, p.start, offset};
            Piece inserted = {Piece::BufferKind::Add, addStart, text.size()};
            Piece after    = {p.buffer, p.start + offset, p.length - offset};

            std::vector<Piece> newPieces;
            if (before.length) newPieces.push_back(before);
            newPieces.push_back(inserted);
            if (after.length) newPieces.push_back(after);

            pieces.erase(pieces.begin() + i);
            pieces.insert(pieces.begin() + i, newPieces.begin(), newPieces.end());
            return;
        }
        cur += p.length;
    }
    // append at end
    pieces.push_back({Piece::BufferKind::Add, addStart, text.size()});
}

void PieceTable::erase(size_t pos, size_t len) {
    if (len == 0) return;
    size_t cur = 0;
    for (size_t i = 0; i < pieces.size() && len > 0;) {
        auto& p = pieces[i];
        if (pos < cur + p.length) {
            size_t offset   = pos - cur;
            size_t eraseLen = std::min(len, p.length - offset);

            Piece before = {p.buffer, p.start, offset};
            Piece after  = {p.buffer, p.start + offset + eraseLen, p.length - offset - eraseLen};

            std::vector<Piece> newPieces;
            if (before.length) newPieces.push_back(before);
            if (after.length)  newPieces.push_back(after);

            pieces.erase(pieces.begin() + i);
            pieces.insert(pieces.begin() + i, newPieces.begin(), newPieces.end());

            len -= eraseLen;
            pos += eraseLen;
            cur += offset;
        } else {
            cur += p.length;
            i++;
        }
    }
}

std::string PieceTable::getText() const {
    std::string result;
    result.reserve(size());
    for (auto& p : pieces) {
        const std::string& buf = (p.buffer == Piece::BufferKind::Original) ? originalBuffer : addBuffer;
        result.append(buf, p.start, p.length);
    }
    return result;
}

size_t PieceTable::size() const {
    size_t total = 0;
    for (auto& p : pieces) total += p.length;
    return total;
}

void PieceTable::clear() {
    originalBuffer.clear();
    addBuffer.clear();
    pieces.clear();
}
