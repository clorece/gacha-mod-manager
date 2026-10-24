// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io;
use std::path::PathBuf;

/// Why a relative path string was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("path contains a NUL byte")]
    Nul,
    #[error("path is absolute")]
    Absolute,
    #[error("path contains a parent-directory component (`..`)")]
    ParentDir,
    #[error("path component {0:?} contains `:` (drive letter or alternate data stream)")]
    Colon(String),
    #[error("path component {0:?} contains a character that is invalid in file names")]
    InvalidChar(String),
    #[error("path component {0:?} ends with a dot or space")]
    TrailingDotOrSpace(String),
    #[error("path component {0:?} is a reserved Windows device name")]
    Reserved(String),
    #[error("path component is longer than {max} bytes")]
    ComponentTooLong { max: usize },
    #[error("path is not valid UTF-8")]
    NonUtf8,
}

/// A filesystem operation that the guard refused or that failed.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error(transparent)]
    InvalidPath(#[from] PathError),
    #[error("root path must be absolute: {0}")]
    RootNotAbsolute(PathBuf),
    #[error("root path is not an existing directory: {0}")]
    RootNotADirectory(PathBuf),
    #[error("path is not inside any registered root: {0}")]
    OutsideRoots(PathBuf),
    #[error("path resolves outside its root (symlink or junction): {0}")]
    Escape(PathBuf),
    #[error("access to {0:?} is denied by policy")]
    Denied(String),
    #[error("root `{0}` is read-only")]
    ReadOnlyRoot(String),
    #[error("operation spans two different roots")]
    CrossRoot,
    #[error("path belongs to a different guard")]
    ForeignPath,
    #[error("could not move {path} to the Recycle Bin: {message}")]
    Trash { path: PathBuf, message: String },
    #[error("{op} failed for {path}: {source}")]
    Io {
        op: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl FsError {
    pub(crate) fn io(op: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            op,
            path: path.into(),
            source,
        }
    }

    /// True when the underlying I/O error was "not found".
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Io { source, .. } if source.kind() == io::ErrorKind::NotFound)
    }
}
