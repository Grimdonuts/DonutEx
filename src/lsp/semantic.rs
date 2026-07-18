use crate::syntax::{Token, TokenKind};

/// Converts a UTF-16 code-unit offset (LSP's unit for `character`) into a
/// char offset into `line` (ropey/egui's unit). O(n) in the line length,
/// which is fine since this only runs once per line per semantic-tokens
/// response, not per paint frame.
pub fn utf16_offset_to_char_offset(line: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0usize;
    for (char_idx, ch) in line.chars().enumerate() {
        if utf16_count >= utf16_offset {
            return char_idx;
        }
        utf16_count += ch.len_utf16();
    }
    line.chars().count()
}

/// The inverse of `utf16_offset_to_char_offset`: how many UTF-16 code units
/// the first `char_offset` chars of `line` take up.
pub fn char_offset_to_utf16_offset(line: &str, char_offset: usize) -> usize {
    line.chars().take(char_offset).map(|c| c.len_utf16()).sum()
}

fn map_token_type(name: &str) -> TokenKind {
    match name {
        "keyword" | "modifier" => TokenKind::Keyword,
        "type" | "class" | "interface" | "struct" | "typeParameter" | "enum" | "builtinType" => {
            TokenKind::Type
        }
        "string" | "regexp" => TokenKind::String,
        "number" => TokenKind::Number,
        "comment" => TokenKind::Comment,
        "function" | "method" => TokenKind::Function,
        "macro" => TokenKind::Macro,
        "variable" | "selfKeyword" => TokenKind::Variable,
        "parameter" => TokenKind::Parameter,
        "property" | "event" => TokenKind::Property,
        "namespace" => TokenKind::Namespace,
        "enumMember" => TokenKind::EnumMember,
        "decorator" | "attribute" => TokenKind::Decorator,
        _ => TokenKind::Plain,
    }
}

/// Decodes the LSP semantic-tokens delta encoding (each entry gives
/// deltaLine, deltaStartChar (UTF-16), length (UTF-16), tokenType index and
/// a modifiers bitmask we ignore) into per-line token lists indexed the same
/// way as `syntax::tokenize_line`'s output, using `legend` to translate the
/// server's token-type indices into our `TokenKind`.
pub fn decode(
    legend: &[String],
    data: &[lsp_types::SemanticToken],
    lines: &[String],
) -> Vec<Vec<Token>> {
    let mut result: Vec<Vec<Token>> = vec![Vec::new(); lines.len()];
    let mut line = 0usize;
    let mut char_utf16 = 0usize;

    for tok in data {
        if tok.delta_line > 0 {
            line += tok.delta_line as usize;
            char_utf16 = tok.delta_start as usize;
        } else {
            char_utf16 += tok.delta_start as usize;
        }

        let Some(line_text) = lines.get(line) else {
            continue;
        };
        let start_char = utf16_offset_to_char_offset(line_text, char_utf16);
        let end_char = utf16_offset_to_char_offset(line_text, char_utf16 + tok.length as usize);
        let kind = legend
            .get(tok.token_type as usize)
            .map(|s| map_token_type(s))
            .unwrap_or(TokenKind::Plain);

        if let Some(bucket) = result.get_mut(line) {
            bucket.push(Token {
                kind,
                start: start_char,
                end: end_char,
            });
        }
    }

    result
}
