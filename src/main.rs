mod app;
mod config;
mod shell;
mod types;
mod utils;
mod backend;
mod fixed_config;

use crate::app::TerminalApp;
use crate::fixed_config::FixedConfig;
use eframe::egui;

fn main() -> eframe::Result<()> {
    // CRITICAL: Load FixedConfig FIRST
    // This determines the terminal's existence conditions
    // Failure here MUST abort startup
    let fixed_config = FixedConfig::load()
        .expect("FATAL: Failed to load fixed configuration (terminal.toml)");
    
    // Validate FixedConfig
    if let Err(e) = fixed_config.validate() {
        panic!("FATAL: Invalid fixed configuration: {}", e);
    }

    // Initialize Backend based on FixedConfig
    // Currently only StdBackend is supported
    let backend = Box::new(backend::StdBackend);

    // Initialize Renderer based on FixedConfig
    // Currently only egui is supported
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([fixed_config.window.initial_width as f32, fixed_config.window.initial_height as f32])
            .with_title(&format!("[INSERT] axiomterm"))
            .with_transparent(fixed_config.window.transparent),
        ..Default::default()
    };

    eframe::run_native(
        "axiomterm",
        options,
        Box::new(move |cc| Ok(Box::new(TerminalApp::new(cc, backend, &fixed_config)))),
    )
}

#[cfg(test)]
mod tests {
    use crate::utils::{parse_hex_color, tokenize_command};
    use crate::types::TerminalColor;

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
        assert_eq!(
            parse_hex_color("#FF0000"),
            Some(TerminalColor::from_rgb(255, 0, 0))
        );
        assert_eq!(
            parse_hex_color("00FF00"),
            Some(TerminalColor::from_rgb(0, 255, 0))
        );
        assert_eq!(parse_hex_color("invalid"), None);
    }

    #[test]
    fn test_headless_operation() {
        use crate::shell::spawn_shell_thread;
        use crate::types::{ShellState, TerminalMode, Screen, ShellEvent, ScreenOperation, TerminalColor};
        use crossbeam_channel::unbounded;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;

        let (cmd_tx, cmd_rx) = unbounded();
        let (out_tx, out_rx) = unbounded();
        let state = Arc::new(Mutex::new(ShellState {
            prompt: "> ".to_string(),
            prompt_color: TerminalColor::GREEN,
            text_color: TerminalColor::LIGHT_GRAY,
            window_title_base: "Test".to_string(),
            window_title_full: "Test".to_string(),
            title_updated: false,
            mode: TerminalMode::Insert,
            shortcuts: Vec::new(),
            opacity: 1.0,
            font_size: 14.0,
            current_dir: ".".to_string(),
            directory_color: TerminalColor::BLUE,
            screen: Screen::new(),
            input_buffer: String::new(),
            mode_definitions: vec![
                crate::types::ModeDefinition {
                    mode: TerminalMode::Insert,
                    bindings: vec![
                        crate::types::KeyBinding { 
                            event: crate::types::InputEvent::Key { code: "Enter".to_string(), ctrl: false, alt: false, shift: false }, 
                            action: crate::types::Action::Submit 
                        },
                    ],
                },
            ],
        }));

        spawn_shell_thread(cmd_rx, out_tx, Arc::clone(&state), Box::new(crate::backend::StdBackend));

        use crate::types::Action;
        // Simulate typing "echo hello" and submitting
        for ch in "echo hello".chars() {
            cmd_tx.send(Action::AppendChar(ch)).unwrap();
        }
        cmd_tx.send(Action::Submit).unwrap();

        // 1st operation should be the echo of the command
        let event = out_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if let ShellEvent::Operation(ScreenOperation::PushLine(line)) = event {
            let text: String = line.cells.iter().map(|c| c.ch).collect();
            assert!(text.contains("> echo hello"));
        } else {
            panic!("Expected PushLine operation for echo");
        }

        // 2nd operation should be the output of the echo command
        let event = out_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if let ShellEvent::Operation(ScreenOperation::PushLine(line)) = event {
            let text: String = line.cells.iter().map(|c| c.ch).collect();
            assert_eq!(text, "hello");
        } else {
            panic!("Expected PushLine operation for command output");
        }
    }
}
