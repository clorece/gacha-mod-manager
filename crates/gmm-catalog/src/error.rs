// SPDX-License-Identifier: MIT OR Apache-2.0

use gmm_net::NetError;

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error(transparent)]
    Net(#[from] NetError),
    #[error("not found on the catalogue")]
    NotFound,
    #[error("the catalogue rejected the request: {0}")]
    BadRequest(String),
    #[error("the catalogue answered HTTP {0}")]
    Http(u16),
    #[error("unexpected response for {what}: {message}")]
    Decode { what: String, message: String },
}
