// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;

use gmm_fsguard::FsError;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Fs(#[from] FsError),
    #[error("database error: {0}")]
    Sql(#[from] sqlx::Error),
    /// Also what an older GMM reports when the database was created by a newer one.
    #[error("could not update the database schema: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("path is not valid UTF-8, so it can't be stored: {0}")]
    NonUtf8Path(PathBuf),
    #[error("value out of range for {0}")]
    OutOfRange(&'static str),
}
