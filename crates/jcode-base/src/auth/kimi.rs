use anyhow::{Context, Result, bail};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

use crate::provider_catalog::{
    ApiKeyCredentialSource, KIMI_PROFILE, load_api_key, load_env_value_from_config_file,
    save_env_value_to_env_file,
};

pub const CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";
pub const AUTH_HOST: &str = "https://auth.kimi.com";
pub const DEVICE_AUTHORIZATION_ENDPOINT: &str =
    "https://auth.kimi.com/api/oauth/device_authorization";
pub const TOKEN_ENDPOINT: &str = "https://auth.kimi.com/api/oauth/token";
pub const AUTH_MODE_ENV: &str = "JCODE_KIMI_AUTH_MODE";
const PROTOCOL_PLATFORM: &str = "kimi_cli";
const REFRESH_THRESHOLD_MS: i64 = 5 * 60 * 1000;
const REFRESH_KEY: &str = "kimi";

static REFRESH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KimiAuthMode {
    OAuth,
    ApiKey,
}

impl KimiAuthMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::OAuth => "oauth",
            Self::ApiKey => "api_key",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KimiTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_ms: i64,
    #[serde(default)]
    pub scope: String,
    #[serde(default = "default_token_type")]
    pub token_type: String,
    #[serde(default)]
    pub expires_in: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceAuthorization {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: Option<u64>,
    #[serde(default = "default_poll_interval")]
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: f64,
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

fn api_key_source() -> ApiKeyCredentialSource {
    ApiKeyCredentialSource::from_catalog_profile(KIMI_PROFILE)
}

pub fn tokens_path() -> Result<PathBuf> {
    Ok(jcode_storage::app_config_dir()?
        .join("kimi")
        .join("credentials.json"))
}

fn device_id_path() -> Result<PathBuf> {
    Ok(jcode_storage::app_config_dir()?
        .join("kimi")
        .join("device_id"))
}

fn new_device_id() -> String {
    let mut bytes: [u8; 16] = rand::random();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn sanitize_header(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .chars()
        .map(|ch| {
            if ch.is_ascii_graphic() || ch == ' ' {
                ch
            } else {
                '?'
            }
        })
        .collect()
}

fn device_id() -> Result<String> {
    let path = device_id_path()?;
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim();
        if !existing.is_empty() {
            return Ok(existing.to_string());
        }
    }
    let id = new_device_id();
    jcode_storage::write_text_secret(&path, &id)?;
    Ok(id)
}

fn device_name() -> String {
    sanitize_header(
        std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "unknown".to_string()),
    )
}

fn os_version() -> String {
    #[cfg(target_os = "linux")]
    if let Ok(value) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
        let value = value.trim();
        if !value.is_empty() {
            return sanitize_header(value);
        }
    }
    sanitize_header(std::env::consts::OS)
}

fn device_model() -> String {
    sanitize_header(format!(
        "{} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ))
}

pub fn user_agent() -> String {
    format!("Jcode/{}", env!("CARGO_PKG_VERSION"))
}

pub fn apply_identity_headers(request: RequestBuilder) -> Result<RequestBuilder> {
    Ok(request
        .header(reqwest::header::USER_AGENT, user_agent())
        // Kimi's OAuth service currently requires this protocol-family value.
        // Jcode identifies itself separately and truthfully via User-Agent and version.
        .header("X-Msh-Platform", PROTOCOL_PLATFORM)
        .header("X-Msh-Version", env!("CARGO_PKG_VERSION"))
        .header("X-Msh-Device-Name", device_name())
        .header("X-Msh-Device-Model", device_model())
        .header("X-Msh-Os-Version", os_version())
        .header("X-Msh-Device-Id", device_id()?))
}

fn http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build Kimi OAuth HTTP client")
}

pub fn load_tokens() -> Result<Option<KimiTokens>> {
    let path = tokens_path()?;
    if !path.exists() {
        return Ok(None);
    }
    jcode_storage::read_json(&path)
        .with_context(|| {
            format!(
                "failed to read Kimi OAuth credentials from {}",
                path.display()
            )
        })
        .map(Some)
}

pub fn save_tokens(tokens: &KimiTokens) -> Result<()> {
    jcode_storage::write_json_secret(&tokens_path()?, tokens)
        .context("failed to save Kimi OAuth credentials")
}

pub fn clear_tokens() -> Result<()> {
    let path = tokens_path()?;
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

pub fn has_oauth_tokens() -> bool {
    load_tokens()
        .ok()
        .flatten()
        .is_some_and(|tokens| !tokens.access_token.is_empty() && !tokens.refresh_token.is_empty())
}

pub fn has_api_key() -> bool {
    load_api_key(&api_key_source()).is_some()
}

pub fn configured_auth_mode() -> Option<KimiAuthMode> {
    let value = std::env::var(AUTH_MODE_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| load_env_value_from_config_file(AUTH_MODE_ENV, KIMI_PROFILE.env_file));
    match value.as_deref().map(str::trim) {
        Some("oauth") => Some(KimiAuthMode::OAuth),
        Some("api_key") => Some(KimiAuthMode::ApiKey),
        _ => None,
    }
}

pub fn selected_auth_mode() -> Option<KimiAuthMode> {
    configured_auth_mode().or_else(|| {
        if has_oauth_tokens() {
            Some(KimiAuthMode::OAuth)
        } else if has_api_key() {
            Some(KimiAuthMode::ApiKey)
        } else {
            None
        }
    })
}

pub fn is_configured() -> bool {
    match selected_auth_mode() {
        Some(KimiAuthMode::OAuth) => has_oauth_tokens(),
        Some(KimiAuthMode::ApiKey) => has_api_key(),
        None => false,
    }
}

pub fn set_auth_mode(mode: Option<KimiAuthMode>) -> Result<()> {
    save_env_value_to_env_file(
        AUTH_MODE_ENV,
        KIMI_PROFILE.env_file,
        mode.map(KimiAuthMode::as_str),
    )
}

pub async fn request_device_authorization() -> Result<DeviceAuthorization> {
    let client = http_client()?;
    let request = client
        .post(DEVICE_AUTHORIZATION_ENDPOINT)
        .form(&[("client_id", CLIENT_ID)]);
    let response = apply_identity_headers(request)?.send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        bail!(
            "Kimi device authorization failed (HTTP {}): {}",
            status,
            body
        );
    }
    serde_json::from_str(&body).context("Kimi returned an invalid device authorization response")
}

fn token_from_response(
    response: TokenResponse,
    previous_refresh_token: Option<&str>,
) -> Result<KimiTokens> {
    let refresh_token = response
        .refresh_token
        .filter(|value| !value.is_empty())
        .or_else(|| previous_refresh_token.map(str::to_string))
        .context("Kimi token response did not contain a refresh token")?;
    Ok(KimiTokens {
        access_token: response.access_token,
        refresh_token,
        expires_at_ms: now_ms() + (response.expires_in.max(0.0) * 1000.0) as i64,
        scope: response.scope.unwrap_or_default(),
        token_type: response.token_type.unwrap_or_else(default_token_type),
        expires_in: response.expires_in,
    })
}

pub async fn poll_for_tokens(auth: &DeviceAuthorization) -> Result<KimiTokens> {
    let client = http_client()?;
    let interval = Duration::from_secs(auth.interval.max(1));
    let deadline = SystemTime::now() + Duration::from_secs(auth.expires_in.unwrap_or(600));

    loop {
        if SystemTime::now() >= deadline {
            bail!("Kimi device authorization expired before login completed");
        }
        let request = client.post(TOKEN_ENDPOINT).form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", auth.device_code.as_str()),
            ("client_id", CLIENT_ID),
        ]);
        let response = apply_identity_headers(request)?.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if status.is_success() {
            let parsed: TokenResponse = serde_json::from_str(&body)
                .context("Kimi returned an invalid OAuth token response")?;
            return token_from_response(parsed, None);
        }

        let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
        let error = payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown_error");
        if error == "expired_token" {
            bail!("Kimi device authorization expired before login completed");
        }
        if error != "authorization_pending" && error != "slow_down" {
            let description = payload
                .get("error_description")
                .and_then(Value::as_str)
                .unwrap_or(&body);
            bail!(
                "Kimi OAuth token request failed (HTTP {}): {}",
                status,
                description
            );
        }
        tokio::time::sleep(interval).await;
    }
}

async fn refresh_once(client: &Client, refresh_token: &str) -> Result<KimiTokens> {
    let request = client.post(TOKEN_ENDPOINT).form(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLIENT_ID),
    ]);
    let response = apply_identity_headers(request)?.send().await?;
    let status = response.status();
    let body = response.text().await?;
    if status.is_success() {
        let parsed: TokenResponse =
            serde_json::from_str(&body).context("Kimi returned an invalid refresh response")?;
        return token_from_response(parsed, Some(refresh_token));
    }
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        bail!("Kimi rejected the refresh token; run `/login kimi` again");
    }
    bail!("Kimi token refresh failed (HTTP {}): {}", status, body)
}

async fn refresh_tokens(current: &KimiTokens) -> Result<KimiTokens> {
    let client = http_client()?;
    let mut delay = Duration::from_secs(1);
    let mut last_error = None;
    for attempt in 0..3 {
        match refresh_once(&client, &current.refresh_token).await {
            Ok(tokens) => return Ok(tokens),
            Err(error) => {
                let permanent = error.to_string().contains("run `/login kimi` again");
                if permanent {
                    return Err(error);
                }
                last_error = Some(error);
                if attempt < 2 {
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Kimi token refresh failed")))
}

pub async fn access_token() -> Result<String> {
    let current = load_tokens()?.context("Kimi OAuth is not configured; run `/login kimi`")?;
    if current.expires_at_ms - now_ms() > REFRESH_THRESHOLD_MS {
        return Ok(current.access_token);
    }

    let lock = REFRESH_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().await;
    let current = load_tokens()?.context("Kimi OAuth is not configured; run `/login kimi`")?;
    if current.expires_at_ms - now_ms() > REFRESH_THRESHOLD_MS {
        return Ok(current.access_token);
    }

    match refresh_tokens(&current).await {
        Ok(tokens) => {
            save_tokens(&tokens)?;
            let _ = crate::auth::refresh_state::record_success(REFRESH_KEY);
            Ok(tokens.access_token)
        }
        Err(error) => {
            let _ = crate::auth::refresh_state::record_failure(REFRESH_KEY, error.to_string());
            Err(error)
        }
    }
}

/// Refresh after a request is rejected with the access token it used.
///
/// The rejected token is compared again after taking the process-wide refresh
/// lock. If another request already rotated the token, reuse that newer token
/// instead of issuing a second refresh request.
pub async fn refresh_after_unauthorized(rejected_access_token: &str) -> Result<String> {
    let lock = REFRESH_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().await;
    let current = load_tokens()?.context("Kimi OAuth is not configured; run `/login kimi`")?;
    if current.access_token != rejected_access_token {
        return Ok(current.access_token);
    }

    match refresh_tokens(&current).await {
        Ok(tokens) => {
            save_tokens(&tokens)?;
            let _ = crate::auth::refresh_state::record_success(REFRESH_KEY);
            Ok(tokens.access_token)
        }
        Err(error) => {
            let _ = crate::auth::refresh_state::record_failure(REFRESH_KEY, error.to_string());
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let previous = std::env::var_os(key);
            crate::env::set_var(key, value);
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            crate::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                crate::env::set_var(self.key, previous);
            } else {
                crate::env::remove_var(self.key);
            }
        }
    }

    fn test_tokens(access_token: &str) -> KimiTokens {
        KimiTokens {
            access_token: access_token.to_string(),
            refresh_token: "refresh".to_string(),
            expires_at_ms: now_ms() + 3_600_000,
            scope: "openid".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600.0,
        }
    }

    #[test]
    fn token_response_preserves_rotated_refresh_token_and_expiry() {
        let before = now_ms();
        let token = token_from_response(
            TokenResponse {
                access_token: "access".to_string(),
                refresh_token: Some("rotated".to_string()),
                expires_in: 60.0,
                scope: Some("openid".to_string()),
                token_type: Some("Bearer".to_string()),
            },
            Some("old"),
        )
        .expect("token");
        assert_eq!(token.refresh_token, "rotated");
        assert!(token.expires_at_ms >= before + 60_000);
    }

    #[test]
    fn refresh_response_can_reuse_existing_refresh_token() {
        let token = token_from_response(
            TokenResponse {
                access_token: "access".to_string(),
                refresh_token: None,
                expires_in: 60.0,
                scope: None,
                token_type: None,
            },
            Some("old"),
        )
        .expect("token");
        assert_eq!(token.refresh_token, "old");
        assert_eq!(token.token_type, "Bearer");
    }

    #[test]
    fn jcode_identifies_itself_in_user_agent() {
        assert!(user_agent().starts_with("Jcode/"));
        assert!(!user_agent().contains("KimiCLI"));
    }

    #[test]
    fn oauth_tokens_make_kimi_configured_without_an_api_key() {
        let _env_lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _home = EnvVarGuard::set("JCODE_HOME", temp.path());
        let _api_key = EnvVarGuard::remove(KIMI_PROFILE.api_key_env);
        let _mode = EnvVarGuard::remove(AUTH_MODE_ENV);

        save_tokens(&test_tokens("oauth-access")).expect("save OAuth tokens");

        assert_eq!(selected_auth_mode(), Some(KimiAuthMode::OAuth));
        assert!(is_configured());
        assert!(crate::provider_catalog::openai_compatible_profile_is_configured(KIMI_PROFILE));
    }

    #[test]
    fn explicit_api_key_mode_wins_when_both_credentials_exist() {
        let _env_lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _home = EnvVarGuard::set("JCODE_HOME", temp.path());
        let _api_key = EnvVarGuard::set(KIMI_PROFILE.api_key_env, "sk-kimi-test");
        let _mode = EnvVarGuard::set(AUTH_MODE_ENV, "api_key");
        save_tokens(&test_tokens("oauth-access")).expect("save OAuth tokens");

        assert_eq!(selected_auth_mode(), Some(KimiAuthMode::ApiKey));
        assert!(is_configured());
    }

    #[tokio::test]
    async fn unauthorized_refresh_reuses_token_rotated_by_another_request() {
        let _env_lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _home = EnvVarGuard::set("JCODE_HOME", temp.path());
        save_tokens(&test_tokens("new-access")).expect("save OAuth tokens");

        let token = refresh_after_unauthorized("rejected-access")
            .await
            .expect("reuse rotated token");

        assert_eq!(token, "new-access");
    }

    #[test]
    fn identity_headers_include_protocol_and_truthful_jcode_user_agent() {
        let _env_lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _home = EnvVarGuard::set("JCODE_HOME", temp.path());
        let request = apply_identity_headers(Client::new().get("https://example.test"))
            .expect("headers")
            .build()
            .expect("request");

        assert_eq!(
            request.headers().get("X-Msh-Platform").unwrap(),
            PROTOCOL_PLATFORM
        );
        assert!(
            request
                .headers()
                .get(reqwest::header::USER_AGENT)
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("Jcode/")
        );
        assert!(request.headers().contains_key("X-Msh-Device-Id"));
    }
}
