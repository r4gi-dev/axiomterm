use eframe::egui;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct LogLine {
    pub text: String,
    pub color: egui::Color32,
}

impl LogLine {
    pub fn new(text: impl Into<String>, color: egui::Color32) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ShellEvent {
    Output(LogLine),
    Clear,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TerminalMode {
    Insert,
    Normal,
}

#[derive(Clone, Debug)]
pub struct Shortcut {
    pub key: String,
    pub cmd: String,
}

#[derive(Default)]
pub struct ConfigUpdate {
    pub prompt: Option<String>,
    pub prompt_color: Option<egui::Color32>,
    pub text_color: Option<egui::Color32>,
    pub window_title: Option<String>,
    pub shortcuts: Option<Vec<Shortcut>>,
    pub opacity: Option<f32>,
    pub font_size: Option<f32>,
    pub default_cwd: Option<String>,
    pub directory_color: Option<egui::Color32>,
}

pub struct ShellState {
    pub prompt: String,
    pub prompt_color: egui::Color32,
    pub text_color: egui::Color32,
    pub window_title_base: String,
    pub window_title_full: String,
    pub title_updated: bool,
    pub mode: TerminalMode,
    pub shortcuts: Vec<Shortcut>,
    pub opacity: f32,
    pub font_size: f32,
    pub current_dir: String,
    pub directory_color: egui::Color32,
}
