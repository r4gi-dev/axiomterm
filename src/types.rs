
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl TerminalColor {
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const BLACK: Self = Self::from_rgb(0, 0, 0);
    pub const RED: Self = Self::from_rgb(255, 0, 0);
    pub const GREEN: Self = Self::from_rgb(0, 255, 0);
    pub const BLUE: Self = Self::from_rgb(100, 150, 255);
    pub const LIGHT_GRAY: Self = Self::from_rgb(211, 211, 211);
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);
    pub const GOLD: Self = Self::from_rgb(255, 215, 0);
    pub const GRAY: Self = Self::from_rgb(128, 128, 128);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellAttr {
    pub bold: bool,
    pub underline: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: TerminalColor,
    pub bg: TerminalColor,
    pub attrs: CellAttr,
}

impl Cell {
    pub fn new(ch: char, fg: TerminalColor) -> Self {
        Self {
            ch,
            fg,
            bg: TerminalColor::BLACK,
            attrs: CellAttr::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub cells: Vec<Cell>,
}

impl Line {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { cells: Vec::new() }
    }

    pub fn from_string(s: &str, fg: TerminalColor) -> Self {
        Self {
            cells: s.chars().map(|c| Cell::new(c, fg)).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScreenMeta {
    pub dirty: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineImpact {
    Single(usize),      // Affects only one specific line index
    Multi(Vec<usize>),  // Affects specific multiple lines
    Unbounded,          // Might affect everything or cause shifts
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationMetadata {
    pub impact: LineImpact,
    pub caused_scroll: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScreenOperation {
    PushLine(Line),
    Clear,
    #[allow(dead_code)]
    SetCursor(Cursor),
    #[allow(dead_code)]
    UpdateLine(usize, Line), // Visual update: row index, new content
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationCategory {
    Structural, // Affects layout (scroll, resize, clear)
    #[allow(dead_code)]
    Visual,     // Affects content only (no layout shift)
    Cursor,     // Affects cursor layer only
}

impl ScreenOperation {
    pub fn category(&self) -> OperationCategory {
        match self {
            Self::PushLine(_) => OperationCategory::Structural,
            Self::Clear => OperationCategory::Structural,
            Self::SetCursor(_) => OperationCategory::Cursor,
            Self::UpdateLine(_, _) => OperationCategory::Visual,
        }
    }

    pub fn metadata(&self) -> OperationMetadata {
        match self {
            Self::PushLine(_) => OperationMetadata {
                impact: LineImpact::Unbounded,
                caused_scroll: true,
            },
            Self::Clear => OperationMetadata {
                impact: LineImpact::Unbounded,
                caused_scroll: true,
            },
            Self::SetCursor(_) => OperationMetadata {
                impact: LineImpact::Single(0), // Cursor layer logic handles this, effectively "Single" (affects one row visually) but separate layer
                caused_scroll: false,
            },
            Self::UpdateLine(row, _) => OperationMetadata {
                impact: LineImpact::Single(*row),
                caused_scroll: false,
            },
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Screen {
    pub lines: Vec<Line>,
    pub cursor: Cursor,
    pub meta: ScreenMeta,
}

impl Screen {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_line(&mut self, line: Line) -> ScreenOperation {
        self.lines.push(line.clone());
        self.meta.dirty = true;
        ScreenOperation::PushLine(line)
    }

    pub fn clear(&mut self) -> ScreenOperation {
        self.lines.clear();
        self.cursor = Cursor::default();
        self.meta.dirty = true;
        ScreenOperation::Clear
    }

    #[allow(dead_code)]
    pub fn set_cursor(&mut self, cursor: Cursor) -> ScreenOperation {
        self.cursor = cursor;
        self.meta.dirty = true;
        ScreenOperation::SetCursor(cursor)
    }

    #[allow(dead_code)]
    pub fn update_line(&mut self, row: usize, line: Line) -> ScreenOperation {
        if row < self.lines.len() {
            self.lines[row] = line.clone();
            self.meta.dirty = true;
            ScreenOperation::UpdateLine(row, line)
        } else {
            // If out of bounds, maybe just ignore or push? For now, strict update.
            // Returning NoOp essentially if we had one. But ScreenOperation must be valid.
            // Fallback to push if row == len?
             if row == self.lines.len() {
                self.push_line(line)
             } else {
                 // Invalid update, treat as force refresh or ignore.
                 // Let's return Clear as "Something went wrong" or just ignore safely?
                 // Ideally we shouldn't panic. Let's assume caller checks bounds.
                 // For safety in this conceptual phase, let's just push it to be safe (Structural)
                 // or better, do nothing effectively by sending a dummy update?
                 // Implementation detail: for now, assume valid.
                self.lines.push(line.clone());
                ScreenOperation::PushLine(line)
             }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    AppendChar(char),
    Backspace,
    Delete,
    Submit,          // Typically Enter
    Clear,           // Clear screen
    #[allow(dead_code)]
    MoveCursor(i32, i32), // Delta move
    ChangeMode(TerminalMode),
    RunCommand(String),
    NoOp,
}

impl Action {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Backspace" => Some(Self::Backspace),
            "Delete" => Some(Self::Delete),
            "Submit" | "Enter" => Some(Self::Submit),
            "Clear" => Some(Self::Clear),
            "NoOp" => Some(Self::NoOp),
            _ if s.starts_with("ChangeMode(") && s.ends_with(')') => {
                let mode_str = &s[11..s.len()-1];
                TerminalMode::from_str(mode_str).map(Self::ChangeMode)
            },
            _ if s.starts_with("RunCommand(") && s.ends_with(')') => {
                let cmd = &s[11..s.len()-1];
                Some(Self::RunCommand(cmd.to_string()))
            },
            _ if s.starts_with("InsertChar(") && s.ends_with(')') => {
                let char_str = &s[11..s.len()-1];
                char_str.chars().next().map(Self::AppendChar)
            },
            _ if s.len() == 1 => Some(Self::AppendChar(s.chars().next().unwrap())),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InputEvent {
    Key { code: String, ctrl: bool, alt: bool, shift: bool },
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TerminalMode {
    Insert,
    Normal,
    Visual,
    Custom(String),
}

impl TerminalMode {
    pub fn name(&self) -> &str {
        match self {
            Self::Insert => "INSERT",
            Self::Normal => "NORMAL",
            Self::Visual => "VISUAL",
            Self::Custom(s) => s.as_str(),
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Insert" | "INSERT" => Some(Self::Insert),
            "Normal" | "NORMAL" => Some(Self::Normal),
            "Visual" | "VISUAL" => Some(Self::Visual),
            _ => Some(Self::Custom(s.to_string())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingTarget {
    Action(Action),
    Macro(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyBinding {
    pub event: InputEvent,
    pub target: BindingTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeDefinition {
    pub mode: TerminalMode,
    pub bindings: Vec<KeyBinding>,
}

#[derive(Clone, Debug)]
pub enum ShellEvent {
    // Every mutation of the Screen state generates a ScreenOperation.
    Operation(ScreenOperation),
    // Background notifications or control signals.
    #[allow(dead_code)]
    Notification(String),
}


#[derive(Clone, Debug)]
#[allow(dead_code)]
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
    pub mode_definitions: Option<Vec<ModeDefinition>>,
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
    pub screen: Screen,
    pub input_buffer: String,
    pub mode_definitions: Vec<ModeDefinition>,
}
