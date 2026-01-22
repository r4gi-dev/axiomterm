use eframe::egui;
use crate::types::{Action, BindingTarget, InputEvent, ModeDefinition, TerminalMode};

pub fn poll_and_map(ctx: &egui::Context, current_mode: &TerminalMode, definitions: &[ModeDefinition]) -> Vec<BindingTarget> {
    let mut targets = Vec::new();
    let mut events = Vec::new();

    // 1. Capture raw egui events and convert to InputEvents
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

    // 2. Map InputEvents to BindingTargets
    for event in events {
        if let Some(def) = definitions.iter().find(|d| d.mode == *current_mode) {
            for binding in &def.bindings {
                if binding.event == event {
                    // Prevent duplicate processing in Insert mode where TextEdit is active
                    if *current_mode == TerminalMode::Insert {
                        match &binding.target {
                            BindingTarget::Action(action) => {
                                match action {
                                    Action::Backspace | Action::Delete | Action::MoveCursor(_, _) => {
                                        // These are handled by TextEdit
                                    },
                                    _ => {
                                        targets.push(binding.target.clone());
                                    }
                                }
                            },
                            BindingTarget::Macro(_) => {
                                // Macros are always allowed in Insert mode (for now)
                                targets.push(binding.target.clone());
                            }
                        }
                    } else {
                        targets.push(binding.target.clone());
                    }
                    break; 
                }
            }
        }
    }

    targets
}
