# Gemini Terminal

## Introduction
Gemini Terminal is a lightweight, high-performance standalone terminal emulator built with Rust. Designed with the UNIX philosophy in mind—"never interrupt thinking" and "never make the user wait"—it operates as an independent graphical window, offering smooth asynchronous command execution and flexible customization via Lua.

[日本語版のREADMEはこちら (Japanese version)](README.jp.md)

## Technologies Used
- **Language**: [Rust](https://www.rust-lang.org/) (Safety and ultra-fast performance)
- **GUI Framework**: [eframe / egui](https://github.com/emilk/egui) (Low-latency rendering via Immediate Mode GUI)
- **Lua Parser**: [full_moon](https://github.com/Hajime-S/full_moon) (AST analysis for clean, side-effect-free configuration loading)
- **Asynchronous Communication**: [crossbeam-channel](https://github.com/crossbeam-rs/crossbeam) (Smooth I/O management without blocking the UI thread)

## Features
- **Standalone UI**: An independent rendering engine not bound by OS standard consoles.
- **Inline Terminal Flow**: A seamless vertical CLI experience where the prompt and history are unified.
- **Lua-based Configuration**: Hot-reloadable settings for prompt text, colors, and window metadata.
- **Directory Display**: A dedicated status bar at the top showing the real-time working directory.
- **Async Execution**: Non-blocking execution for long-running commands (e.g., `ping`, `dir /s`).
- **Robust Argument Parsing**: A custom tokenizer that correctly handles single/double quotes and backslash escapes.

## What You Can Do
- **Advanced Customization**:
  - `config.font_size`: Set the terminal font size (e.g., `16.0`).
  - `config.window_background_opacity`: Set window transparency (e.g., `0.85`).
  - `config.prompt`: Change the shell prompt string.
  - `config.prompt_color`: Change the prompt color using HEX strings (e.g., `"#00FFFF"`).
  - `config.text_color`: Change the general output text color.
  - `config.window_title`: Set a custom application window title.
  - `config.default_cwd`: Set the starting directory (e.g., `"C:/"`).
  - `config.directory_color`: Set the color for directories in `ls` (e.g., `"#6496FF"`).
  - `config.keys`: Define custom shortcuts using a list of tables.
- **Example Config (Object Style)**:
  ```lua
  local config = {}
  config.font_size = 14.0
  config.window_background_opacity = 0.9
  config.prompt = "gemini> "
  config.keys = {
      { key = "h", cmd = "cd .." },
  }
  return config
  ```
  - Transparent execution of external commands.
  - Built-in management commands (`config load`, `ls`, `cd`, `echo`, `exit`).
- **Standard Config Path**:
  - The default configuration file is located at `%USERPROFILE%\.config\gemini\config.lua`.
  - Running `config load` without arguments will look for this file.
  - **Built-in Command Reference**:
  - `config load [path]`: Reloads the configuration from a file.
  - `ls [-a] [-l] [path]`: List directory contents with colorization.
  - `cd <path>`: Change the current working directory.
  - `pwd`: Print the current working directory.
  - `clear`: Clear the terminal history.
  - `mkdir <path>`: Create a new directory.
  - `touch <path>`: Create a new empty file.
  - `cat <path>`: Display file contents.
  - `rm <path>`: Remove a file or empty directory.
  - `mv <src> <dest>`: Rename or move a file/directory.
  - `cp <src> <dest>`: Copy a file.
  - `echo [text]`: Print text to the terminal.
  - `exit`: Close the terminal.
- **Optimized Experience**:
  - Immediate submission via Enter key.
  - Support for empty line submission to navigate the terminal naturally.

## Development Process
1. **Phase 1: Foundations**: Implemented a basic REPL loop and external process execution in Rust.
2. **Phase 2: Advanced Parsing**: Built a robust tokenizer for UNIX-like argument handling.
3. **Phase 3: Lua Integration**: Integrated `full_moon` for static AST-based configuration parsing.
4. **Phase 4: GUI Transition**: Transitioned to a full graphical UI using `eframe` with asynchronous worker threads.
5. **Phase 5: Polish**: Refined the inline input flow, implemented color support, and optimized for MSVC toolchain.

## How to Build

### Prerequisites
- [Rust (stable-x86_64-pc-windows-msvc)](https://www.rust-lang.org/tools/install)
- Visual Studio Build Tools (with "Desktop development with C++" workload)

### Build Steps
1. Navigate to the project directory.
2. Build the release binary:
   ```powershell
   cargo build --release
   ```
3. Find the executable at:
   ```text
   target/release/terminal.exe
   ```

### Running the App
```powershell
./target/release/terminal.exe
```
Place your `config.lua` in `%USERPROFILE%\.config\gemini\` and run `config load` inside the terminal to experience the customized settings.
