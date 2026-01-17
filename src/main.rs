mod app;
mod config;
mod shell;
mod types;
mod utils;

use crate::app::TerminalApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
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
}
