
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl TerminalColor {
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const RED: Self = Self::from_rgb(255, 0, 0);
    pub const GREEN: Self = Self::from_rgb(0, 255, 0);
    pub const BLUE: Self = Self::from_rgb(100, 150, 255);
    pub const LIGHT_GRAY: Self = Self::from_rgb(211, 211, 211);
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);
    pub const GOLD: Self = Self::from_rgb(255, 215, 0);
    pub const GRAY: Self = Self::from_rgb(128, 128, 128);
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub text: String,
    pub color: TerminalColor,
}

impl LogLine {
    pub fn new(text: impl Into<String>, color: TerminalColor) -> Self {
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
    pub prompt_color: Option<TerminalColor>,
    pub text_color: Option<TerminalColor>,
    pub window_title: Option<String>,
    pub shortcuts: Option<Vec<Shortcut>>,
    pub opacity: Option<f32>,
    pub font_size: Option<f32>,
    pub default_cwd: Option<String>,
    pub directory_color: Option<TerminalColor>,
}

pub struct ShellState {
    pub prompt: String,
    pub prompt_color: TerminalColor,
    pub text_color: TerminalColor,
    pub window_title_base: String,
    pub window_title_full: String,
    pub title_updated: bool,
    pub mode: TerminalMode,
    pub shortcuts: Vec<Shortcut>,
    pub opacity: f32,
    pub font_size: f32,
    pub current_dir: String,
    pub directory_color: TerminalColor,
}
