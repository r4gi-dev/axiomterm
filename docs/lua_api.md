# axiomterm Lua API Definition

This document freezes the Lua API surface for axiomterm. It defines what is permitted, what is deprecated, and strict prohibitions.

## 1. Official API Surface

The Lua environment is strictly sandboxed. Interaction with the host terminal occurs solely through the `config` table and `Action` strings.

### 1.1 Configuration Table (`config`)

The `config` table is the primary output of the Lua script.

| Field | Type | Description |
| :--- | :--- | :--- |
| `font_size` | `float` | Font size in points. |
| `window_background_opacity` | `float` | Window opacity (0.0 - 1.0). |
| `window_title` | `string` | Custom window title base. |
| `prompt` | `string` | Prompt string (e.g., "axiom> "). |
| `prompt_color` | `string` | Hex color code (e.g., "#FF0000"). |
| `text_color` | `string` | Default text color (Hex). |
| `directory_color` | `string` | Directory listing color (Hex). |
| `default_cwd` | `string` | Startup directory. |
| `keys` | `table` | **Deprecated**. Use `modes` instead. |
| `modes` | `table` | List of Mode Definitions. |

### 1.2 Mode Definition Structure

```lua
{
    name = "ModeName", -- String (e.g., "Normal", "Insert")
    bindings = {
        { key = "KeyName", action = "ActionString" },
        ...
    }
}
```

### 1.3 Action Strings (The "System Calls")

Lua triggers behavior by returning these strings in `bindings`.

*   **State Mutation**:
    *   `ChangeMode(ModeName)`: Switch to the specified mode.
    *   `Clear`: Clears the terminal screen buffer and resets cursor to (0,0).
        *   *Context*: UI Screen Only.
        *   *Config*: **No effect** (settings remain unchanged).
        *   *Side Effect*: Triggers a full structural repaint.
*   **Movement**:
    *   `MoveCursor(dRow, dCol)`: Move cursor relative to current position.
*   **Execution**:
    *   `RunCommand(cmd)`: Execute a shell command string.
    *   `RunCommand("config load")`: Reload configuration (Built-in shell command).
*   **Input**:
    *   `Submit`: Trigger command execution (Enter key).
    *   `Backspace`: Remove character.
    *   `InsertChar(X)`: Insert a single character (used in macros).

### 1.4 Macro System (Architecture 5.0)

Macros allow defining **composite actions** (sequences of actions) in Lua. They are invoked via key bindings and return a list of action strings.

#### Macro Definition

```lua
axiom.macros.macro_name = function()
    return {
        "InsertChar(H)",
        "InsertChar(e)",
        "Submit"
    }
end
```

#### Macro Invocation

In mode bindings, reference the macro name as the action:

```lua
{
    name = "Normal",
    bindings = {
        { key = "Ctrl+M", action = "macro_name" }
    }
}
```

#### Supported Action Strings in Macros

All action strings from section 1.3 are supported, plus:
*   `InsertChar(X)`: Insert character `X` (e.g., `InsertChar(A)`, `InsertChar(1)`)

#### Safety Limits

*   **Max Actions**: 100 actions per macro (enforced at runtime)
*   **No Recursion**: Macros cannot call other macros
*   **No State Access**: Macros are pure functions (no access to `ShellState`)

#### Example: Save and Exit

```lua
axiom.macros.save_and_exit = function()
    return {
        "RunCommand(save)",
        "ChangeMode(Normal)",
        "Clear"
    }
end
```

## 2. Deprecation Policy

*   **`config.keys`**: Replaced by `config.modes` with "Insert" mode bindings.
    *   *Status*: Supported for backward compatibility until v1.0.
    *   *Behavior*: Merged into `Insert` mode bindings if present.

## 3. Forbidden Actions (Explicit Prohibitions)

To maintain "The Strongest Terminal" philosophy (predictability, speed, security):

1.  **No OS APIs**: Lua MUST NOT have access to `os.execute`, `io.open`, or FFI; all file I/O is handled exclusively by the Host (Rust) during the configuration load phase.
2.  **No Event Loops**: Lua scripts MUST NOT define valid long-running loops or callbacks. Configuration is evaluated **once** per load.
3.  **No UI Drawing**: Lua cannot draw directly to the screen. It can only toggle settings that affect the Rust renderer.
4.  **No State Inspection**: Lua cannot read the current `ShellState` (e.g., "get current cursor position"). It relies on one-way data flow: Config -> Rust.

## 4. Future Extensions

*   **Variables in Prompt**: Future permission to inject dynamic variables (git branch, etc.) into `prompt` string via strict placeholders, NOT via Lua function callbacks.
