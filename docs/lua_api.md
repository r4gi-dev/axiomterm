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
    *   `Clear`: Clear config/screen state (context dependent).
*   **Movement**:
    *   `MoveCursor(dRow, dCol)`: Move cursor relative to current position.
*   **Execution**:
    *   `RunCommand(cmd)`: Execute a shell command string.
    *   `config load`: Reload configuration (aliased to RunCommand).
*   **Input**:
    *   `Submit`: Trigger command execution (Enter key).
    *   `Backspace`: Remove character.

## 2. Deprecation Policy

*   **`config.keys`**: Replaced by `config.modes` with "Insert" mode bindings.
    *   *Status*: Supported for backward compatibility until v1.0.
    *   *Behavior*: Merged into `Insert` mode bindings if present.

## 3. Forbidden Actions (Explicit Prohibitions)

To maintain "The Strongest Terminal" philosophy (predictability, speed, security):

1.  **No OS APIs**: Lua MUST NOT have access to `os.execute`, `io.open` (except read-only config), or FFI.
2.  **No Event Loops**: Lua scripts MUST NOT define valid long-running loops or callbacks. Configuration is evaluated **once** per load.
3.  **No UI Drawing**: Lua cannot draw directly to the screen. It can only toggle settings that affect the Rust renderer.
4.  **No State Inspection**: Lua cannot read the current `ShellState` (e.g., "get current cursor position"). It relies on one-way data flow: Config -> Rust.

## 4. Future Extensions

*   **Variables in Prompt**: Future permission to inject dynamic variables (git branch, etc.) into `prompt` string via strict placeholders, NOT via Lua function callbacks.
