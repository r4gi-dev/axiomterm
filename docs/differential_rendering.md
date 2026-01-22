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

## 4. Contract with `Shell`

The `Shell` guarantees that `ScreenOperation`s are atomic and sequential. The Renderer guarantees that applying these operations sequentially to a local cache will result in an identical state to the Shell's `Screen`.
