use crate::shell::spawn_shell_thread;
use crate::types::{Action, InputEvent, KeyBinding, ModeDefinition, ShellState, TerminalMode, Screen, ShellEvent, TerminalColor};
use crate::backend::ProcessBackend;
use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui;
use std::env;
use std::sync::{Arc, Mutex};

pub struct TerminalApp {
    pub shell_state: Arc<Mutex<ShellState>>,
    pub action_tx: Sender<Action>,
    pub output_rx: Receiver<ShellEvent>,
}

impl TerminalApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, backend: Box<dyn ProcessBackend>) -> Self {
        let (action_tx, action_rx) = unbounded::<Action>();
        let (output_tx, output_rx) = unbounded::<ShellEvent>();

        let current_dir = env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        let state = Arc::new(Mutex::new(ShellState {
            prompt: "> ".to_string(),
            prompt_color: TerminalColor::GREEN,
            text_color: TerminalColor::LIGHT_GRAY,
            window_title_base: "Gemini Terminal".to_string(),
            window_title_full: "[INSERT] Gemini Terminal".to_string(),
            title_updated: false,
            mode: TerminalMode::Insert,
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
        }
    }

    fn map_input(&self, event: &InputEvent, mode: &TerminalMode) -> Option<Action> {
        let s = self.shell_state.lock().unwrap();
        
        // Find definition for current mode
        if let Some(def) = s.mode_definitions.iter().find(|d| d.mode == *mode) {
            for binding in &def.bindings {
                if binding.event == *event {
                    return Some(binding.action.clone());
                }
            }
        }

        // Fallback or Insert mode text handling
        if *mode == TerminalMode::Insert {
            if let InputEvent::Text(s) = event {
                if let Some(ch) = s.chars().next() {
                    return Some(Action::AppendChar(ch));
                }
            }
        }

        None
    }
}

impl From<TerminalColor> for egui::Color32 {
    fn from(c: TerminalColor) -> Self {
        egui::Color32::from_rgb(c.r, c.g, c.b)
    }
}

impl eframe::App for TerminalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for new events (Operations are the primary driver of state changes)
        while let Ok(event) = self.output_rx.try_recv() {
            match event {
                ShellEvent::Operation(_op) => {
                    // In a more advanced renderer, we would use _op to do partial invalidation.
                    // For now, receiving an operation just triggers a natural repaint.
                    ctx.request_repaint();
                }
                ShellEvent::Notification(msg) => {
                    println!("Notification: {}", msg);
                }
            }
        }

        // Fetch state for interpretation and rendering
        let (current_mode, _shortcuts, opacity, font_size, current_dir, text_color, dir_color) = {
            let s = self.shell_state.lock().unwrap();
            (
                s.mode.clone(),
                s.shortcuts.clone(),
                s.opacity,
                s.font_size,
                s.current_dir.clone(),
                s.text_color,
                s.directory_color,
            )
        };

        // Capture and process InputEvents
        let mut events = Vec::new();
        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        events.push(InputEvent::Key {
                            code: format!("{:?}", key),
                            ctrl: modifiers.command, // command maps to ctrl on Windows/Linux, cmd on Mac
                            alt: modifiers.alt,
                            shift: modifiers.shift,
                        });
                    }
                    egui::Event::Text(text) => {
                        if !text.is_empty() {
                            events.push(InputEvent::Text(text.clone()));
                        }
                    }
                    _ => {}
                }
            }
        });

        for event in events {
            if let Some(action) = self.map_input(&event, &current_mode) {
                let _ = self.action_tx.send(action);
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
                ui.style_mut().visuals.extreme_bg_color = egui::Color32::BLACK;
                ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::BLACK;

                let (prompt_text, prompt_color, mode, lines) = {
                    let s = self.shell_state.lock().unwrap();
                    (
                        s.prompt.clone(),
                        s.prompt_color,
                        s.mode.clone(),
                        s.screen.lines.clone(),
                    )
                };

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        // History (Screen Lines)
                        for line in &lines {
                            ui.horizontal(|ui| {
                                ui.style_mut().spacing.item_spacing.x = 0.0;
                                for cell in &line.cells {
                                    ui.label(
                                        egui::RichText::new(cell.ch.to_string())
                                            .color(egui::Color32::from(cell.fg)),
                                    );
                                }
                            });
                        }

                        // Current Prompt/Input Line
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(prompt_text)
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
                            // We don't need to manually send enter here anymore as it's handled by map_input -> Action::Submit
                            if mode == TerminalMode::Insert {
                                re.request_focus();
                            }
                        });
                    });
            });

        ctx.request_repaint();
    }
}
