# axiomterm 環境設定ガイド

このドキュメントでは、axiomterm のインストール、設定ファイルの配置、環境変数の設定など、ターミナル単位での環境構築方法を説明します。

## インストール

### ビルド済みバイナリの使用

1. リリースページから最新版をダウンロード
2. 任意のディレクトリに展開
3. PATH に追加（推奨）

### ソースからビルド

```bash
# リポジトリをクローン
git clone https://github.com/yourusername/axiomterm.git
cd axiomterm

# ビルド
cargo build --release

# バイナリは target/release/axiomterm.exe に生成されます
```

---

## 設定ファイルの配置

### デフォルト設定ファイルパス

axiomterm は起動時に以下の場所から `config.lua` を読み込みます:

| OS | パス |
|----|------|
| **Windows** | `%APPDATA%\axiomterm\config.lua` |
| **macOS** | `~/.config/axiomterm/config.lua` |
| **Linux** | `~/.config/axiomterm/config.lua` |

### 実際のパス例

**Windows**:
```
C:\Users\YourName\AppData\Roaming\axiomterm\config.lua
```

**macOS/Linux**:
```
/home/username/.config/axiomterm/config.lua
```

### 初回セットアップ

1. **ディレクトリを作成**:
   ```bash
   # Windows (PowerShell)
   New-Item -ItemType Directory -Path "$env:APPDATA\axiomterm"
   
   # macOS/Linux
   mkdir -p ~/.config/axiomterm
   ```

2. **設定ファイルを作成**:
   ```bash
   # Windows (PowerShell)
   New-Item -ItemType File -Path "$env:APPDATA\axiomterm\config.lua"
   
   # macOS/Linux
   touch ~/.config/axiomterm/config.lua
   ```

3. **設定を記述** (詳細は [config_guide.md](config_guide.md) を参照)

---

## 環境変数の設定

### PATH への追加

axiomterm をどこからでも起動できるようにするため、PATH に追加することを推奨します。

#### Windows

**方法1: システム設定から追加**
1. `Win + X` → 「システム」→ 「システムの詳細設定」
2. 「環境変数」ボタンをクリック
3. 「Path」を選択 → 「編集」
4. axiomterm.exe があるディレクトリを追加

**方法2: PowerShell で追加（ユーザー環境変数）**
```powershell
$env:Path += ";C:\path\to\axiomterm"
[Environment]::SetEnvironmentVariable("Path", $env:Path, "User")
```

#### macOS/Linux

`~/.bashrc` または `~/.zshrc` に追加:
```bash
export PATH="$PATH:/path/to/axiomterm"
```

変更を反映:
```bash
source ~/.bashrc  # または source ~/.zshrc
```

---

## ターミナル単位の設定

### デフォルトシェルの変更

axiomterm は現在、以下のバックエンドをサポートしています:

- **Windows**: `cmd.exe`, `powershell.exe`
- **macOS/Linux**: `bash`, `zsh`, `sh`

デフォルトシェルは自動検出されますが、`config.lua` で明示的に指定することも可能です（将来実装予定）。

### 起動ディレクトリの設定

`config.lua` で起動時のディレクトリを指定できます:

```lua
default_cwd = "C:/Projects"  -- Windows
-- または
default_cwd = "/home/user/projects"  -- macOS/Linux
```

### フォント設定

現在、axiomterm は **等幅フォント** を使用します。システムのデフォルト等幅フォントが自動的に選択されます。

フォントサイズは `config.lua` で変更可能:
```lua
font_size = 16.0  -- ポイント単位
```

---

## 設定のリロード

axiomterm は設定ファイルの変更を自動的に検出し、リロードします。

### リロードのタイミング

- `config.lua` が保存されると、約1秒以内に自動的に反映されます
- 再起動は不要です

### リロード対象

以下の設定は即座に反映されます:
- プロンプト文字列・色
- テキスト色
- ウィンドウタイトル
- 不透明度
- フォントサイズ
- モード定義
- キーバインディング
- マクロ定義

---

## 複数プロファイルの管理

### プロファイルの切り替え（手動）

異なる設定を使い分けたい場合、複数の設定ファイルを用意し、起動時に切り替えることができます（将来実装予定）。

**現在の回避策**:
1. 複数の設定ファイルを作成（例: `config_work.lua`, `config_home.lua`）
2. 使用したい設定を `config.lua` にコピー

```bash
# Windows (PowerShell)
Copy-Item "$env:APPDATA\axiomterm\config_work.lua" "$env:APPDATA\axiomterm\config.lua"

# macOS/Linux
cp ~/.config/axiomterm/config_work.lua ~/.config/axiomterm/config.lua
```

---

## トラブルシューティング

### 設定ファイルが読み込まれない

**確認事項**:
1. ファイルパスが正しいか
   ```bash
   # Windows (PowerShell)
   Test-Path "$env:APPDATA\axiomterm\config.lua"
   
   # macOS/Linux
   ls -la ~/.config/axiomterm/config.lua
   ```

2. ファイルに構文エラーがないか
   - axiomterm 起動時のエラーメッセージを確認
   - Lua の構文チェッカーで検証

3. ファイルの文字エンコーディングが UTF-8 か

### 起動時にエラーが出る

**Lua 構文エラーの例**:
```
Parse error: unexpected symbol near '='
```

**対処法**:
- `config.lua` の該当行を確認
- Lua の構文規則に従っているか確認（例: 文字列は `"` または `'` で囲む）

### パフォーマンスの問題

**症状**: 起動が遅い、描画が重い

**対処法**:
1. 不透明度を上げる（`window_background_opacity = 1.0`）
2. フォントサイズを下げる
3. マクロの複雑さを減らす

---

## ベストプラクティス

### 設定ファイルのバックアップ

設定ファイルはバージョン管理システム（Git など）で管理することを推奨します:

```bash
# 設定ディレクトリを Git リポジトリ化
cd ~/.config/axiomterm  # または %APPDATA%\axiomterm
git init
git add config.lua
git commit -m "Initial config"
```

### 段階的な設定変更

大規模な設定変更を行う際は:
1. 現在の `config.lua` をバックアップ
2. 少しずつ変更を加える
3. 各変更後に動作確認

### コメントの活用

設定ファイルにコメントを残すことで、後から見返したときに意図が分かりやすくなります:

```lua
-- プロンプトを λ に変更（関数型プログラミング風）
axiomterm_prompt = "λ "

-- 背景を少し透明に（デスクトップが見えるように）
window_background_opacity = 0.85
```

---

## 参考資料

- [config.lua 設定ガイド](config_guide.md)
- [Lua API リファレンス](lua_api.md)
- [アーキテクチャドキュメント](architecture.md)
