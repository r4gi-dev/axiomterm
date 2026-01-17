# Gemini Terminal

## 紹介
Gemini Terminalは、「思考を中断させない」「操作を待たせない」というUNIX的思想に基づいて設計された、Rust製の軽量かつ強力なスタンドアロン・ターミナルエミュレータです。一般的なコンソール画面に依存せず、独自のGUIウィンドウとして動作し、スムーズな非同期コマンド実行とLuaによる柔軟なカスタマイズを提供します。

## 使用したテクノロジー
- **言語**: [Rust](https://www.rust-lang.org/) (安全性と超高速な実行性能)
- **GUIフレームワーク**: [eframe / egui](https://github.com/emilk/egui) (即時モードGUI（Immediate Mode GUI）による低遅延な描画)
- **Luaパーサー**: [full_moon](https://github.com/Hajime-S/full_moon) (AST解析による、副作用のないクリーンな設定読み込み)
- **非同期通信**: [crossbeam-channel](https://github.com/crossbeam-rs/crossbeam) (UIスレッドをブロックしない、スムーズなプロセスの入出力管理)

## 機能
- **独自のスタンドアロンUI**: OSの標準コンソールに縛られない、独自の描画エンジン。
- **インライン・ターミナルフロー**: 入力プロンプトとコマンド履歴が垂直に流れる、直感的なCLIエクスペリエンス。
- **Luaベースの設定システム**: 設定ファイルをロードすることで、再起動なしに動作や見た目を変更。
- **非同期外部コマンド実行**: 重いコマンド（pingやdir /sなど）を実行してもUIがフリーズしません。
- **Unixスタイル・引数解析**: 引用符（" "、' '）やバックスラッシュ（\）を正しく扱う堅牢なトークナイザ。

## ユーザーができること
- **高度なカスタマイズ**:
  - `config.font_size`: フォントサイズの変更
  - `config.window_background_opacity`: ウィンドウ透過率の変更
  - `config.prompt`: プロンプト文字列の変更
  - `config.prompt_color`: プロンプトの色の変更（HEXカラー対応）
  - `config.text_color`: 出力テキストの色の変更
  - `config.window_title`: ウィンドウタイトルの変更
  - `config.default_cwd`: 起動時のカレントディレクトリ
  - `config.keys`: カスタムショートカットの定義
- **標準コンフィグパス**:
  - デフォルトのコンフィグファイルは `%USERPROFILE%\.config\gemini\config.lua` に配置されます。
  - `config load` を引数なしで実行すると、このファイルが読み込まれます。
- **設定例 (オブジェクト形式)**:
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
- **柔軟なコマンド操作**:
  - 外部コマンドの透過的な実行。
  - 内蔵コマンド（`config load`, `cd`, `echo`, `exit`）による制御。
- **シームレスな体験**:
  - エンターキー、または空行入力による高速なシェル操作。
  - 画面上部のステータスバーによる現在のディレクトリ表示。

## プロセス
1. **フェーズ1: 基礎設計**: Rustによる基本的なREPL（Read-Eval-Print Loop）と外部プロセス実行機能の実装。
2. **フェーズ2: 高度な解析**: クォートやエスケープを考慮したUnixライクな引数トークナイザの構築。
3. **フェーズ3: Lua統合**: `full_moon`ライブラリを採用し、副作用を排除した「定義としてのLua設定」を実現。
4. **フェーズ4: GUI移行**: `eframe`による完全なグラフィカルUIへの転換。非同期スレッドによる入出力の分離を実装。
5. **フェーズ5: 磨き上げ**: インライン入力フローの改善、色のカスタマイズ、およびビルド環境の最適化（MSVCツールチェーンへの移行）。

## 構築方法

### 前提条件
- [Rust (stable-x86_64-pc-windows-msvc)](https://www.rust-lang.org/tools/install) がインストールされていること。
- Visual Studio Build Tools (C++によるデスクトップ開発ワークロード) がインストールされていること。

### ビルド手順
1. リポジトリのディレクトリに移動します。
2. 以下のコマンドでリリース用バイナリをビルドします。
   ```powershell
   cargo build --release
   ```
3. ビルド完了後、以下のパスにある生成物を確認します。
   ```text
   target/release/terminal.exe
   ```

### 実行
```powershell
./target/release/terminal.exe
```
`config.lua` を `%USERPROFILE%\.config\gemini\` に配置し、ターミナル内で `config load` を実行することで、カスタマイズされた設定を体験できます。
