use eframe::egui;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::path::PathBuf;

// --- Helper Functions ---

fn get_default_config_path() -> Option<PathBuf> {
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
        p.push("gemini");
        p.push("config.lua");
        p
    })
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

// --- Structured Log ---

#[derive(Clone, Debug)]
struct LogLine {
    text: String,
    color: egui::Color32,
}

impl LogLine {
    fn new(text: impl Into<String>, color: egui::Color32) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
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
    directory_color: Option<egui::Color32>,
}

fn parse_config(path: &std::path::Path) -> Result<ConfigUpdate, Box<dyn std::error::Error>> {
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
    directory_color: egui::Color32,
}

struct TerminalApp {
    input: String,
    history: Vec<LogLine>,
    shell_state: Arc<Mutex<ShellState>>,
    command_tx: Sender<String>,
    output_rx: Receiver<LogLine>,
}

impl TerminalApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (command_tx, command_rx) = unbounded::<String>();
        let (output_tx, output_rx) = unbounded::<LogLine>();

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
            directory_color: egui::Color32::from_rgb(100, 150, 255), // Light blue
        }));
        let thread_state = Arc::clone(&state);

        // Worker thread for command execution
        thread::spawn(move || {
            loop {
                let cmd_line = match command_rx.recv() {
                    Ok(line) => line,
                    Err(_) => break, // Channel closed
                };

                // Echo input with prompt color
                let (prompt, prompt_color) = {
                    let s = thread_state.lock().unwrap();
                    (s.prompt.clone(), s.prompt_color)
                };
                let _ = output_tx.send(LogLine::new(format!("{}{}", prompt, cmd_line), prompt_color));

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

                // Base text color for output
                let (text_color, dir_color) = {
                    let s = thread_state.lock().unwrap();
                    (s.text_color, s.directory_color)
                };

                match command.as_str() {
                    "exit" => std::process::exit(0),
                    "cd" => {
                        let new_dir = args.get(0).map_or("/", |x| x.as_str());
                        let root = std::path::Path::new(new_dir);
                        if let Err(e) = env::set_current_dir(&root) {
                            let _ = output_tx.send(LogLine::new(format!("Error: {}", e), egui::Color32::RED));
                        } else if let Ok(cwd) = env::current_dir() {
                            let new_cwd_str = cwd.to_string_lossy().to_string();
                            thread_state.lock().unwrap().current_dir = new_cwd_str;
                        }
                    }
                    "echo" => {
                        let output = args.join(" ");
                        let _ = output_tx.send(LogLine::new(output, text_color));
                    }
                    "ls" => {
                        let mut show_all = false;
                        let mut long_format = false;
                        let mut target_path = ".";

                        for arg in args {
                            if arg == "-a" || arg == "--all" {
                                show_all = true;
                            } else if arg == "-l" {
                                long_format = true;
                            } else if !arg.starts_with('-') {
                                target_path = arg;
                            }
                        }

                        match std::fs::read_dir(target_path) {
                            Ok(entries) => {
                                let mut entry_list: Vec<_> = entries.filter_map(Result::ok).collect();
                                entry_list.sort_by_key(|e| e.file_name());

                                for entry in entry_list {
                                    let file_name = entry.file_name().to_string_lossy().to_string();
                                    if !show_all && file_name.starts_with('.') {
                                        continue;
                                    }

                                    let mut line_color = text_color;
                                    if let Ok(metadata) = entry.metadata() {
                                        let is_dir = metadata.is_dir();
                                        if is_dir {
                                            line_color = dir_color;
                                        }
                                        
                                        if long_format {
                                            let type_indicator = if is_dir { "<DIR>" } else { "     " };
                                            let size = metadata.len();
                                            let _ = output_tx.send(LogLine::new(
                                                format!("{} {:>12} {}", type_indicator, size, file_name),
                                                line_color
                                            ));
                                        } else {
                                            let _ = output_tx.send(LogLine::new(file_name, line_color));
                                        }
                                    } else {
                                        let _ = output_tx.send(LogLine::new(file_name, text_color));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = output_tx.send(LogLine::new(format!("ls: {}: {}", target_path, e), egui::Color32::RED));
                            }
                        }
                    }
                    "config" => {
                         if args.first().map(|s| s.as_str()) == Some("load") {
                            let path = if let Some(path_arg) = args.get(1) {
                                std::path::PathBuf::from(path_arg)
                            } else {
                                match get_default_config_path() {
                                    Some(p) => p,
                                    None => {
                                        let _ = output_tx.send(LogLine::new("Error: Could not determine default config path".to_string(), egui::Color32::RED));
                                        continue;
                                    }
                                }
                            };

                            match parse_config(&path) {
                                Ok(update) => {
                                    let mut actual_cwd = None;
                                    let mut cwd_error = None;
                                    if let Some(new_cwd) = &update.default_cwd {
                                        let root = std::path::Path::new(new_cwd);
                                        if let Err(e) = env::set_current_dir(&root) {
                                            cwd_error = Some(format!("Failed to set default_cwd to {}: {}", new_cwd, e));
                                        } else {
                                            match env::current_dir() {
                                                Ok(cwd) => {
                                                    actual_cwd = Some(cwd.to_string_lossy().to_string());
                                                }
                                                Err(e) => {
                                                    cwd_error = Some(format!("Failed to read current dir '{}': {}", new_cwd, e));
                                                }
                                            }
                                        }
                                    }

                                    {
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
                                        if let Some(dc) = update.directory_color { s.directory_color = dc; }
                                        if let Some(cwd_str) = actual_cwd {
                                            s.current_dir = cwd_str;
                                        }
                                        
                                        s.window_title_full = format!("[{:?}] {}", s.mode, s.window_title_base);
                                        s.title_updated = true;
                                    }

                                    if let Some(e) = cwd_error {
                                        let _ = output_tx.send(LogLine::new(e, egui::Color32::RED));
                                    }
                                    let _ = output_tx.send(LogLine::new(format!("Config loaded from: {}", path.display()), egui::Color32::GOLD));
                                }
                                Err(e) => {
                                    let _ = output_tx.send(LogLine::new(format!("Failed to load config at {}: {}", path.display(), e), egui::Color32::RED));
                                }
                            }
                        } else {
                            let _ = output_tx.send(LogLine::new("Usage: config load [path]".to_string(), text_color));
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
                                                let _ = out_tx.send(LogLine::new(l, text_color));
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
                                                let _ = out_tx.send(LogLine::new(l, egui::Color32::RED));
                                            }
                                        }
                                    });
                                }

                                let _ = child.wait();
                            }
                            Err(_) => {
                                let _ = output_tx.send(LogLine::new("program not found".to_string(), egui::Color32::RED));
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
        while let Ok(line) = self.output_rx.try_recv() {
            self.history.push(line);
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
                    let char_key = sc.key.to_lowercase();
                    if ctx.input(|i| {
                        i.events.iter().any(|e| {
                            if let egui::Event::Text(s) = e {
                                s.to_lowercase() == char_key
                            } else {
                                false
                            }
                        })
                    }) {
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
                    ui.label(egui::RichText::new(current_dir).color(egui::Color32::from_rgb(100, 200, 255)));
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_black_alpha((opacity.clamp(0.0, 1.0) * 255.0) as u8)))
            .show(ctx, |ui| {
                ui.style_mut().visuals.extreme_bg_color = egui::Color32::BLACK;
                ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::BLACK;
                
                let (prompt_text, prompt_color, mode) = {
                    let s = self.shell_state.lock().unwrap();
                    (s.prompt.clone(), s.prompt_color, s.mode.clone())
                };

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        // History
                        for line in &self.history {
                            ui.label(egui::RichText::new(&line.text).color(line.color));
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
