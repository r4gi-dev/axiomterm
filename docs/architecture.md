# axiomterm Architecture Definitions

This document defines the core contracts and lifecycles of the axiomterm architecture. It serves as the single source of truth for design decisions regarding Modes, Actions, and the integration of Lua.

## 1. Mode Lifecycle

Modes in axiomterm are state machines that dictate how User Inputs are translated into Actions. They are not merely "configurations" but active **Execution Units**.

### 1.1 State Transition Model

A Mode exists in one of four states:

1.  **Defined**: The Mode is parsed from `config.lua` or hardcoded in Rust. It exists in the `ShellState.mode_definitions` registry but is not currently influencing input.
2.  **Active**: The Mode is currently selected (`ShellState.mode` points to it). It intercepts all `InputEvent`s.
3.  **Inactive**: The Mode is defined but not selected. It holds no runtime state other than its definition.
4.  **Destroyed**: The Mode has been removed (e.g., deleted from `config.lua` and reloaded).

### 1.2 Reload Behavior

When `config.lua` is reloaded:

*   **Persistence**: If the currently **Active** Mode exists in the new configuration (matched by name), it remains Active. Its keybindings are updated in-place.
*   **Fallback**: If the currently **Active** Mode is *removed* in the new configuration:
    *   The shell MUST fallback to the `Normal` mode (if defined) or `Insert` mode (hard fallback).
    *   A notification MUST be displayed to the user: "Mode 'X' removed, falling back to 'Normal'".
*   **State Preservation**: Modes are stateless regarding input history. Switching modes or reloading configuration resets any transient input accumulation (though currently axiomterm InputEvents are immediate).

### 1.3 Semantic Role

*   **Mode as Execution Unit**: A Mode is a complete environment for input processing. It owns the keybinding table.
*   **Not Just Config**: A Mode can theoretically own local state (future extension: e.g., "Vim Visual Selection Range"), making it an instance of a running behavior, not just a static lookup table.

## 2. Action Boundaries

Actions are the **only** mechanism by which the ShellState is mutated. They define the boundary between Intent (Lua/User) and Execution (Rust).

### 2.1 Side Effects & Purity

*   **Pure Actions**: Most Actions (`MoveCursor`, `InsertChar`, `ChangeMode`) MUST be pure state mutations on `ShellState`. They enable perfect record/replay and testing.
*   **Impure Actions**: Actions involving I/O (`RunCommand`, `ReloadConfig`) are permitted but explicitly tagged.
    *   *Constraint*: Impure actions MUST NOT block the rendering thread. They are dispatched to the `Shell` worker thread.

### 2.2 ShellState Access Permissions

*   **Lua Generated Actions**: Actions generated via `config.lua` (e.g., `keys` table) operate with **User Privileges**.
    *   Can: Move cursor, Change mode, Run external commands, Insert text.
    *   Cannot: Modify `FixedConfig` (backend selection, security flags), Access raw OS window handles, Execute arbitrary unsafe Rust code.
*   **Internal Actions**: Native Rust actions have **System Privileges** (e.g., window resizing, process termination handling).

### 2.3 Lua Privileges

*   Lua is an **Action Generator**, not a State Mutator.
*   Lua cannot directly touch `ShellState`. It must return an `Action` string (e.g., "MoveCursor(0, 1)").
*   This indirection guarantees that all Lua behavior passes through the `Action` processing pipeline, ensuring validation and preventing undefined states.

## 3. Lua API Contract (Draft)

(Detailed API surface will be defined in `docs/lua_api.md`)

*   **Principle**: "If it's not an Action, Lua can't do it."

---

## 4. Undo/Redo Architecture (Architecture 6.0)

### 4.1 Purpose

Undo/Redo is **proof of state machine completeness**, not a convenience feature.

**Guarantees**:
- State transitions are **reversible**
- Macro/Action/ScreenOperation hierarchy remains **intact**
- Truth Layer (Rust) remains **unpolluted**

### 4.2 Core Principles

#### Undo Reverses Transitions, Not State

Undo does **not** save/restore Screen snapshots. Only **changes** are recorded.

**Forbidden**:
- Screen snapshots
- Renderer state
- Lua state

**Allowed**:
- State transitions
- Operation sequences
- Intent-preserving units

#### Undo is Truth Layer Responsibility

- Undo/Redo completes **within ShellState**
- Lua/Renderer/Adapter are **unaware** of Undo
- Lua **cannot** invoke Undo directly

### 4.3 Undo Granularity

**Undo unit = Action Transaction**

| Event | Transaction Scope |
|-------|-------------------|
| Normal key input | 1 Action = 1 Transaction |
| **Macro execution** | **Entire Macro = 1 Transaction** |
| External process output | **Not undoable** |
| Config reload | **Not undoable** |

### 4.4 Reversible Operations

All `ScreenOperation`s must have logical inverse pairs:

```
PushLine ⇄ PopLine
UpdateLine(old → new) ⇄ UpdateLine(new → old)
CursorMove(from → to) ⇄ CursorMove(to → from)
```

**Responsibility**: `ShellState`, not `Renderer`

### 4.5 Macro Metrics and Undo

**Macro Metrics** is a **design observation structure** and does **not** affect macro execution semantics.

- Metrics record macro invocations and action counts
- Undo/Redo does **not** generate Metrics
- Metrics does **not** affect Undo/Redo

**Reference**: See [undo_architecture.md](undo_architecture.md) for complete specification.
