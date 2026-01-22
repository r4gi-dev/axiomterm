use eframe::egui;
use crate::types::{Action, InputEvent, ModeDefinition, TerminalMode};

pub fn poll_and_map(ctx: &egui::Context, current_mode: &TerminalMode, definitions: &[ModeDefinition]) -> Vec<Action> {
    let mut actions = Vec::new();
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

    // 2. Map InputEvents to Actions
    for event in events {
        if let Some(def) = definitions.iter().find(|d| d.mode == *current_mode) {
                    // matched = true; // Unused
                    break; 
            // If Text input in Insert mode and NOT matched by binding? 
            // TextEdit handles it directly, so we typically don't emit Action::AppendChar here 
            // unless we want to bypass TextEdit. Current architecture uses TextEdit.
        }
    }

    actions
}
