use eframe::egui;
use std::path::{Path, PathBuf};

pub(crate) const IGNORE: &[&str] = &[".git", "target", "node_modules", ".claude", ".DS_Store"];

pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

pub fn build_tree(root: &Path) -> FileNode {
    build_node(root)
}

fn build_node(path: &Path) -> FileNode {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let is_dir = path.is_dir();
    let mut children = Vec::new();
    if is_dir {
        if let Ok(entries) = std::fs::read_dir(path) {
            let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            entries.sort_by_key(|e| {
                (
                    !e.path().is_dir(),
                    e.file_name().to_string_lossy().to_lowercase(),
                )
            });
            for entry in entries {
                let fname = entry.file_name().to_string_lossy().to_string();
                if IGNORE.contains(&fname.as_str()) {
                    continue;
                }
                children.push(build_node(&entry.path()));
            }
        }
    }
    FileNode {
        name,
        path: path.to_path_buf(),
        is_dir,
        children,
    }
}

/// Renders the tree and returns a file path if the user clicked one to open.
pub fn show(ui: &mut egui::Ui, node: &FileNode) -> Option<PathBuf> {
    let mut opened = None;
    for child in &node.children {
        show_node(ui, child, &mut opened);
    }
    opened
}

fn show_node(ui: &mut egui::Ui, node: &FileNode, opened: &mut Option<PathBuf>) {
    if node.is_dir {
        egui::CollapsingHeader::new(format!("\u{1F4C1} {}", node.name))
            .id_salt(&node.path)
            .default_open(false)
            .show(ui, |ui| {
                for child in &node.children {
                    show_node(ui, child, opened);
                }
            });
    } else if ui
        .selectable_label(false, format!("\u{1F4C4} {}", node.name))
        .clicked()
    {
        *opened = Some(node.path.clone());
    }
}
