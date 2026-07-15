# herdr-imebox 設計

日本語IMEでherdr内のAIエージェントと対話する際の問題を解決するテキストボックスツール。

## 解決する問題

1. IME変換確定のEnterと送信のEnterが同じキーのため、誤送信が起きる
2. 変換確定までテキストがターミナルに渡らないため、変換中にエージェント側で
   インタラクティブな操作（確認プロンプト等）が発生すると下書きが消える

## 構成

- `launcher.sh` — herdrキーバインド（`type = "shell"`）から起動。フォーカス中ペインを
  ターゲットとして記録し、その下に分割ペインを開いて imebox を起動する
- `imebox.py` — テキストボックスTUI本体。Python + prompt_toolkit。
  `uv run --script`（PEP 723 インライン依存）で実行し、環境構築不要
- `~/.config/herdr/config.toml` に `[[keys.command]]` を追記（`prefix+i`）

## 起動フロー

1. `prefix+i` → launcher.sh が起動（herdrサーバーからdetachedで実行される）
2. `herdr pane list` で `focused:true` のペインIDを取得 → これが送信先ターゲット
3. 二重起動チェック: ロックファイル（`~/.cache/herdr-imebox/lock`）に生きている
   imebox ペインIDがあればそのペインにフォーカスして終了
4. ターゲットペインを `--direction down --ratio 0.25` で分割し、imebox.py を
   ターゲットID引数付きで起動（フォーカスは imebox へ）
5. imebox 終了時にペインが閉じ、フォーカスはターゲットペインに戻る

## テキストボックスのキー操作

| キー | 動作 |
|---|---|
| Enter | 改行のみ（IME確定Enterは画面に影響なし → 誤送信ゼロ） |
| Ctrl+D | 送信: `herdr pane send-text <target> <text>` → `send-keys <target> enter` |
| Ctrl+C / Esc Esc | キャンセル: 下書きを保存して閉じる |

- 画面下部にキーヒントを常時表示
- 起動時に前回の下書き（`~/.cache/herdr-imebox/draft.txt`）があれば復元
- 送信成功時に下書きをクリア

## エラー処理

- 送信失敗（ターゲットペイン消失等）: 下書きを保持したままエラーをツールバーに表示。
  テキストは失わない
- フォーカスペインが取得できない: launcher は何もせず終了
- 複数行テキスト: send-text が改行で早期送信しないことをe2eで確認する
  （問題があればbracketed paste等で対処）

## 補助設定

`reveal_hidden_cursor_for_cjk_ime = true` を config.toml で有効化し、
通常入力時のIME候補ウィンドウ追従も改善する。

## テスト

herdr自身をテストハーネスに使うe2e:

1. ターゲット用のダミーペイン（`cat` 実行）を作る
2. launcher.sh 相当の手順で imebox ペインを開く
3. `herdr pane send-text` で imebox に日本語テキストを注入（タイピングの模擬）
4. `send-keys` で Ctrl+D を送り、ターゲットペインの表示にテキスト+改行が
   届いたことを `herdr pane read` で確認
5. キャンセル→再起動で下書きが復元されることを確認
