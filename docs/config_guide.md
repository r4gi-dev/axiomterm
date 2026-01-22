# config.lua 設定ガイド

axiomterm の設定は Lua スクリプト (`config.lua`) で行います。このファイルは起動時に自動的に読み込まれ、ターミナルの外観・動作・キーバインディングを制御します。

## 設定ファイルの場所

デフォルトの設定ファイルパス:
- **Windows**: `%APPDATA%\axiomterm\config.lua`
- **macOS/Linux**: `~/.config/axiomterm/config.lua`

## 基本構造

```lua
-- 外観設定
axiomterm_prompt = "axiom> "
axiomterm_prompt_color = "#00FF00"
axiomterm_text_color = "#CCCCCC"
window_background_opacity = 0.95
font_size = 14.0
window_title = "axiomterm"
directory_color = "#6495ED"
default_cwd = "C:/Users/YourName"

-- モード定義
axiomterm_modes = {
    {
        name = "Insert",
        bindings = {
            { key = "Enter", action = "Submit" },
            { key = "Escape", action = "ChangeMode(Normal)" },
        }
    },
    {
        name = "Normal",
        bindings = {
            { key = "i", action = "ChangeMode(Insert)" },
            { key = "Escape", action = "Clear" },
        }
    }
}

-- マクロ定義（オプション）
axiom.macros.save_and_exit = function()
    return { "Submit", "ChangeMode(Normal)" }
end
```

---

## 設定項目リファレンス

### 外観設定

| 項目 | 型 | 説明 | デフォルト値 |
|------|-----|------|-------------|
| `axiomterm_prompt` | `string` | プロンプト文字列 | `"> "` |
| `axiomterm_prompt_color` | `string` | プロンプトの色（16進数） | `"#00FF00"` |
| `axiomterm_text_color` | `string` | テキストの色（16進数） | `"#D3D3D3"` |
| `directory_color` | `string` | ディレクトリ表示の色 | `"#6495ED"` |
| `window_background_opacity` | `number` | 背景の不透明度（0.0～1.0） | `0.95` |
| `font_size` | `number` | フォントサイズ（pt） | `14.0` |
| `window_title` | `string` | ウィンドウタイトル | `"axiomterm"` |
| `default_cwd` | `string` | 起動時のディレクトリ | カレントディレクトリ |

### 色指定フォーマット

色は `"#RRGGBB"` 形式の16進数文字列で指定します。

**例**:
```lua
axiomterm_prompt_color = "#FF0000"  -- 赤
axiomterm_text_color = "#00FF00"    -- 緑
directory_color = "#0000FF"         -- 青
```

---

## モード定義

axiomterm は **モーダル編集** をサポートしています。各モードごとに異なるキーバインディングを定義できます。

### モード構造

```lua
axiomterm_modes = {
    {
        name = "ModeName",
        bindings = {
            { key = "KeyName", action = "ActionName" },
            -- または
            { key = "Ctrl+C", action = "MacroName" },
        }
    }
}
```

### 標準モード

- **Insert**: テキスト入力モード（デフォルト）
- **Normal**: コマンドモード（vim風）
- **Visual**: 選択モード（将来実装予定）

### キー指定フォーマット

| 形式 | 例 |
|------|-----|
| 単一キー | `"i"`, `"Escape"`, `"Enter"` |
| 修飾キー付き | `"Ctrl+C"`, `"Alt+F4"`, `"Shift+Tab"` |

**注意**: 修飾キーは大文字小文字を区別しません（`ctrl+c` でも可）。

---

## アクション一覧

### 基本アクション

| アクション | 説明 |
|-----------|------|
| `Submit` | 現在の入力を実行 |
| `Backspace` | 1文字削除 |
| `Delete` | カーソル位置の文字を削除 |
| `Clear` | 画面をクリア |
| `NoOp` | 何もしない |

### モード切り替え

```lua
{ key = "Escape", action = "ChangeMode(Normal)" }
{ key = "i", action = "ChangeMode(Insert)" }
```

### コマンド実行

```lua
{ key = "Ctrl+L", action = "RunCommand(cls)" }
```

### マクロ呼び出し

```lua
{ key = "Ctrl+S", action = "save_macro" }
```

---

## マクロ機能

マクロは **複数のアクションを順次実行** する機能です。Lua 関数として定義します。

### マクロの定義

```lua
axiom.macros.macro_name = function()
    return {
        "InsertChar(H)",
        "InsertChar(e)",
        "InsertChar(l)",
        "InsertChar(l)",
        "InsertChar(o)",
        "Submit"
    }
end
```

### マクロで使用可能なアクション文字列

| 形式 | 例 |
|------|-----|
| 基本アクション | `"Submit"`, `"Clear"`, `"Backspace"` |
| 文字挿入 | `"InsertChar(A)"`, `"InsertChar(1)"` |
| モード変更 | `"ChangeMode(Normal)"` |
| コマンド実行 | `"RunCommand(echo hello)"` |

### マクロの制限

- **最大アクション数**: 100個まで
- **再帰呼び出し**: 禁止
- **State アクセス**: 不可（純粋関数のみ）

### 実用例

#### 例1: 保存して終了

```lua
axiom.macros.save_and_exit = function()
    return {
        "RunCommand(save)",
        "ChangeMode(Normal)",
        "Clear"
    }
end

-- キーバインディング
axiomterm_modes = {
    {
        name = "Normal",
        bindings = {
            { key = "Ctrl+S", action = "save_and_exit" }
        }
    }
}
```

#### 例2: よく使うコマンドのショートカット

```lua
axiom.macros.git_status = function()
    return { "RunCommand(git status)" }
end

axiom.macros.clear_and_ls = function()
    return {
        "Clear",
        "RunCommand(ls -la)"
    }
end
```

---

## 設定例

### ミニマル設定

```lua
axiomterm_prompt = "> "
axiomterm_prompt_color = "#00FF00"
font_size = 16.0
```

### フル機能設定

```lua
-- 外観
axiomterm_prompt = "λ "
axiomterm_prompt_color = "#FFD700"
axiomterm_text_color = "#E0E0E0"
directory_color = "#87CEEB"
window_background_opacity = 0.92
font_size = 14.0
window_title = "axiomterm - Terminal Emulator"
default_cwd = "C:/Projects"

-- モード定義
axiomterm_modes = {
    {
        name = "Insert",
        bindings = {
            { key = "Enter", action = "Submit" },
            { key = "Escape", action = "ChangeMode(Normal)" },
            { key = "Ctrl+C", action = "Clear" },
        }
    },
    {
        name = "Normal",
        bindings = {
            { key = "i", action = "ChangeMode(Insert)" },
            { key = "Escape", action = "Clear" },
            { key = "Ctrl+L", action = "RunCommand(cls)" },
            { key = "g", action = "git_status" },
        }
    }
}

-- マクロ
axiom.macros.git_status = function()
    return { "RunCommand(git status)" }
end

axiom.macros.quick_commit = function()
    return {
        "RunCommand(git add .)",
        "RunCommand(git commit -m 'quick commit')"
    }
end
```

---

## トラブルシューティング

### 設定が反映されない

1. ファイルパスが正しいか確認
2. Lua 構文エラーがないか確認（起動時のエラーメッセージを確認）
3. axiomterm を再起動

### 色が表示されない

- 色指定が `"#RRGGBB"` 形式になっているか確認
- 不透明度が `0.0` になっていないか確認

### キーバインディングが動作しない

- キー名が正しいか確認（`"Enter"`, `"Escape"` など大文字小文字に注意）
- モード名が正しいか確認
- アクション名が正しいか確認

### マクロが実行されない

- `axiom.macros.macro_name` の形式で定義されているか確認
- 返り値が文字列のテーブル（配列）になっているか確認
- アクション数が100個を超えていないか確認

---

## 参考資料

- [Lua API リファレンス](lua_api.md)
- [アーキテクチャドキュメント](architecture.md)
