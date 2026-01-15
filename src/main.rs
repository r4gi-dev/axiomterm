use eframe::egui;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

// --- Helper Functions ---

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
                         let val = s.token().to_string(); 
                         if val.len() >= 2 {
                             let unquoted = val[1..val.len()-1].to_string();
                             return Ok(unquoted);
                         }
                     }
                 }
            }
        }
    }
    
    Ok(String::new())
}

// --- App State & GUI ---

struct ShellState {
    prompt: String,
}

struct TerminalApp {
    input: String,
    history: Vec<String>,
    shell_state: Arc<Mutex<ShellState>>,
    command_tx: Sender<String>,
    output_rx: Receiver<String>,
}

impl TerminalApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (command_tx, command_rx) = unbounded::<String>();
        let (output_tx, output_rx) = unbounded::<String>();

        let state = Arc::new(Mutex::new(ShellState {
            prompt: "> ".to_string(),
        }));
        let thread_state = Arc::clone(&state);

        // Worker thread for command execution
        thread::spawn(move || {
            loop {
                let cmd_line = match command_rx.recv() {
                    Ok(line) => line,
                    Err(_) => break, // Channel closed
                };

                // Echo input
                let prompt = { thread_state.lock().unwrap().prompt.clone() };
                let _ = output_tx.send(format!("{}{}", prompt, cmd_line));

                let cmd_line = cmd_line.trim();
                if cmd_line.is_empty() {
                    continue;
                }

                let parts = tokenize_command(cmd_line);
                if parts.is_empty() {
                    continue;
                }

                let command = &parts[0];
                let args = &parts[1..];

                match command.as_str() {
                    "exit" => std::process::exit(0),
                    "cd" => {
                        let new_dir = args.get(0).map_or("/", |x| x.as_str());
                        let root = std::path::Path::new(new_dir);
                        if let Err(e) = env::set_current_dir(&root) {
                            let _ = output_tx.send(format!("Error: {}", e));
                        }
                    }
                    "echo" => {
                        let output = args.join(" ");
                        let _ = output_tx.send(output);
                    }
                    "config" => {
                         if args.len() >= 2 && args[0] == "load" {
                            let path = &args[1];
                            match parse_config(path) {
                                Ok(new_prompt) => {
                                    if !new_prompt.is_empty() {
                                        thread_state.lock().unwrap().prompt = new_prompt;
                                        let _ = output_tx.send("Config loaded. Prompt updated.".to_string());
                                    } else {
                                        let _ = output_tx.send("Config loaded, but no 'gemini_prompt' found.".to_string());
                                    }
                                }
                                Err(e) => {
                                    let _ = output_tx.send(format!("Failed to load config: {}", e));
                                }
                            }
                        } else {
                            let _ = output_tx.send("Usage: config load <path>".to_string());
                        }
                    }
                    command_name => {
                        match Command::new(command_name)
                            .args(args)
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped()) // Merge stderr? Or separate? 
                            .spawn() 
                        {
                            Ok(mut child) => {
                                // Stream stdout
                                if let Some(stdout) = child.stdout.take() {
                                    let out_tx = output_tx.clone();
                                    thread::spawn(move || {
                                        let reader = BufReader::new(stdout);
                                        for line in reader.lines() {
                                            if let Ok(l) = line {
                                                let _ = out_tx.send(l);
                                            }
                                        }
                                    });
                                }
                                
                                // Stream stderr
                                if let Some(stderr) = child.stderr.take() {
                                     let out_tx = output_tx.clone();
                                    thread::spawn(move || {
                                        let reader = BufReader::new(stderr);
                                        for line in reader.lines() {
                                            if let Ok(l) = line {
                                                let _ = out_tx.send(l); // Maybe prefix with error color?
                                            }
                                        }
                                    });
                                }

                                let _ = child.wait(); // Wait for finish
                            }
                            Err(_) => {
                                let _ = output_tx.send("program not found".to_string());
                            }
                        }
                    }
                }
            }
        });

        Self {
            input: String::new(),
            history: Vec::new(),
            shell_state: state,
            command_tx,
            output_rx,
        }
    }
}

impl eframe::App for TerminalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for new output
        while let Ok(msg) = self.output_rx.try_recv() {
            self.history.push(msg);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_black_alpha(255)))
            .show(ctx, |ui| {
                ui.style_mut().visuals.extreme_bg_color = egui::Color32::BLACK;
                ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::BLACK;
                
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        // History
                        for line in &self.history {
                            ui.label(egui::RichText::new(line).monospace().color(egui::Color32::LIGHT_GRAY));
                        }

                        // Current Prompt/Input Line
                        ui.horizontal(|ui| {
                            let prompt_text = { self.shell_state.lock().unwrap().prompt.clone() };
                            ui.label(egui::RichText::new(prompt_text).monospace().color(egui::Color32::GREEN).strong());
                            
                            let re = ui.add(egui::TextEdit::singleline(&mut self.input)
                                .desired_width(ui.available_width())
                                .font(egui::TextStyle::Monospace)
                                .frame(false)
                                .text_color(egui::Color32::WHITE)
                                .lock_focus(true));
                            
                            // Auto-focus the input
                            re.request_focus();

                            // Submit when Enter is pressed
                            if re.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let cmd = std::mem::take(&mut self.input);
                                let _ = self.command_tx.send(cmd);
                                // Request focus immediately for the next prompt
                                re.request_focus();
                            }
                        });
                    });
            });

        // Repaint constantly to see updates immediately? Or use request_repaint only when needed?
        // For a terminal, output can come anytime.
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Gemini Terminal"),
        ..Default::default()
    };
    
    eframe::run_native(
        "Gemini Terminal",
        options,
        Box::new(|cc| Ok(Box::new(TerminalApp::new(cc)))),
    )
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
