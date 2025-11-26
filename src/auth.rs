use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    pub client_id: Option<String>,
    pub tenant_id: Option<String>,
}

fn get_app_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not find config directory")?;
    let app_dir = config_dir.join(crate::config::APP_DIR_NAME);
    fs::create_dir_all(&app_dir)?;
    Ok(app_dir)
}

fn load_config() -> Option<Config> {
    let app_dir = get_app_dir().ok()?;
    let config_path = app_dir.join("config.json");
    
    if !config_path.exists() {
        return None;
    }
    
    let json = fs::read_to_string(config_path).ok()?;
    serde_json::from_str(&json).ok()
}

fn get_client_id() -> String {
    // 1. Try env var
    dotenv::dotenv().ok();
    if let Ok(id) = std::env::var("CLIENT_ID") {
        return id;
    }

    // 2. Try config file
    if let Some(config) = load_config() {
        if let Some(id) = config.client_id {
            return id;
        }
    }

    // 3. Fallback
    eprintln!("Warning: CLIENT_ID not found in environment or config, using default fallback.");
    "d3590ed6-52b3-4102-aeff-aad2292ab01c".to_string()
}

const TENANT: &str = "common";
const SCOPES: &str = "User.Read Chat.ReadWrite offline_access";

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenErrorResponse {
    error: String,
}

fn get_token_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not find config directory")?;
    let app_dir = config_dir.join(crate::config::APP_DIR_NAME);
    fs::create_dir_all(&app_dir)?;
    Ok(app_dir.join("token.json"))
}

fn save_token(token: &TokenResponse) -> Result<()> {
    let path = get_token_path()?;
    let json = serde_json::to_string_pretty(token)?;
    fs::write(path, json)?;
    Ok(())
}

fn load_token() -> Result<Option<TokenResponse>> {
    let path = get_token_path()?;
    if !path.exists() {
        return Ok(None);
    }
    
    let json = fs::read_to_string(path)?;
    let mut token: TokenResponse = serde_json::from_str(&json)?;
    
    // Set expires_at based on current time if not set
    if token.expires_at == 0 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        token.expires_at = now + token.expires_in;
    }
    
    Ok(Some(token))
}

pub async fn start_device_flow() -> Result<DeviceCodeResponse> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/devicecode",
        TENANT
    );

    let client_id = get_client_id();
    let params = [
        ("client_id", client_id.as_str()),
        ("scope", SCOPES),
    ];

    let response = client
        .post(&url)
        .form(&params)
        .send()
        .await?;

    // Check if the request was successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        anyhow::bail!("Failed to start device flow ({}): {}", status, error_text);
    }

    // Try to parse as DeviceCodeResponse, but show the actual response if it fails
    let response_text = response.text().await?;
    match serde_json::from_str::<DeviceCodeResponse>(&response_text) {
        Ok(device_code) => Ok(device_code),
        Err(e) => {
            eprintln!("Failed to parse response. Raw response:");
            eprintln!("{}", response_text);
            anyhow::bail!("Failed to parse device code response: {}", e)
        }
    }
}

pub async fn poll_for_token(device_code: &str, interval: u64) -> Result<TokenResponse> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
        TENANT
    );

    let client_id = get_client_id();

    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;

        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", client_id.as_str()),
            ("device_code", device_code),
        ];

        let response = client
            .post(&url)
            .form(&params)
            .send()
            .await?;

        if response.status().is_success() {
            let mut token = response.json::<TokenResponse>().await?;
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            token.expires_at = now + token.expires_in;
            save_token(&token)?;
            return Ok(token);
        } else {
            let error = response.json::<TokenErrorResponse>().await?;
            if error.error == "authorization_pending" {
                // Continue polling
                continue;
            } else if error.error == "authorization_declined" {
                anyhow::bail!("User declined authorization");
            } else if error.error == "expired_token" {
                anyhow::bail!("Device code expired");
            } else {
                anyhow::bail!("Error: {}", error.error);
            }
        }
    }
}

pub async fn get_valid_token_silent() -> Result<String> {
    // Try to load existing token
    if let Some(token) = load_token()? {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        
        // If token is still valid (with 5 min buffer), return it
        if token.expires_at > now + 300 {
            return Ok(token.access_token);
        }
        
        // Try to refresh if we have a refresh token
        if let Some(refresh_token) = token.refresh_token {
            if let Ok(new_token) = refresh_access_token(&refresh_token).await {
                return Ok(new_token.access_token);
            }
        }
    }
    anyhow::bail!("No valid token found and refresh failed")
}

pub async fn get_access_token() -> Result<String> {
    // Try to get silent token first
    if let Ok(token) = get_valid_token_silent().await {
        return Ok(token);
    }

    // Need to do full device flow
    let device_code_response = start_device_flow().await?;
    println!("\n{}", device_code_response.message);
    println!("\nWaiting for authentication...\n");
    
    let token = poll_for_token(&device_code_response.device_code, device_code_response.interval).await?;
    Ok(token.access_token)
}

async fn refresh_access_token(refresh_token: &str) -> Result<TokenResponse> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
        TENANT
    );

    let client_id = get_client_id();
    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", client_id.as_str()),
        ("refresh_token", refresh_token),
        ("scope", SCOPES),
    ];

    let response = client
        .post(&url)
        .form(&params)
        .send()
        .await?;

    if response.status().is_success() {
        let mut token = response.json::<TokenResponse>().await?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        token.expires_at = now + token.expires_in;
        save_token(&token)?;
        Ok(token)
    } else {
        anyhow::bail!("Failed to refresh token")
    }
}
