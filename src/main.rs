use std::env;
use std::io::{self, Write};
use std::process::Command;
use std::sync::{Arc, Mutex};

struct ShellState {
    prompt: String,
}

fn tokenize_command(input: &str) -> Vec<String> {
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

fn parse_config(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let code = std::fs::read_to_string(path)?;
    let ast = full_moon::parse(&code).map_err(|e| {
        let msg = e.into_iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ");
        format!("Parse error: {}", msg)
    })?;

    for stmt in ast.nodes().stmts() {
        if let full_moon::ast::Stmt::Assignment(assign) = stmt {
            for (var, expr) in assign.variables().iter().zip(assign.expressions().iter()) {
                 if var.to_string().trim() == "gemini_prompt" {
                     if let full_moon::ast::Expression::String(s) = expr {
                         // Remove quotes from the string token: "value" -> value
                         let val = s.token().to_string(); 
                         if val.len() >= 2 {
                             // Basic unquoting
                             // Note: full_moon returns the raw token text including quotes
                             let unquoted = val[1..val.len()-1].to_string();
                             return Ok(unquoted);
                         }
                     }
                 }
            }
        }
    }
    
    // Return empty if not found, let caller decide default
    Ok(String::new())
}

fn main() {
    let state = Arc::new(Mutex::new(ShellState {
        prompt: "> ".to_string(),
    }));

    let state_clone = Arc::clone(&state);
    ctrlc::set_handler(move || {
        // Just print a newline and maybe the prompt again, but we can't easily access stdout cleanly without lock
        // For simplicity, just print a newline. 
        println!(); 
        print!("\r{}", state_clone.lock().unwrap().prompt);
        let _ = io::stdout().flush();
    }).expect("Error setting Ctrl-C handler");

    loop {
        {
            let s = state.lock().unwrap();
            print!("{}", s.prompt);
        }
        io::stdout().flush().expect("Failed to flush stdout");

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {}
            Err(_) => {
                // This might happen on interrupt, just continue
                continue;
            }
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let parts = tokenize_command(input);
        if parts.is_empty() {
            continue;
        }

        let command = &parts[0];
        let args = &parts[1..];

        match command.as_str() {
            "exit" => break,
            "cd" => {
                let new_dir = args.get(0).map_or("/", |x| x.as_str());
                let root = std::path::Path::new(new_dir);
                if let Err(e) = env::set_current_dir(&root) {
                    eprintln!("{}", e);
                }
            }
            "echo" => {
                let output = args.join(" ");
                println!("{}", output);
            }
            "config" => {
                if args.len() >= 2 && args[0] == "load" {
                    let path = &args[1];
                    match parse_config(path) {
                        Ok(new_prompt) => {
                            if !new_prompt.is_empty() {
                                state.lock().unwrap().prompt = new_prompt;
                                println!("Config loaded. Prompt updated.");
                            } else {
                                println!("Config loaded, but no 'gemini_prompt' found.");
                            }
                        }
                        Err(e) => eprintln!("Failed to load config: {}", e),
                    }
                } else {
                    eprintln!("Usage: config load <path>");
                }
            }
            command_name => {
                let child = Command::new(command_name)
                    .args(args)
                    .spawn();

                match child {
                    Ok(mut child) => {
                        child.wait().expect("Failed to wait on child");
                    }
                    Err(e) => {
                        eprintln!("program not found");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        let input = "ls -la";
        let tokens = tokenize_command(input);
        assert_eq!(tokens, vec!["ls", "-la"]);
    }

    #[test]
    fn test_double_quotes() {
        let input = "echo \"hello world\"";
        let tokens = tokenize_command(input);
        assert_eq!(tokens, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_single_quotes() {
        let input = "echo 'hello world'";
        let tokens = tokenize_command(input);
        assert_eq!(tokens, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_escapes() {
        let input = "echo hello\\ world";
        let tokens = tokenize_command(input);
        assert_eq!(tokens, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_mixed_quotes() {
        let input = "echo \"foo 'bar'\"";
        let tokens = tokenize_command(input);
        assert_eq!(tokens, vec!["echo", "foo 'bar'"]);
    }

    #[test]
    fn test_empty_quotes() {
        let input = "echo \"\"";
        let tokens = tokenize_command(input);
        assert_eq!(tokens, vec!["echo", ""]);
    }
}
