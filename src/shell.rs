use crate::config::parse_config;
use crate::types::{Action, Line, ShellEvent, ShellState, TerminalColor};
use crate::backend::ProcessBackend;
use crate::utils::{get_default_config_path, tokenize_command};
use crossbeam_channel::{Receiver, Sender};
use std::env;
// use std::io; // Removed unused import
// use std::process::{Command, Stdio}; // Removed unused imports
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

pub fn spawn_shell_thread(
    action_rx: Receiver<Action>,
    output_tx: Sender<ShellEvent>,
    thread_state: Arc<Mutex<ShellState>>,
    backend: Box<dyn ProcessBackend>,
) {
    thread::spawn(move || {
        loop {
            let action = match action_rx.recv() {
                Ok(a) => a,
                Err(_) => break, // Channel closed
            };

            match action {
                Action::AppendChar(ch) => {
                    let mut s = thread_state.lock().unwrap();
                    s.input_buffer.push(ch);
                    // For now, simple echo: we don't redraw the whole line, just push char to current line logic?
                    // Actually, the current line logic is "push_line".
                    // Let's just update the buffer. The renderer will need to show the prompt + buffer.
                }
                Action::Backspace => {
                    let mut s = thread_state.lock().unwrap();
                    s.input_buffer.pop();
                }
                Action::Submit => {
                    let cmd_line = {
                        let mut s = thread_state.lock().unwrap();
                        let line = std::mem::take(&mut s.input_buffer);
                        
                        // Echo the final submitted command
                        let prompt = s.prompt.clone();
                        let prompt_color = s.prompt_color;
                        let op = s.screen.push_line(Line::from_string(&format!("{}{}", prompt, line), prompt_color));
                        let _ = output_tx.send(ShellEvent::Operation(op));
                        line
                    };

                    execute_command(&cmd_line, &thread_state, &output_tx, &*backend);
                }
                Action::Clear => {
                    let mut s = thread_state.lock().unwrap();
                    let op = s.screen.clear();
                    let _ = output_tx.send(ShellEvent::Operation(op));
                }
                Action::ChangeMode(new_mode) => {
                    let mut s = thread_state.lock().unwrap();
                    s.mode = new_mode;
                    s.window_title_full = format!("[{}] {}", s.mode.name(), s.window_title_base);
                    s.title_updated = true;
                }
                Action::RunCommand(cmd) => {
                    execute_command(&cmd, &thread_state, &output_tx, &*backend);
                }
                _ => {}
            }
        }
    });
}

fn execute_command(
    cmd_line: &str,
    thread_state: &Arc<Mutex<ShellState>>,
    output_tx: &Sender<ShellEvent>,
    backend: &dyn ProcessBackend,
) {
            let cmd_line = cmd_line.trim();
            if cmd_line.is_empty() {
                return;
            }

            let parts = tokenize_command(cmd_line);
            if parts.is_empty() {
                return;
            }

            let command = &parts[0];
            let args = &parts[1..];

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
                        let mut s = thread_state.lock().unwrap();
                        let op = s.screen.push_line(Line::from_string(&format!("Error: {}", e), TerminalColor::RED));
                        let _ = output_tx.send(ShellEvent::Operation(op));
                    } else if let Ok(cwd) = env::current_dir() {
                        let new_cwd_str = cwd.to_string_lossy().to_string();
                        thread_state.lock().unwrap().current_dir = new_cwd_str;
                    }
                }
                "pwd" => {
                    let mut s = thread_state.lock().unwrap();
                    let current_dir = s.current_dir.clone();
                    let text_color = s.text_color;
                    let op = s.screen.push_line(Line::from_string(&current_dir, text_color));
                    let _ = output_tx.send(ShellEvent::Operation(op));
                }
                "clear" => {
                    let mut s = thread_state.lock().unwrap();
                    let op = s.screen.clear();
                    let _ = output_tx.send(ShellEvent::Operation(op));
                }
                "echo" => {
                    let output = args.join(" ");
                    let mut s = thread_state.lock().unwrap();
                    let text_color = s.text_color;
                    let op = s.screen.push_line(Line::from_string(&output, text_color));
                    let _ = output_tx.send(ShellEvent::Operation(op));
                }
                "mkdir" => {
                    for path in args {
                        if let Err(e) = std::fs::create_dir_all(path) {
                            let mut s = thread_state.lock().unwrap();
                            let op = s.screen.push_line(Line::from_string(&format!("mkdir: {}: {}", path, e), TerminalColor::RED));
                            let _ = output_tx.send(ShellEvent::Operation(op));
                        }
                    }
                }
                "touch" => {
                    for path in args {
                        match std::fs::OpenOptions::new().create(true).write(true).open(path) {
                            Ok(_) => {
                                if let Err(e) = filetime::set_file_mtime(path, filetime::FileTime::from_system_time(SystemTime::now())) {
                                    let mut s = thread_state.lock().unwrap();
                                    let op = s.screen.push_line(Line::from_string(&format!("touch (mtime): {}: {}", path, e), TerminalColor::RED));
                                    let _ = output_tx.send(ShellEvent::Operation(op));
                                }
                            }
                            Err(e) => {
                                let mut s = thread_state.lock().unwrap();
                                let op = s.screen.push_line(Line::from_string(&format!("touch: {}: {}", path, e), TerminalColor::RED));
                                let _ = output_tx.send(ShellEvent::Operation(op));
                            }
                        }
                    }
                }
                "cat" => {
                    for path in args {
                        match std::fs::read_to_string(path) {
                            Ok(content) => {
                                let mut s = thread_state.lock().unwrap();
                                for line in content.lines() {
                                    let op = s.screen.push_line(Line::from_string(line, text_color));
                                    let _ = output_tx.send(ShellEvent::Operation(op));
                                }
                            }
                            Err(e) => {
                                let mut s = thread_state.lock().unwrap();
                                let op = s.screen.push_line(Line::from_string(&format!("cat: {}: {}", path, e), TerminalColor::RED));
                                let _ = output_tx.send(ShellEvent::Operation(op));
                            }
                        }
                    }
                }
                "rm" => {
                    for path in args {
                        if let Err(e) = std::fs::remove_file(path).or_else(|_| std::fs::remove_dir(path)) {
                            let mut s = thread_state.lock().unwrap();
                            let op = s.screen.push_line(Line::from_string(&format!("rm: {}: {}", path, e), TerminalColor::RED));
                            let _ = output_tx.send(ShellEvent::Operation(op));
                        }
                    }
                }
                "mv" => {
                    if args.len() == 2 {
                        if let Err(e) = std::fs::rename(&args[0], &args[1]) {
                            let mut s = thread_state.lock().unwrap();
                            let op = s.screen.push_line(Line::from_string(&format!("mv: {}", e), TerminalColor::RED));
                            let _ = output_tx.send(ShellEvent::Operation(op));
                        }
                    } else {
                        let mut s = thread_state.lock().unwrap();
                        let op = s.screen.push_line(Line::from_string("Usage: mv <source> <dest>", text_color));
                        let _ = output_tx.send(ShellEvent::Operation(op));
                    }
                }
                "cp" => {
                    if args.len() == 2 {
                        if let Err(e) = std::fs::copy(&args[0], &args[1]) {
                            let mut s = thread_state.lock().unwrap();
                            let op = s.screen.push_line(Line::from_string(&format!("cp: {}", e), TerminalColor::RED));
                            let _ = output_tx.send(ShellEvent::Operation(op));
                        }
                    } else {
                        let mut s = thread_state.lock().unwrap();
                        let op = s.screen.push_line(Line::from_string("Usage: cp <source> <dest>", text_color));
                        let _ = output_tx.send(ShellEvent::Operation(op));
                    }
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

                                    let mut s = thread_state.lock().unwrap();
                                    let op = if long_format {
                                        let type_indicator = if is_dir { "<DIR>" } else { "     " };
                                        let size = metadata.len();
                                        s.screen.push_line(Line::from_string(
                                            &format!("{} {:>12} {}", type_indicator, size, file_name),
                                            line_color,
                                        ))
                                    } else {
                                        s.screen.push_line(Line::from_string(&file_name, line_color))
                                    };
                                    let _ = output_tx.send(ShellEvent::Operation(op));
                                } else {
                                    let mut s = thread_state.lock().unwrap();
                                    let op = s.screen.push_line(Line::from_string(&file_name, text_color));
                                    let _ = output_tx.send(ShellEvent::Operation(op));
                                }
                            }
                        }
                        Err(e) => {
                            let mut s = thread_state.lock().unwrap();
                            let op = s.screen.push_line(Line::from_string(&format!("ls: {}: {}", target_path, e), TerminalColor::RED));
                            let _ = output_tx.send(ShellEvent::Operation(op));
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
                                    let mut s = thread_state.lock().unwrap();
                                    let op = s.screen.push_line(Line::from_string("Error: Could not determine default config path", TerminalColor::RED));
                                    let _ = output_tx.send(ShellEvent::Operation(op));
                                    return;
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
                                        cwd_error = Some(format!(
                                            "Failed to set default_cwd to {}: {}",
                                            new_cwd, e
                                        ));
                                    } else {
                                        match env::current_dir() {
                                            Ok(cwd) => {
                                                actual_cwd = Some(cwd.to_string_lossy().to_string());
                                            }
                                            Err(e) => {
                                                cwd_error = Some(format!(
                                                    "Failed to read current dir '{}': {}",
                                                    new_cwd, e
                                                ));
                                            }
                                        }
                                    }
                                }

                                {
                                    let mut s = thread_state.lock().unwrap();
                                    if let Some(p) = update.prompt {
                                        s.prompt = p;
                                    }
                                    if let Some(pc) = update.prompt_color {
                                        s.prompt_color = pc;
                                    }
                                    if let Some(tc) = update.text_color {
                                        s.text_color = tc;
                                    }
                                    if let Some(wt) = update.window_title {
                                        s.window_title_base = wt;
                                    }
                                    if let Some(sh) = update.shortcuts {
                                        s.shortcuts = sh;
                                    }
                                    if let Some(op) = update.opacity {
                                        s.opacity = op;
                                    }
                                    if let Some(fs) = update.font_size {
                                        s.font_size = fs;
                                    }
                                    if let Some(dc) = update.directory_color {
                                        s.directory_color = dc;
                                    }
                                    if let Some(md) = update.mode_definitions {
                                        s.mode_definitions = md;
                                    }
                                    if let Some(cwd_str) = actual_cwd {
                                        s.current_dir = cwd_str;
                                    }

                                    s.window_title_full =
                                        format!("[{}] {}", s.mode.name(), s.window_title_base);
                                    s.title_updated = true;
                                }

                                if let Some(e) = cwd_error {
                                    let mut s = thread_state.lock().unwrap();
                                    let op = s.screen.push_line(Line::from_string(&e, TerminalColor::RED));
                                    let _ = output_tx.send(ShellEvent::Operation(op));
                                }
                                let mut s = thread_state.lock().unwrap();
                                let op = s.screen.push_line(Line::from_string(
                                    &format!("Config loaded from: {}", path.display()),
                                    TerminalColor::GOLD,
                                ));
                                let _ = output_tx.send(ShellEvent::Operation(op));
                            }
                            Err(e) => {
                                let mut s = thread_state.lock().unwrap();
                                let op = s.screen.push_line(Line::from_string(&format!("Failed to load config at {}: {}", path.display(), e), TerminalColor::RED));
                                let _ = output_tx.send(ShellEvent::Operation(op));
                            }
                        }
                    } else {
                        let mut s = thread_state.lock().unwrap();
                        let op = s.screen.push_line(Line::from_string("Usage: config load [path]", text_color));
                        let _ = output_tx.send(ShellEvent::Operation(op));
                    }
                }
                command_name => {
                    if let Err(e) = backend.spawn(command_name, args, output_tx.clone(), Arc::clone(thread_state)) {
                        let mut s = thread_state.lock().unwrap();
                        let op = s.screen.push_line(Line::from_string(&format!("Failed to spawn {}: {}", command_name, e), TerminalColor::RED));
                        let _ = output_tx.send(ShellEvent::Operation(op));
                    }
                }
            }
}
