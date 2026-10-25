// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::deny::{denied_component, is_denied_name};
use crate::error::{FsError, PathError};
use crate::relpath::RelPath;

static NEXT_GUARD_ID: AtomicU32 = AtomicU32::new(1);
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// What GMM may do inside a root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    ReadOnly,
    ReadWrite,
}

/// Handle to a root registered with a specific [`Guard`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootId {
    guard: u32,
    index: u32,
}

/// A path that is known to sit inside a registered root.
///
/// Only a [`Guard`] can create one. Whether the path exists, and whether a symlink inside it
/// leads elsewhere, is checked again on every operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopedPath {
    root: RootId,
    rel: RelPath,
}

impl ScopedPath {
    pub fn root(&self) -> RootId {
        self.root
    }

    pub fn rel(&self) -> &RelPath {
        &self.rel
    }

    pub fn file_name(&self) -> Option<&str> {
        self.rel.file_name()
    }

    pub fn parent(&self) -> Option<Self> {
        Some(Self {
            root: self.root,
            rel: self.rel.parent()?,
        })
    }

    pub fn join(&self, rel: &str) -> Result<Self, PathError> {
        Ok(self.join_rel(&RelPath::parse(rel)?))
    }

    pub fn join_rel(&self, rel: &RelPath) -> Self {
        Self {
            root: self.root,
            rel: self.rel.join(rel),
        }
    }
}

/// One entry returned by [`Guard::read_dir`].
#[derive(Debug, Clone)]
pub struct DirEntryInfo {
    pub path: ScopedPath,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub len: u64,
}

#[derive(Debug)]
struct Root {
    label: String,
    /// Canonical form, so prefix checks compare like with like.
    path: PathBuf,
    access: Access,
}

/// The registry of roots, and the only way GMM touches the filesystem.
#[derive(Debug)]
pub struct Guard {
    id: u32,
    roots: Vec<Root>,
}

impl Default for Guard {
    fn default() -> Self {
        Self::new()
    }
}

impl Guard {
    pub fn new() -> Self {
        Self {
            id: NEXT_GUARD_ID.fetch_add(1, Ordering::Relaxed),
            roots: Vec::new(),
        }
    }

    /// Registers an existing directory as a root. Registering the same directory with the same
    /// access twice returns the existing id.
    pub fn register_root(
        &mut self,
        label: impl Into<String>,
        path: &Path,
        access: Access,
    ) -> Result<RootId, FsError> {
        if !path.is_absolute() {
            return Err(FsError::RootNotAbsolute(path.to_path_buf()));
        }
        let canonical = fs::canonicalize(path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => FsError::RootNotADirectory(path.to_path_buf()),
            _ => FsError::io("canonicalize", path, e),
        })?;
        if !canonical.is_dir() {
            return Err(FsError::RootNotADirectory(path.to_path_buf()));
        }
        if let Some(name) = denied_component(&canonical) {
            return Err(FsError::Denied(name));
        }
        if let Some(index) = self
            .roots
            .iter()
            .position(|r| r.path == canonical && r.access == access)
        {
            return Ok(self.id_for(index));
        }
        self.roots.push(Root {
            label: label.into(),
            path: canonical,
            access,
        });
        Ok(self.id_for(self.roots.len() - 1))
    }

    /// Like [`Guard::register_root`], but creates the directory first if it doesn't exist.
    /// For GMM's own app folders (cache, data), which may not exist on first run.
    pub fn ensure_root(
        &mut self,
        label: impl Into<String>,
        path: &Path,
        access: Access,
    ) -> Result<RootId, FsError> {
        if !path.is_absolute() {
            return Err(FsError::RootNotAbsolute(path.to_path_buf()));
        }
        if let Some(name) = denied_component(path) {
            return Err(FsError::Denied(name));
        }
        fs::create_dir_all(path).map_err(|e| FsError::io("create_dir_all", path, e))?;
        self.register_root(label, path, access)
    }

    pub fn root_label(&self, id: RootId) -> Option<&str> {
        self.root(id).ok().map(|r| r.label.as_str())
    }

    pub fn root_path(&self, id: RootId) -> Option<&Path> {
        self.root(id).ok().map(|r| r.path.as_path())
    }

    pub fn root_access(&self, id: RootId) -> Option<Access> {
        self.root(id).ok().map(|r| r.access)
    }

    /// A path inside `root`, from a relative path string.
    pub fn path(&self, root: RootId, rel: &str) -> Result<ScopedPath, FsError> {
        self.root(root)?;
        Ok(ScopedPath {
            root,
            rel: RelPath::parse(rel)?,
        })
    }

    /// Maps an absolute path (e.g. from detection or a folder picker) to the most specific
    /// registered root that contains it.
    pub fn scope_absolute(&self, abs: &Path) -> Result<ScopedPath, FsError> {
        if !abs.is_absolute() {
            return Err(FsError::OutsideRoots(abs.to_path_buf()));
        }
        let resolved = canonicalize_existing_prefix(abs)?;
        let (index, root) = self
            .roots
            .iter()
            .enumerate()
            .filter(|(_, r)| resolved.starts_with(&r.path))
            .max_by_key(|(_, r)| r.path.components().count())
            .ok_or_else(|| FsError::OutsideRoots(abs.to_path_buf()))?;
        let rest = resolved
            .strip_prefix(&root.path)
            .map_err(|_| FsError::OutsideRoots(abs.to_path_buf()))?;
        let rest = rest.to_str().ok_or(PathError::NonUtf8)?;
        Ok(ScopedPath {
            root: self.id_for(index),
            rel: RelPath::parse(rest)?,
        })
    }

    /// Human-readable form for logs and messages: `label:/rel/path`.
    pub fn display(&self, p: &ScopedPath) -> String {
        let label = self.root_label(p.root).unwrap_or("?");
        format!("{label}:/{}", p.rel)
    }

    // ---- reading -------------------------------------------------------------------------

    pub fn read(&self, p: &ScopedPath) -> Result<Vec<u8>, FsError> {
        let path = self.resolve(p, Access::ReadOnly)?;
        fs::read(&path).map_err(|e| FsError::io("read", path, e))
    }

    pub fn read_to_string(&self, p: &ScopedPath) -> Result<String, FsError> {
        let path = self.resolve(p, Access::ReadOnly)?;
        fs::read_to_string(&path).map_err(|e| FsError::io("read", path, e))
    }

    /// Opens a file for streaming reads (archives, hashing).
    pub fn open_read(&self, p: &ScopedPath) -> Result<File, FsError> {
        let path = self.resolve(p, Access::ReadOnly)?;
        File::open(&path).map_err(|e| FsError::io("open", path, e))
    }

    pub fn metadata(&self, p: &ScopedPath) -> Result<fs::Metadata, FsError> {
        let path = self.resolve(p, Access::ReadOnly)?;
        fs::metadata(&path).map_err(|e| FsError::io("metadata", path, e))
    }

    /// The checked absolute path of an existing directory, to hand to the OS shell's "open
    /// folder" (security rule R7: folders only). Fails for files and missing paths.
    pub fn os_path_of_dir(&self, p: &ScopedPath) -> Result<PathBuf, FsError> {
        let path = self.resolve(p, Access::ReadOnly)?;
        let meta = fs::metadata(&path).map_err(|e| FsError::io("open folder", &path, e))?;
        if !meta.is_dir() {
            let not_dir = io::Error::new(io::ErrorKind::InvalidInput, "not a folder");
            return Err(FsError::io("open folder", path, not_dir));
        }
        Ok(path)
    }

    pub fn exists(&self, p: &ScopedPath) -> Result<bool, FsError> {
        let path = self.resolve(p, Access::ReadOnly)?;
        Ok(fs::symlink_metadata(path).is_ok())
    }

    /// Lists a directory, sorted by name. Entries with denied names, non-UTF-8 names, or names
    /// that fail [`RelPath`] validation are skipped, since GMM could not address them anyway.
    pub fn read_dir(&self, p: &ScopedPath) -> Result<Vec<DirEntryInfo>, FsError> {
        let path = self.resolve(p, Access::ReadOnly)?;
        let entries = fs::read_dir(&path).map_err(|e| FsError::io("read_dir", &path, e))?;
        let mut out = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| FsError::io("read_dir", &path, e))?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if is_denied_name(&name) {
                continue;
            }
            let Ok(rel) = RelPath::parse(&name) else {
                continue;
            };
            let file_type = entry
                .file_type()
                .map_err(|e| FsError::io("read_dir", entry.path(), e))?;
            let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(DirEntryInfo {
                path: p.join_rel(&rel),
                name,
                is_dir: file_type.is_dir(),
                is_symlink: file_type.is_symlink(),
                len,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    // ---- writing (read-write roots only) -------------------------------------------------

    pub fn create_dir_all(&self, p: &ScopedPath) -> Result<(), FsError> {
        let path = self.resolve(p, Access::ReadWrite)?;
        fs::create_dir_all(&path).map_err(|e| FsError::io("create_dir_all", path, e))
    }

    /// Creates a new file, failing if it already exists (downloads, staging).
    pub fn create_new(&self, p: &ScopedPath) -> Result<File, FsError> {
        let path = self.resolve_below_root(p, Access::ReadWrite)?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| FsError::io("create", path, e))
    }

    /// Opens an existing file for appending (resuming a partial download).
    pub fn open_append(&self, p: &ScopedPath) -> Result<File, FsError> {
        let path = self.resolve_below_root(p, Access::ReadWrite)?;
        OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(|e| FsError::io("open", path, e))
    }

    /// Writes a whole file via a temporary sibling and a rename, so readers never see a
    /// half-written file.
    pub fn write_atomic(&self, p: &ScopedPath, contents: &[u8]) -> Result<(), FsError> {
        let path = self.resolve_below_root(p, Access::ReadWrite)?;
        let name = p.file_name().unwrap_or("file");
        let tmp = path.with_file_name(format!(
            ".{name}.gmm-tmp-{}-{}",
            std::process::id(),
            TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let write = || -> io::Result<()> {
            let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
            file.write_all(contents)?;
            file.sync_all()
        };
        if let Err(e) = write().and_then(|()| fs::rename(&tmp, &path)) {
            let _ = fs::remove_file(&tmp);
            return Err(FsError::io("write", path, e));
        }
        Ok(())
    }

    /// Renames within one root (e.g. toggling the `DISABLED` prefix, or moving a staged mod
    /// into place).
    pub fn rename(&self, from: &ScopedPath, to: &ScopedPath) -> Result<(), FsError> {
        if from.root != to.root {
            return Err(FsError::CrossRoot);
        }
        let src = self.resolve_below_root(from, Access::ReadWrite)?;
        let dst = self.resolve_below_root(to, Access::ReadWrite)?;
        fs::rename(&src, &dst).map_err(|e| FsError::io("rename", src, e))
    }

    /// Permanently removes a file. Only for GMM's own temporary and staging data; deletes the
    /// user asks for go to the Recycle Bin (security rule R8).
    pub fn remove_file(&self, p: &ScopedPath) -> Result<(), FsError> {
        let path = self.resolve_below_root(p, Access::ReadWrite)?;
        fs::remove_file(&path).map_err(|e| FsError::io("remove_file", path, e))
    }

    /// Permanently removes a directory tree, without following symlinks or junctions inside
    /// it. Same restriction as [`Guard::remove_file`].
    pub fn remove_dir_all(&self, p: &ScopedPath) -> Result<(), FsError> {
        let path = self.resolve_below_root(p, Access::ReadWrite)?;
        fs::remove_dir_all(&path).map_err(|e| FsError::io("remove_dir_all", path, e))
    }

    /// Free space, in bytes, on the drive that holds this path (for the storage report).
    pub fn available_space(&self, p: &ScopedPath) -> Result<u64, FsError> {
        let path = self.resolve(p, Access::ReadOnly)?;
        fs4::available_space(&path).map_err(|e| FsError::io("available_space", path, e))
    }

    /// Moves a file or folder to the Recycle Bin (the trash on Linux), so the user can get it
    /// back. This is how GMM deletes anything the user asked to delete (security rule R8).
    pub fn move_to_trash(&self, p: &ScopedPath) -> Result<(), FsError> {
        let path = self.resolve_below_root(p, Access::ReadWrite)?;
        trash::delete(&path).map_err(|e| FsError::Trash {
            path,
            message: e.to_string(),
        })
    }

    /// The checked absolute path of a file in a read-write root, for a library that must open
    /// the file itself (SQLite opens its database, journal and WAL files by name). Only use it
    /// to hand the path straight to that library; the guard can't see what happens next.
    pub fn os_path_for_library(&self, p: &ScopedPath) -> Result<PathBuf, FsError> {
        self.resolve_below_root(p, Access::ReadWrite)
    }

    // ---- internals -----------------------------------------------------------------------

    fn id_for(&self, index: usize) -> RootId {
        RootId {
            guard: self.id,
            index: u32::try_from(index).unwrap_or(u32::MAX),
        }
    }

    fn root(&self, id: RootId) -> Result<&Root, FsError> {
        if id.guard != self.id {
            return Err(FsError::ForeignPath);
        }
        let index = usize::try_from(id.index).map_err(|_| FsError::ForeignPath)?;
        self.roots.get(index).ok_or(FsError::ForeignPath)
    }

    /// Checks access, the denylist, and that the real location (after symlinks and junctions)
    /// is still inside the root. Returns the resolved absolute path.
    fn resolve(&self, p: &ScopedPath, need: Access) -> Result<PathBuf, FsError> {
        let root = self.root(p.root)?;
        if need == Access::ReadWrite && root.access == Access::ReadOnly {
            return Err(FsError::ReadOnlyRoot(root.label.clone()));
        }
        if let Some(name) = p.rel.components().iter().find(|c| is_denied_name(c)) {
            return Err(FsError::Denied(name.clone()));
        }
        let full = root.path.join(p.rel.to_path_buf());
        let resolved = canonicalize_existing_prefix(&full)?;
        if !resolved.starts_with(&root.path) {
            return Err(FsError::Escape(full));
        }
        if let Some(name) = denied_component(&resolved) {
            return Err(FsError::Denied(name));
        }
        Ok(resolved)
    }

    /// Like [`Guard::resolve`], but refuses the root directory itself (it must never be
    /// replaced, renamed or removed).
    fn resolve_below_root(&self, p: &ScopedPath, need: Access) -> Result<PathBuf, FsError> {
        if p.rel.is_root() {
            return Err(FsError::io(
                "modify",
                self.root_path(p.root).unwrap_or(Path::new("")),
                io::Error::new(io::ErrorKind::InvalidInput, "cannot modify a root itself"),
            ));
        }
        self.resolve(p, need)
    }
}

/// Canonicalizes the longest existing prefix of `path` and re-appends the rest unchanged.
/// This resolves every symlink and junction that already exists on disk.
fn canonicalize_existing_prefix(path: &Path) -> Result<PathBuf, FsError> {
    let mut existing = path;
    let mut tail: Vec<&OsStr> = Vec::new();
    loop {
        match fs::canonicalize(existing) {
            Ok(mut out) => {
                for part in tail.iter().rev() {
                    out.push(part);
                }
                return Ok(out);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name() else {
                    return Err(FsError::io("canonicalize", path, e));
                };
                tail.push(name);
                existing = existing
                    .parent()
                    .ok_or_else(|| FsError::OutsideRoots(path.to_path_buf()))?;
            }
            Err(e) => return Err(FsError::io("canonicalize", path, e)),
        }
    }
}
