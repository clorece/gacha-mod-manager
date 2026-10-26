// SPDX-License-Identifier: MIT OR Apache-2.0

//! Errors as the webview sees them: a `kind` the UI can branch on, a sentence to show, and
//! for the errors that ask the user something, the data to ask with.

use std::sync::{Arc, OnceLock};

use gmm_core::{Conflict, CoreError, ErrorLog};
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CmdError {
    /// The archive holds several mods: show the variant picker, then install again with
    /// the choice.
    ChooseVariant {
        message: String,
        variants: Vec<String>,
    },
    /// The mod contains executables or scripts: list them, then install again with
    /// `skip_blocked` if the user agrees.
    Blocked { message: String, files: Vec<String> },
    /// Switching the mod on would conflict with mods that are already on: show them, then
    /// try again with `anyway` if the user agrees.
    Conflicts {
        message: String,
        conflicts: Vec<Conflict>,
    },
    /// The game isn't set up far enough (no XXMI, no importer, or not `Ready` for launching).
    NotReady { message: String },
    /// Anything else, as a sentence.
    Other { message: String },
}

pub type CmdResult<T> = Result<T, CmdError>;

/// The app's error log (Report a Bug), once `main` set it up. Tests don't, so their
/// refusals stay out of any log.
static LOG: OnceLock<Arc<ErrorLog>> = OnceLock::new();

pub fn install_log(log: Arc<ErrorLog>) {
    let _ = LOG.set(log);
}

/// Writes an error to the log (redacted there). Errors that ask the user something
/// (versions, blocked files, conflicts) and setup states aren't errors and aren't logged.
pub fn record(source: &str, message: &str) {
    if let Some(log) = LOG.get() {
        log.record(source, message);
    }
}

impl CmdError {
    pub fn other(message: impl Into<String>) -> Self {
        let message = message.into();
        record("app", &message);
        Self::Other { message }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::ChooseVariant { message, .. }
            | Self::Blocked { message, .. }
            | Self::Conflicts { message, .. }
            | Self::NotReady { message }
            | Self::Other { message } => message,
        }
    }
}

impl From<CoreError> for CmdError {
    fn from(e: CoreError) -> Self {
        let message = e.to_string();
        match e {
            CoreError::ChooseVariant(variants) => Self::ChooseVariant { message, variants },
            CoreError::Blocked(files) => Self::Blocked { message, files },
            CoreError::NotReady { .. } => Self::NotReady { message },
            _ => Self::other(message),
        }
    }
}

impl From<gmm_db::DbError> for CmdError {
    fn from(e: gmm_db::DbError) -> Self {
        CoreError::from(e).into()
    }
}

impl From<gmm_catalog::CatalogError> for CmdError {
    fn from(e: gmm_catalog::CatalogError) -> Self {
        CoreError::from(e).into()
    }
}
