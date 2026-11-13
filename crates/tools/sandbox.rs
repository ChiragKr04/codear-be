//! `Sandbox`: the one folder the tools may touch.

use std::io;
use std::path::{Component, Path, PathBuf};

use super::ToolError;

/// The only folder the agent may touch. Created once from `--repo` (or the
/// current directory) and passed to every tool.
#[derive(Debug, Clone)]
pub struct Sandbox {
    /// Canonical (absolute, symlink-free) path of the repo folder.
    root: PathBuf,
}

impl Sandbox {
    /// Fails if `root` does not exist or is not a directory.
    pub fn new(root: &Path) -> io::Result<Self> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotADirectory, "not a directory"));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Turns a path from the model into a real path inside the repo, or refuses.
    ///
    /// `rel` must be relative and free of `..`. Symlinks along the part of
    /// the path that exists are followed, and the result must still be inside
    /// the repo, so a link pointing out of it is refused. The path need not
    /// exist (`write_file` creates files). `""` and `"."` are the repo root.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf, ToolError> {
        let outside = || ToolError::Failed(format!("path {rel:?} is outside the repo"));

        let p = Path::new(rel);
        // 1. No absolute paths (join would drop the root) and no `..`.
        if p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(outside());
        }

        // 2. Follow symlinks on the deepest part that exists. `symlink_metadata`
        //    (not `exists`) so a broken link counts as existing and is
        //    canonicalized (and refused) instead of being skipped.
        let full = self.root.join(p);
        let mut existing = full.as_path();
        while existing.symlink_metadata().is_err() {
            existing = existing.parent().ok_or_else(outside)?;
        }
        let mut real = existing
            .canonicalize()
            .map_err(|e| ToolError::Failed(format!("cannot resolve {rel:?}: {e}")))?;
        let rest = full.strip_prefix(existing).expect("`existing` is an ancestor of `full`");
        // Joining an empty path would add a trailing `/`, turning `f.txt` into
        // `f.txt/`, which then fails to open with "Not a directory".
        if !rest.as_os_str().is_empty() {
            real.push(rest);
        }

        // 3. The real path must still be inside the repo.
        if !real.starts_with(&self.root) {
            return Err(outside());
        }
        Ok(real)
    }
}
