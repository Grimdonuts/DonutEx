use crate::syntax::Language;
use std::path::PathBuf;

/// Candidate language-server binaries for a given language, tried in order
/// until one is found on PATH. Empty slice means "no LSP support attempted".
pub fn candidates(lang: Language) -> &'static [(&'static str, &'static [&'static str])] {
    match lang {
        Language::Rust => &[("rust-analyzer", &[])],
        Language::C => &[("clangd", &[])],
        Language::Python => &[
            ("pyright-langserver", &["--stdio"]),
            ("pylsp", &[]),
        ],
        Language::JavaScript | Language::TypeScript => {
            &[("typescript-language-server", &["--stdio"])]
        }
        Language::Lua => &[("lua-language-server", &[])],
        Language::Json | Language::Toml | Language::Shell | Language::PlainText => &[],
    }
}

/// The LSP `languageId` string for a document of this language.
///
/// `.jsx`/`.tsx` need `javascriptreact`/`typescriptreact` rather than the
/// plain `javascript`/`typescript` ids - typescript-language-server *will*
/// still classify these files by their extension either way, but sending the
/// wrong id measurably corrupts its analysis: goto-definition on a `.tsx`
/// file opened as `typescript` resolves to a bogus self-referencing location
/// instead of the real declaration, and semantic-token counts differ too.
/// `Language::TypeScript`/`JavaScript` don't otherwise distinguish JSX from
/// non-JSX (our own tokenizer doesn't need to), so the extension is checked
/// here rather than adding new `Language` variants just for this.
pub fn language_id(lang: Language, path: &std::path::Path) -> &'static str {
    let jsx_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("tsx") || e.eq_ignore_ascii_case("jsx"));
    match lang {
        Language::Rust => "rust",
        Language::C => "c",
        Language::Python => "python",
        Language::Lua => "lua",
        Language::JavaScript if jsx_ext => "javascriptreact",
        Language::JavaScript => "javascript",
        Language::TypeScript if jsx_ext => "typescriptreact",
        Language::TypeScript => "typescript",
        Language::Json => "json",
        Language::Toml => "toml",
        Language::Shell => "shellscript",
        Language::PlainText => "plaintext",
    }
}

/// Minimal PATH search so we don't pull in a `which` dependency for one lookup.
pub fn find_on_path(cmd: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{cmd}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}
