use crate::config::parse_config;
use crate::types::{LogLine, ShellEvent, ShellState};
use crate::utils::{get_default_config_path, tokenize_command};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

pub fn spawn_shell_thread(
    command_rx: Receiver<String>,
    output_tx: Sender<ShellEvent>,
    thread_state: Arc<Mutex<ShellState>>,
) {
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
            let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                format!("{}{}", prompt, cmd_line),
                prompt_color,
            )));

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
            let (text_color, dir_color, current_dir) = {
                let s = thread_state.lock().unwrap();
                (s.text_color, s.directory_color, s.current_dir.clone())
            };

            match command.as_str() {
                "exit" => std::process::exit(0),
                "cd" => {
                    let new_dir = args.get(0).map_or("/", |x| x.as_str());
                    let root = std::path::Path::new(new_dir);
                    if let Err(e) = env::set_current_dir(&root) {
                        let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                            format!("Error: {}", e),
                            egui::Color32::RED,
                        )));
                    } else if let Ok(cwd) = env::current_dir() {
                        let new_cwd_str = cwd.to_string_lossy().to_string();
                        thread_state.lock().unwrap().current_dir = new_cwd_str;
                    }
                }
                "pwd" => {
                    let _ = output_tx.send(ShellEvent::Output(LogLine::new(current_dir, text_color)));
                }
                "clear" => {
                    let _ = output_tx.send(ShellEvent::Clear);
                }
                "echo" => {
                    let output = args.join(" ");
                    let _ = output_tx.send(ShellEvent::Output(LogLine::new(output, text_color)));
                }
                "mkdir" => {
                    for path in args {
                        if let Err(e) = std::fs::create_dir_all(path) {
                            let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                format!("mkdir: {}: {}", path, e),
                                egui::Color32::RED,
                            )));
                        }
                    }
                }
                "touch" => {
                    for path in args {
                        if let Err(e) = std::fs::OpenOptions::new().create(true).write(true).open(path) {
                            let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                format!("touch: {}: {}", path, e),
                                egui::Color32::RED,
                            )));
                        }
                    }
                }
                "cat" => {
                    for path in args {
                        match std::fs::read_to_string(path) {
                            Ok(content) => {
                                for line in content.lines() {
                                    let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                        line, text_color,
                                    )));
                                }
                            }
                            Err(e) => {
                                let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                    format!("cat: {}: {}", path, e),
                                    egui::Color32::RED,
                                )));
                            }
                        }
                    }
                }
                "rm" => {
                    for path in args {
                        if let Err(e) = std::fs::remove_file(path).or_else(|_| std::fs::remove_dir(path)) {
                            let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                format!("rm: {}: {}", path, e),
                                egui::Color32::RED,
                            )));
                        }
                    }
                }
                "mv" => {
                    if args.len() == 2 {
                        if let Err(e) = std::fs::rename(&args[0], &args[1]) {
                            let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                format!("mv: {}", e),
                                egui::Color32::RED,
                            )));
                        }
                    } else {
                        let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                            "Usage: mv <source> <dest>",
                            text_color,
                        )));
                    }
                }
                "cp" => {
                    if args.len() == 2 {
                        if let Err(e) = std::fs::copy(&args[0], &args[1]) {
                            let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                format!("cp: {}", e),
                                egui::Color32::RED,
                            )));
                        }
                    } else {
                        let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                            "Usage: cp <source> <dest>",
                            text_color,
                        )));
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

                                    if long_format {
                                        let type_indicator = if is_dir { "<DIR>" } else { "     " };
                                        let size = metadata.len();
                                        let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                            format!("{} {:>12} {}", type_indicator, size, file_name),
                                            line_color,
                                        )));
                                    } else {
                                        let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                            file_name, line_color,
                                        )));
                                    }
                                } else {
                                    let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                        file_name, text_color,
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                format!("ls: {}: {}", target_path, e),
                                egui::Color32::RED,
                            )));
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
                                    let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                        "Error: Could not determine default config path".to_string(),
                                        egui::Color32::RED,
                                    )));
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
                                    if let Some(cwd_str) = actual_cwd {
                                        s.current_dir = cwd_str;
                                    }

                                    s.window_title_full =
                                        format!("[{:?}] {}", s.mode, s.window_title_base);
                                    s.title_updated = true;
                                }

                                if let Some(e) = cwd_error {
                                    let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                        e,
                                        egui::Color32::RED,
                                    )));
                                }
                                let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                    format!("Config loaded from: {}", path.display()),
                                    egui::Color32::GOLD,
                                )));
                            }
                            Err(e) => {
                                let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                    format!("Failed to load config at {}: {}", path.display(), e),
                                    egui::Color32::RED,
                                )));
                            }
                        }
                    } else {
                        let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                            "Usage: config load [path]".to_string(),
                            text_color,
                        )));
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
                                            let _ = out_tx.send(ShellEvent::Output(LogLine::new(
                                                l, text_color,
                                            )));
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
                                            let _ = out_tx.send(ShellEvent::Output(LogLine::new(
                                                l,
                                                egui::Color32::RED,
                                            )));
                                        }
                                    }
                                });
                            }

                            let _ = child.wait();
                        }
                        Err(_) => {
                            let _ = output_tx.send(ShellEvent::Output(LogLine::new(
                                "program not found".to_string(),
                                egui::Color32::RED,
                            )));
                        }
                    }
                }
            }
        }
    });
}
