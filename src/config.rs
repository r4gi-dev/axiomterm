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
                     "gemini_prompt" | "prompt" => {
                        if let Some(val) = extract_string(expr) { update.prompt = Some(val); }
                     },
                     "gemini_prompt_color" | "prompt_color" => {
                        if let Some(val) = extract_string(expr) { update.prompt_color = parse_hex_color(&val); }
                     },
                     "gemini_text_color" | "text_color" => {
                        if let Some(val) = extract_string(expr) { update.text_color = parse_hex_color(&val); }
                     },
                     "gemini_window_title" | "window_title" => {
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
                     "gemini_shortcuts" | "keys" => {
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
                     "gemini_modes" | "modes" => {
                         // For now, skip complex mode parsing due to full_moon API complexity
                         // Users can still define modes via shortcuts for basic functionality
                         // TODO: Implement robust Lua mode parsing in future iteration
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
