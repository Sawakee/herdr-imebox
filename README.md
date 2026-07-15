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
3. `Ctrl+D` — or pressing Enter three times in a row — sends the text (plus
   Enter) to the agent and closes the box. The two blank newlines from the
   triple-Enter are stripped before sending. (An IME's conversion-confirm
   Enter never reaches the box, so it doesn't count toward the three.)
4. `Ctrl+C` / `Esc Esc` closes the box, saving your draft. It is restored the
   next time the box opens for that pane (drafts are kept per target pane).
5. `Ctrl+P` / `Ctrl+N` walk back / forward through previously sent messages,
   so you can tweak and resend an earlier prompt.

Long lines soft-wrap to the box width (CJK-width aware); Up/Down move by
visual line.

## Install

Requires a Rust toolchain (stable).

```sh
git clone <this-repository>
cd herdr-imebox
cargo build --release
```

Then add a key binding to `~/.config/herdr/config.toml`:

```toml
[[keys.command]]
key = "prefix+i"
type = "shell"
command = "/path/to/herdr-imebox/target/release/imebox launch"
```

Apply it with `herdr server reload-config`.

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
ratio = 0.25             # box height as a fraction of the target pane
triple_enter_send = true # three consecutive Enters send the message
history_size = 100       # sent messages kept for Ctrl+P / Ctrl+N recall
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
