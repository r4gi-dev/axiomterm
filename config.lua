local config = {}

config.font_size = 16.0
config.window_background_opacity = 0.8
config.window_title = "Gemini Wez-Edition"
config.prompt = "wez-shell> "
config.prompt_color = "#FF8800" -- Orange
config.directory_color = "#6496FF" -- Sky Blue
config.default_cwd = "C:/" -- Set your default starting directory here

-- Keybinds (using our internal 'keys' mapping)
config.keys = {
    { key = "h", cmd = "cd .." },
    { key = "j", cmd = "echo 'moving down...'" },
    { key = "k", cmd = "echo 'moving up...'" },
    { key = "q", cmd = "exit" }, -- Leader-like quit
}

return config
