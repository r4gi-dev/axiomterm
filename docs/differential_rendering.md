# Differential Rendering Architecture

This document defines the architecture for the "Differential Rendering" phase, aiming to optimize performance by transitioning from a "redraw-all" model to an "apply-operation" model.

## 1. Core Philosophy

> **"The Shell emits Truth; The Renderer decides Representation."**

*   **Shell**: Responsible for semantic state (lines, cells, cursor). Emits atomic `ScreenOperation`s indicating *what* changed. It does NOT care about dirty rectangles or GPU buffers.
*   **Renderer**: Responsible for performance. Receives operations and decides the cheapest way to reflect that change on screen.

## 2. Operation Categories

To optimize rendering, we classify every `ScreenOperation` into a category that dictates the update cost and strategy.

### 2.1 Structural Operations
**Definition**: Changes that affect the layout flow or scroll position.
**Cost**: High (Requires recalculating available space, scrollbar thumb, text wrapping).
**Strategy**: `request_full_layout()` / `egui::Context::request_repaint()`.

*   `PushLine`: Adds a new line, shifting all previous lines up (scrolling).
*   `Clear`: Resets the entire view.
*   `Resize`: Changes available rows/cols.

### 2.2 Visual Operations (Future)
**Definition**: Changes to the content of existing cells without moving them.
**Cost**: Medium (Requires repainting specific text meshes, but no layout shift).
**Strategy**: Invalidate specific row region.

*   `UpdateCell(row, col, cell)`: Changes char or color at specific coordinate.
*   `InvertRange(start, end)`: For selection highlighting.

### 2.3 Cursor Operations
**Definition**: Movement of the cursor indicator.
**Cost**: Low.
**Strategy**: Render cursor on a separate overlay layer. Do not trigger text layout recalc.

*   `SetCursor(row, col)`: Updates cursor position.

## 3. Implementation Phases

### Phase 1: Taxonomy & Instrumentation (Current Scope)
1.  Define `OperationCategory` enum in `src/types.rs`.
2.  Implement `ScreenOperation::category()` method.
3.  Instrument `src/app.rs` to log which category triggered the frame.

### Phase 2: Structural Optimization
1.  Verify that `PushLine` triggers scroll/layout updates.
2.  Optimize the storage of `Screen` to avoid cloning `Vec<Line>` every frame. (Reference `Arc<Vec<Line>>` or jagged array).

### Phase 3: Visual Optimization (Fine-grained)
1.  Introduce standard "Dirty Rect" tracking in the Receiver.
2.  Use `egui`'s `Area` or fine-grained painting if possible.

The `Shell` guarantees that `ScreenOperation`s are atomic and sequential. The Renderer guarantees that applying these operations sequentially to a local cache will result in an identical state to the Shell's `Screen`.

## 5. Visual Partial Invalidation Architecture (Phase 4-6)

This phase aims to enable "Row-based Partial Invalidation" safely.

### 5.1 Operation Metadata & Line Impact

We extend `ScreenOperation` (or wrap it) to provide metadata about its scope.

```rust
pub enum LineImpact {
    Single(usize),      // Affects only one specific line index
    Multi(Vec<usize>),  // Affects specific multiple lines (rare)
    Unbounded,          // Might affect everything or cause shifts (Clear, Resize, Scroll)
}

pub struct OperationMetadata {
    pub impact: LineImpact,
    pub caused_scroll: bool,
}
```

### 5.2 Classification Strategy

| Operation | Impact Classification | Reasoning |
| :--- | :--- | :--- |
| `CharUpdate` / `ColorChange` | `Single(row)` | Content only. No layout shift. |
| `PushLine` | `Unbounded` | Causes scroll shift. All cache indices become invalid. |
| `Clear` | `Unbounded` | Everything changes. |
| `SetCursor` | `Single` (Cursor Layer) | Handled by Cursor Layer Optimization. |

### 5.3 Dirty Detection (Step 2)

The Renderer will maintain a `dirty_line_count` metric.
- On `Single(row)`: `dirty_line_count += 1`.
- On `Unbounded`: `dirty_line_count = SCREEN_HEIGHT` (effectively infinite).

### 5.4 LineRenderCache Concept (Step 3)

Instead of `Vec<egui::Shape>`, we move to:

```rust
struct LineRenderCache {
    line_index: usize,
    shapes: Vec<egui::Shape>,
    version: u64, // To track updates
}

// In App
screen_cache: Vec<Option<LineRenderCache>>, // sparse or full
```

### 5.5 Safety Nets (Trigger Full Repaint)

If any of these occur, drop all optimization and `request_repaint()`:
1.  `dirty_line_count > 1` (Initially, start with simplest case).
2.  Window resize.
3.  Font change.
4.  Mode change.
5.  `Unbounded` operation received.
