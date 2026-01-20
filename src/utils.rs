use crate::types::TerminalColor;
use std::env;
use std::path::PathBuf;

pub fn get_default_config_path() -> Option<PathBuf> {
    // Try environment variables first for explicit control
    let base = if let Ok(profile) = env::var("USERPROFILE") {
        Some(PathBuf::from(profile).join(".config"))
    } else if let Ok(home) = env::var("HOME") {
        Some(PathBuf::from(home).join(".config"))
    } else if let Some(config_dir) = dirs::config_dir() {
        Some(config_dir)
    } else {
        dirs::home_dir().map(|h| h.join(".config"))
    };

    base.map(|mut p| {
        p.push("axiomterm");
        p.push("config.lua");
        p
    })
}

pub fn tokenize_command(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape = false;
    let mut token_started = false;

    for c in input.chars() {
        if escape {
            current_token.push(c);
            escape = false;
            token_started = true;
        } else if in_single_quote {
            if c == '\'' {
                in_single_quote = false;
            } else {
                current_token.push(c);
            }
            token_started = true;
        } else if in_double_quote {
            if c == '"' {
                in_double_quote = false;
            } else if c == '\\' {
                escape = true;
            } else {
                current_token.push(c);
            }
            token_started = true;
        } else {
            match c {
                '\'' => {
                    in_single_quote = true;
                    token_started = true;
                }
                '"' => {
                    in_double_quote = true;
                    token_started = true;
                }
                '\\' => {
                    escape = true;
                    token_started = true;
                }
                c if c.is_whitespace() => {
                    if token_started {
                        tokens.push(current_token);
                        current_token = String::new();
                        token_started = false;
                    }
                }
                _ => {
                    current_token.push(c);
                    token_started = true;
                }
            }
        }
    }

    if token_started {
        tokens.push(current_token);
    }

    tokens
}

pub fn parse_hex_color(hex: &str) -> Option<TerminalColor> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(TerminalColor::from_rgb(r, g, b))
}
