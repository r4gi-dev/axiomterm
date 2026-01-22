use crate::types::{ConfigUpdate, Shortcut};
use crate::utils::parse_hex_color;
use std::path::Path;

pub fn parse_config(path: &Path) -> Result<ConfigUpdate, Box<dyn std::error::Error>> {
    let code = std::fs::read_to_string(path)?;
    let ast = match full_moon::parse(&code) {
        Ok(ast) => ast,
        Err(e) => {
            let msg = e.into_iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ");
            return Err(format!("Parse error: {}", msg).into());
        }
    };

    let mut update = ConfigUpdate::default();

    for stmt in ast.nodes().stmts() {
        if let full_moon::ast::Stmt::Assignment(assign) = stmt {
            for (var, expr) in assign.variables().iter().zip(assign.expressions().iter()) {
                 let var_str = var.to_string();
                 let var_name = if var_str.contains('.') {
                     var_str.split('.').last().unwrap_or("").trim()
                 } else {
                     var_str.trim()
                 };
                 
                 match var_name {
                     "axiomterm_prompt" | "prompt" => {
                        if let Some(val) = extract_string(expr) { update.prompt = Some(val); }
                     },
                     "axiomterm_prompt_color" | "prompt_color" => {
                        if let Some(val) = extract_string(expr) { update.prompt_color = parse_hex_color(&val); }
                     },
                     "axiomterm_text_color" | "text_color" => {
                        if let Some(val) = extract_string(expr) { update.text_color = parse_hex_color(&val); }
                     },
                     "axiomterm_window_title" | "window_title" => {
                        if let Some(val) = extract_string(expr) { update.window_title = Some(val); }
                     },
                     "window_background_opacity" => {
                        if let Some(val) = extract_float(expr) { update.opacity = Some(val); }
                     },
                     "font_size" => {
                        if let Some(val) = extract_float(expr) { update.font_size = Some(val); }
                     },
                     "default_cwd" => {
                        if let Some(val) = extract_string(expr) { update.default_cwd = Some(val); }
                     },
                     "directory_color" => {
                        if let Some(val) = extract_string(expr) { update.directory_color = parse_hex_color(&val); }
                     },
                     "axiomterm_shortcuts" | "keys" => {
                         if let full_moon::ast::Expression::TableConstructor(table) = expr {
                             let mut shortcuts = Vec::new();
                             for field in table.fields() {
                                 if let full_moon::ast::Field::NoKey(expr) = field {
                                     if let full_moon::ast::Expression::TableConstructor(inner) = expr {
                                         let mut key = String::new();
                                         let mut cmd = String::new();
                                         for inner_field in inner.fields() {
                                             let field_str = inner_field.to_string();
                                             if field_str.contains('=') {
                                                 let parts: Vec<&str> = field_str.splitn(2, '=').collect();
                                                 let name_part = parts[0].trim();
                                                 let value_part = parts[1].trim();
                                                 if name_part == "key" {
                                                     key = value_part.trim_matches(|c| c == '"' || c == '\'').to_string();
                                                 } else if name_part == "cmd" || name_part == "action" {
                                                     cmd = value_part.trim_matches(|c| c == '"' || c == '\'').to_string();
                                                 }
                                             }
                                         }
                                         if !key.is_empty() && !cmd.is_empty() {
                                             shortcuts.push(Shortcut { key, cmd });
                                         }
                                     }
                                 }
                             }
                             update.shortcuts = Some(shortcuts);
                         }
                     },
                     "axiomterm_modes" | "modes" => {
                         if let full_moon::ast::Expression::TableConstructor(table) = expr {
                             let mut mode_definitions = Vec::new();
                             for field in table.fields() {
                                 // Iterate through each mode definition block
                                 // e.g. { name = "Normal", bindings = { ... } }
                                 if let full_moon::ast::Field::NoKey(expr) = field {
                                     if let full_moon::ast::Expression::TableConstructor(inner) = expr {
                                         let mut mode_name = String::new();
                                         let mut bindings = Vec::new();
                                         
                                         // Parse fields of the mode definition
                                         for inner_field in inner.fields() {
                                            // Handle bindings table: bindings = { ... }
                                            if let full_moon::ast::Field::NameKey { key, value, .. } = inner_field {
                                                let key_name = key.token().to_string().trim().to_string();
                                                
                                                if key_name == "bindings" || key_name == "keys" {
                                                    if let full_moon::ast::Expression::TableConstructor(b_table) = value {
                                                        for b_field in b_table.fields() {
                                                            // Each binding: { key = "...", action = "..." }
                                                            if let full_moon::ast::Field::NoKey(b_expr) = b_field {
                                                                if let full_moon::ast::Expression::TableConstructor(b_inner) = b_expr {
                                                                    let mut key = String::new();
                                                                    let mut action_str = String::new();
                                                                    for bi_field in b_inner.fields() {
                                                                        let bi_str = bi_field.to_string();
                                                                        if bi_str.contains('=') {
                                                                            let bi_parts: Vec<&str> = bi_str.splitn(2, '=').collect();
                                                                            let bik = bi_parts[0].trim();
                                                                            let biv = bi_parts[1].trim().trim_matches(|c| c == '"' || c == '\'' || c == ',' || c == ' ');
                                                                            if bik == "key" { key = biv.to_string(); }
                                                                            else if bik == "action" { action_str = biv.to_string(); }
                                                                        }
                                                                    }
                                                                    if !key.is_empty() && !action_str.is_empty() {
                                                                        if let Some(action) = crate::types::Action::from_str(&action_str) {
                                                                            bindings.push(crate::types::KeyBinding {
                                                                                event: crate::types::InputEvent::Key { code: key, ctrl: false, alt: false, shift: false },
                                                                                action,
                                                                            });
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            
                                            // Handle simple key-value pairs like name = "Normal" (fallback logic)
                                            let field_str = inner_field.to_string();
                                            if field_str.contains('=') {
                                                let parts: Vec<&str> = field_str.splitn(2, '=').collect();
                                                let k = parts[0].trim();
                                                let v = parts[1].trim().trim_matches(|c| c == '"' || c == '\'' || c == ',' || c == ' ');
                                                if k == "name" || k == "mode" {
                                                    mode_name = v.to_string();
                                                }
                                            }
                                         }
                                         
                                         if !mode_name.is_empty() {
                                             if let Some(m) = crate::types::TerminalMode::from_str(&mode_name) {
                                                 mode_definitions.push(crate::types::ModeDefinition { mode: m, bindings });
                                             }
                                         }
                                     }
                                 }
                             }
                             update.mode_definitions = Some(mode_definitions);
                         }
                     },
                     _ => {}
                 }
            }
        }
    }
    
    Ok(update)
}

fn extract_string(expr: &full_moon::ast::Expression) -> Option<String> {
    if let full_moon::ast::Expression::String(s) = expr {
        let val = s.token().to_string();
        if val.len() >= 2 {
            return Some(val[1..val.len()-1].to_string());
        }
    }
    None
}

fn extract_float(expr: &full_moon::ast::Expression) -> Option<f32> {
    if let full_moon::ast::Expression::Number(n) = expr {
        return n.token().to_string().parse::<f32>().ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Action, InputEvent, TerminalMode};

    #[test]
    fn test_mode_parsing() {
        let config = r#"
            config = {}
            config.modes = {
                {
                    name = "TestMode",
                    bindings = {
                        { key = "i", action = "ChangeMode(Insert)" },
                        { key = "Escape", action = "Clear" }
                    }
                }
            }
            return config
        "#;

        // Use a temporary file for testing parse_config behavior
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_config_modes.lua");
        std::fs::write(&temp_file, config).unwrap();

        let update = parse_config(&temp_file).unwrap();
        // Clean up
        let _ = std::fs::remove_file(&temp_file);

        assert!(update.mode_definitions.is_some());
        let modes = update.mode_definitions.unwrap();
        assert_eq!(modes.len(), 1);
        
        let def = &modes[0];
        assert_eq!(def.mode, TerminalMode::Custom("TestMode".to_string()));
        assert_eq!(def.bindings.len(), 2);

        // Check bindings
        let has_insert = def.bindings.iter().any(|b| 
            matches!(b.action, Action::ChangeMode(TerminalMode::Insert)) && 
            matches!(&b.event, InputEvent::Key { code, .. } if code == "i")
        );
        assert!(has_insert);

        let has_clear = def.bindings.iter().any(|b| 
            matches!(b.action, Action::Clear) && 
            matches!(&b.event, InputEvent::Key { code, .. } if code == "Escape")
        );
        assert!(has_clear);
    }
}
