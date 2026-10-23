// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::RETRY_AFTER;
use tokio::time::sleep;
use url::Url;

use crate::allowlist::{Allowlist, redact};
use crate::error::NetError;
use crate::limiter::{Limiter, RateLimit};

#[derive(Debug, Clone)]
pub struct NetConfig {
    pub allowlist: Allowlist,
    pub rate_limit: RateLimit,
    pub user_agent: String,
    /// Cap for [`Client::get`] bodies (API responses). Downloads have their own cap.
    pub max_body_bytes: u64,
    /// Retries for 429, 5xx, connection errors and timeouts.
    pub max_retries: u32,
    /// Ceiling for any single wait between retries, including server-sent `Retry-After`.
    pub max_backoff: Duration,
    /// Whole-request timeout for [`Client::get`].
    pub request_timeout: Duration,
}

impl NetConfig {
    pub fn production() -> Self {
        Self {
            allowlist: Allowlist::production(),
            rate_limit: RateLimit::default(),
            user_agent: format!(
                "GMM/{} (+https://github.com/clorece/gacha-mod-manager)",
                env!("CARGO_PKG_VERSION")
            ),
            max_body_bytes: 8 * 1024 * 1024,
            max_retries: 3,
            max_backoff: Duration::from_secs(60),
            request_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Cheap to clone; clones share the connection pool and the rate limiter.
#[derive(Debug, Clone)]
pub struct Client {
    pub(crate) http: reqwest::Client,
    pub(crate) config: Arc<NetConfig>,
    pub(crate) limiter: Arc<Limiter>,
}

impl Client {
    pub fn new(config: NetConfig) -> Result<Self, NetError> {
        // reqwest is built without a bundled crypto provider; use ring. Installing twice is
        // harmless (the second call just reports the first one is already set).
        let _ = rustls::crypto::ring::default_provider().install_default();

        let allowlist = config.allowlist.clone();
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("too many redirects")
            } else if allowlist.check(attempt.url()).is_err() {
                attempt.error("redirect to a host that is not on the allowlist")
            } else {
                attempt.follow()
            }
        });
        let http = reqwest::Client::builder()
            .user_agent(config.user_agent.clone())
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .redirect(policy)
            .build()
            .map_err(|e| NetError::Setup(e.to_string()))?;
        Ok(Self {
            http,
            limiter: Arc::new(Limiter::new(config.rate_limit)),
            config: Arc::new(config),
        })
    }

    pub fn config(&self) -> &NetConfig {
        &self.config
    }

    /// GET for API calls: allowlisted, rate-limited, retried on 429/5xx/connection errors,
    /// size-capped. Other statuses (including 4xx) are returned for the caller to interpret.
    pub async fn get(&self, url: &Url) -> Result<HttpResponse, NetError> {
        self.config.allowlist.check(url)?;
        let mut attempt = 0;
        loop {
            self.limiter.acquire().await;
            let sent = self
                .http
                .get(url.clone())
                .timeout(self.config.request_timeout)
                .send()
                .await;
            match sent {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let retryable = status == 429 || status >= 500;
                    if retryable && attempt < self.config.max_retries {
                        let wait = retry_after(&resp).unwrap_or_else(|| backoff(attempt));
                        attempt += 1;
                        sleep(wait.min(self.config.max_backoff)).await;
                        continue;
                    }
                    if retryable {
                        return Err(NetError::Status {
                            url: redact(url),
                            status,
                        });
                    }
                    let body = read_capped(resp, url, self.config.max_body_bytes).await?;
                    return Ok(HttpResponse { status, body });
                }
                Err(e)
                    if attempt < self.config.max_retries && (e.is_connect() || e.is_timeout()) =>
                {
                    sleep(backoff(attempt).min(self.config.max_backoff)).await;
                    attempt += 1;
                }
                Err(e) => return Err(request_error(url, e)),
            }
        }
    }
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_secs(1u64 << attempt.min(6))
}

fn retry_after(resp: &reqwest::Response) -> Option<Duration> {
    let value = resp.headers().get(RETRY_AFTER)?.to_str().ok()?;
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

async fn read_capped(
    mut resp: reqwest::Response,
    url: &Url,
    limit: u64,
) -> Result<Vec<u8>, NetError> {
    let too_large = || NetError::BodyTooLarge {
        url: redact(url),
        limit,
    };
    if resp.content_length().is_some_and(|len| len > limit) {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| request_error(url, e))? {
        if (body.len() + chunk.len()) as u64 > limit {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub(crate) fn request_error(url: &Url, e: reqwest::Error) -> NetError {
    if e.is_redirect() {
        return NetError::Disallowed(format!("redirect from {}", redact(url)));
    }
    // reqwest's Display hides the cause (DNS failure, TLS error, ...); append the chain.
    // `without_url` because URLs may carry query strings; we show a redacted one instead.
    let e = e.without_url();
    let mut message = e.to_string();
    let mut source = std::error::Error::source(&e);
    while let Some(inner) = source {
        message.push_str(": ");
        message.push_str(&inner.to_string());
        source = inner.source();
    }
    NetError::Request {
        url: redact(url),
        message,
    }
}
