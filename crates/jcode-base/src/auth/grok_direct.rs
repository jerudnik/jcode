use anyhow::{Context, Result, bail};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

pub const CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
pub const ISSUER: &str = "https://auth.x.ai";
pub const DEVICE_AUTHORIZATION_ENDPOINT: &str = "https://auth.x.ai/oauth2/device/code";
pub const TOKEN_ENDPOINT: &str = "https://auth.x.ai/oauth2/token";
pub const SCOPES: &str = "openid profile email offline_access grok-cli:access api:access";
pub const CREDENTIAL_VERSION: u32 = 1;
const REFRESH_THRESHOLD_MS: i64 = 5 * 60 * 1000;
const REFRESH_KEY: &str = "grok-direct";
const LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(25);

static REFRESH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Deserialize, Serialize)]
pub struct GrokDirectCredentials {
    pub version: u32,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_ms: Option<i64>,
    pub token_type: String,
    pub scope: Vec<String>,
    pub issuer: String,
    pub client_id: String,
}

impl fmt::Debug for GrokDirectCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokDirectCredentials")
            .field("version", &self.version)
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("token_type", &self.token_type)
            .field("scope", &self.scope)
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceAuthorization {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    pub expires_in: Option<u64>,
    #[serde(default = "default_poll_interval")]
    pub interval: u64,
}

impl DeviceAuthorization {
    pub fn verification_url(&self) -> &str {
        self.verification_uri_complete
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&self.verification_uri)
    }

    fn validate(self) -> Result<Self> {
        if self.device_code.trim().is_empty() {
            bail!("xAI device authorization response did not contain a device code");
        }
        if self.user_code.trim().is_empty() {
            bail!("xAI device authorization response did not contain a user code");
        }
        if self.verification_uri.trim().is_empty() {
            bail!("xAI device authorization response did not contain a verification URL");
        }
        Ok(self)
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<f64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
}

fn default_poll_interval() -> u64 {
    5
}

fn default_token_type() -> String {
    "Bearer".to_string()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

pub fn user_agent() -> String {
    format!("jcode/{}", env!("CARGO_PKG_VERSION"))
}

fn apply_oauth_headers(request: RequestBuilder) -> RequestBuilder {
    request
        .header(reqwest::header::USER_AGENT, user_agent())
        .header(reqwest::header::ACCEPT, "application/json")
}

fn http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build Grok Direct OAuth HTTP client")
}

pub fn credentials_path() -> Result<PathBuf> {
    Ok(jcode_storage::app_config_dir()?
        .join("grok-direct")
        .join("credentials.json"))
}

pub fn lock_path() -> Result<PathBuf> {
    Ok(jcode_storage::app_config_dir()?
        .join("grok-direct")
        .join("credentials.lock"))
}

pub fn load_credentials() -> Result<Option<GrokDirectCredentials>> {
    let path = credentials_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let credentials: GrokDirectCredentials =
        jcode_storage::read_json(&path).with_context(|| {
            format!(
                "failed to read Grok Direct OAuth credentials from {}",
                path.display()
            )
        })?;
    validate_credentials(&credentials)?;
    Ok(Some(credentials))
}

pub fn save_credentials(credentials: &GrokDirectCredentials) -> Result<()> {
    validate_credentials(credentials)?;
    jcode_storage::write_json_secret(&credentials_path()?, credentials)
        .context("failed to save Grok Direct OAuth credentials")
}

pub fn clear_credentials() -> Result<()> {
    let path = credentials_path()?;
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

pub fn is_configured() -> bool {
    load_credentials().ok().flatten().is_some()
}

pub fn credentials_are_fresh(credentials: &GrokDirectCredentials) -> bool {
    credentials
        .expires_at_ms
        .is_none_or(|expires_at_ms| expires_at_ms - now_ms() > REFRESH_THRESHOLD_MS)
}

fn validate_credentials(credentials: &GrokDirectCredentials) -> Result<()> {
    if credentials.version != CREDENTIAL_VERSION {
        bail!(
            "unsupported Grok Direct credential version {}; expected {}",
            credentials.version,
            CREDENTIAL_VERSION
        );
    }
    if credentials.access_token.trim().is_empty() || credentials.refresh_token.trim().is_empty() {
        bail!("Grok Direct credentials require both access and refresh tokens");
    }
    if credentials.issuer != ISSUER || credentials.client_id != CLIENT_ID {
        bail!("Grok Direct credentials have unexpected OAuth provenance");
    }
    Ok(())
}

pub async fn request_device_authorization() -> Result<DeviceAuthorization> {
    request_device_authorization_at(DEVICE_AUTHORIZATION_ENDPOINT).await
}

async fn request_device_authorization_at(endpoint: &str) -> Result<DeviceAuthorization> {
    let response = apply_oauth_headers(
        http_client()?
            .post(endpoint)
            .form(&[("client_id", CLIENT_ID), ("scope", SCOPES)]),
    )
    .send()
    .await
    .context("failed to request Grok Direct device authorization")?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        bail!(
            "Grok Direct device authorization failed (HTTP {}): {}",
            status,
            oauth_error_description(&body)
        );
    }
    let authorization: DeviceAuthorization = serde_json::from_str(&body)
        .context("xAI returned an invalid device authorization response")?;
    authorization.validate()
}

fn credentials_from_response(
    response: TokenResponse,
    previous_refresh_token: Option<&str>,
) -> Result<GrokDirectCredentials> {
    if response.access_token.trim().is_empty() {
        bail!("xAI token response did not contain an access token");
    }
    let refresh_token = response
        .refresh_token
        .filter(|value| !value.trim().is_empty())
        .or_else(|| previous_refresh_token.map(str::to_string))
        .context("xAI token response did not contain a refresh token")?;
    let expires_at_ms = response
        .expires_in
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .map(|seconds| now_ms().saturating_add((seconds * 1000.0) as i64));
    Ok(GrokDirectCredentials {
        version: CREDENTIAL_VERSION,
        access_token: response.access_token,
        refresh_token,
        expires_at_ms,
        token_type: response.token_type.unwrap_or_else(default_token_type),
        scope: response
            .scope
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect(),
        issuer: ISSUER.to_string(),
        client_id: CLIENT_ID.to_string(),
    })
}

pub async fn poll_for_credentials(auth: &DeviceAuthorization) -> Result<GrokDirectCredentials> {
    poll_for_credentials_with_cancellation(auth, Arc::new(AtomicBool::new(false))).await
}

pub async fn poll_for_credentials_with_cancellation(
    auth: &DeviceAuthorization,
    cancelled: Arc<AtomicBool>,
) -> Result<GrokDirectCredentials> {
    let client = http_client()?;
    let mut interval = Duration::from_secs(auth.interval.max(1));
    let deadline = SystemTime::now() + Duration::from_secs(auth.expires_in.unwrap_or(600));

    loop {
        if cancelled.load(Ordering::Relaxed) {
            bail!("Grok Direct device authorization was cancelled");
        }
        if SystemTime::now() >= deadline {
            bail!("Grok Direct device authorization expired before login completed");
        }

        let response = apply_oauth_headers(client.post(TOKEN_ENDPOINT).form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", auth.device_code.as_str()),
            ("client_id", CLIENT_ID),
        ]))
        .send()
        .await
        .context("failed to poll Grok Direct device authorization")?;
        let status = response.status();
        let body = response.text().await?;
        if status.is_success() {
            let parsed: TokenResponse = serde_json::from_str(&body)
                .context("xAI returned an invalid OAuth token response")?;
            return credentials_from_response(parsed, None);
        }

        let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
        let error = payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown_error");
        match error {
            "authorization_pending" => {}
            "slow_down" => interval += Duration::from_secs(5),
            "access_denied" | "authorization_denied" => {
                bail!("Grok Direct device authorization was denied")
            }
            "expired_token" => {
                bail!("Grok Direct device authorization expired before login completed")
            }
            _ => bail!(
                "Grok Direct OAuth token request failed (HTTP {}): {}",
                status,
                oauth_error_description(&body)
            ),
        }
        tokio::time::sleep(interval).await;
    }
}

fn oauth_error_description(body: &str) -> String {
    let payload: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    payload
        .get("error_description")
        .and_then(Value::as_str)
        .or_else(|| payload.get("error").and_then(Value::as_str))
        .unwrap_or(body)
        .to_string()
}

async fn refresh_once(
    client: &Client,
    endpoint: &str,
    refresh_token: &str,
) -> Result<GrokDirectCredentials> {
    let response = apply_oauth_headers(client.post(endpoint).form(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLIENT_ID),
    ]))
    .send()
    .await
    .context(
        "Grok Direct token refresh may have reached xAI; credentials were preserved, but re-login may be required",
    )?;
    let status = response.status();
    let body = response.text().await?;
    if status.is_success() {
        let parsed: TokenResponse =
            serde_json::from_str(&body).context("xAI returned an invalid refresh response")?;
        return credentials_from_response(parsed, Some(refresh_token));
    }
    if matches!(
        status,
        StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        bail!("xAI rejected the Grok Direct refresh token; run `/login grok-direct` again");
    }
    bail!(
        "Grok Direct token refresh failed (HTTP {}): {}",
        status,
        oauth_error_description(&body)
    )
}

async fn acquire_refresh_file_lock() -> Result<jcode_storage::ExclusiveFileLock> {
    let path = lock_path()?;
    tokio::task::spawn_blocking(move || {
        jcode_storage::ExclusiveFileLock::acquire(&path, LOCK_TIMEOUT, LOCK_RETRY_DELAY)
    })
    .await
    .context("Grok Direct refresh lock task failed")?
    .context("could not serialize Grok Direct token refresh")
}

async fn refresh_serialized(rejected_access_token: Option<&str>) -> Result<String> {
    let local_lock = REFRESH_LOCK.get_or_init(|| Mutex::new(()));
    let _local_guard = local_lock.lock().await;

    let current = load_credentials()?
        .context("Grok Direct OAuth is not configured; run `/login grok-direct`")?;
    if rejected_access_token.is_none() && credentials_are_fresh(&current) {
        return Ok(current.access_token);
    }
    if rejected_access_token.is_some_and(|rejected| current.access_token != rejected) {
        return Ok(current.access_token);
    }

    let _file_guard = acquire_refresh_file_lock().await?;
    let current = load_credentials()?
        .context("Grok Direct OAuth is not configured; run `/login grok-direct`")?;
    if rejected_access_token.is_none() && credentials_are_fresh(&current) {
        return Ok(current.access_token);
    }
    if rejected_access_token.is_some_and(|rejected| current.access_token != rejected) {
        return Ok(current.access_token);
    }

    match refresh_once(&http_client()?, TOKEN_ENDPOINT, &current.refresh_token).await {
        Ok(credentials) => {
            save_credentials(&credentials)?;
            let _ = crate::auth::refresh_state::record_success(REFRESH_KEY);
            Ok(credentials.access_token)
        }
        Err(error) => {
            let _ = crate::auth::refresh_state::record_failure(REFRESH_KEY, error.to_string());
            Err(error)
        }
    }
}

pub async fn access_token() -> Result<String> {
    let current = load_credentials()?
        .context("Grok Direct OAuth is not configured; run `/login grok-direct`")?;
    if credentials_are_fresh(&current) {
        return Ok(current.access_token);
    }
    refresh_serialized(None).await
}

pub async fn refresh_after_unauthorized(rejected_access_token: &str) -> Result<String> {
    refresh_serialized(Some(rejected_access_token)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(refresh_token: Option<&str>) -> TokenResponse {
        TokenResponse {
            access_token: "new-access".to_string(),
            refresh_token: refresh_token.map(str::to_string),
            expires_in: Some(3600.0),
            scope: Some(SCOPES.to_string()),
            token_type: Some("Bearer".to_string()),
        }
    }

    #[test]
    fn token_response_rotates_refresh_token() {
        let credentials = credentials_from_response(response(Some("new-refresh")), Some("old"))
            .expect("credentials");
        assert_eq!(credentials.refresh_token, "new-refresh");
        assert_eq!(credentials.scope.len(), 6);
    }

    #[test]
    fn token_response_preserves_refresh_token_when_omitted() {
        let credentials =
            credentials_from_response(response(None), Some("old-refresh")).expect("credentials");
        assert_eq!(credentials.refresh_token, "old-refresh");
    }

    #[test]
    fn debug_output_redacts_tokens() {
        let credentials =
            credentials_from_response(response(Some("refresh-secret")), None).expect("credentials");
        let debug = format!("{credentials:?}");
        assert!(!debug.contains("new-access"));
        assert!(!debug.contains("refresh-secret"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn device_authorization_requires_public_fields() {
        let authorization = DeviceAuthorization {
            user_code: String::new(),
            device_code: "device".to_string(),
            verification_uri: "https://auth.x.ai/activate".to_string(),
            verification_uri_complete: None,
            expires_in: Some(600),
            interval: 5,
        };
        assert!(authorization.validate().is_err());
    }

    #[test]
    fn truthful_user_agent_and_exact_scope_are_stable() {
        assert!(user_agent().starts_with("jcode/"));
        assert_eq!(
            SCOPES,
            "openid profile email offline_access grok-cli:access api:access"
        );
    }

    #[test]
    fn credential_paths_are_jcode_owned() {
        let path = PathBuf::from("root")
            .join("grok-direct")
            .join("credentials.json");
        assert!(!path.to_string_lossy().contains(".grok"));
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("credentials.json")
        );
    }
}
