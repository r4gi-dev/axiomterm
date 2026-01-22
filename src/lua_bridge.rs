use mlua::{Lua, Result, Value, Table};
use crate::types::Action;
use std::path::Path;

const MAX_MACRO_ACTIONS: usize = 100;

pub struct LuaEngine {
    lua: Lua,
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

        Self { lua }
    }

    pub fn load_config(&self, path: &Path) -> Result<()> {
        if path.exists() {
            let code = std::fs::read_to_string(path).map_err(mlua::Error::external)?;
            self.lua.load(&code).exec()?;
        }
        Ok(())
    }

    pub fn resolve_macro(&self, name: &str) -> Vec<Action> {
        let globals = self.lua.globals();
        
        if let Ok(axiom) = globals.get::<Table>("axiom") {
            if let Ok(macros) = axiom.get::<Table>("macros") {
                if let Ok(macro_val) = macros.get::<Value>(name) {
                    if let Value::Function(macro_func) = macro_val {
                        if let Ok(result_val) = macro_func.call::<Value>(()) {
                            if let Value::Table(result_table) = result_val {
                                return self.parse_action_table(result_table);
                            }
                        }
                    }
                }
            }
        }
        
        println!("DEBUG: Macro '{}' not found or failed to execute", name);
        Vec::new()
    }

    fn parse_action_table(&self, table: Table) -> Vec<Action> {
        let mut actions = Vec::new();
        // pairs() infers K, V
        for pair in table.pairs::<Value, Value>() {
            if let Ok((_k, v)) = pair {
                if let Value::String(s) = v {
                    if let Ok(s_str) = s.to_str() {
                        if let Some(action) = Action::from_str(&s_str) {
                            if actions.len() >= MAX_MACRO_ACTIONS {
                                println!("WARNING: Macro exceeded MAX_MACRO_ACTIONS ({}), truncating", MAX_MACRO_ACTIONS);
                                break;
                            }
                            actions.push(action);
                        }
                    }
                }
            }
        }
        actions
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

        let actions = engine.resolve_macro("test_macro");
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0], Action::AppendChar('A'));
        assert_eq!(actions[1], Action::Submit);
    }
}
