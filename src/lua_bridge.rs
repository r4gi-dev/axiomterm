use mlua::{Lua, Result, Value, Table};
use crate::types::Action;
use std::path::Path;
use std::fmt;

const MAX_MACRO_ACTIONS: usize = 100;

#[derive(Debug, Clone)]
pub enum MacroError {
    NotFound(String),
    InvalidReturnType(String),
    ActionParseError { macro_name: String, value: String },
    ActionLimitExceeded { macro_name: String, limit: usize },
}

impl fmt::Display for MacroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MacroError::NotFound(name) => write!(f, "Macro '{}' is not defined", name),
            MacroError::InvalidReturnType(name) => write!(f, "Macro '{}' must return a list of Actions", name),
            MacroError::ActionParseError { macro_name, value } => {
                write!(f, "Invalid action '{}' in macro '{}'", value, macro_name)
            },
            MacroError::ActionLimitExceeded { macro_name, limit } => {
                write!(f, "Macro '{}' exceeded max actions ({})", macro_name, limit)
            },
        }
    }
}

impl std::error::Error for MacroError {}

#[derive(Debug, Clone)]
pub struct MacroInvocation {
    pub macro_name: String,
    pub total_invocations: usize,
    pub total_actions_emitted: usize,
    pub max_actions_emitted: usize,
    pub last_error: Option<MacroError>,
}

#[derive(Debug, Default)]
pub struct MacroMetrics {
    invocations: std::collections::HashMap<String, MacroInvocation>,
}

impl MacroMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn record_success(&mut self, macro_name: &str, action_count: usize) {
        let entry = self.invocations.entry(macro_name.to_string())
            .or_insert_with(|| MacroInvocation {
                macro_name: macro_name.to_string(),
                total_invocations: 0,
                total_actions_emitted: 0,
                max_actions_emitted: 0,
                last_error: None,
            });
        
        entry.total_invocations += 1;
        entry.total_actions_emitted += action_count;
        entry.max_actions_emitted = entry.max_actions_emitted.max(action_count);
        entry.last_error = None;
    }

    pub(crate) fn record_error(&mut self, macro_name: &str, error: MacroError) {
        let entry = self.invocations.entry(macro_name.to_string())
            .or_insert_with(|| MacroInvocation {
                macro_name: macro_name.to_string(),
                total_invocations: 0,
                total_actions_emitted: 0,
                max_actions_emitted: 0,
                last_error: None,
            });
        
        entry.total_invocations += 1;
        entry.last_error = Some(error);
    }

    /// Get snapshot of all macro invocations
    pub fn snapshot(&self) -> Vec<MacroInvocation> {
        self.invocations.values().cloned().collect()
    }

    /// Get snapshot of specific macro
    pub fn get(&self, macro_name: &str) -> Option<MacroInvocation> {
        self.invocations.get(macro_name).cloned()
    }
}

pub struct LuaEngine {
    lua: Lua,
    pub(crate) metrics: std::sync::Mutex<MacroMetrics>,
}

impl LuaEngine {
    pub fn new() -> Self {
        let lua = Lua::new();
        // Initialize axiom global table
        // We strictly control what is available.
        let globals = lua.globals();
        let axiom = lua.create_table().unwrap();
        let macros = lua.create_table().unwrap();
        
        let _ = axiom.set("macros", macros);
        let _ = globals.set("axiom", axiom);

        Self { 
            lua,
            metrics: std::sync::Mutex::new(MacroMetrics::new()),
        }
    }

    pub fn load_config(&self, path: &Path) -> Result<()> {
        if path.exists() {
            let code = std::fs::read_to_string(path).map_err(mlua::Error::external)?;
            self.lua.load(&code).exec()?;
        }
        Ok(())
    }

    pub fn resolve_macro(&self, name: &str) -> std::result::Result<Vec<Action>, MacroError> {
        let result = self.resolve_macro_internal(name);
        
        // Observation hook: record metrics without affecting execution
        match &result {
            Ok(actions) => {
                if let Ok(mut metrics) = self.metrics.lock() {
                    metrics.record_success(name, actions.len());
                }
            },
            Err(e) => {
                if let Ok(mut metrics) = self.metrics.lock() {
                    metrics.record_error(name, e.clone());
                }
            }
        }
        
        result
    }

    fn resolve_macro_internal(&self, name: &str) -> std::result::Result<Vec<Action>, MacroError> {
        let globals = self.lua.globals();
        
        let axiom = globals.get::<Table>("axiom")
            .map_err(|_| MacroError::NotFound(name.to_string()))?;
        
        let macros = axiom.get::<Table>("macros")
            .map_err(|_| MacroError::NotFound(name.to_string()))?;
        
        let macro_val = macros.get::<Value>(name)
            .map_err(|_| MacroError::NotFound(name.to_string()))?;
        
        let macro_func = match macro_val {
            Value::Function(f) => f,
            _ => return Err(MacroError::NotFound(name.to_string())),
        };
        
        let result_val = macro_func.call::<Value>(())
            .map_err(|_| MacroError::InvalidReturnType(name.to_string()))?;
        
        let result_table = match result_val {
            Value::Table(t) => t,
            _ => return Err(MacroError::InvalidReturnType(name.to_string())),
        };
        
        self.parse_action_table(name, result_table)
    }

    fn parse_action_table(&self, macro_name: &str, table: Table) -> std::result::Result<Vec<Action>, MacroError> {
        let mut actions = Vec::new();
        
        for pair in table.pairs::<Value, Value>() {
            if let Ok((_k, v)) = pair {
                if let Value::String(s) = v {
                    if let Ok(s_str) = s.to_str() {
                        if actions.len() >= MAX_MACRO_ACTIONS {
                            return Err(MacroError::ActionLimitExceeded {
                                macro_name: macro_name.to_string(),
                                limit: MAX_MACRO_ACTIONS,
                            });
                        }
                        
                        match Action::from_str(&s_str) {
                            Some(action) => actions.push(action),
                            None => {
                                return Err(MacroError::ActionParseError {
                                    macro_name: macro_name.to_string(),
                                    value: s_str.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Ok(actions)
    }

    /// List all defined macro names
    pub fn list_macros(&self) -> Vec<String> {
        let mut macro_names = Vec::new();
        
        if let Ok(globals) = self.lua.globals().get::<Table>("axiom") {
            if let Ok(macros) = globals.get::<Table>("macros") {
                for pair in macros.pairs::<Value, Value>() {
                    if let Ok((key, _)) = pair {
                        if let Value::String(s) = key {
                            if let Ok(name) = s.to_str() {
                                macro_names.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        
        macro_names
    }

    /// Validate a macro without executing it
    pub fn validate_macro(&self, name: &str) -> std::result::Result<(), MacroError> {
        let globals = self.lua.globals();
        
        let axiom = globals.get::<Table>("axiom")
            .map_err(|_| MacroError::NotFound(name.to_string()))?;
        
        let macros = axiom.get::<Table>("macros")
            .map_err(|_| MacroError::NotFound(name.to_string()))?;
        
        let macro_val = macros.get::<Value>(name)
            .map_err(|_| MacroError::NotFound(name.to_string()))?;
        
        match macro_val {
            Value::Function(_) => Ok(()),
            _ => Err(MacroError::NotFound(name.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_resolution() {
        let engine = LuaEngine::new();
        let lua = &engine.lua;
        
        // Define a macro manually in Lua environment
        let script = r#"
            axiom.macros.test_macro = function()
                return { "InsertChar(A)", "Submit" }
            end
        "#;
        lua.load(script).exec().expect("Failed to define macro");

        let actions = engine.resolve_macro("test_macro").expect("Macro resolution failed");
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0], Action::AppendChar('A'));
        assert_eq!(actions[1], Action::Submit);
    }

    #[test]
    fn test_macro_not_found() {
        let engine = LuaEngine::new();
        let result = engine.resolve_macro("nonexistent");
        assert!(result.is_err());
        match result {
            Err(MacroError::NotFound(name)) => assert_eq!(name, "nonexistent"),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_list_macros() {
        let engine = LuaEngine::new();
        let lua = &engine.lua;
        
        let script = r#"
            axiom.macros.macro1 = function() return {} end
            axiom.macros.macro2 = function() return {} end
        "#;
        lua.load(script).exec().expect("Failed to define macros");

        let macros = engine.list_macros();
        assert_eq!(macros.len(), 2);
        assert!(macros.contains(&"macro1".to_string()));
        assert!(macros.contains(&"macro2".to_string()));
    }

    #[test]
    fn test_validate_macro() {
        let engine = LuaEngine::new();
        let lua = &engine.lua;
        
        let script = r#"
            axiom.macros.valid_macro = function() return {} end
        "#;
        lua.load(script).exec().expect("Failed to define macro");

        assert!(engine.validate_macro("valid_macro").is_ok());
        assert!(engine.validate_macro("invalid_macro").is_err());
    }

    #[test]
    fn test_macro_metrics() {
        let engine = LuaEngine::new();
        let lua = &engine.lua;
        
        let script = r#"
            axiom.macros.test_macro = function()
                return { "Submit", "Clear" }
            end
        "#;
        lua.load(script).exec().expect("Failed to define macro");

        // Execute macro twice
        let _ = engine.resolve_macro("test_macro");
        let _ = engine.resolve_macro("test_macro");

        // Check metrics
        let metrics = engine.metrics.lock().unwrap();
        let invocation = metrics.get("test_macro").expect("Metrics not recorded");
        
        assert_eq!(invocation.total_invocations, 2);
        assert_eq!(invocation.total_actions_emitted, 4); // 2 actions * 2 invocations
        assert_eq!(invocation.max_actions_emitted, 2);
        assert!(invocation.last_error.is_none());
    }

    #[test]
    fn test_macro_metrics_error() {
        let engine = LuaEngine::new();
        
        // Try to resolve non-existent macro
        let _ = engine.resolve_macro("nonexistent");

        // Check error was recorded
        let metrics = engine.metrics.lock().unwrap();
        let invocation = metrics.get("nonexistent").expect("Error not recorded");
        
        assert_eq!(invocation.total_invocations, 1);
        assert_eq!(invocation.total_actions_emitted, 0);
        assert!(invocation.last_error.is_some());
    }
}
