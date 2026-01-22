use crate::shell::spawn_shell_thread;
use crate::types::{Action, InputEvent, KeyBinding, ModeDefinition, ShellState, TerminalMode, Screen, ShellEvent, TerminalColor, ScreenOperation};
use crate::backend::ProcessBackend;
use crate::fixed_config::FixedConfig;
use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui;
use std::env;
use std::sync::{Arc, Mutex};

use crate::utils::get_default_config_path;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::time::{Duration, Instant};

use crate::renderer::TerminalRenderer;

pub struct TerminalApp {
    pub shell_state: Arc<Mutex<ShellState>>,
    pub action_tx: Sender<Action>,
    pub output_rx: Receiver<ShellEvent>,
    pub _watcher: Option<RecommendedWatcher>,
    pub config_rx: Receiver<()>,
    pub last_reload: Instant,
    pub renderer: TerminalRenderer,
}

impl TerminalApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, backend: Box<dyn ProcessBackend>, fixed_config: &FixedConfig) -> Self {
        let (action_tx, action_rx) = unbounded::<Action>();
        let (output_tx, output_rx) = unbounded::<ShellEvent>();
        let (config_tx, config_rx) = unbounded::<()>();

        let current_dir = env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        // Set up config watcher
        let mut watcher: Option<RecommendedWatcher> = None;
        if let Some(config_path) = get_default_config_path() {
            if let Some(config_dir) = config_path.parent() {
                 let tx = config_tx.clone();
                 if let Ok(mut w) = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                     match res {
                         Ok(event) => {
                             if let notify::EventKind::Modify(_) = event.kind {
                                 let _ = tx.send(());
                             }
                         },
                         Err(_) => {},
                     }
                 }) {
                     if let Ok(_) = w.watch(config_dir, RecursiveMode::NonRecursive) {
                         watcher = Some(w);
                     }
                 }
            }
        }

        // Determine initial mode from FixedConfig
        let initial_mode = match fixed_config.core.initial_mode.as_str() {
            "insert" => TerminalMode::Insert,
            "normal" => TerminalMode::Normal,
            "visual" => TerminalMode::Visual,
            _ => TerminalMode::Insert, // Fallback
        };

        let state = Arc::new(Mutex::new(ShellState {
            prompt: "> ".to_string(),
            prompt_color: TerminalColor::GREEN,
            text_color: TerminalColor::LIGHT_GRAY,
            window_title_base: "axiomterm".to_string(),
            window_title_full: format!("[{}] {}", initial_mode.name(), "axiomterm"),
            title_updated: false,
            mode: initial_mode,
            shortcuts: Vec::new(),
            opacity: 1.0,
            font_size: 14.0,
            current_dir: current_dir.clone(),
            directory_color: TerminalColor::BLUE,
            screen: Screen::new(),
            input_buffer: String::new(),
            mode_definitions: vec![
                ModeDefinition {
                    mode: TerminalMode::Insert,
                    bindings: vec![
                        KeyBinding { event: InputEvent::Key { code: "Enter".to_string(), ctrl: false, alt: false, shift: false }, action: Action::Submit },
                        KeyBinding { event: InputEvent::Key { code: "Backspace".to_string(), ctrl: false, alt: false, shift: false }, action: Action::Backspace },
                        KeyBinding { event: InputEvent::Key { code: "Escape".to_string(), ctrl: false, alt: false, shift: false }, action: Action::ChangeMode(TerminalMode::Normal) },
                    ],
                },
                ModeDefinition {
                    mode: TerminalMode::Normal,
                    bindings: vec![
                        KeyBinding { event: InputEvent::Key { code: "I".to_string(), ctrl: false, alt: false, shift: false }, action: Action::ChangeMode(TerminalMode::Insert) },
                        KeyBinding { event: InputEvent::Key { code: "Escape".to_string(), ctrl: false, alt: false, shift: false }, action: Action::Clear },
                    ],
                },
            ],
        }));

        spawn_shell_thread(action_rx, output_tx, Arc::clone(&state), backend);

        Self {
            shell_state: state,
            action_tx,
            output_rx,
            _watcher: watcher,
            config_rx,
            last_reload: Instant::now(),
            renderer: TerminalRenderer::new(),
        }
    }

    // map_input has been moved into the input module
}

impl From<TerminalColor> for egui::Color32 {
    fn from(c: TerminalColor) -> Self {
        egui::Color32::from_rgb(c.r, c.g, c.b)
    }
}

impl TerminalApp {
    fn on_structural_change(&mut self, ctx: &egui::Context, _op: &ScreenOperation) {
        self.renderer.on_structural_change(ctx);
    }

    fn on_visual_change(&mut self, ctx: &egui::Context, op: &ScreenOperation) {
        self.renderer.on_visual_change(ctx, op);
    }

    fn on_cursor_change(&mut self, ctx: &egui::Context, _op: &ScreenOperation) {
        self.renderer.on_cursor_change(ctx);
    }
}

impl eframe::App for TerminalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for new events (Operations are the primary driver of state changes)
        // Check for config file changes
        let mut config_updated = false;
        while let Ok(_) = self.config_rx.try_recv() {
            config_updated = true;
        }

        if config_updated {
            if self.last_reload.elapsed() > Duration::from_millis(500) {
                let _ = self.action_tx.send(Action::RunCommand("config load".to_string()));
                self.last_reload = Instant::now();
            }
        }

        while let Ok(event) = self.output_rx.try_recv() {
            match event {
                ShellEvent::Operation(op) => {
                    use crate::types::OperationCategory;
                    match op.category() {
                        OperationCategory::Structural => self.on_structural_change(ctx, &op),
                        OperationCategory::Visual => self.on_visual_change(ctx, &op),
                        OperationCategory::Cursor => self.on_cursor_change(ctx, &op),
                    }
                }
                ShellEvent::Notification(msg) => {
                    println!("Notification: {}", msg);
                }
            }
        }

        // Fetch state for interpretation and rendering
        // Fetch state for interpretation and rendering
        let (current_mode, _shortcuts, opacity, font_size, current_dir, text_color, dir_color, prompt_text, prompt_color, mode_defs) = {
            let s = self.shell_state.lock().unwrap();
            (
                s.mode.clone(),
                s.shortcuts.clone(),
                s.opacity,
                s.font_size,
                s.current_dir.clone(),
                s.text_color,
                s.directory_color,
                s.prompt.clone(),
                s.prompt_color,
                s.mode_definitions.clone(),
            )
        };

        // Capture and process InputEvents
        // Capture and process InputEvents via extracted input module
        let actions = crate::input::poll_and_map(ctx, &current_mode, &mode_defs);
        for action in actions {
            let _ = self.action_tx.send(action);
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
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_black_alpha(200))
                    .inner_margin(4.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("PWD:").color(egui::Color32::from(text_color)));
                    ui.label(
                        egui::RichText::new(current_dir)
                            .color(egui::Color32::from(dir_color)),
                    );
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_black_alpha(
                (opacity.clamp(0.0, 1.0) * 255.0) as u8,
            )))
            .show(ctx, |ui| {
                // Delegate rendering to renderer
                {
                    let state = self.shell_state.lock().unwrap();
                    self.renderer.draw(ui, &state);
                }

                // Current Prompt/Input Line
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&prompt_text)
                            .color(egui::Color32::from(prompt_color))
                            .strong(),
                    );

                    let mut s = self.shell_state.lock().unwrap();
                    let text_edit = egui::TextEdit::singleline(&mut s.input_buffer)
                        .desired_width(ui.available_width())
                        .frame(false)
                        .text_color(egui::Color32::WHITE)
                        .lock_focus(true);

                    let re = ui.add(text_edit);
                    if current_mode == TerminalMode::Insert {
                        re.request_focus();
                    }
                });
            });

        ctx.request_repaint();
    }
}
