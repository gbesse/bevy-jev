use crate::{JevRequest, JevResponse};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("gateway configuration is invalid: {0}")]
    Configuration(String),
    #[error("gateway request failed: {0}")]
    Transport(String),
    #[error("gateway returned an invalid response: {0}")]
    Response(String),
}

/// Implement this trait to connect the plugin to a gateway or deterministic test double.
pub trait JevProvider: Send + Sync + 'static {
    /// Runs one decision request.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration, transport, or response decoding fails.
    fn decide(&self, request: &JevRequest) -> Result<JevResponse, ProviderError>;
}

#[cfg(feature = "gateway")]
pub struct GatewayProvider {
    url: String,
    session_token: String,
    client: reqwest::blocking::Client,
}

#[cfg(feature = "gateway")]
impl GatewayProvider {
    /// Creates an HTTPS gateway provider with a short-lived session token.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Configuration`] when the URL is insecure or the token is empty.
    pub fn new(
        url: impl Into<String>,
        session_token: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let url = url.into();
        let is_loopback =
            url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:");
        if !url.starts_with("https://") && !is_loopback {
            return Err(ProviderError::Configuration(
                "HTTPS is required except for loopback development".into(),
            ));
        }
        let session_token = session_token.into();
        if session_token.trim().is_empty() {
            return Err(ProviderError::Configuration(
                "a short-lived gateway session token is required".into(),
            ));
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(35))
            .build()
            .map_err(|error| ProviderError::Configuration(error.to_string()))?;
        Ok(Self {
            url,
            session_token,
            client,
        })
    }
}

#[cfg(feature = "gateway")]
impl JevProvider for GatewayProvider {
    fn decide(&self, request: &JevRequest) -> Result<JevResponse, ProviderError> {
        let response = self
            .client
            .post(&self.url)
            .bearer_auth(&self.session_token)
            .json(request)
            .send()
            .map_err(|error| ProviderError::Transport(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::Transport(format!("HTTP {status}")));
        }
        response
            .json()
            .map_err(|error| ProviderError::Response(error.to_string()))
    }
}
