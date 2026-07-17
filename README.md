# herdr-imebox

A pop-up text box for typing CJK (Japanese, Chinese, Korean) text into AI
agents running inside [herdr](https://herdr.dev) — without IME accidents.

## Why

Typing with an IME inside a terminal agent (Claude Code, codex, etc.) has two
chronic problems:

- The Enter key both confirms IME conversion and submits the message, so a
  mistimed Enter sends a half-written prompt.
- Composed text does not reach the terminal until the conversion is confirmed,
  so if the agent redraws or pops an interactive prompt mid-composition, your
  draft is wiped.

herdr-imebox opens a small dedicated pane where Enter only inserts a newline.
When you are done, one keystroke sends the whole message to the agent pane.

## Usage

1. Focus the agent pane you are talking to and press the bound key
   (e.g. `prefix+i`).
2. A text box opens below the pane. Type freely — Enter is just a newline.
3. `Ctrl+D` — or pressing Enter twice in a row — sends the text (plus
   Enter) to the agent and closes the box. The blank newline from the
   double-Enter is stripped before sending. (An IME's conversion-confirm
   Enter never reaches the box, so it doesn't count toward the two.)
   The count is configurable via `enter_send_count`; set it to 3 if you
   want double-Enter to keep inserting blank lines instead.
4. `Ctrl+C` / `Esc Esc` closes the box, saving your draft. It is restored the
   next time the box opens for that pane (drafts are kept per target pane).
5. `Ctrl+P` / `Ctrl+N` walk back / forward through previously sent messages,
   so you can tweak and resend an earlier prompt.
6. `Ctrl+R` opens a full-pane history search: type to filter (whitespace-
   separated words are AND-ed, ASCII case-insensitive), move with `↑` `↓` /
   `Ctrl+P` `Ctrl+N`, pick with Enter to load it into the editor, or cancel
   with `Esc` / `Ctrl+R`.

Long lines soft-wrap to the box width (CJK-width aware); Up/Down move by
visual line.

## Install

Requires herdr 0.7+. The install step downloads a prebuilt, SHA-256-verified
binary for your platform (macOS arm64/x86_64, Linux x86_64/arm64) — no Rust
toolchain needed.

```sh
herdr plugin install Sawakee/herdr-imebox
```

Then bind the action to a key in `~/.config/herdr/config.toml`:

```toml
[[keys.command]]
key = "prefix+i"
type = "shell"
command = "herdr plugin action invoke open-imebox --plugin herdr-imebox"
```

Apply it with `herdr server reload-config`.

<details>
<summary>Manual install (without the plugin system)</summary>

```sh
git clone https://github.com/Sawakee/herdr-imebox
cd herdr-imebox
cargo build --release
```

Bind the binary directly instead of the plugin action:

```toml
[[keys.command]]
key = "prefix+i"
type = "shell"
command = "/path/to/herdr-imebox/target/release/imebox launch"
```

</details>

Optionally, also enable herdr's built-in IME candidate-window fix for normal
typing:

```toml
[experimental]
reveal_hidden_cursor_for_cjk_ime = true
```

## Configuration

Optional, at `~/.config/herdr-imebox/config.toml` (or under
`$XDG_CONFIG_HOME`). Defaults shown:

```toml
ratio = 0.25         # box height as a fraction of the target pane
enter_send_count = 2 # consecutive Enters that send the message; 0 disables
history_size = 100   # sent messages kept for Ctrl+P / Ctrl+N / Ctrl+R recall
```

## How it works

A single binary with two subcommands:

- `imebox launch` runs from the key binding, records the currently focused
  pane as the send target, and opens `imebox edit <target>` in a 25% split
  below it. If a box is already open, it focuses that box instead of opening
  another.
- `imebox edit` is a small ratatui TUI. It keeps the hardware terminal cursor
  at the edit position so IME candidate windows track it correctly.
- Multi-line text is wrapped in bracketed-paste sequences, so agents such as
  Claude Code receive it as a single message with embedded newlines.
- If sending fails (e.g. the target pane is gone), the draft is kept and an
  error is shown in the status bar. Per-pane drafts, the sent-message
  history, and the lock file live in `~/.cache/herdr-imebox/`.
