use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedConfig {
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub window: WindowConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_renderer")]
    pub renderer: String,
    #[serde(default = "default_initial_mode")]
    pub initial_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_false")]
    pub lua_allow_io: bool,
    #[serde(default = "default_false")]
    pub lua_allow_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_width")]
    pub initial_width: u32,
    #[serde(default = "default_height")]
    pub initial_height: u32,
    #[serde(default = "default_true")]
    pub transparent: bool,
}

// Default functions
fn default_backend() -> String { "std".to_string() }
fn default_renderer() -> String { "egui".to_string() }
fn default_initial_mode() -> String { "insert".to_string() }
fn default_false() -> bool { false }
fn default_true() -> bool { true }
fn default_width() -> u32 { 800 }
fn default_height() -> u32 { 600 }

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            renderer: default_renderer(),
            initial_mode: default_initial_mode(),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            lua_allow_io: default_false(),
            lua_allow_network: default_false(),
        }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            initial_width: default_width(),
            initial_height: default_height(),
            transparent: default_true(),
        }
    }
}

impl Default for FixedConfig {
    fn default() -> Self {
        Self {
            core: CoreConfig::default(),
            security: SecurityConfig::default(),
            window: WindowConfig::default(),
        }
    }
}

impl FixedConfig {
    /// Load FixedConfig from terminal.toml
    /// Search order:
    /// 1. ./terminal.toml (current directory)
    /// 2. ~/.config/terminal/terminal.toml (XDG config)
    /// 3. Default values
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Try current directory first
        let current_dir_path = PathBuf::from("./terminal.toml");
        if current_dir_path.exists() {
            return Self::load_from_path(&current_dir_path);
        }

        // Try XDG config directory
        if let Some(config_dir) = Self::get_config_dir() {
            let config_path = config_dir.join("terminal").join("terminal.toml");
            if config_path.exists() {
                return Self::load_from_path(&config_path);
            }
        }

        // Use defaults if no config file found
        Ok(Self::default())
    }

    fn load_from_path(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: FixedConfig = toml::from_str(&contents)?;
        Ok(config)
    }

    fn get_config_dir() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("APPDATA").ok().map(PathBuf::from)
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::env::var("XDG_CONFIG_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|h| PathBuf::from(h).join(".config"))
                })
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate backend
        match self.core.backend.as_str() {
            "std" => {},
            "wasm" => return Err("WASM backend not yet implemented".to_string()),
            "remote" => return Err("Remote backend not yet implemented".to_string()),
            other => return Err(format!("Unknown backend: {}", other)),
        }

        // Validate renderer
        match self.core.renderer.as_str() {
            "egui" => {},
            "headless" => return Err("Headless renderer not yet implemented".to_string()),
            other => return Err(format!("Unknown renderer: {}", other)),
        }

        // Validate initial mode
        match self.core.initial_mode.as_str() {
            "insert" | "normal" | "visual" => {},
            other => return Err(format!("Unknown initial mode: {}", other)),
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FixedConfig::default();
        assert_eq!(config.core.backend, "std");
        assert_eq!(config.core.renderer, "egui");
        assert_eq!(config.core.initial_mode, "insert");
        assert_eq!(config.security.lua_allow_io, false);
        assert_eq!(config.security.lua_allow_network, false);
        assert_eq!(config.window.initial_width, 800);
        assert_eq!(config.window.initial_height, 600);
        assert_eq!(config.window.transparent, true);
    }

    #[test]
    fn test_validate_valid_config() {
        let config = FixedConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_backend() {
        let mut config = FixedConfig::default();
        config.core.backend = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_toml_parsing() {
        let toml_str = r#"
[core]
backend = "std"
renderer = "egui"
initial_mode = "normal"

[security]
lua_allow_io = false
lua_allow_network = false

[window]
initial_width = 1024
initial_height = 768
transparent = false
"#;
        let config: FixedConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.core.backend, "std");
        assert_eq!(config.core.initial_mode, "normal");
        assert_eq!(config.window.initial_width, 1024);
        assert_eq!(config.window.transparent, false);
    }
}
