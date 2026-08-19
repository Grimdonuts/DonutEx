//! Thin wrapper around the `git` CLI. Shelling out (rather than linking
//! libgit2) keeps this dependency-free and matches how VS Code's own git
//! integration works under the hood.

use std::path::{Path, PathBuf};
use std::process::Command;

/// One entry from `git status`, either in the index (staged) or the
/// worktree (unstaged). `status` is the raw porcelain status letter:
/// M(odified) A(dded) D(eleted) R(enamed) C(opied) U(nmerged) ?(untracked).
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub orig_path: Option<PathBuf>,
    pub status: char,
}

#[derive(Default)]
pub struct Status {
    pub branch: Option<String>,
    pub staged: Vec<FileEntry>,
    pub unstaged: Vec<FileEntry>,
}

fn git(root: &Path, build: impl FnOnce(&mut Command)) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root);
    build(&mut cmd);
    let output = cmd
        .output()
        .map_err(|e| format!("failed to run git: {}", e))?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if msg.is_empty() {
            "git command failed".to_string()
        } else {
            msg
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn is_repo(root: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn current_branch(root: &Path) -> Option<String> {
    let name = git(root, |c| {
        c.args(["branch", "--show-current"]);
    })
    .ok()?;
    let name = name.trim();
    if !name.is_empty() {
        return Some(name.to_string());
    }
    // Detached HEAD: fall back to a short commit hash.
    git(root, |c| {
        c.args(["rev-parse", "--short", "HEAD"]);
    })
    .ok()
    .map(|h| format!("({})", h.trim()))
}

/// Parses `git status --porcelain=v1` output. Each line is `XY PATH`, where
/// X is the index (staged) status and Y is the worktree (unstaged) status;
/// renames appear as `PATH_OLD -> PATH_NEW`.
pub fn status(root: &Path) -> Result<Status, String> {
    let out = git(root, |c| {
        c.args(["status", "--porcelain=v1"]);
    })?;

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    for line in out.lines() {
        if line.len() < 4 {
            continue;
        }
        let x = line.as_bytes()[0] as char;
        let y = line.as_bytes()[1] as char;
        let rest = &line[3..];
        let (orig, path) = match rest.find(" -> ") {
            Some(idx) => (
                Some(PathBuf::from(&rest[..idx])),
                PathBuf::from(&rest[idx + 4..]),
            ),
            None => (None, PathBuf::from(rest)),
        };

        if x == '?' && y == '?' {
            unstaged.push(FileEntry {
                path,
                orig_path: None,
                status: '?',
            });
            continue;
        }
        if x != ' ' {
            staged.push(FileEntry {
                path: path.clone(),
                orig_path: orig.clone(),
                status: x,
            });
        }
        if y != ' ' {
            unstaged.push(FileEntry {
                path,
                orig_path: orig,
                status: y,
            });
        }
    }

    Ok(Status {
        branch: current_branch(root),
        staged,
        unstaged,
    })
}

/// Unified diff for a tracked file, staged (`--cached`) or against the
/// worktree.
pub fn diff(root: &Path, path: &Path, staged: bool) -> Result<String, String> {
    git(root, |c| {
        c.arg("diff");
        if staged {
            c.arg("--cached");
        }
        c.arg("--").arg(path);
    })
}

/// Diff for an untracked file: `git diff` shows nothing for a file that
/// isn't in the index, so this diffs against `/dev/null` to present it the
/// same way VS Code does - the whole file as added lines.
pub fn diff_untracked(root: &Path, path: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff", "--no-index", "--"])
        .arg("/dev/null")
        .arg(path)
        .output()
        .map_err(|e| format!("failed to run git: {}", e))?;
    // --no-index exits 1 when the inputs differ (the normal case here), so
    // success can't be used to detect failure - only a status >1 means an
    // actual error running the diff.
    if let Some(code) = output.status.code() {
        if code > 1 {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn stage(root: &Path, path: &Path) -> Result<(), String> {
    git(root, |c| {
        c.arg("add").arg("--").arg(path);
    })
    .map(|_| ())
}

pub fn stage_all(root: &Path) -> Result<(), String> {
    git(root, |c| {
        c.args(["add", "-A"]);
    })
    .map(|_| ())
}

pub fn unstage(root: &Path, path: &Path) -> Result<(), String> {
    git(root, |c| {
        c.arg("reset").arg("--").arg(path);
    })
    .map(|_| ())
}

pub fn unstage_all(root: &Path) -> Result<(), String> {
    git(root, |c| {
        c.arg("reset");
    })
    .map(|_| ())
}

/// Discards local changes to `path`. For an untracked file that means
/// deleting it outright (there's nothing in the index to restore); for a
/// tracked file it means checking the index/HEAD copy back out over the
/// worktree copy. Irreversible either way - callers should confirm first.
pub fn discard(root: &Path, entry: &FileEntry) -> Result<(), String> {
    if entry.status == '?' {
        std::fs::remove_file(root.join(&entry.path)).map_err(|e| e.to_string())
    } else {
        git(root, |c| {
            c.arg("checkout").arg("--").arg(&entry.path);
        })
        .map(|_| ())
    }
}

pub fn commit(root: &Path, message: &str) -> Result<String, String> {
    git(root, |c| {
        c.arg("commit").arg("-m").arg(message);
    })
}

/// Every file git considers "not ignored": tracked files (`--cached`) plus
/// untracked files that survive `.gitignore`/`.git/info/exclude`/the global
/// excludes file (`--others --exclude-standard`). Used by the search panel
/// so it automatically skips whatever a project already excludes -
/// `node_modules`, build output, etc. - without maintaining a guessed list
/// of directory names ourselves.
pub fn list_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let out = git(root, |c| {
        c.args(["ls-files", "--cached", "--others", "--exclude-standard"]);
    })?;
    Ok(out.lines().map(|l| root.join(l)).collect())
}
