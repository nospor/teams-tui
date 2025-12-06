pub const APP_DIR_NAME: &str = "teams-tui";

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::app::NotificationMode;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub client_id: Option<String>,
    pub notification_mode: Option<NotificationMode>,
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

pub fn save_config(config: &Config) -> Result<()> {
    let app_dir = get_app_dir()?;
    let config_path = app_dir.join("config.json");
    let json = serde_json::to_string_pretty(config)?;
    fs::write(config_path, json)?;
    Ok(())
}
