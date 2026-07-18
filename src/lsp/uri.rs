use std::path::{Path, PathBuf};

/// Builds a `file://` URI from a filesystem path. lsp-types 0.97 dropped the
/// `url` crate in favor of its own `Uri` newtype with no `from_file_path`
/// convenience, so this does the percent-encoding by hand.
pub fn path_to_uri(path: &Path) -> Option<lsp_types::Uri> {
    let raw = path.to_str()?;
    let normalized = raw.replace('\\', "/");
    let mut encoded = String::with_capacity(normalized.len() + 8);
    encoded.push_str("file://");
    if !normalized.starts_with('/') {
        encoded.push('/');
    }
    for byte in normalized.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded.parse().ok()
}

/// The inverse of `path_to_uri`; returns `None` for non-`file://` schemes
/// (e.g. some servers link to synthetic sources the editor can't open).
pub fn uri_to_path(uri: &lsp_types::Uri) -> Option<PathBuf> {
    let s = uri.as_str();
    let rest = s.strip_prefix("file://")?;
    Some(PathBuf::from(percent_decode(rest)))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
