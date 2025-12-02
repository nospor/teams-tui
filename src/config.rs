use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};

pub const APP_DIR_NAME: &str = "teams-tui";

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub client_id: Option<String>,
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub load_images: bool,
}

pub fn get_app_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not find config directory")?;
    let app_dir = config_dir.join(APP_DIR_NAME);
    fs::create_dir_all(&app_dir)?;
    Ok(app_dir)
}

pub fn load_config() -> Option<Config> {
    let app_dir = get_app_dir().ok()?;
    let config_path = app_dir.join("config.json");
    
    if !config_path.exists() {
        return None;
    }
    
    let json = fs::read_to_string(config_path).ok()?;
    serde_json::from_str(&json).ok()
}
