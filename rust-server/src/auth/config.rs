use super::types::Config;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Get the path to the authentication configuration file
pub fn config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Cannot determine config directory")?;
    
    let auth_dir = config_dir.join("autoreply-mcp");
    Ok(auth_dir.join("config.json"))
}

/// Load the authentication configuration from disk
pub fn load_config() -> Result<Config> {
    let path = config_path()?;
    
    // If file doesn't exist, return default config
    if !path.exists() {
        return Ok(Config::default());
    }
    
    let data = fs::read_to_string(&path)
        .context("Failed to read config file")?;
    
    let config: Config = serde_json::from_str(&data)
        .context("Failed to parse config file")?;
    
    Ok(config)
}

/// Save the configuration to disk
pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;
    
    // Create directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .context("Failed to create config directory")?;
    }
    
    let data = serde_json::to_string_pretty(config)
        .context("Failed to serialize config")?;
    
    // Write with user-only permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(&path, data)
            .context("Failed to write config file")?;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }
    
    #[cfg(not(unix))]
    {
        fs::write(&path, data)
            .context("Failed to write config file")?;
    }
    
    Ok(())
}
