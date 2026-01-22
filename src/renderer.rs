use eframe::egui;
use crate::types::{ScreenOperation, LineImpact, ShellState};

pub struct LineRenderCache {
    #[allow(dead_code)]
    pub line_index: usize,
    pub shapes: Vec<egui::Shape>,
}

#[derive(Default, Debug)]
pub struct RenderMetrics {
    pub structural_ops: usize,
    pub visual_ops: usize,
    pub cursor_ops: usize,
    pub dirty_line_count: usize,
}

pub struct TerminalRenderer {
    pub metrics: RenderMetrics,
    pub screen_cache: Vec<Option<LineRenderCache>>,
    pub last_render_dims: (f32, f32),
    pub cached_origin: egui::Pos2,
    pub cursor_optimization_mode: bool,
}

impl Default for TerminalRenderer {
    fn default() -> Self {
        Self {
            metrics: RenderMetrics::default(),
            screen_cache: Vec::new(),
            last_render_dims: (0.0, 0.0),
            cached_origin: egui::pos2(0.0, 0.0),
            cursor_optimization_mode: true,
        }
    }
}

impl TerminalRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_structural_change(&mut self, ctx: &egui::Context) {
        self.metrics.structural_ops += 1;
        self.screen_cache.clear();
        self.metrics.dirty_line_count = usize::MAX;
        
        println!("DEBUG: [Structural] Re-layout triggered. Metrics: {:?}", self.metrics);
        ctx.request_repaint();
    }

    pub fn on_visual_change(&mut self, ctx: &egui::Context, op: &ScreenOperation) {
        self.metrics.visual_ops += 1;
        
        // Dirty Line Detection
        let metadata = op.metadata();
        match metadata.impact {
            LineImpact::Single(_) => {
                if self.metrics.dirty_line_count != usize::MAX {
                    self.metrics.dirty_line_count += 1;
                }
            }
            LineImpact::Multi(ref rows) => {
                if self.metrics.dirty_line_count != usize::MAX {
                    self.metrics.dirty_line_count += rows.len();
                }
            }
            LineImpact::Unbounded => {
                self.metrics.dirty_line_count = usize::MAX;
            }
        }

        // Optimization: Single Line Invalidation
        if self.metrics.dirty_line_count == 1 {
            if let LineImpact::Single(row) = metadata.impact {
                if row < self.screen_cache.len() {
                    println!("DEBUG: [Visual] Optimized: Invalidating only row {}", row);
                    self.screen_cache[row] = None;
                } else {
                     self.screen_cache.clear();
                }
            } else {
                 self.screen_cache.clear();
            }
        } else {
            self.screen_cache.clear();
        }

        println!("DEBUG: [Visual] Paint update. Impact: {:?}, Metrics: {:?}", metadata.impact, self.metrics);
        ctx.request_repaint();
    }

    pub fn on_cursor_change(&mut self, ctx: &egui::Context) {
        self.metrics.cursor_ops += 1;
        println!("DEBUG: [Cursor] Cursor update. Total: {}", self.metrics.cursor_ops);
        ctx.request_repaint();
    }

    // This method encapsulates the main rendering loop
    pub fn draw(&mut self, ui: &mut egui::Ui, state: &ShellState) {
         let font_size = state.font_size;
         let lines = &state.screen.lines;
         let cursor = &state.screen.cursor;
         
         // Visual style override
         ui.style_mut().visuals.extreme_bg_color = egui::Color32::BLACK;
         ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::BLACK;

         // Safety Net: Check for window size change
         let curr_dims = (ui.available_width(), ui.available_height());
         if curr_dims != self.last_render_dims {
             self.screen_cache.clear();
             self.last_render_dims = curr_dims;
         }

         if !self.cursor_optimization_mode {
             self.screen_cache.clear();
         }

         // Resize cache vector if lines changed
         if self.screen_cache.len() != lines.len() {
             self.screen_cache.resize_with(lines.len(), || None);
         }

         egui::ScrollArea::vertical()
             .auto_shrink([false; 2])
             .stick_to_bottom(true)
             .show(ui, |ui| {
                 let font_id = egui::FontId::monospace(font_size);
                 
                 // 1. Calculate metrics
                 let (row_height, char_width) = {
                     let painter = ui.painter();
                     let char_dims = painter.layout_no_wrap("A".to_string(), font_id.clone(), egui::Color32::WHITE).size();
                     (char_dims.y, char_dims.x)
                 };

                 // 2. Check Safety Nets (Origin/Scroll)
                 let curr_origin = ui.cursor().min;
                 if curr_origin != self.cached_origin {
                      self.screen_cache.clear();
                      self.screen_cache.resize_with(lines.len(), || None);
                      self.cached_origin = curr_origin;
                 }

                 // 3. Rebuild Cache (Row-based)
                 let start_y = ui.cursor().min.y;
                 
                 for (i, line) in lines.iter().enumerate() {
                     if self.screen_cache[i].is_none() {
                         let painter = ui.painter();
                         let mut shapes = Vec::new();
                         let y = start_y + (i as f32 * row_height);
                         let mut x = ui.cursor().min.x;

                         for cell in &line.cells {
                             let color = egui::Color32::from(cell.fg);
                             let galley = painter.layout_no_wrap(cell.ch.to_string(), font_id.clone(), color);
                             let rect = egui::Rect::from_min_size(egui::pos2(x, y), galley.size());
                             
                             shapes.push(egui::Shape::galley(rect.min, galley, color));
                             x += rect.width();
                         }
                         self.screen_cache[i] = Some(LineRenderCache {
                             line_index: i,
                             shapes,
                         });
                     }
                 }

                 // 4. Draw Cache
                 let painter = ui.painter();
                 for cache_opt in &self.screen_cache {
                     if let Some(cache) = cache_opt {
                         painter.extend(cache.shapes.iter().cloned());
                     }
                 }

                 // 5. Allocate Space
                 let (_id, allocated_rect) = ui.allocate_space(egui::vec2(ui.available_width(), row_height * lines.len() as f32));
                 
                 // 6. Draw Cursor Layer
                 let cursor_rect = egui::Rect::from_min_size(
                     egui::pos2(
                         allocated_rect.min.x + cursor.col as f32 * char_width,
                         allocated_rect.min.y + cursor.row as f32 * row_height
                     ),
                     egui::vec2(char_width, row_height)
                 );
                 ui.painter().rect_filled(cursor_rect, 0.0, egui::Color32::from_white_alpha(100)); // Semi-transparent cursor
                 
                 // Prompt drawing is handled by caller or we can move it here too?
                 // Caller handles prompt input line for now as it contains TextEdit logic.
             });
             
         self.metrics.dirty_line_count = 0;
    }
}
