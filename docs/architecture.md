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
