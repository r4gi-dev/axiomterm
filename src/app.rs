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

pub struct TerminalApp {
    pub shell_state: Arc<Mutex<ShellState>>,
    pub action_tx: Sender<Action>,
    pub output_rx: Receiver<ShellEvent>,
    pub _watcher: Option<RecommendedWatcher>,
    pub config_rx: Receiver<()>,
    pub last_reload: Instant,
    pub metrics: RenderMetrics,
    pub cursor_optimization_mode: bool,
    pub screen_cache: Option<Vec<egui::Shape>>,
    pub last_render_dims: (f32, f32), // Width, Height
    pub cached_origin: egui::Pos2,
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
            window_title_full: "[INSERT] axiomterm".to_string(),
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
            _watcher: watcher,
            config_rx,
            last_reload: Instant::now(),
            metrics: RenderMetrics::default(),
            cursor_optimization_mode: true,
            screen_cache: None,
            last_render_dims: (0.0, 0.0),
            cached_origin: egui::pos2(0.0, 0.0),
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

#[derive(Default, Debug)]
pub struct RenderMetrics {
    pub structural_ops: usize,
    pub visual_ops: usize,
    pub cursor_ops: usize,
}

impl TerminalApp {
    fn on_structural_change(&mut self, ctx: &egui::Context, _op: &ScreenOperation) {
        self.metrics.structural_ops += 1;
        // Invalidate cache on structural changes
        self.screen_cache = None;
        println!("DEBUG: [Structural] Re-layout triggered. Total: {}", self.metrics.structural_ops);
        // Structural changes require full repaint for now
        ctx.request_repaint();
    }

    fn on_visual_change(&mut self, ctx: &egui::Context, _op: &ScreenOperation) {
        self.metrics.visual_ops += 1;
        // Invalidate cache on visual changes
        self.screen_cache = None;
        println!("DEBUG: [Visual] Paint update. Total: {}", self.metrics.visual_ops);
        // Visual changes currently trigger full repaint (optimization pending)
        ctx.request_repaint();
    }

    fn on_cursor_change(&mut self, ctx: &egui::Context, _op: &ScreenOperation) {
        self.metrics.cursor_ops += 1;
        println!("DEBUG: [Cursor] Cursor update. Total: {}", self.metrics.cursor_ops);
        // Cursor changes currently trigger full repaint (optimization pending)
        ctx.request_repaint();
    }
}

impl eframe::App for TerminalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for new events (Operations are the primary driver of state changes)
        // Check for config file changes
        if let Ok(_) = self.config_rx.try_recv() {
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

                // Safety Net: Check for window size change
                let curr_dims = (ui.available_width(), ui.available_height());
                if curr_dims != self.last_render_dims {
                    self.screen_cache = None;
                    self.last_render_dims = curr_dims;
                }

                // Temporary: Enforce no optimization until fully ready
                // self.cursor_optimization_mode = true; // Uncomment to enable
                if !self.cursor_optimization_mode {
                    self.screen_cache = None;
                }

                let (prompt_text, prompt_color, mode, lines, cursor) = {
                    let s = self.shell_state.lock().unwrap();
                    (
                        s.prompt.clone(),
                        s.prompt_color,
                        s.mode.clone(),
                        s.screen.lines.clone(),
                        s.screen.cursor,
                    )
                };

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        // History (Screen Lines)
                        let font_id = egui::FontId::monospace(font_size);
                        
                        // 1. Calculate metrics (Scope painter drop)
                        let (row_height, char_width) = {
                            let painter = ui.painter();
                            let char_dims = painter.layout_no_wrap("A".to_string(), font_id.clone(), egui::Color32::WHITE).size();
                            (char_dims.y, char_dims.x)
                        };

                        // 2. Check Safety Nets (Origin/Scroll)
                        let curr_origin = ui.cursor().min;
                        if curr_origin != self.cached_origin {
                             self.screen_cache = None;
                             self.cached_origin = curr_origin;
                        }

                        // 3. Rebuild Cache if needed
                        if self.screen_cache.is_none() {
                            let painter = ui.painter();
                            let mut shapes = Vec::new();
                            let mut y = ui.cursor().min.y;

                             for line in &lines {
                                let mut x = ui.cursor().min.x;
                                for cell in &line.cells {
                                    let color = egui::Color32::from(cell.fg);
                                    let galley = painter.layout_no_wrap(cell.ch.to_string(), font_id.clone(), color);
                                    let rect = egui::Rect::from_min_size(egui::pos2(x, y), galley.size());
                                    
                                    shapes.push(egui::Shape::galley(rect.min, galley, color));
                                    x += rect.width();
                                }
                                y += row_height;
                            }
                            self.screen_cache = Some(shapes);
                        }

                        // 4. Draw Cache
                        if let Some(shapes) = &self.screen_cache {
                            ui.painter().extend(shapes.iter().cloned());
                        }

                        // 5. Allocate Space (Mutable borrow)
                        ui.allocate_space(egui::vec2(ui.available_width(), row_height * lines.len() as f32));
                        
                        // 6. Draw Cursor Layer
                        let cursor_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                ui.cursor().min.x + cursor.col as f32 * char_width,
                                ui.cursor().min.y + cursor.row as f32 * row_height
                            ),
                            egui::vec2(char_width, row_height)
                        );
                        ui.painter().rect_filled(cursor_rect, 0.0, egui::Color32::from_white_alpha(100)); // Semi-transparent cursor

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
