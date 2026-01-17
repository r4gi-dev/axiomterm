use crate::shell::spawn_shell_thread;
use crate::types::{LogLine, ShellEvent, ShellState, TerminalMode};
use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui;
use std::env;
use std::sync::{Arc, Mutex};

pub struct TerminalApp {
    pub input: String,
    pub history: Vec<LogLine>,
    pub shell_state: Arc<Mutex<ShellState>>,
    pub command_tx: Sender<String>,
    pub output_rx: Receiver<ShellEvent>,
}

impl TerminalApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (command_tx, command_rx) = unbounded::<String>();
        let (output_tx, output_rx) = unbounded::<ShellEvent>();

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

        spawn_shell_thread(command_rx, output_tx, Arc::clone(&state));

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
        while let Ok(event) = self.output_rx.try_recv() {
            match event {
                ShellEvent::Output(line) => self.history.push(line),
                ShellEvent::Clear => self.history.clear(),
            }
        }

        // Global Key Intercept
        let (current_mode, shortcuts, opacity, font_size, current_dir) = {
            let s = self.shell_state.lock().unwrap();
            (
                s.mode.clone(),
                s.shortcuts.clone(),
                s.opacity,
                s.font_size,
                s.current_dir.clone(),
            )
        };

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            let mut s = self.shell_state.lock().unwrap();
            s.mode = if s.mode == TerminalMode::Insert {
                TerminalMode::Normal
            } else {
                TerminalMode::Insert
            };
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
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_black_alpha(200))
                    .inner_margin(4.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("PWD:").color(egui::Color32::GRAY));
                    ui.label(
                        egui::RichText::new(current_dir)
                            .color(egui::Color32::from_rgb(100, 200, 255)),
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
                            ui.label(
                                egui::RichText::new(prompt_text)
                                    .color(prompt_color)
                                    .strong(),
                            );

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
