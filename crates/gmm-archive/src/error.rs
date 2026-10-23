// SPDX-License-Identifier: MIT OR Apache-2.0

use gmm_fsguard::{FsError, PathError};

/// Why an archive was rejected. Entry names are untrusted, so they are always shown with
/// `{:?}` to escape control characters.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error(transparent)]
    Fs(#[from] FsError),
    #[error("not a supported archive (zip or 7z)")]
    UnknownFormat,
    #[error("{0} archives are not supported yet")]
    UnsupportedFormat(&'static str),
    #[error("archive is damaged or unreadable: {0}")]
    Corrupt(String),
    #[error("archive is password-protected")]
    Encrypted,
    #[error("unsafe entry name {name:?}: {reason}")]
    UnsafePath { name: String, reason: PathError },
    #[error("entry with an empty name")]
    EmptyName,
    #[error("entry {0:?} is a symbolic link")]
    Symlink(String),
    #[error("entry {0:?} is not a regular file or directory")]
    SpecialFile(String),
    #[error(
        "entry {0:?} collides with another entry (names differ only in case, or file vs folder)"
    )]
    Collision(String),
    #[error("archive has more than {limit} entries")]
    TooManyEntries { limit: usize },
    #[error("entry {name:?} is nested deeper than {limit} folders")]
    TooDeep { name: String, limit: usize },
    #[error("entry {name:?} has a path longer than {limit} characters")]
    PathTooLong { name: String, limit: usize },
    #[error("archive expands to more than {limit} bytes")]
    TooLarge { limit: u64 },
    #[error("entry {name:?} expands far more than its compressed size (possible zip bomb)")]
    Bomb { name: String },
    #[error("destination already exists")]
    DestinationExists,
    #[error("writing {name:?} failed: {source}")]
    Write {
        name: String,
        #[source]
        source: std::io::Error,
    },
}
