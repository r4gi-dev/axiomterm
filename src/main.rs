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

fn parse_hex_color(hex: &str) -> Option<egui::Color32> {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        if let Ok(r) = u8::from_str_radix(&hex[0..2], 16) {
            if let Ok(g) = u8::from_str_radix(&hex[2..4], 16) {
                if let Ok(b) = u8::from_str_radix(&hex[4..6], 16) {
                    return Some(egui::Color32::from_rgb(r, g, b));
                }
            }
        }
    }
    None
}

// --- Shortcuts & Modes ---

#[derive(Clone, Debug, PartialEq)]
enum TerminalMode {
    Insert,
    Normal,
}

#[derive(Clone, Debug)]
struct Shortcut {
    key: String,
    cmd: String,
}

#[derive(Default)]
struct ConfigUpdate {
    prompt: Option<String>,
    prompt_color: Option<egui::Color32>,
    text_color: Option<egui::Color32>,
    window_title: Option<String>,
    shortcuts: Option<Vec<Shortcut>>,
    opacity: Option<f32>,
    font_size: Option<f32>,
    default_cwd: Option<String>,
}

fn parse_config(path: &str) -> Result<ConfigUpdate, Box<dyn std::error::Error>> {
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
                     }
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

// --- App State & GUI ---

struct ShellState {
    prompt: String,
    prompt_color: egui::Color32,
    text_color: egui::Color32,
    window_title_base: String,
    window_title_full: String,
    title_updated: bool,
    mode: TerminalMode,
    shortcuts: Vec<Shortcut>,
    opacity: f32,
    font_size: f32,
    current_dir: String,
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

        let current_dir = env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        let state = Arc::new(Mutex::new(ShellState {
            prompt: "> ".to_string(),
            prompt_color: egui::Color32::GREEN,
            text_color: egui::Color32::LIGHT_GRAY,
            window_title_base: "Gemini Terminal".to_string(),
            window_title_full: "[INSERT] Gemini Terminal".to_string(),
            title_updated: false,
            mode: TerminalMode::Insert,
            shortcuts: Vec::new(),
            opacity: 1.0,
            font_size: 14.0,
            current_dir: current_dir.clone(),
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
                        } else if let Ok(cwd) = env::current_dir() {
                            let new_cwd_str = cwd.to_string_lossy().to_string();
                            thread_state.lock().unwrap().current_dir = new_cwd_str;
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
                                Ok(update) => {
                                    let mut s = thread_state.lock().unwrap();
                                    if let Some(p) = update.prompt { s.prompt = p; }
                                    if let Some(pc) = update.prompt_color { s.prompt_color = pc; }
                                    if let Some(tc) = update.text_color { s.text_color = tc; }
                                    if let Some(wt) = update.window_title { 
                                        s.window_title_base = wt; 
                                    }
                                    if let Some(sh) = update.shortcuts { s.shortcuts = sh; }
                                    if let Some(op) = update.opacity { s.opacity = op; }
                                    if let Some(fs) = update.font_size { s.font_size = fs; }
                                    if let Some(cwd) = update.default_cwd {
                                        let root = std::path::Path::new(&cwd);
                                        if let Err(e) = env::set_current_dir(&root) {
                                            let _ = output_tx.send(format!("Failed to set default_cwd: {}", e));
                                        } else if let Ok(actual_cwd) = env::current_dir() {
                                            s.current_dir = actual_cwd.to_string_lossy().to_string();
                                        }
                                    }
                                    
                                    s.window_title_full = format!("[{:?}] {}", s.mode, s.window_title_base);
                                    s.title_updated = true;
                                    let _ = output_tx.send("Config loaded.".to_string());
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
                            .stderr(Stdio::piped())
                            .spawn() 
                        {
                            Ok(mut child) => {
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
                                
                                if let Some(stderr) = child.stderr.take() {
                                     let out_tx = output_tx.clone();
                                    thread::spawn(move || {
                                        let reader = BufReader::new(stderr);
                                        for line in reader.lines() {
                                            if let Ok(l) = line {
                                                let _ = out_tx.send(l);
                                            }
                                        }
                                    });
                                }

                                let _ = child.wait();
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

        // Global Key Intercept
        let (current_mode, shortcuts, opacity, font_size, current_dir) = {
            let s = self.shell_state.lock().unwrap();
            (s.mode.clone(), s.shortcuts.clone(), s.opacity, s.font_size, s.current_dir.clone())
        };

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            let mut s = self.shell_state.lock().unwrap();
            s.mode = if s.mode == TerminalMode::Insert { TerminalMode::Normal } else { TerminalMode::Insert };
            s.window_title_full = format!("[{:?}] {}", s.mode, s.window_title_base);
            s.title_updated = true;
        }

        if current_mode == TerminalMode::Normal {
            if ctx.input(|i| i.key_pressed(egui::Key::I)) {
                 let mut s = self.shell_state.lock().unwrap();
                 s.mode = TerminalMode::Insert;
                 s.window_title_full = format!("[{:?}] {}", s.mode, s.window_title_base);
                 s.title_updated = true;
            }

            for sc in shortcuts {
                if let Some(key) = egui::Key::from_name(&sc.key) {
                    if ctx.input(|i| i.key_pressed(key)) {
                        let _ = self.command_tx.send(sc.cmd.clone());
                    }
                } else if sc.key.len() == 1 {
                    let char_key = sc.key.chars().next().unwrap();
                    if ctx.input(|i| i.events.iter().any(|e| {
                        if let egui::Event::Key { key, pressed: true, .. } = e {
                             format!("{:?}", key).to_lowercase() == char_key.to_string().to_lowercase()
                        } else { false }
                    })) {
                         let _ = self.command_tx.send(sc.cmd.clone());
                    }
                }
            }
        }

        // Check for window title update
        {
            let mut s = self.shell_state.lock().unwrap();
            if s.title_updated {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(s.window_title_full.clone()));
                s.title_updated = false;
            }
        }

        // Apply visual style override
        ctx.set_pixels_per_point(1.0); 
        let mut style = (*ctx.style()).clone();
        style.override_font_id = Some(egui::FontId::monospace(font_size));
        ctx.set_style(style);

        egui::TopBottomPanel::top("status_bar")
            .frame(egui::Frame::none().fill(egui::Color32::from_black_alpha(200)).inner_margin(4.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("PWD:").color(egui::Color32::GRAY));
                    ui.label(egui::RichText::new(current_dir).color(egui::Color32::LIGHT_BLUE));
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_black_alpha((opacity * 255.0) as u8)))
            .show(ctx, |ui| {
                ui.style_mut().visuals.extreme_bg_color = egui::Color32::BLACK;
                ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::BLACK;
                
                let (prompt_text, prompt_color, text_color, mode) = {
                    let s = self.shell_state.lock().unwrap();
                    (s.prompt.clone(), s.prompt_color, s.text_color, s.mode.clone())
                };

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        // History
                        for line in &self.history {
                            ui.label(egui::RichText::new(line).color(text_color));
                        }

                        // Current Prompt/Input Line
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(prompt_text).color(prompt_color).strong());
                            
                            let mut text_edit = egui::TextEdit::singleline(&mut self.input)
                                .desired_width(ui.available_width())
                                .frame(false)
                                .text_color(egui::Color32::WHITE)
                                .lock_focus(true);
                            
                            if mode == TerminalMode::Normal {
                                text_edit = text_edit.interactive(false).text_color(egui::Color32::GRAY);
                            }

                            let re = ui.add(text_edit);
                            
                            if mode == TerminalMode::Insert {
                                re.request_focus();
                                if re.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    let cmd = std::mem::take(&mut self.input);
                                    let _ = self.command_tx.send(cmd);
                                    re.request_focus();
                                }
                            }
                        });
                    });
            });

        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("[INSERT] Gemini Terminal")
            .with_transparent(true),
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

    #[test]
    fn test_hex_parsing() {
        assert_eq!(parse_hex_color("#FF0000"), Some(egui::Color32::from_rgb(255, 0, 0)));
        assert_eq!(parse_hex_color("00FF00"), Some(egui::Color32::from_rgb(0, 255, 0)));
        assert_eq!(parse_hex_color("invalid"), None);
    }
}
