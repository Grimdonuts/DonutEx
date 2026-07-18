use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    Rust,
    C,
    Python,
    Lua,
    JavaScript,
    TypeScript,
    Json,
    Toml,
    Shell,
    PlainText,
}

impl Language {
    pub fn from_path(path: Option<&Path>) -> Self {
        let Some(ext) = path.and_then(|p| p.extension()).and_then(|e| e.to_str()) else {
            return Language::PlainText;
        };
        match ext.to_ascii_lowercase().as_str() {
            "rs" => Language::Rust,
            "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Language::C,
            "py" | "pyw" => Language::Python,
            "lua" => Language::Lua,
            "js" | "mjs" | "cjs" | "jsx" => Language::JavaScript,
            "ts" | "tsx" => Language::TypeScript,
            "json" => Language::Json,
            "toml" => Language::Toml,
            "sh" | "bash" | "zsh" => Language::Shell,
            _ => Language::PlainText,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Plain,
    Keyword,
    Type,
    String,
    Number,
    Comment,
    Function,
    Macro,
}

pub struct Token {
    pub kind: TokenKind,
    pub start: usize, // char offset into the line
    pub end: usize,   // exclusive char offset
}

struct LangSpec {
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    line_comment: &'static [&'static str],
    block_comment: Option<(&'static str, &'static str)>,
    string_quotes: &'static [char],
}

const RUST: LangSpec = LangSpec {
    keywords: &[
        "fn", "let", "mut", "if", "else", "match", "for", "while", "loop", "return", "break",
        "continue", "struct", "enum", "impl", "trait", "pub", "use", "mod", "crate", "self",
        "Self", "super", "const", "static", "ref", "move", "async", "await", "dyn", "where",
        "as", "in", "unsafe", "extern", "type", "true", "false",
    ],
    types: &[
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64", "bool", "char", "str", "String", "Vec", "Option", "Result", "Box", "Rc",
        "Arc", "HashMap", "HashSet", "Some", "None", "Ok", "Err",
    ],
    line_comment: &["//"],
    block_comment: Some(("/*", "*/")),
    string_quotes: &['"'],
};

const C: LangSpec = LangSpec {
    keywords: &[
        "if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue",
        "return", "goto", "struct", "class", "enum", "union", "typedef", "public", "private",
        "protected", "virtual", "override", "static", "const", "extern", "inline", "volatile",
        "sizeof", "namespace", "using", "template", "typename", "new", "delete", "this", "true",
        "false", "friend", "operator", "throw", "try", "catch", "constexpr", "nullptr",
    ],
    types: &[
        "int", "char", "float", "double", "long", "short", "unsigned", "signed", "bool", "void",
        "auto", "size_t", "int8_t", "int16_t", "int32_t", "int64_t", "uint8_t", "uint16_t",
        "uint32_t", "uint64_t", "std", "string", "vector",
    ],
    line_comment: &["//"],
    block_comment: Some(("/*", "*/")),
    string_quotes: &['"', '\''],
};

const PYTHON: LangSpec = LangSpec {
    keywords: &[
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
        "try", "while", "with", "yield", "self",
    ],
    types: &["int", "str", "float", "bool", "list", "dict", "set", "tuple", "bytes"],
    line_comment: &["#"],
    block_comment: None,
    string_quotes: &['"', '\''],
};

const LUA: LangSpec = LangSpec {
    keywords: &[
        "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if",
        "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
    ],
    types: &[],
    line_comment: &["--"],
    block_comment: Some(("--[[", "]]")),
    string_quotes: &['"', '\''],
};

const JAVASCRIPT: LangSpec = LangSpec {
    keywords: &[
        "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete",
        "do", "else", "export", "extends", "finally", "for", "function", "if", "import", "in",
        "instanceof", "new", "return", "super", "switch", "this", "throw", "try", "typeof",
        "var", "let", "void", "while", "with", "yield", "async", "await", "static", "get",
        "set", "true", "false", "null", "undefined",
    ],
    types: &[],
    line_comment: &["//"],
    block_comment: Some(("/*", "*/")),
    string_quotes: &['"', '\''],
};

const TYPESCRIPT: LangSpec = LangSpec {
    keywords: &[
        "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete",
        "do", "else", "export", "extends", "finally", "for", "function", "if", "import", "in",
        "instanceof", "new", "return", "super", "switch", "this", "throw", "try", "typeof",
        "var", "let", "void", "while", "with", "yield", "async", "await", "static", "get",
        "set", "true", "false", "null", "undefined", "interface", "type", "enum", "implements",
        "namespace", "declare", "readonly", "public", "private", "protected", "as", "is",
        "keyof", "infer",
    ],
    types: &["string", "number", "boolean", "any", "unknown", "never", "object"],
    line_comment: &["//"],
    block_comment: Some(("/*", "*/")),
    string_quotes: &['"', '\''],
};

const JSON: LangSpec = LangSpec {
    keywords: &["true", "false", "null"],
    types: &[],
    line_comment: &[],
    block_comment: None,
    string_quotes: &['"'],
};

const TOML: LangSpec = LangSpec {
    keywords: &["true", "false"],
    types: &[],
    line_comment: &["#"],
    block_comment: None,
    string_quotes: &['"', '\''],
};

const SHELL: LangSpec = LangSpec {
    keywords: &[
        "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
        "function", "in", "local", "export", "return",
    ],
    types: &[],
    line_comment: &["#"],
    block_comment: None,
    string_quotes: &['"', '\''],
};

fn spec_for(lang: Language) -> Option<&'static LangSpec> {
    match lang {
        Language::Rust => Some(&RUST),
        Language::C => Some(&C),
        Language::Python => Some(&PYTHON),
        Language::Lua => Some(&LUA),
        Language::JavaScript => Some(&JAVASCRIPT),
        Language::TypeScript => Some(&TYPESCRIPT),
        Language::Json => Some(&JSON),
        Language::Toml => Some(&TOML),
        Language::Shell => Some(&SHELL),
        Language::PlainText => None,
    }
}

fn starts_with_at(chars: &[char], i: usize, pat: &str) -> bool {
    let pat_chars: Vec<char> = pat.chars().collect();
    if i + pat_chars.len() > chars.len() {
        return false;
    }
    chars[i..i + pat_chars.len()] == pat_chars[..]
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Tokenizes a single line for coloring. `in_block_comment` is whether the
/// line begins inside an unterminated block comment; the returned bool says
/// whether the line ends still inside one (multi-line comments are tracked
/// this way so we only ever look at one line at a time when painting).
pub fn tokenize_line(
    line: &str,
    lang: Language,
    in_block_comment: bool,
) -> (Vec<Token>, bool) {
    let mut tokens = Vec::new();
    let Some(spec) = spec_for(lang) else {
        return (tokens, false);
    };
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut in_block = in_block_comment;

    while i < n {
        // Continuing / entering a block comment.
        if in_block {
            let (_open, close) = spec.block_comment.unwrap();
            let start = i;
            match find_from(&chars, i, close) {
                Some(end_at) => {
                    i = end_at + close.chars().count();
                    in_block = false;
                    tokens.push(Token {
                        kind: TokenKind::Comment,
                        start,
                        end: i,
                    });
                }
                None => {
                    tokens.push(Token {
                        kind: TokenKind::Comment,
                        start,
                        end: n,
                    });
                    i = n;
                }
            }
            continue;
        }

        let c = chars[i];

        // Line comment: rest of line.
        if spec.line_comment.iter().any(|lc| starts_with_at(&chars, i, lc)) {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                end: n,
            });
            i = n;
            continue;
        }

        // Block comment open.
        if let Some((open, _close)) = spec.block_comment {
            if starts_with_at(&chars, i, open) {
                let start = i;
                i += open.chars().count();
                in_block = true;
                // will be closed (or not) by the loop iteration above
                if let Some((_o, close)) = spec.block_comment {
                    match find_from(&chars, i, close) {
                        Some(end_at) => {
                            i = end_at + close.chars().count();
                            in_block = false;
                        }
                        None => {
                            i = n;
                        }
                    }
                }
                tokens.push(Token {
                    kind: TokenKind::Comment,
                    start,
                    end: i,
                });
                continue;
            }
        }

        // String literal.
        if spec.string_quotes.contains(&c) {
            let quote = c;
            let start = i;
            i += 1;
            while i < n {
                if chars[i] == '\\' && i + 1 < n {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::String,
                start,
                end: i,
            });
            continue;
        }

        // Number literal.
        if c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Number,
                start,
                end: i,
            });
            continue;
        }

        // Identifier / keyword / type / function call / macro.
        if is_ident_start(c) {
            let start = i;
            i += 1;
            while i < n && is_ident_continue(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let kind = if spec.keywords.contains(&word.as_str()) {
                TokenKind::Keyword
            } else if spec.types.contains(&word.as_str())
                || word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
            {
                TokenKind::Type
            } else if i < n && chars[i] == '!' {
                TokenKind::Macro
            } else if i < n && chars[i] == '(' {
                TokenKind::Function
            } else {
                TokenKind::Plain
            };
            tokens.push(Token { kind, start, end: i });
            continue;
        }

        // Punctuation / whitespace / operators: skip, left uncolored.
        i += 1;
    }

    (tokens, in_block)
}

fn find_from(chars: &[char], from: usize, pat: &str) -> Option<usize> {
    let pat_chars: Vec<char> = pat.chars().collect();
    if pat_chars.is_empty() || from >= chars.len() {
        return None;
    }
    let last_start = chars.len().checked_sub(pat_chars.len())?;
    for i in from..=last_start {
        if chars[i..i + pat_chars.len()] == pat_chars[..] {
            return Some(i);
        }
    }
    None
}
